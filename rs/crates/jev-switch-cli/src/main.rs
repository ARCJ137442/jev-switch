use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::{header::AUTHORIZATION, redirect::Policy, Client as HttpClient, Method, Response};
use serde_json::{json, Value};
use std::{io::Read, path::PathBuf};

const DEFAULT_BASE: &str = "http://127.0.0.1:11435";
const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_EVENT_LIMIT: usize = 500;

#[derive(Debug, Parser)]
#[command(
    name = "jev-switch-cli",
    version,
    about = "Jev-Switch HTTP client for Termux and headless workflows"
)]
struct Cli {
    /// Daemon base URL. Defaults to JEV_SWITCH_URL or the local daemon.
    #[arg(long, env = "JEV_SWITCH_URL")]
    base: Option<String>,
    /// Output mode. JSON is stable for scripts; table is for a terminal.
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show health and admin runtime status.
    Status,
    /// List published models and upstream capabilities.
    Models,
    /// Send a Jev-native request from a JSON file or stdin.
    Invoke {
        /// Read the request from this file. Without it, stdin is consumed.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Read the current routing DAG (read-only in Phase 1).
    Routes,
    /// Read persisted call activity using the durable event cursor.
    Events {
        #[arg(long, default_value_t = 0)]
        since: u64,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum AuthScope {
    Call,
    Admin,
}

#[derive(Clone)]
struct ApiClient {
    http: HttpClient,
    base: String,
    call_token: Option<String>,
    admin_token: Option<String>,
}

impl ApiClient {
    fn from_cli(cli: &Cli) -> Result<Self> {
        let base = cli
            .base
            .clone()
            .unwrap_or_else(|| DEFAULT_BASE.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            http: HttpClient::builder()
                .redirect(Policy::none())
                .build()
                .context("failed to create HTTP client")?,
            base,
            call_token: std::env::var("JEV_SWITCH_TOKEN")
                .ok()
                .filter(|token| !token.is_empty()),
            admin_token: std::env::var("JEV_SWITCH_ADMIN_TOKEN")
                .ok()
                .filter(|token| !token.is_empty()),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    async fn get(&self, path: &str, scope: Option<AuthScope>) -> Result<Value> {
        self.request(Method::GET, path, scope, None).await
    }

    async fn post_json(
        &self,
        path: &str,
        scope: AuthScope,
        body: Value,
    ) -> Result<(Value, Option<String>)> {
        let response = self
            .request_response(Method::POST, path, Some(scope), Some(body))
            .await?;
        let request_id = response
            .headers()
            .get("x-jev-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Ok((read_json_response(response).await?, request_id))
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        scope: Option<AuthScope>,
        body: Option<Value>,
    ) -> Result<Value> {
        let response = self.request_response(method, path, scope, body).await?;
        read_json_response(response).await
    }

    async fn request_response(
        &self,
        method: Method,
        path: &str,
        scope: Option<AuthScope>,
        body: Option<Value>,
    ) -> Result<Response> {
        let token = match scope {
            Some(AuthScope::Call) => self.call_token.as_deref(),
            Some(AuthScope::Admin) => self.admin_token.as_deref(),
            None => None,
        };
        let mut request = self.http.request(method, self.url(path));
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .context("request to Jev-Switch daemon failed")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("daemon returned HTTP {status}: {}", compact_error(&body));
        }
        Ok(response)
    }
}

async fn read_json_response(response: Response) -> Result<Value> {
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read daemon response")?;
    serde_json::from_str(&body).with_context(|| {
        format!(
            "daemon returned non-JSON HTTP {status}: {}",
            compact_error(&body)
        )
    })
}

fn compact_error(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let shortened = chars.by_ref().take(400).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

fn read_request(file: Option<PathBuf>) -> Result<Value> {
    let mut bytes = Vec::new();
    match file {
        Some(path) => std::fs::File::open(&path)
            .with_context(|| format!("failed to open request file {}", path.display()))?
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to read request file {}", path.display()))?,
        None => std::io::stdin()
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("failed to read Jev request from stdin")?,
    };
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        bail!("request input exceeds the 2 MiB limit");
    }
    let input = String::from_utf8(bytes).context("request input must be UTF-8 JSON")?;
    serde_json::from_str(input.trim()).context("request input is not valid JSON")
}

fn print_value(value: &Value, format: OutputFormat, request_id: Option<&str>) -> Result<()> {
    match format {
        OutputFormat::Json => {
            if let Some(request_id) = request_id {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"request_id": request_id, "response": value})
                    )?
                );
            } else {
                println!("{}", serde_json::to_string_pretty(value)?);
            }
        }
        OutputFormat::Table => print_table(value, request_id),
    }
    Ok(())
}

fn print_table(value: &Value, request_id: Option<&str>) {
    if let Some(request_id) = request_id {
        println!("request_id  {request_id}");
    }
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        println!("status      {status}");
    }
    if let Some(version) = value.get("version").and_then(Value::as_str) {
        println!("version     {version}");
    }
    if let Some(bind) = value.get("bind").and_then(Value::as_str) {
        println!("bind        {bind}");
    }
    if let Some(models) = value.get("data").and_then(Value::as_array) {
        println!("models      {}", models.len());
        for model in models {
            println!(
                "  - {}",
                model
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
            );
        }
        return;
    }
    if let Some(routes) = value.get("routes").and_then(Value::as_array) {
        println!("routes      {}", routes.len());
        for route in routes {
            let left = route.get("left").and_then(Value::as_str).unwrap_or("?");
            let right = route.get("right").and_then(Value::as_str).unwrap_or("?");
            let priority = route
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            println!("  - {left} -> {right} (priority {priority})");
        }
        return;
    }
    if let Some(events) = value.get("events").and_then(Value::as_array) {
        println!("events      {}", events.len());
        for event in events {
            let id = event.get("id").and_then(Value::as_u64).unwrap_or_default();
            let kind = event.get("kind").and_then(Value::as_str).unwrap_or("?");
            println!("  - #{id} {kind}");
        }
        return;
    }
    if value.get("response").is_some() {
        println!("response    JSON response received");
        if let Some(answers) = value.get("answers").and_then(Value::as_object) {
            println!("answers     {}", answers.len());
            for (question, answer) in answers {
                let kind = answer
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("answer");
                let detail = answer
                    .get("answer")
                    .or_else(|| answer.get("value"))
                    .or_else(|| answer.get("noul"))
                    .cloned()
                    .unwrap_or_else(|| answer.clone());
                println!("  - {question} [{kind}]: {detail}");
            }
        }
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let format = cli.format;
    let client = ApiClient::from_cli(&cli)?;
    match cli.command {
        Command::Status => {
            let health = client.get("/health", None).await?;
            let admin = client
                .get("/v1/admin/status", Some(AuthScope::Admin))
                .await?;
            print_value(&json!({"health": health, "admin": admin}), format, None)?;
        }
        Command::Models => print_value(
            &client.get("/v1/models", Some(AuthScope::Call)).await?,
            format,
            None,
        )?,
        Command::Invoke { file } => {
            let request = read_request(file)?;
            let (response, request_id) = client
                .post_json("/v1/systemone", AuthScope::Call, request)
                .await?;
            print_value(&response, format, request_id.as_deref())?;
        }
        Command::Routes => print_value(
            &client
                .get("/v1/admin/routes", Some(AuthScope::Admin))
                .await?,
            format,
            None,
        )?,
        Command::Events { since, limit } => {
            if limit == 0 || limit > MAX_EVENT_LIMIT {
                bail!("event limit must be between 1 and {MAX_EVENT_LIMIT}");
            }
            let path = format!("/v1/admin/events?since={since}&limit={limit}");
            print_value(
                &client.get(&path, Some(AuthScope::Admin)).await?,
                format,
                None,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_json, header, method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn builds_paths_without_duplicate_slashes() {
        let client = ApiClient {
            http: HttpClient::new(),
            base: "http://localhost:11435".into(),
            call_token: None,
            admin_token: None,
        };
        assert_eq!(client.url("/v1/models"), "http://localhost:11435/v1/models");
    }

    #[tokio::test]
    async fn sends_call_token_and_native_request_body() {
        let server = MockServer::start().await;
        let client = ApiClient {
            http: HttpClient::new(),
            base: server.uri(),
            call_token: Some("call-test".into()),
            admin_token: None,
        };
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .and(header("authorization", "Bearer call-test"))
            .and(body_json(json!({"model":"jev","state":{},"questions":{}})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers":{}})))
            .mount(&server)
            .await;
        let (value, request_id) = client
            .post_json(
                "/v1/systemone",
                AuthScope::Call,
                json!({"model":"jev","state":{},"questions":{}}),
            )
            .await
            .unwrap();
        assert_eq!(value, json!({"answers":{}}));
        assert_eq!(request_id, None);
    }

    #[tokio::test]
    async fn uses_admin_token_and_event_cursor() {
        let server = MockServer::start().await;
        let client = ApiClient {
            http: HttpClient::new(),
            base: server.uri(),
            call_token: None,
            admin_token: Some("admin-test".into()),
        };
        Mock::given(method("GET"))
            .and(path("/v1/admin/events"))
            .and(query_param("since", "4"))
            .and(query_param("limit", "2"))
            .and(header("authorization", "Bearer admin-test"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"events":[],"next_since":4})),
            )
            .mount(&server)
            .await;
        let value = client
            .get("/v1/admin/events?since=4&limit=2", Some(AuthScope::Admin))
            .await
            .unwrap();
        assert_eq!(value["next_since"], 4);
    }

    #[test]
    fn truncates_unicode_error_text_without_panicking() {
        let error = compact_error(&"错".repeat(401));
        assert!(error.ends_with('…'));
        assert_eq!(error.chars().count(), 401);
    }
}
