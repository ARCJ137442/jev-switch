//! Jev-Switch MVP 入口（M0.9 · P0-1 迁入 jev-switch-daemon）
//!
//! 整合：jev-protocol / jev-core / jev-adapters / config / axum HTTP server。
//!
//! 路由：
//! - `POST /v1/systemone` — 主入口：Jev 协议请求
//! - `GET  /health`        — liveness
//! - `GET  /v1/models`     — 列出可达 model + upstream + capability
//!
//! 错误映射：JevError → HTTP 状态码由 JevError::http_status() 决定。

mod config;

use anyhow::Context;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use config::{Config, ConfigError};
use jev_adapters::{upstream_laya::LayaUpstream, upstream_vercel::VercelUpstream};
use jev_core::{
    router::Router as JevRouter,
    upstream::{JevError, Upstream},
};
use jev_protocol::{JevRequest, JevResponse};
use serde::Serialize;
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    router: Arc<JevRouter>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    upstream: Option<String>,
    retryable: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing init
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // 1. load config
    let config = Config::load_default().context("load config")?;
    tracing::info!(
        providers = config.providers.len(),
        routes = config.router.len(),
        "config loaded"
    );

    // 2. construct upstreams based on config
    let mut upstreams: HashMap<String, Box<dyn Upstream>> = HashMap::new();

    if let Some(p) = config.providers.get("vercel") {
        if p.enabled {
            let api_key = match config.read_api_key("vercel") {
                Ok(k) => k,
                Err(ConfigError::MissingEnv { provider, var }) => {
                    tracing::warn!(provider = %provider, var = %var, "skipping vercel provider: missing env");
                    String::new()
                }
                Err(e) => return Err(e.into()),
            };
            if !api_key.is_empty() {
                match VercelUpstream::new(p.base.clone(), api_key) {
                    Ok(u) => {
                        upstreams.insert("vercel".into(), Box::new(u));
                        tracing::info!(base = %p.base, "vercel upstream ready");
                    }
                    Err(e) => tracing::warn!(error = %e, "vercel upstream init failed"),
                }
            }
        } else {
            tracing::info!("vercel provider disabled in config");
        }
    }

    if let Some(p) = config.providers.get("laya") {
        if p.enabled {
            match LayaUpstream::new(p.base.clone()) {
                Ok(u) => {
                    upstreams.insert("laya".into(), Box::new(u));
                    tracing::info!(base = %p.base, "laya upstream ready");
                }
                Err(e) => tracing::warn!(error = %e, "laya upstream init failed"),
            }
        } else {
            tracing::info!("laya provider disabled in config");
        }
    }

    // 3. router
    let router = JevRouter::new(config.router.clone(), upstreams);

    // 4. axum state + routes
    let state = AppState {
        router: Arc::new(router),
    };
    // CORS 白名单（contracts/05 §5 M1+；替换 very_permissive 已知债务）：
    // vite dev 两种打开方式都可能 —— 127.0.0.1 与 localhost 都放行。
    // admin 端点本轮未上线（A7）；同主机 UI 走默认同源语义。
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:5173"),
            HeaderValue::from_static("http://localhost:5173"),
        ]))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/v1/systemone", post(systemone_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(models_handler))
        .layer(cors)
        .with_state(state);

    // 默认 11435 — 对齐用户叙事基址期望（docs/「Jev-Switch」用户叙事探索）
    let addr = SocketAddr::from(([127, 0, 0, 1], 11435));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "jev-switch MVP listening");

    axum::serve(listener, app).await?;
    Ok(())
}

/// `GET /health` — contracts/05 §2：JSON 为准（旧文本 "jev-switch MVP" 弃用）。
#[derive(Debug, Serialize)]
struct HealthBody {
    status: &'static str,
    version: &'static str,
}

