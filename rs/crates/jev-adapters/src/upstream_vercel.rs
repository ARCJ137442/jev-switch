//! Vercel AI Gateway 上游实现（M0.5 + M0.7 翻译层 · A5：`UpstreamAdapter` + 能力注册制）
//!
//! 参考 jev-decision-lab `decision-bricks.ts::VercelGatewayJevRuntime`：
//! - POST TypeSafe-compatible Jev requests to `/typesafe/v1/systemone`
//! - Headers: `Authorization: Bearer <AI_GATEWAY_API_KEY>`
//! - Preserve the Jev request body, including `model` and `type: noul`
//! - Normalize the TypeSafe response through [`VercelProtocol::incoming`]
//!
//! 当前 TypeSafe-compatible 接口接收原生 Jev question type；Choice 与 Score 的
//! confidence 由上游返回并原样保留。旧 Evaluation API 的 boolean 翻译不参与活动请求路径。
//!
//! 能力注册制（07 P2）：[`VERCEL_CAPABILITIES`] 是本 adapter 自报的能力表 ——
//! 内核不再按 id 硬编码查表（原 `capabilities_of("vercel")` 已删）。

use crate::vercel_protocol::{VercelProtocol, VERCEL_UPSTREAM_ID};
use jev_core::adapter::{IncomingCtx, ProtocolAdapter, UpstreamAdapter};
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use reqwest::Client;
use std::time::Duration;

/// Vercel 自报能力表（与 A4 硬编码表逐字段一致 —— 行为不回潮）。
pub const VERCEL_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[
        QuestionType::Choice,
        QuestionType::Score,
        QuestionType::Noul,
    ],
    has_confidence: true,
    has_usage: true,
    noul_via_boolean: false,
    retryable_status: &[408, 429, 500, 502, 503, 504],
};

pub struct VercelUpstream {
    id: String,
    base: String,
    api_key: String,
    http: Client,
    protocol: VercelProtocol,
}

impl VercelUpstream {
    pub fn new(base: String, api_key: String) -> Result<Self, JevError> {
        Self::new_with_id(VERCEL_UPSTREAM_ID.into(), base, api_key)
    }

    pub fn new_with_id(id: String, base: String, api_key: String) -> Result<Self, JevError> {
        if api_key.is_empty() {
            return Err(JevError::Config {
                upstream_id: id.clone(),
                message: "AI_GATEWAY_API_KEY is empty".into(),
            });
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            // 强制 IPv4 first：Cloudflare IPv6 在很多机器不可达（MVP 依赖系统 dns_order）
            .build()
            .map_err(|e| JevError::Config {
                upstream_id: id.clone(),
                message: format!("reqwest build failed: {e}"),
            })?;
        Ok(Self {
            id,
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
        // The TypeSafe endpoint speaks Jev directly; model and question types belong
        // in the request body. Keeping this whole avoids silently changing noul to
        // the legacy Evaluation API's boolean dialect.
        let resp = self
            .http
            .post(&self.base)
            .bearer_auth(&self.api_key)
            .json(&req)
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

        // Normalize the TypeSafe response through the adapter's single incoming path.
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
    use crate::vercel_protocol::normalize_request_for_vercel;
    use jev_protocol::{Criteria, Question};
    use serde_json::json;
    use std::collections::BTreeMap;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
        assert!(!cap.supports(QuestionType::Boolean));
        assert!(cap.supports(QuestionType::Noul));
        assert!(cap.supports(QuestionType::Choice));
        assert!(cap.supports(QuestionType::Score));
        assert!(cap.has_confidence);
        assert!(cap.has_usage);
        assert!(!cap.noul_via_boolean);
        assert!(cap.is_retryable_status(429));
        assert!(!cap.is_retryable_status(400));
    }

    #[tokio::test]
    async fn vercel_upstream_capabilities_via_trait() {
        // daemon 装配路径经 UpstreamAdapter trait 取能力（注册制接线）
        let u = VercelUpstream::new("http://127.0.0.1:1/x".into(), "k".into()).unwrap();
        let cap: Capabilities = UpstreamAdapter::capabilities(&u);
        assert_eq!(cap.noul_via_boolean, false);
        assert!(cap.retryable_status.contains(&429));
        assert_eq!(VercelProtocol.dialect(), "typesafe_jev");
    }

    #[tokio::test]
    async fn typesafe_http_api_sends_jev_body_and_parses_usage() {
        let server = MockServer::start().await;
        let mut questions = std::collections::BTreeMap::new();
        questions.insert(
            "is_true".to_string(),
            Question::Noul {
                instructions: "Is the statement true?".into(),
                criteria: Criteria::Bool {
                    r#true: "The statement is true.".into(),
                    r#false: "The statement is false.".into(),
                },
            },
        );
        let request = JevRequest {
            model: "typesafe-ai/jev".into(),
            state: json!({"statement":"The meeting begins at 10 AM."}),
            questions,
        };
        Mock::given(method("POST"))
            .and(path("/typesafe/v1/systemone"))
            .and(header("authorization", "Bearer test-vck-fixture-key"))
            .and(body_json(json!({
                "model":"typesafe-ai/jev",
                "state":{"statement":"The meeting begins at 10 AM."},
                "questions":{"is_true":{
                    "type":"noul",
                    "instructions":"Is the statement true?",
                    "criteria":{"true":"The statement is true.","false":"The statement is false."}
                }}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model":"typesafe-ai/jev",
                "answers":{"is_true":{"type":"noul","noul":0.98}},
                "usage":{"inputTokens":12,"outputTokens":3},
                "providerMetadata":{"gateway":{"gatewayCost":"0.000042","routing":{"finalProvider":"typesafe-ai"}}}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let upstream = VercelUpstream::new(
            format!("{}/typesafe/v1/systemone", server.uri()),
            "test-vck-fixture-key".into(),
        )
        .unwrap();
        let response = upstream.evaluate(request).await.unwrap();
        assert_eq!(response.model.as_deref(), Some("typesafe-ai/jev"));
        assert_eq!(response.usage.as_ref().unwrap().input_tokens, Some(12));
        assert_eq!(response.usage.as_ref().unwrap().output_tokens, Some(3));
        assert_eq!(response.cost_usd, Some(0.000042));
        assert_eq!(
            response.extra["providerMetadata"]["gateway"]["routing"]["finalProvider"],
            "typesafe-ai"
        );
        let jev_protocol::Answer::Noul(answer) = &response.answers["is_true"] else {
            panic!("expected a Jev noul answer");
        };
        assert_eq!(answer.noul, Some(0.98));
    }
}
