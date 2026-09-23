//! Vercel AI Gateway 上游实现（M0.5 + M0.7 翻译层 · A5：`UpstreamAdapter` + 能力注册制）
//!
//! 参考 jev-decision-lab `decision-bricks.ts::VercelGatewayJevRuntime`：
//! - POST 到 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`
//! - Headers: `Authorization: Bearer <AI_GATEWAY_API_KEY>` + ai-* header
//! - `noul` → `boolean` 出站在组 body 时做（[`normalize_request_for_vercel`]）
//! - `boolean/probability` → `noul` 入站走 [`VercelProtocol::incoming`]（唯一出口路径）
//! - Vercel 不报 `usage`：写 `None`
//!
//! 不变量：调用方在 handler 层只看到 Jev 标准 `type: noul`；Vercel 看到的永远是
//! `type: boolean`（除非原本就是 choice / score）。
//!
//! 能力注册制（07 P2）：[`VERCEL_CAPABILITIES`] 是本 adapter 自报的能力表 ——
//! 内核不再按 id 硬编码查表（原 `capabilities_of("vercel")` 已删）。

use crate::vercel_protocol::{normalize_request_for_vercel, VercelProtocol, VERCEL_UPSTREAM_ID};
use jev_core::adapter::{IncomingCtx, ProtocolAdapter, UpstreamAdapter};
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use reqwest::Client;
use std::time::Duration;

/// Vercel 自报能力表（与 A4 硬编码表逐字段一致 —— 行为不回潮）。
pub const VERCEL_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Boolean],
    has_confidence: false,
    has_usage: false,
    noul_via_boolean: true,
    retryable_status: &[408, 429, 500, 502, 503, 504],
};

/// Vercel 强制要求的 ai-* header（参考 TS 实现）。
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
    protocol: VercelProtocol,
}

impl VercelUpstream {
    pub fn new(base: String, api_key: String) -> Result<Self, JevError> {
        if api_key.is_empty() {
            return Err(JevError::Config {
                upstream_id: VERCEL_UPSTREAM_ID.into(),
                message: "AI_GATEWAY_API_KEY is empty".into(),
            });
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            // 强制 IPv4 first：Cloudflare IPv6 在很多机器不可达（MVP 依赖系统 dns_order）
            .build()
            .map_err(|e| JevError::Config {
                upstream_id: VERCEL_UPSTREAM_ID.into(),
                message: format!("reqwest build failed: {e}"),
            })?;
        Ok(Self {
            id: VERCEL_UPSTREAM_ID.into(),
            base,
            api_key,
            http,
            protocol: VercelProtocol,
        })
    }
}

#[async_trait::async_trait]
impl UpstreamAdapter for VercelUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    /// 能力注册制：返回本 adapter 的静态能力表（不再查 `capabilities_of`）。
    fn capabilities(&self) -> Capabilities {
        VERCEL_CAPABILITIES
    }

    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        // 1. 出站方言化（wire 级 noul → boolean；req 原样保留，语义层出口统一 noul）
        let normalized = normalize_request_for_vercel(&req).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!("serialize normalized request: {e}"),
        })?;

        // 2. 构造请求。Vercel gateway body 顶层只需要 state + questions
        //    （model 走 ai-model-id header）。
        let vercel_body = serde_json::json!({
            "state": normalized.get("state").cloned().unwrap_or(serde_json::Value::Null),
            "questions": normalized.get("questions").cloned().unwrap_or(serde_json::json!({})),
        });

        let mut req_builder = self
            .http
            .post(&self.base)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("ai-model-id", &req.model);
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
            // 可重试判定用自家能力表（注册制；替代已删 is_retryable_status(id, …)）
            let retryable = self.capabilities().is_retryable_status(status.as_u16());
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: body_text,
                retryable,
            });
        }

        // 4. 入站归一化（冻结 ProtocolAdapter::incoming —— boolean → noul 唯一出口）
        let ctx = IncomingCtx {
            request: &req,
            original: &req,
            status: status.as_u16(),
        };
        self.protocol.incoming(&raw_bytes, &ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::{Criteria, Question};
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn vercel_request_translation_in_normalize() {
        // 验证 `normalize_request_for_vercel` 把 noul 改 boolean（wire 级）
        let mut questions = BTreeMap::new();
        questions.insert(
            "q".to_string(),
            Question::Noul {
                instructions: "test".into(),
                criteria: Criteria::Bool {
                    r#true: "y".into(),
                    r#false: "n".into(),
                },
            },
        );
        let req = JevRequest {
            model: "typesafe-ai/jev".into(),
            state: serde_json::json!("s"),
            questions,
        };
        let normalized = normalize_request_for_vercel(&req).unwrap();
        assert_eq!(normalized["questions"]["q"]["type"], "boolean");
        // criteria 原样
        assert_eq!(normalized["questions"]["q"]["criteria"]["true"], "y");
    }

    #[tokio::test]
    async fn empty_api_key_rejected() {
        let r = VercelUpstream::new("https://x".into(), "".into());
        assert!(r.is_err());
    }

    /* ── 能力注册制回归（替代原 capabilities_of 三连测） ───────── */

    #[test]
    fn vercel_capability_self_reported() {
        let cap = VERCEL_CAPABILITIES;
        assert!(cap.supports(QuestionType::Boolean));
        assert!(cap.supports(QuestionType::Noul)); // via translation
        assert!(!cap.has_confidence);
        assert!(cap.noul_via_boolean);
        assert!(cap.is_retryable_status(429));
        assert!(!cap.is_retryable_status(400));
    }

    #[tokio::test]
    async fn vercel_upstream_capabilities_via_trait() {
        // daemon 装配路径经 UpstreamAdapter trait 取能力（注册制接线）
        let u = VercelUpstream::new("http://127.0.0.1:1/x".into(), "k".into()).unwrap();
        let cap: Capabilities = UpstreamAdapter::capabilities(&u);
        assert_eq!(cap.noul_via_boolean, true);
        assert!(cap.retryable_status.contains(&429));
        assert_eq!(VercelProtocol.dialect(), "vercel_boolean");
    }
}
