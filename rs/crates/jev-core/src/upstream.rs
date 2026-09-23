//! 错误分类 + Capability 表（M0.3/M0.4 · A5：能力改为 adapter 注册制）
//!
//! - [`JevError`]：上游/路由错误统一分类；`retryable()` 是 failover 唯一门闸
//!   （429/5xx/Timeout/Network）；`http_status()` 给出对外映射（contracts/05 §3）
//! - [`Capabilities`]：能力描述结构；**取值一律来自 `UpstreamAdapter::capabilities()`**
//!   （07 P2：删除 `capabilities_of` 按 id 硬编码 match）
//! - 冻结双 trait（`ProtocolAdapter` / `UpstreamAdapter`）+ `Registry` 见 [`crate::adapter`]

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
    // A9：原 `#[allow(dead_code)]` 已删 —— `Registry::invoke` 与
    // `Router::check_capability` 均在构造本变体（422 路径），非死代码。
    #[error("capability mismatch for upstream {upstream_id}: {detail}")]
    Capability {
        upstream_id: String,
        detail: String,
    },
    #[error("upstream {upstream_id} returned invalid JSON: {message}")]
    BadResponse { upstream_id: String, message: String },
    #[error("upstream {upstream_id} config error: {message}")]
    Config { upstream_id: String, message: String },
    /// 无匹配边（contracts/03 §4 → 404 `UnknownModel`）。
    /// A5 扩充：`Registry::invoke` 冻结返回 `Result<_, JevError>`，路由错误并入本枚举。
    #[error("no upstream registered for model '{0}'")]
    UnknownModel(String),
    /// 边终点既非已注册上游也无出边（→ 404，文案与旧 `RouterError` 一致）。
    #[error("upstream '{0}' not registered in router")]
    UnknownUpstream(String),
}

impl JevError {
    pub fn upstream_id(&self) -> &str {
        match self {
            JevError::Upstream { upstream_id, .. }
            | JevError::Timeout { upstream_id }
            | JevError::Network { upstream_id, .. }
            | JevError::Capability { upstream_id, .. }
            | JevError::BadResponse { upstream_id, .. }
            | JevError::Config { upstream_id, .. }
            | JevError::UnknownUpstream(upstream_id) => upstream_id,
            JevError::UnknownModel(m) => m,
        }
    }

    /// 是否携带 `upstream` 字段语义（错误体里 None = 纯路由/协议错误）。
    /// `UnknownModel` / `UnknownUpstream` 的错误体 `upstream` 为 None（A4 行为保留）。
    pub fn error_body_upstream(&self) -> Option<&str> {
        match self {
            JevError::UnknownModel(_) | JevError::UnknownUpstream(_) => None,
            other => Some(other.upstream_id()),
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
            JevError::UnknownModel(_) | JevError::UnknownUpstream(_) => 404,
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
            // 经翻译层可接 noul 的上游（如 boolean 方言）返回 true 让路由能选它
            return true;
        }
        self.question_types.contains(&qt)
    }

    /// 按本能力表判定 HTTP status 是否可重试（adapter 报错时用自家表，替代
    /// 已删除的 `capabilities_of` 全局查表）。
    pub fn is_retryable_status(&self, status: u16) -> bool {
        self.retryable_status.contains(&status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_noul_via_boolean_shortcut() {
        // 翻译层型能力（boolean 方言上游）对 Noul 短路为 true
        let cap = Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Boolean],
            has_confidence: false,
            has_usage: false,
            noul_via_boolean: true,
            retryable_status: &[429],
        };
        assert!(cap.supports(QuestionType::Noul)); // via translation
        assert!(cap.supports(QuestionType::Boolean));
        assert!(cap.is_retryable_status(429));
        assert!(!cap.is_retryable_status(400));
    }

    #[test]
    fn local_style_caps_do_not_retry() {
        // 本地类上游 retryable_status=[0]（空表语义）→ 任何真实 status 都不重试
        let cap = Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Noul],
            has_confidence: true,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[0],
        };
        assert!(cap.supports(QuestionType::Noul));
        assert!(!cap.supports(QuestionType::Boolean));
        assert!(!cap.is_retryable_status(429));
        assert!(!cap.is_retryable_status(500));
    }

    #[test]
    fn unknown_model_error_is_404_not_retryable() {
        let e = JevError::UnknownModel("ghost".into());
        assert_eq!(e.http_status(), 404);
        assert!(!e.retryable());
        assert!(e.error_body_upstream().is_none());
        assert_eq!(e.to_string(), "no upstream registered for model 'ghost'");
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
