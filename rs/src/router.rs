//! 简单 Router（M0.6）
//!
//! MVP 策略：静态 `model → upstream` 映射（来自 config.toml）。
//! capability 校验由 handler 在转发前调用 `check_capability`，不匹配转 422
//! （见 main.rs `systemone_handler`）。
//!
//! 参考 06- §阶段 5 Router。

use crate::upstream::{Capabilities, JevError, QuestionType, Upstream};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("no upstream registered for model '{0}'")]
    UnknownModel(String),
    #[error("upstream '{0}' not registered in router")]
    UnknownUpstream(String),
}

pub struct Router {
    /// model id → upstream id（如 "laya-english" → "laya"）
    mapping: HashMap<String, String>,
    upstreams: HashMap<String, Box<dyn Upstream>>,
}

impl Router {
    pub fn new(
        mapping: HashMap<String, String>,
        upstreams: HashMap<String, Box<dyn Upstream>>,
    ) -> Self {
        Self { mapping, upstreams }
    }

    /// 根据 model id 拿到对应 upstream。
    pub fn route(&self, model: &str) -> Result<&dyn Upstream, RouterError> {
        let upstream_id = self
            .mapping
            .get(model)
            .ok_or_else(|| RouterError::UnknownModel(model.to_string()))?;
        self.upstreams
            .get(upstream_id)
            .map(|b| b.as_ref())
            .ok_or_else(|| RouterError::UnknownUpstream(upstream_id.clone()))
    }

    /// 所有对外可见的 model id（router 持有的 mapping keys）。
    pub fn list_models(&self) -> Vec<String> {
        let mut v: Vec<String> = self.mapping.keys().cloned().collect();
        v.sort();
        v
    }

    /// 所有上游的 capability 列表（用于 `/v1/models` 端点）。
    pub fn list_capabilities(&self) -> Vec<(String, String, Capabilities)> {
        let mut v: Vec<(String, String, Capabilities)> = self
            .upstreams
            .iter()
            .map(|(id, up)| (id.clone(), up.id().to_string(), up.capabilities()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// 校验 capability：检查 request 的所有问题类型是否被目标 upstream 接受。
    pub fn check_capability(
        &self,
        model: &str,
        question_types: &[QuestionType],
    ) -> Result<(), JevError> {
        let upstream = self.route(model).map_err(|e| match e {
            RouterError::UnknownModel(m) => JevError::Capability {
                upstream_id: m.clone(),
                detail: format!("unknown model: {m}"),
            },
            RouterError::UnknownUpstream(u) => JevError::Capability {
                upstream_id: u.clone(),
                detail: format!("upstream '{u}' not registered"),
            },
        })?;
        let cap = upstream.capabilities();
        for qt in question_types {
            if !cap.supports(*qt) {
                return Err(JevError::Capability {
                    upstream_id: upstream.id().to_string(),
                    detail: format!(
                        "model '{}' requires question type '{}' but upstream '{}' cannot handle it",
                        model,
                        qt.as_str(),
                        upstream.id()
                    ),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SystemOneRequest;
    use crate::upstream::QuestionType;


    /// Mock upstream for router unit tests.
    struct MockUpstream {
        id: String,
        cap: Capabilities,
    }

    #[async_trait::async_trait]
    impl Upstream for MockUpstream {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                question_types: self.cap.question_types,
                has_confidence: self.cap.has_confidence,
                has_usage: self.cap.has_usage,
                noul_via_boolean: self.cap.noul_via_boolean,
                retryable_status: self.cap.retryable_status,
            }
        }
        async fn evaluate(
            &self,
            _req: SystemOneRequest,
        ) -> Result<serde_json::Value, JevError> {
            Ok(serde_json::json!({"answers": {}}))
        }
    }

    fn mock(id: &str, qts: &'static [QuestionType]) -> Box<dyn Upstream> {
        Box::new(MockUpstream {
            id: id.to_string(),
            cap: Capabilities {
                question_types: qts,
                has_confidence: true,
                has_usage: false,
                noul_via_boolean: false,
                retryable_status: &[0],
            },
        })
    }

    #[test]
    fn route_known_model() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let mut upstreams: HashMap<String, Box<dyn Upstream>> = HashMap::new();
        upstreams.insert("u-a".into(), mock("u-a", &[QuestionType::Noul]));

        let r = Router::new(mapping, upstreams);
        let up = r.route("m-a").unwrap();
        assert_eq!(up.id(), "u-a");
    }

    #[test]
    fn route_unknown_model() {
        let r = Router::new(HashMap::new(), HashMap::new());
        assert!(matches!(r.route("nope"), Err(RouterError::UnknownModel(_))));
    }

    #[test]
    fn route_known_model_unknown_upstream() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "ghost".into());
        let r = Router::new(mapping, HashMap::new());
        assert!(matches!(
            r.route("m-a"),
            Err(RouterError::UnknownUpstream(_))
        ));
    }

    #[test]
    fn list_models_sorted() {
        let mut mapping = HashMap::new();
        mapping.insert("zeta".into(), "z".into());
        mapping.insert("alpha".into(), "a".into());
        let r = Router::new(mapping, HashMap::new());
        assert_eq!(r.list_models(), vec!["alpha", "zeta"]);
    }

    #[test]
    fn capability_check_passes() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let mut upstreams = HashMap::new();
        upstreams.insert("u-a".into(), mock("u-a", &[QuestionType::Noul]));
        let r = Router::new(mapping, upstreams);
        r.check_capability("m-a", &[QuestionType::Noul]).unwrap();
    }

    #[test]
    fn capability_check_fails() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let mut upstreams = HashMap::new();
        upstreams.insert(
            "u-a".into(),
            mock("u-a", &[QuestionType::Choice, QuestionType::Score]),
        );
        let r = Router::new(mapping, upstreams);
        let err = r.check_capability("m-a", &[QuestionType::Noul]).unwrap_err();
        assert_eq!(err.http_status(), 422);
    }
}
