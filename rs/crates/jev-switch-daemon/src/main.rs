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
    http::StatusCode,
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
use tower_http::cors::CorsLayer;
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
    let app = Router::new()
        .route("/v1/systemone", post(systemone_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(models_handler))
        // MVP: 允许浏览器从任意 origin 直连 /v1/systemone（dev demo 用）。
        // 后续 M1+ 会收紧 origin 白名单。
        .layer(CorsLayer::very_permissive())
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

async fn health_handler() -> &'static str {
    "jev-switch MVP"
}

#[derive(Debug, Serialize)]
struct ModelEntry {
    model: String,
    upstream: String,
}

#[derive(Debug, Serialize)]
struct UpstreamCapabilityEntry {
    upstream: String,
    question_types: Vec<&'static str>,
    has_confidence: bool,
    has_usage: bool,
    noul_via_boolean: bool,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    models: Vec<ModelEntry>,
    upstreams: Vec<UpstreamCapabilityEntry>,
}

async fn models_handler(State(state): State<AppState>) -> Json<ModelsResponse> {
    // 只列出真正可路由的 model（修复：原先把 upstream 未注册的 model 也列出，
    // upstream 显示为空串，UI 判断可用、点击即 404）
    let mut models: Vec<ModelEntry> = state
        .router
        .list_models()
        .into_iter()
        .filter_map(|m| {
            let upstream = state.router.route(&m).ok()?.id().to_string();
            Some(ModelEntry { model: m, upstream })
        })
        .collect();
    models.sort_by(|a, b| a.model.cmp(&b.model));

    let upstreams: Vec<UpstreamCapabilityEntry> = state
        .router
        .list_capabilities()
        .into_iter()
        .map(|(_id, _uid, cap)| UpstreamCapabilityEntry {
            upstream: _id,
            question_types: cap.question_types.iter().map(|q| q.as_str()).collect(),
            has_confidence: cap.has_confidence,
            has_usage: cap.has_usage,
            noul_via_boolean: cap.noul_via_boolean,
        })
        .collect();

    Json(ModelsResponse { models, upstreams })
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
