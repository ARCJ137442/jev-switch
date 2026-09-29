//! OpenRouter upstream adapter.
//!
//! OpenRouter exposes an OpenAI-shaped chat endpoint rather than a native Jev
//! wire format. This adapter keeps that translation at the upstream boundary:
//! Jev requests become a strict JSON decision prompt and the returned JSON is
//! validated back into the Jev response contract.

use jev_core::adapter::UpstreamAdapter;
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse, Usage};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub const OPENROUTER_OFFICIAL_BASE: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const OPENROUTER_DEFAULT_MODEL: &str = "openrouter/auto";
pub const OPENROUTER_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[
        QuestionType::Choice,
        QuestionType::Score,
        QuestionType::Noul,
    ],
    has_confidence: true,
    has_usage: true,
    noul_via_boolean: false,
    retryable_status: &[408, 429, 500, 502, 503, 504, 529],
};

pub struct OpenRouterUpstream {
    id: String,
    base: String,
    api_key: Option<String>,
    http: Client,
}

impl OpenRouterUpstream {
    pub fn new_with_id(
        id: String,
        base: String,
        api_key: Option<String>,
    ) -> Result<Self, JevError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| JevError::Config {
                upstream_id: id.clone(),
                message: format!("reqwest build failed: {error}"),
            })?;
        Ok(Self {
            id,
            base,
            api_key: api_key.filter(|key| !key.is_empty()),
            http,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    model: Option<String>,
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for OpenRouterUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Capabilities {
        OPENROUTER_CAPABILITIES
    }

    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        let request_model = req.model.clone();
        let prompt = serde_json::to_string(&req).map_err(|error| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!("serialize OpenRouter decision prompt: {error}"),
        })?;
        let body = json!({
            "model": request_model,
            "messages": [
                {"role": "system", "content": "You are a Jev decision adapter. Return ONLY a JSON object with an `answers` object. For each question, preserve its id and return the exact Jev answer shape: choice requires type, choice, probabilities, confidence; score requires type, score, probabilities, confidence and may include legend; noul requires type and noul or probability. Do not add markdown or commentary."},
                {"role": "user", "content": prompt}
            ],
            "response_format": {"type": "json_object"},
            "stream": false
        });

        let mut request = self.http.post(&self.base).json(&body);
        if let Some(key) = self.api_key.as_deref() {
            request = request.bearer_auth(key);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                JevError::Timeout {
                    upstream_id: self.id.clone(),
                }
            } else {
                JevError::Network {
                    upstream_id: self.id.clone(),
                    message: error.to_string(),
                }
            }
        })?;
        let status = response.status();
        let raw_bytes = response.bytes().await.map_err(|error| JevError::Network {
            upstream_id: self.id.clone(),
            message: format!("read OpenRouter body: {error}"),
        })?;
        if !status.is_success() {
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: String::from_utf8_lossy(&raw_bytes).to_string(),
                retryable: self.capabilities().is_retryable_status(status.as_u16()),
            });
        }
        let chat: ChatResponse =
            serde_json::from_slice(&raw_bytes).map_err(|error| JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse OpenRouter response: {error}"),
            })?;
        let content = chat
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .and_then(content_text)
            .ok_or_else(|| JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: "OpenRouter response contained no assistant content".into(),
            })?;
        let decision = extract_json_object(&content).ok_or_else(|| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: "OpenRouter assistant content was not a JSON object".into(),
        })?;
        let mut result: JevResponse =
            serde_json::from_value(decision).map_err(|error| JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse OpenRouter Jev answers: {error}"),
            })?;
        result.model = chat.model.or(Some(request_model));
        result.usage = chat.usage.map(|usage| Usage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            reasoning_tokens: usage.reasoning_tokens,
        });
        Ok(result)
    }
}

fn content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn extract_json_object(text: &str) -> Option<Value> {
    let trimmed = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("``")
        .trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    serde_json::from_str(&trimmed[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::{Instructions, Question};
    use serde_json::json;
    use std::collections::BTreeMap;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn request() -> JevRequest {
        JevRequest {
            model: "openai/gpt-4o-mini".into(),
            state: json!({"text": "hello"}),
            questions: BTreeMap::from([(
                "positive".into(),
                Question::Noul {
                    instructions: Instructions::Text("is this positive?".into()),
                    criteria: None,
                },
            )]),
        }
    }

    #[tokio::test]
    async fn translates_chat_json_back_to_jev_and_maps_usage() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/v1/chat/completions")).and(header("authorization", "Bearer test-openrouter-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model": "openai/gpt-4o-mini-2024",
                "choices": [{"message": {"content": "{\"answers\":{\"positive\":{\"type\":\"noul\",\"noul\":0.87}}}"}}],
                "usage": {"prompt_tokens": 41, "completion_tokens": 7}
            }))).expect(1).mount(&server).await;
        let upstream = OpenRouterUpstream::new_with_id(
            "openrouter".into(),
            format!("{}/api/v1/chat/completions", server.uri()),
            Some("test-openrouter-key".into()),
        )
        .unwrap();
        let response = upstream.evaluate(request()).await.unwrap();
        assert_eq!(response.model.as_deref(), Some("openai/gpt-4o-mini-2024"));
        assert_eq!(
            response.usage.as_ref().and_then(|u| u.input_tokens),
            Some(41)
        );
        assert_eq!(
            response.answers["positive"],
            serde_json::from_value(json!({"type":"noul","noul":0.87})).unwrap()
        );
    }

    #[tokio::test]
    async fn maps_rate_limit_to_retryable_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(json!({"error":{"message":"rate limited"}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let upstream = OpenRouterUpstream::new_with_id(
            "openrouter".into(),
            format!("{}/api/v1/chat/completions", server.uri()),
            None,
        )
        .unwrap();
        let error = upstream.evaluate(request()).await.unwrap_err();
        assert!(error.retryable());
        assert_eq!(error.http_status(), 503);
    }

    #[test]
    fn extracts_fenced_json() {
        assert_eq!(
            extract_json_object("```json\n{\"answers\":{}}\n```").unwrap()["answers"],
            json!({})
        );
    }
}
