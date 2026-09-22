//! Laya 本地后端上游实现（M0.5）
//!
//! 参考 jev-decision-lab `decision-bricks.ts::createLayaRuntime`：
//! - POST `http://127.0.0.1:18765/v1/systemone`
//! - Headers: `Content-Type: application/json`
//! - Body: 原样转发（不翻译 — Laya 真 Jev）
//! - Response: 原样返回
//!
//! 与 Vercel 不同：Laya 完全透传；不需要任何 capability 翻译。

use crate::protocol::SystemOneRequest;
use crate::upstream::{Capabilities, JevError, Upstream};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

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
impl Upstream for LayaUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Capabilities {
        crate::upstream::capabilities_of("laya")
            .expect("laya capability is hardcoded; this is a bug if missing")
    }

    async fn evaluate(&self, req: SystemOneRequest) -> Result<Value, JevError> {
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
            let retryable = crate::upstream::is_retryable_status("laya", status.as_u16());
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: body_text,
                retryable,
            });
        }

        // 3. 解析响应（Laya 给的是 Jev 标准 SystemOneResponse）
        let value: Value = serde_json::from_slice(&raw_bytes).map_err(|e| {
            JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse Laya JSON: {e}"),
            }
        })?;

        // 验证是合法响应（有 answers 字段）
        if !value.is_object() || value.get("answers").is_none() {
            return Err(JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!(
                    "Laya response missing 'answers' field; raw={}",
                    String::from_utf8_lossy(&raw_bytes)
                ),
            });
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn laya_construction_ok() {
        let u = LayaUpstream::new("http://127.0.0.1:18765/v1/systemone".into()).unwrap();
        assert_eq!(u.id(), "laya");
        assert!(u.capabilities().supports(crate::upstream::QuestionType::Noul));
    }
}
