//! Upstream trait + Capability table（M0.3 + M0.4 · P0-1 迁入 jev-core）
//!
//! 参考 03-上游类别与协议兼容矩阵 §2.3 capability 表（精简为 M0 范围：C2 Vercel + C5 Laya）。
//! 参考 04-架构设计 §2.4 关键 trait。
//!
//! 设计选择：
//! - `evaluate` 返回 `Result<serde_json::Value, JevError>`：上游能产出 Vercel 形态 / Laya 形态
//!   / 未来 broker 形态，统一用 Value 在边界换形态，让 Vercel boolean→noul 等翻译
//!   在外层 `translate.rs` 处理。
//! - 使用 Rust 1.75+ 的 native `async fn` in trait，无需 `async_trait`。

use jev_protocol::SystemOneRequest;
use serde_json::Value;
use thiserror::Error;

// P0-1：QuestionType 随协议类型迁入 jev-protocol（DecisionQuestion::question_type() 需要它），
// 这里重导出以保持 `jev_core::upstream::QuestionType` 路径不变。
pub use jev_protocol::QuestionType;

/// 上游错误分类。`retryable` 标志由调用方（router / handler）决定是否重试。
#[derive(Debug, Error)]
pub enum JevError {
    #[error("upstream {upstream_id} returned HTTP {status}: {body}")]
    Upstream {
        upstream_id: String,
        status: u16,
        body: String,
        retryable: bool,
    },
    #[error("upstream {upstream_id} timed out")]
    Timeout { upstream_id: String },
    #[error("upstream {upstream_id} network error: {message}")]
    Network { upstream_id: String, message: String },
    #[allow(dead_code)]
    #[error("capability mismatch for upstream {upstream_id}: {detail}")]
    Capability {
        upstream_id: String,
        detail: String,
    },
    #[error("upstream {upstream_id} returned invalid JSON: {message}")]
    BadResponse { upstream_id: String, message: String },
    #[error("upstream {upstream_id} config error: {message}")]
    Config { upstream_id: String, message: String },
}

impl JevError {
    pub fn upstream_id(&self) -> &str {
        match self {
            JevError::Upstream { upstream_id, .. }
            | JevError::Timeout { upstream_id }
            | JevError::Network { upstream_id, .. }
            | JevError::Capability { upstream_id, .. }
            | JevError::BadResponse { upstream_id, .. }
            | JevError::Config { upstream_id, .. } => upstream_id,
        }
    }

    pub fn retryable(&self) -> bool {
        match self {
            JevError::Upstream { retryable, .. } => *retryable,
            JevError::Timeout { .. } | JevError::Network { .. } => true,
            _ => false,
        }
    }

    /// 转换为 HTTP status code（给 axum handler 用）。
    pub fn http_status(&self) -> u16 {
        match self {
            JevError::Upstream { status, retryable, .. } => {
                if *retryable && (*status == 429 || *status == 504 || *status >= 500) {
                    503
                } else {
                    *status
                }
            }
            JevError::Timeout { .. } | JevError::Network { .. } => 503,
            JevError::Capability { .. } => 422,
            JevError::BadResponse { .. } => 502,
            JevError::Config { .. } => 500,
        }
    }
}

/// 上游能力描述。
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// 该上游收哪些问题类型。
    pub question_types: &'static [QuestionType],
    /// 能否给 confidence 字段。
    pub has_confidence: bool,
    /// 能否给 usage 字段。
    pub has_usage: bool,
    /// noul 是否需要翻译为 boolean（Vercel 适配）。
    pub noul_via_boolean: bool,
    /// 该上游重试策略：哪些 HTTP status 视为瞬时错误。
    pub retryable_status: &'static [u16],
}

impl Capabilities {
    pub fn supports(&self, qt: QuestionType) -> bool {
        if self.noul_via_boolean && qt == QuestionType::Noul {
            // Vercel 通过翻译层接 noul；这里返回 true 让路由能选它
            return true;
        }
        self.question_types.contains(&qt)
    }
}

/// 按上游 id 查 capability（M0.4 capability 表）。
///
/// M0 只硬编码 2 个：Vercel + Laya。其他 id 返回 `None`。
pub fn capabilities_of(id: &str) -> Option<Capabilities> {
    match id {
        "vercel" => Some(Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Boolean],
            has_confidence: false,
            has_usage: false,
            noul_via_boolean: true,
            retryable_status: &[408, 429, 500, 502, 503, 504],
        }),
        "laya" => Some(Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Noul],
            has_confidence: true,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[0], // 本地不需要重试（空表语义：非 retriable）
        }),
        _ => None,
    }
}

/// 上游抽象。用 `async_trait` 是为了让 trait dyn-compatible（Rust 1.93
/// 还未稳定 `async fn` in dyn trait）。
#[async_trait::async_trait]
pub trait Upstream: Send + Sync {
    /// 上游在 router 里的逻辑 id（如 "vercel" / "laya"）。
    fn id(&self) -> &str;

    /// 上游能力。
    fn capabilities(&self) -> Capabilities;

    /// 真正调用上游。返回上游原始响应（Vercel 形态 / Laya 形态等），
    /// 翻译在 `translate.rs` 里做，upstream 只负责 HTTP。
    async fn evaluate(&self, req: SystemOneRequest) -> Result<Value, JevError>;
}

/// 根据 HTTP status 判定是否 retryable（结合 capability 表）。
pub fn is_retryable_status(upstream_id: &str, status: u16) -> bool {
    match capabilities_of(upstream_id) {
        Some(cap) => cap.retryable_status.contains(&status),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vercel_capability() {
        let cap = capabilities_of("vercel").unwrap();
        assert!(cap.supports(QuestionType::Boolean));
        assert!(cap.supports(QuestionType::Noul)); // via translation
        assert!(!cap.has_confidence);
        assert!(cap.noul_via_boolean);
        assert!(cap.retryable_status.contains(&429));
    }

    #[test]
    fn laya_capability() {
        let cap = capabilities_of("laya").unwrap();
        assert!(cap.supports(QuestionType::Noul));
        assert!(cap.supports(QuestionType::Choice));
        assert!(!cap.supports(QuestionType::Boolean));
        assert!(cap.has_confidence);
        assert!(!cap.noul_via_boolean);
    }

    #[test]
    fn unknown_capability() {
        assert!(capabilities_of("openrouter").is_none());
    }

    #[test]
    fn error_status_mapping() {
        let e = JevError::Upstream {
            upstream_id: "vercel".into(),
            status: 429,
            body: "rate limited".into(),
            retryable: true,
        };
        // 429 retryable → 503 to caller
        assert_eq!(e.http_status(), 503);
        assert!(e.retryable());

        let e2 = JevError::Upstream {
            upstream_id: "vercel".into(),
            status: 400,
            body: "bad".into(),
            retryable: false,
        };
        assert_eq!(e2.http_status(), 400);
        assert!(!e2.retryable());

        assert_eq!(
            JevError::Capability {
                upstream_id: "x".into(),
                detail: "no noul".into()
            }
            .http_status(),
            422
        );
    }
}
