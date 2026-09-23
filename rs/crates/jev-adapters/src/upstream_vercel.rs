//! Vercel AI Gateway 上游实现（M0.5 + M0.7 翻译层落点 · P0-1 迁入 jev-adapters）
//!
//! 参考 jev-decision-lab `decision-bricks.ts::VercelGatewayJevRuntime`：
//! - POST 到 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`
//! - Headers: `Authorization: Bearer <AI_GATEWAY_API_KEY>` + 4 个 `ai-*` header
//! - `noul` → `boolean` 翻译在入口（`jev_core::translate::normalize_request_for_vercel`）
//! - `boolean: true/false` → `noul` 在出口（`jev_core::translate::build_jev_response_from_vercel`）
//! - Vercel 不报 `usage`：写 `None`
//!
//! 不变量：调用方在 handler 层只看到 Jev 标准 `type: noul`；Vercel 看到的永远是
//! `type: boolean`（除非原本就是 choice / score）。

use jev_core::translate::{build_jev_response_from_vercel, normalize_request_for_vercel};
use jev_core::upstream::{Capabilities, JevError, Upstream};
use jev_protocol::{SystemOneRequest, VercelResponse};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// Vercel 强制要求的 4 个 ai-* header（参考 TS 实现）。
const VERCEL_HEADERS: &[(&str, &str)] = &[
    ("ai-gateway-protocol-version", "0.0.1"),
    ("ai-gateway-auth-method", "api-key"),
    ("ai-evaluation-model-specification-version", "4"),
];

pub struct VercelUpstream {
    id: String,
    base: String,
    api_key: String,
    http: Client,
}

impl VercelUpstream {
    pub fn new(base: String, api_key: String) -> Result<Self, JevError> {
        if api_key.is_empty() {
            return Err(JevError::Config {
                upstream_id: "vercel".into(),
                message: "AI_GATEWAY_API_KEY is empty".into(),
            });
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            // 强制 IPv4 first：Cloudflare IPv6 在很多机器不可达
            // reqwest 默认走 tokio 的 DNS（系统解析），借 set_override_dns 不太灵活；
            // MVP 不强制，依赖系统 dns_order；本机若遇 ENETUNREACH 可改成
            // 在 Client::builder().resolve(...) 强制解析。
            .build()
            .map_err(|e| JevError::Config {
                upstream_id: "vercel".into(),
                message: format!("reqwest build failed: {e}"),
            })?;
        Ok(Self {
            id: "vercel".into(),
            base,
            api_key,
            http,
        })
    }
}

#[async_trait::async_trait]
impl Upstream for VercelUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Capabilities {
        // Capability 与 `capabilities_of("vercel")` 一致：直接查表。
        jev_core::upstream::capabilities_of("vercel")
            .expect("vercel capability is hardcoded; this is a bug if missing")
    }

    async fn evaluate(&self, req: SystemOneRequest) -> Result<Value, JevError> {
        // 1. 翻译请求：noul → boolean
        let normalized = normalize_request_for_vercel(req);
        let original_request_for_denormalize = normalized.clone();
        let body = serde_json::to_value(&normalized).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!("serialize normalized request: {e}"),
        })?;

        // 2. 构造请求。Vercel gateway 路径是 `/v4/ai/evaluation-model`，
        //    body 顶层只需要 `state` + `questions`（model 走 header）。
        let vercel_body = serde_json::json!({
            "state": body.get("state").cloned().unwrap_or(serde_json::Value::Null),
            "questions": body.get("questions").cloned().unwrap_or(serde_json::json!({})),
        });

        let mut req_builder = self
            .http
            .post(&self.base)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("ai-model-id", &normalized.model);
        for (k, v) in VERCEL_HEADERS {
            req_builder = req_builder.header(*k, *v);
        }

        // 3. 发请求
        let resp = req_builder
            .json(&vercel_body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    JevError::Timeout {
                        upstream_id: self.id.clone(),
                    }
                } else {
                    JevError::Network {
                        upstream_id: self.id.clone(),
                        message: e.to_string(),
                    }
                }
            })?;

        let status = resp.status();
        let raw_bytes = resp.bytes().await.map_err(|e| JevError::Network {
            upstream_id: self.id.clone(),
            message: format!("read body: {e}"),
        })?;

        if !status.is_success() {
            let body_text = String::from_utf8_lossy(&raw_bytes).to_string();
            let retryable = jev_core::upstream::is_retryable_status("vercel", status.as_u16());
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: body_text,
                retryable,
            });
        }

        // 4. 解析 Vercel 响应
        let vercel_resp: VercelResponse = serde_json::from_slice(&raw_bytes).map_err(|e| {
            JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse Vercel JSON: {e}; raw={}", String::from_utf8_lossy(&raw_bytes)),
            }
        })?;

        // 5. 翻译回 Jev 标准：boolean → noul
        let jev_resp = build_jev_response_from_vercel(&vercel_resp, &original_request_for_denormalize);

        // 6. 返回 serde_json::Value（router / handler 进一步用）
        serde_json::to_value(&jev_resp).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!("serialize Jev response: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::DecisionQuestion;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn vercel_request_translation_in_normalize() {
        // 验证 `normalize_request_for_vercel` 把 noul 改 boolean
        // （在 evaluate 内部调，这里我们单独 verify 翻译函数）
        let mut questions = BTreeMap::new();
        questions.insert(
            "q".to_string(),
            DecisionQuestion::Noul {
                instructions: "test".into(),
                criteria: Some(serde_json::json!({"true": "y", "false": "n"})),
            },
        );
        let req = SystemOneRequest {
            model: "typesafe-ai/jev".into(),
            state: serde_json::json!("s"),
            questions,
        };
        let normalized = normalize_request_for_vercel(req);
        assert_eq!(
            normalized.questions.get("q").unwrap().question_type_str(),
            "boolean"
        );
    }

    #[tokio::test]
    async fn empty_api_key_rejected() {
        let r = VercelUpstream::new("https://x".into(), "".into());
        assert!(r.is_err());
    }
}