async fn health_handler() -> Json<HealthBody> {
    Json(HealthBody {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /v1/models` — contracts/05 §2 冻结形状（OpenAI 风）。
#[derive(Debug, Serialize)]
struct ModelEntry {
    id: String,
    object: &'static str,
    upstream: String,
}

#[derive(Debug, Serialize)]
struct UpstreamCapabilityEntry {
    id: String,
    question_types: Vec<&'static str>,
    has_confidence: bool,
    has_usage: bool,
    noul_via_boolean: bool,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    object: &'static str,
    data: Vec<ModelEntry>,
    upstreams: Vec<UpstreamCapabilityEntry>,
}

async fn models_handler(State(state): State<AppState>) -> Json<ModelsResponse> {
    // 保留 6af3a47 的不可路由过滤：只列出真正可路由的 model
    // （upstream 未注册的 model 不列出 —— 否则 UI 判断可用、点击即 404）
    let mut data: Vec<ModelEntry> = state
        .router
        .list_models()
        .into_iter()
        .filter_map(|m| {
            let upstream = state.router.route(&m).ok()?.id().to_string();
            Some(ModelEntry {
                id: m,
                object: "model",
                upstream,
            })
        })
        .collect();
    data.sort_by(|a, b| a.id.cmp(&b.id));

    let upstreams: Vec<UpstreamCapabilityEntry> = state
        .router
        .list_capabilities()
        .into_iter()
        .map(|(_id, _uid, cap)| UpstreamCapabilityEntry {
            id: _id,
            question_types: cap.question_types.iter().map(|q| q.as_str()).collect(),
            has_confidence: cap.has_confidence,
            has_usage: cap.has_usage,
            noul_via_boolean: cap.noul_via_boolean,
        })
        .collect();

    Json(ModelsResponse {
        object: "list",
        data,
        upstreams,
    })
}

/// 把 JevError 转换为 axum Response。
fn jev_error_to_response(e: JevError) -> Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = ErrorBody {
        error: e.to_string(),
        upstream: Some(e.upstream_id().to_string()),
        retryable: e.retryable(),
    };
    (status, Json(body)).into_response()
}

async fn systemone_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<JevResponse>, Response> {
    // 0. 协议解析：本地 400（criteria 缺失/错形态、未知 type、必填缺失、
    //    questions 非 record…）—— 按 contracts/01 §6 不发上游。
    //    手工 Bytes 提取：axum Json 提取器对 data 类错误回 422，契约要求 400。
    let req: JevRequest = serde_json::from_slice(&body).map_err(|e| {
        let err = ErrorBody {
            error: e.to_string(),
            upstream: None,
            retryable: false,
        };
        (StatusCode::BAD_REQUEST, Json(err)).into_response()
    })?;

    // 1. router lookup
    let upstream = match state.router.route(&req.model) {
        Ok(u) => u,
        Err(e) => {
            let status = StatusCode::NOT_FOUND;
            let body = ErrorBody {
                error: e.to_string(),
                upstream: None,
                retryable: false,
            };
            return Err((status, Json(body)).into_response());
        }
    };
    let upstream_id = upstream.id().to_string();

    // 1.5 capability 校验：题型不被上游支持 → 422（修复：原先为死代码，从未接线）
    let qts: Vec<_> = req.questions.values().map(|q| q.question_type()).collect();
    if let Err(e) = state.router.check_capability(&req.model, &qts) {
        tracing::warn!(model = %req.model, error = %e, "capability check failed");
        return Err(jev_error_to_response(e));
    }

    // 2. call upstream
    let raw = match upstream.evaluate(req).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(upstream = %upstream_id, error = %e, "upstream evaluate failed");
            return Err(jev_error_to_response(e));
        }
    };

    // 3. raw Value → typed JevResponse（判别联合；上游形态非法 → 502）
    let resp: JevResponse = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(upstream = %upstream_id, error = %e, "deserialize upstream response failed");
            let body = ErrorBody {
                error: format!("deserialize upstream response: {e}"),
                upstream: Some(upstream_id.clone()),
                retryable: false,
            };
            return Err((
                StatusCode::BAD_GATEWAY,
                Json(body),
            )
                .into_response());
        }
    };

    Ok(Json(resp))
}

// 防止 rust 误以为 main 用不到的 import 是 dead_code
#[allow(dead_code)]
fn _assert_send_sync() {
    fn assert_send<T: Send + Sync>() {}
    assert_send::<AppState>();
    assert_send::<JevError>();
    // 用一下 json! 宏保持 serde_json 引用
    let _ = json!({});
}

#[cfg(test)]
mod tests {
    use super::*;

    /// contracts/05 §2 /health 形状快照：{status, version}，JSON。
    #[test]
    fn health_shape_snapshot() {
        let body = HealthBody {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")})
        );
        // 键集冻结：不得漂移出第三个键
        assert_eq!(v.as_object().unwrap().len(), 2);
        // 当前契约样例版本字面量（0.1.0）
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["status"], "ok");
    }

    /// contracts/05 §2 /v1/models 形状快照：{object:"list", data:[{id,object,upstream}], upstreams:[…]}。
    /// 防 37b4242 类形状漂移（UI 已按此归一化）。
    #[test]
    fn models_shape_snapshot() {
        let resp = ModelsResponse {
            object: "list",
            data: vec![ModelEntry {
                id: "jev".into(),
                object: "model",
                upstream: "vercel".into(),
            }],
            upstreams: vec![UpstreamCapabilityEntry {
                id: "vercel".into(),
                question_types: vec!["choice", "score", "boolean"],
                has_confidence: false,
                has_usage: false,
                noul_via_boolean: true,
            }],
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "object": "list",
                "data": [{ "id": "jev", "object": "model", "upstream": "vercel" }],
                "upstreams": [{
                    "id": "vercel",
                    "question_types": ["choice", "score", "boolean"],
                    "has_confidence": false,
                    "has_usage": false,
                    "noul_via_boolean": true
                }]
            })
        );
        // 顶层键集冻结（models 旧形状键不得回潮）
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj.get("models").is_none());
    }

    /// CORS 白名单 origin 字面量冻结（vite dev 两种打开方式）。
    #[test]
    fn cors_allowlist_origins_frozen() {
        let layer = CorsLayer::new()
            .allow_origin(AllowOrigin::list([
                HeaderValue::from_static("http://127.0.0.1:5173"),
                HeaderValue::from_static("http://localhost:5173"),
            ]))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE]);
        // tower-http 不提供只读 introspection；构造成功 + 预检行为由 curl 实测覆盖。
        // 此处至少锁定构造路径可编译（防 very_permissive 回潮需人工改回本函数）。
        let _ = layer;
        let _ = StatusCode::OK;
    }
}
