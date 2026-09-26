//! Managed gateway caller tokens and caller-scoped observability.

use axum::{
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures_util::{stream, StreamExt};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{convert::Infallible, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub enum TokenRole {
    Admin,
    Readonly,
}

impl TokenRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Readonly => "readonly",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TokenSummary {
    pub id: String,
    pub name: String,
    pub role: TokenRole,
    pub enabled: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub created_at: i64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CreateTokenRequest {
    pub name: String,
    pub role: TokenRole,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct UpdateTokenRequest {
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub role: Option<TokenRole>,
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CreateTokenResponse {
    pub token: TokenSummary,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CallerStats {
    pub token_id: String,
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub total_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub total_cost: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub avg_latency_ms: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub error_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TokenStats {
    pub token_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub from: Option<i64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub to: Option<i64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub total_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub total_cost: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub avg_latency_ms: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub error_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerIdentity {
    pub id: String,
    pub role: TokenRole,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TokenListResponse {
    pub tokens: Vec<TokenSummary>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TokenStatsResponse {
    pub stats: TokenStats,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CallerStatsResponse {
    pub tokens: Vec<CallerStats>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct EventsPage {
    pub events: Vec<crate::events::Event>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub next_since: u64,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    pub since: u64,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_limit() -> usize {
    100
}

#[derive(Debug, Deserialize)]
pub struct TimeRange {
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct DeletedToken {
    pub id: String,
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct CallerMe {
    pub id: String,
    pub role: TokenRole,
}

pub fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub fn lookup_secret(conn: &Connection, secret: &str) -> rusqlite::Result<Option<CallerIdentity>> {
    conn.query_row(
        "SELECT id,name,role FROM call_tokens WHERE secret_hash=? AND enabled=1",
        [hash_secret(secret)],
        |row| {
            let role: String = row.get(2)?;
            Ok(CallerIdentity {
                id: row.get(0)?,
                name: row.get(1)?,
                role: if role == "admin" {
                    TokenRole::Admin
                } else {
                    TokenRole::Readonly
                },
            })
        },
    )
    .optional()
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<TokenSummary>> {
    let mut stmt = conn.prepare("SELECT id,name,role,enabled,created_at,last_used_at FROM call_tokens ORDER BY created_at DESC,id")?;
    let rows = stmt.query_map([], |row| {
        let role: String = row.get(2)?;
        Ok(TokenSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            role: if role == "admin" {
                TokenRole::Admin
            } else {
                TokenRole::Readonly
            },
            enabled: row.get::<_, i64>(3)? != 0,
            created_at: row.get(4)?,
            last_used_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn update_last_used(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE call_tokens SET last_used_at=? WHERE id=?",
        params![now_ms(), id],
    )?;
    Ok(())
}

/// Upgrade pre-token-manager config credentials to hashed, caller-identifiable
/// readonly tokens. INSERT OR IGNORE means a revoked legacy credential stays revoked
/// after restart; rotation is done by managing the database token inventory.
pub fn import_legacy_config_tokens(conn: &Connection, secrets: &[String]) -> rusqlite::Result<()> {
    for secret in secrets.iter().filter(|secret| !secret.trim().is_empty()) {
        let hash = hash_secret(secret);
        let id = format!("legacy_{}", &hash[..20]);
        conn.execute("INSERT OR IGNORE INTO call_tokens (id,name,role,secret_hash,enabled,created_at) VALUES (?, 'Imported config token', 'readonly', ?, 1, ?)",
            params![id, hash, now_ms()])?;
    }
    Ok(())
}

pub fn create(
    conn: &Connection,
    name: &str,
    role: TokenRole,
) -> rusqlite::Result<CreateTokenResponse> {
    if name.trim().is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "name must not be empty".into(),
        ));
    }
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
    })?;
    let secret = format!(
        "jev_{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let secret_hash = hash_secret(&secret);
    let id = format!("tok_{}", &secret_hash[..20]);
    let created_at = now_ms();
    conn.execute("INSERT INTO call_tokens (id,name,role,secret_hash,enabled,created_at) VALUES (?,?,?,?,1,?)", params![id, name.trim(), role.as_str(), secret_hash, created_at])?;
    Ok(CreateTokenResponse {
        token: TokenSummary {
            id,
            name: name.trim().into(),
            role,
            enabled: true,
            created_at,
            last_used_at: None,
        },
        secret,
    })
}

pub fn update(
    conn: &Connection,
    id: &str,
    request: &UpdateTokenRequest,
) -> rusqlite::Result<Option<TokenSummary>> {
    let changed = conn.execute("UPDATE call_tokens SET name=COALESCE(?,name), role=COALESCE(?,role), enabled=COALESCE(?,enabled) WHERE id=?",
        params![request.name.as_deref().map(str::trim).filter(|s| !s.is_empty()), request.role.map(TokenRole::as_str), request.enabled.map(|value| if value { 1 } else { 0 }), id])?;
    if changed == 0 {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id,name,role,enabled,created_at,last_used_at FROM call_tokens WHERE id=?",
        [id],
        |row| {
            let role: String = row.get(2)?;
            Ok(TokenSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                role: if role == "admin" {
                    TokenRole::Admin
                } else {
                    TokenRole::Readonly
                },
                enabled: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
                last_used_at: row.get(5)?,
            })
        },
    )
    .optional()
}

pub fn stats(
    conn: &Connection,
    token_id: &str,
    name: &str,
    from: Option<i64>,
    to: Option<i64>,
) -> rusqlite::Result<TokenStats> {
    let (total_requests, total_cost, avg_latency_ms, error_rate) = conn.query_row(
        "SELECT COUNT(*), CASE WHEN COUNT(*)=SUM(CASE WHEN cost_usd IS NOT NULL AND COALESCE(upstream_calls,1)=1 THEN 1 ELSE 0 END) THEN SUM(cost_usd) ELSE NULL END, AVG(latency_ms), AVG(CASE WHEN success=0 THEN 1.0 ELSE 0.0 END) FROM call_logs WHERE token_id=? AND (? IS NULL OR timestamp*1000>=?) AND (? IS NULL OR timestamp*1000<=?)",
        params![token_id, from, from, to, to], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, Option<f64>>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?)))?;
    let _ = name;
    Ok(TokenStats {
        token_id: token_id.into(),
        from,
        to,
        total_requests,
        total_cost,
        avg_latency_ms,
        error_rate,
    })
}

pub fn all_stats(
    conn: &Connection,
    from: Option<i64>,
    to: Option<i64>,
) -> rusqlite::Result<Vec<CallerStats>> {
    let mut stmt = conn.prepare("SELECT t.id,t.name,COUNT(l.id),CASE WHEN COUNT(l.id)=SUM(CASE WHEN l.cost_usd IS NOT NULL AND COALESCE(l.upstream_calls,1)=1 THEN 1 ELSE 0 END) THEN SUM(l.cost_usd) ELSE NULL END,AVG(l.latency_ms),AVG(CASE WHEN l.success=0 THEN 1.0 ELSE 0.0 END) FROM call_tokens t LEFT JOIN call_logs l ON l.token_id=t.id AND (? IS NULL OR l.timestamp*1000>=?) AND (? IS NULL OR l.timestamp*1000<=?) GROUP BY t.id,t.name ORDER BY COUNT(l.id) DESC,t.name")?;
    let rows = stmt.query_map(params![from, from, to, to], |row| {
        Ok(CallerStats {
            token_id: row.get(0)?,
            name: row.get(1)?,
            total_requests: row.get(2)?,
            total_cost: row.get(3)?,
            avg_latency_ms: row.get(4)?,
            error_rate: row.get(5)?,
        })
    })?;
    rows.collect()
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    crate::error_response(status, message, None, false, &[])
}

pub async fn get_me(Extension(caller): Extension<CallerIdentity>) -> Json<CallerMe> {
    Json(CallerMe {
        id: caller.id,
        role: caller.role,
    })
}

pub async fn list_tokens(State(state): State<crate::AppState>) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match list(&conn) {
        Ok(tokens) => Json(TokenListResponse { tokens }).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn create_token(State(state): State<crate::AppState>, body: Bytes) -> Response {
    let request: CreateTokenRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match create(&conn, &request.name, request.role) {
        Ok(value) => Json(value).into_response(),
        Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

pub async fn update_token(
    State(state): State<crate::AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let request: UpdateTokenRequest = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match update(&conn, &id, &request) {
        Ok(Some(token)) => Json(serde_json::json!({"token": token})).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "token not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn revoke_token(
    State(state): State<crate::AppState>,
    Path(id): Path<String>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match conn.execute("UPDATE call_tokens SET enabled=0 WHERE id=?", [&id]) {
        Ok(0) => error(StatusCode::NOT_FOUND, "token not found"),
        Ok(_) => Json(DeletedToken { id, revoked: true }).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn stats_my(
    State(state): State<crate::AppState>,
    Extension(caller): Extension<CallerIdentity>,
    Query(range): Query<TimeRange>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match stats(&conn, &caller.id, &caller.name, range.from, range.to) {
        Ok(stats) => Json(TokenStatsResponse { stats }).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn stats_admin(
    State(state): State<crate::AppState>,
    Query(range): Query<TimeRange>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match all_stats(&conn, range.from, range.to) {
        Ok(tokens) => Json(CallerStatsResponse { tokens }).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn stats_admin_token(
    State(state): State<crate::AppState>,
    Path(id): Path<String>,
    Query(range): Query<TimeRange>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let name = match conn
        .query_row("SELECT name FROM call_tokens WHERE id=?", [&id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
    {
        Ok(Some(name)) => name,
        Ok(None) => return error(StatusCode::NOT_FOUND, "token not found"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match stats(&conn, &id, &name, range.from, range.to) {
        Ok(stats) => Json(TokenStatsResponse { stats }).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn events_admin(
    State(state): State<crate::AppState>,
    Query(query): Query<PageQuery>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(db_error) => return error(StatusCode::INTERNAL_SERVER_ERROR, db_error.to_string()),
    };
    match request_events(&conn, None, query.since, query.limit) {
        Ok(page) => Json(page).into_response(),
        Err(db_error) => error(StatusCode::INTERNAL_SERVER_ERROR, db_error.to_string()),
    }
}

pub async fn events_my(
    State(state): State<crate::AppState>,
    Extension(caller): Extension<CallerIdentity>,
    Query(query): Query<PageQuery>,
) -> Response {
    let conn = match state.db_conn.lock() {
        Ok(conn) => conn,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    match caller_events(&conn, &caller.id, query.since, query.limit) {
        Ok(page) => Json(page).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// HTTP history and caller SSE share the durable call-log cursor. EventBus IDs
/// belong to the admin stream and must never advance this cursor.
fn caller_events(
    conn: &Connection,
    token_id: &str,
    since: u64,
    limit: usize,
) -> rusqlite::Result<EventsPage> {
    request_events(conn, Some(token_id), since, limit)
}

/// Admin and caller activity use the call-log primary key as their durable
/// cursor. A zero cursor returns the newest page; subsequent requests stream
/// forward from that stable ID. Only routing metadata is attached to history.
fn request_events(
    conn: &Connection,
    token_id: Option<&str>,
    since: u64,
    limit: usize,
) -> rusqlite::Result<EventsPage> {
    let limit = limit.clamp(1, 500);
    let initial_page = since == 0;
    let sql = if initial_page {
        "SELECT id,timestamp,success,http_status,endpoint_id,route_key,error_message,upstream_provider,upstream_model,latency_ms,upstream_calls,cost_usd,usage_json,request_id,route_trace_json,token_id
         FROM call_logs WHERE (?1 IS NULL OR token_id=?1) ORDER BY id DESC LIMIT ?2"
    } else {
        "SELECT id,timestamp,success,http_status,endpoint_id,route_key,error_message,upstream_provider,upstream_model,latency_ms,upstream_calls,cost_usd,usage_json,request_id,route_trace_json,token_id
         FROM call_logs WHERE id>?1 AND (?2 IS NULL OR token_id=?2) ORDER BY id LIMIT ?3"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if initial_page {
        stmt.query_map(rusqlite::params![token_id, limit], activity_event_from_row)?
    } else {
        stmt.query_map(
            rusqlite::params![since, token_id, limit],
            activity_event_from_row,
        )?
    };
    let mut events = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if initial_page {
        events.reverse();
    }
    let next_since = events.last().map(|event| event.id).unwrap_or(since);
    Ok(EventsPage { events, next_since })
}

fn activity_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::events::Event> {
    let id: i64 = row.get(0)?;
    let timestamp: i64 = row.get(1)?;
    let success: i64 = row.get(2)?;
    let status: Option<i64> = row.get(3)?;
    let endpoint_id: String = row.get(4)?;
    let route_key: String = row.get(5)?;
    let error_message: Option<String> = row.get(6)?;
    let provider: String = row.get(7)?;
    let upstream_model: String = row.get(8)?;
    let latency_ms: i64 = row.get(9)?;
    let upstream_calls: Option<i64> = row.get(10)?;
    let cost_usd: Option<f64> = row.get(11)?;
    let usage = row
        .get::<_, Option<String>>(12)?
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let request_id: Option<String> = row.get(13)?;
    let route_trace = row
        .get::<_, Option<String>>(14)?
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let token_id: Option<String> = row.get(15)?;
    let status = status.or_else(|| {
        error_message
            .as_deref()
            .and_then(|message| message.strip_prefix("HTTP "))
            .and_then(|value| value.parse().ok())
    });
    let detail = serde_json::json!({
        "endpoint_id": endpoint_id,
        "success": success != 0,
        "status": status,
        "route_key": route_key,
        "provider": provider,
        "upstream_model": upstream_model,
        "latency_ms": latency_ms,
        "upstream_calls": upstream_calls,
        "cost_usd": cost_usd,
        "usage": usage,
        "request_id": request_id,
        "route_trace": route_trace
    })
    .to_string();
    Ok(crate::events::Event {
        id: id.max(0) as u64,
        timestamp: timestamp.max(0) as u64 * 1000,
        kind: "request".into(),
        detail,
        token_id,
    })
}

fn sse_event(event: crate::events::Event) -> SseEvent {
    SseEvent::default()
        .id(event.id.to_string())
        .event(event.kind.clone())
        .json_data(event)
        .unwrap_or_else(|_| SseEvent::default().comment("serialization error"))
}

pub async fn events_stream_admin(
    State(state): State<crate::AppState>,
    Query(query): Query<PageQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = stream::unfold(
        (state.events.clone(), query.since),
        |(bus, mut since)| async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let events = bus.page(since, 100, None);
            if let Some(last) = events.last() {
                since = last.id;
            }
            let items = if events.is_empty() {
                vec![SseEvent::default().comment("keep-alive")]
            } else {
                events.into_iter().map(sse_event).collect()
            };
            Some((stream::iter(items.into_iter().map(Ok)), (bus, since)))
        },
    )
    .flatten();
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn caller_event_stream(
    db: std::sync::Arc<std::sync::Mutex<Connection>>,
    since: u64,
    token_id: String,
) -> impl futures_util::Stream<Item = Result<SseEvent, std::io::Error>> {
    stream::unfold(Some((db, since, token_id)), |state| async move {
        let (db, since, token_id) = state?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let page = match db.lock() {
            Ok(conn) => caller_events(&conn, &token_id, since, 100).map_err(std::io::Error::other),
            Err(e) => Err(std::io::Error::other(e.to_string())),
        };
        let (items, next) = match page {
            Ok(page) => {
                let items = if page.events.is_empty() {
                    vec![Ok(SseEvent::default().comment("keep-alive"))]
                } else {
                    page.events
                        .into_iter()
                        .map(|event| Ok(sse_event(event)))
                        .collect()
                };
                (items, Some((db, page.next_since, token_id)))
            }
            // Close on database failure so clients can show the error/fall back;
            // an empty successful page would hide missing call records.
            Err(e) => (vec![Err(e)], None),
        };
        Some((stream::iter(items), next))
    })
    .flatten()
}

pub async fn events_stream_my(
    State(state): State<crate::AppState>,
    Extension(caller): Extension<CallerIdentity>,
    Query(query): Query<PageQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<SseEvent, std::io::Error>>> {
    Sse::new(caller_event_stream(state.db_conn, query.since, caller.id)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn caller_sse_uses_history_ids_and_resumes_without_duplicates_or_other_callers() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_database(&conn).unwrap();
        crate::db::endpoints::create(&conn, "public", "follow_global", true).unwrap();
        let first = create(&conn, "first", TokenRole::Readonly).unwrap();
        let other = create(&conn, "other", TokenRole::Readonly).unwrap();
        let insert = |conn: &Connection, id: i64, token: &str| {
            conn.execute("INSERT INTO call_logs (id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,token_id,upstream_calls) VALUES (?,1,'public','public','fake','m1',1,12,?,1)", params![id, token]).unwrap();
        };
        insert(&conn, 101, &first.token.id);
        insert(&conn, 102, &other.token.id);
        let history = caller_events(&conn, &first.token.id, 0, 50).unwrap();
        assert_eq!(history.next_since, 101);
        assert_eq!(history.events.len(), 1);
        let db = std::sync::Arc::new(std::sync::Mutex::new(conn));
        let mut body = Sse::new(caller_event_stream(db.clone(), 0, first.token.id.clone()))
            .into_response()
            .into_body()
            .into_data_stream();
        let frame = String::from_utf8(body.next().await.unwrap().unwrap().to_vec()).unwrap();
        let data = frame
            .lines()
            .find_map(|line| {
                line.strip_prefix("data: ")
                    .or_else(|| line.strip_prefix("data:"))
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(data).unwrap(),
            serde_json::to_value(&history.events[0]).unwrap()
        );

        // Reconnect using the polling cursor, even when another caller advanced
        // the global database sequence; history must not be replayed.
        let mut resumed = Sse::new(caller_event_stream(
            db.clone(),
            history.next_since,
            first.token.id.clone(),
        ))
        .into_response()
        .into_body()
        .into_data_stream();
        let frame = String::from_utf8(resumed.next().await.unwrap().unwrap().to_vec()).unwrap();
        assert!(!frame.contains("data:"));
        insert(&db.lock().unwrap(), 103, &first.token.id);
        let frame = String::from_utf8(resumed.next().await.unwrap().unwrap().to_vec()).unwrap();
        assert!(frame.contains("id: 103"));
        assert!(!frame.contains(&other.token.id));
        assert_eq!(
            caller_events(&db.lock().unwrap(), &first.token.id, 101, 50)
                .unwrap()
                .next_since,
            103
        );
    }

    #[test]
    fn admin_history_starts_at_the_latest_page_then_advances_without_replay() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_database(&conn).unwrap();
        for id in 1..=60_i64 {
            let request_id = format!("jev-{id}");
            let trace = serde_json::json!({"selected_provider":"fake", "request_id":request_id})
                .to_string();
            conn.execute(
                "INSERT INTO call_logs (id,timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,http_status,request_id,route_trace_json)
                 VALUES (?,1,'public','public→fake','fake','m1',1,12,200,?,?)",
                params![id, request_id, trace],
            ).unwrap();
        }

        let latest = request_events(&conn, None, 0, 50).unwrap();
        assert_eq!(latest.events.len(), 50);
        assert_eq!(latest.events.first().unwrap().id, 11);
        assert_eq!(latest.events.last().unwrap().id, 60);
        assert_eq!(latest.next_since, 60);
        let detail: serde_json::Value =
            serde_json::from_str(&latest.events.last().unwrap().detail).unwrap();
        assert_eq!(detail["route_trace"]["request_id"], "jev-60");

        let forward = request_events(&conn, None, 10, 3).unwrap();
        assert_eq!(
            forward
                .events
                .iter()
                .map(|event| event.id)
                .collect::<Vec<_>>(),
            vec![11, 12, 13]
        );
        let caught_up = request_events(&conn, None, latest.next_since, 50).unwrap();
        assert!(caught_up.events.is_empty());
        assert_eq!(caught_up.next_since, latest.next_since);
    }

    #[test]
    fn aggregate_cost_is_unknown_when_any_dispatched_call_cost_is_unaccounted() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_database(&conn).unwrap();
        crate::db::endpoints::create(&conn, "public", "follow_global", true).unwrap();
        let token = create(&conn, "caller", TokenRole::Readonly).unwrap();
        conn.execute(
            "INSERT INTO call_logs (timestamp,endpoint_id,route_key,upstream_provider,upstream_model,success,latency_ms,token_id,cost_usd,upstream_calls) VALUES (1,'public','public','fake','m1',1,12,?,0.25,2)",
            [&token.token.id],
        ).unwrap();
        let summary = stats(&conn, &token.token.id, "caller", None, None).unwrap();
        assert_eq!(
            summary.total_cost, None,
            "a multi-dispatch response only reports selected-provider cost, not aggregate billing"
        );
        assert_eq!(all_stats(&conn, None, None).unwrap()[0].total_cost, None);

        conn.execute(
            "UPDATE call_logs SET upstream_calls=1 WHERE token_id=?",
            [&token.token.id],
        )
        .unwrap();
        assert_eq!(
            stats(&conn, &token.token.id, "caller", None, None)
                .unwrap()
                .total_cost,
            Some(0.25)
        );
        assert_eq!(
            all_stats(&conn, None, None).unwrap()[0].total_cost,
            Some(0.25)
        );
    }
}
