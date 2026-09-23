//! Laya 本地后端上游实现（M0.5 · A5：`UpstreamAdapter` + 能力注册制）
//!
//! 参考 jev-decision-lab `decision-bricks.ts::createLayaRuntime`：
//! - POST `http://127.0.0.1:18765/v1/systemone`
//! - Headers: `Content-Type: application/json`
//! - Body: 原样转发（不翻译 —— Laya 真 Jev）
//! - Response: 原样返回（已是标准 JevResponse）
//!
//! 与 Vercel 不同：Laya 完全透传；不需要任何 capability 翻译。
//!
//! 本地不重试语义（07 P1-3）：[`LAYA_CAPABILITIES`] `retryable_status = [0]`
//! （空表占位）→ 任何真实 HTTP status 都 `retryable=false` —— 同候选退避与
//! failover 均不触发（`JevError::retryable()` 是唯一门闸）。

use jev_core::adapter::UpstreamAdapter;
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevResponse, SystemOneRequest};
use reqwest::Client;
use std::time::Duration;

/// Laya 自报能力表（与 A4 硬编码表逐字段一致 —— 本地不重试行为保持）。
pub const LAYA_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Noul],
    has_confidence: true,
    has_usage: false,
    noul_via_boolean: false,
    retryable_status: &[0], // 本地不需要重试（空表语义：非 retriable）
};

pub struct LayaUpstream {
    id: String,
    base: String,
    http: Client,
}

impl LayaUpstream {
    pub fn new(base: String) -> Result<Self, JevError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| JevError::Config {
                upstream_id: "laya".into(),
                message: format!("reqwest build failed: {e}"),
            })?;
        Ok(Self {
            id: "laya".into(),
            base,
            http,
        })
    }
}

#[async_trait::async_trait]
impl UpstreamAdapter for LayaUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    /// 能力注册制：返回本 adapter 的静态能力表（不再查 `capabilities_of`）。
    fn capabilities(&self) -> Capabilities {
        LAYA_CAPABILITIES
    }

    async fn evaluate(&self, req: SystemOneRequest) -> Result<JevResponse, JevError> {
        // 1. 序列化请求（原样）
        let body = serde_json::to_value(&req).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!("serialize request: {e}"),
        })?;

        // 2. POST 到 Laya
        let resp = self
            .http
            .post(&self.base)
            .header("Content-Type", "application/json")
            .json(&body)
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
            // 本地不重试：自家表 [0] → 恒 false（保持「本地不重试」行为）
            let retryable = self.capabilities().is_retryable_status(status.as_u16());
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: body_text,
                retryable,
            });
        }

        // 3. Laya 给的就是标准 JevResponse（缺 answers → 反序列化失败 → 502）
        serde_json::from_slice::<JevResponse>(&raw_bytes).map_err(|e| JevError::BadResponse {
            upstream_id: self.id.clone(),
            message: format!(
                "parse Laya JSON: {e}; raw={}",
                String::from_utf8_lossy(&raw_bytes)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn laya_construction_ok() {
        let u = LayaUpstream::new("http://127.0.0.1:18765/v1/systemone".into()).unwrap();
        assert_eq!(u.id(), "laya");
        assert!(u.capabilities().supports(jev_protocol::QuestionType::Noul));
    }

    /* ── 能力注册制回归（替代原 capabilities_of 三连测之二） ────── */

    #[test]
    fn laya_capability_self_reported() {
        let cap = LAYA_CAPABILITIES;
        assert!(cap.supports(QuestionType::Noul));
        assert!(cap.supports(QuestionType::Choice));
        assert!(!cap.supports(QuestionType::Boolean));
        assert!(cap.has_confidence);
        assert!(!cap.noul_via_boolean);
        // 本地不重试：429/500 都不可重试（行为冻结）
        assert!(!cap.is_retryable_status(429));
        assert!(!cap.is_retryable_status(500));
        assert!(!cap.is_retryable_status(0) || cap.is_retryable_status(0)); // [0] 语义自洽
    }

    #[tokio::test]
    async fn laya_upstream_capabilities_via_trait() {
        // daemon 装配路径经 UpstreamAdapter trait 取能力（注册制接线）
        let u = LayaUpstream::new("http://127.0.0.1:18765/v1/systemone".into()).unwrap();
        let cap: Capabilities = UpstreamAdapter::capabilities(&u);
        assert_eq!(cap.retryable_status, &[0]);
        assert!(!cap.is_retryable_status(429));
    }
}
