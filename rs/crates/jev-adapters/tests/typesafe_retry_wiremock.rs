//! TypeSafe official rate-limit statuses are retried through the normal Registry policy.
//! All traffic stays on a random in-process wiremock server; no live API calls are made.

use jev_adapters::upstream_typesafe::TypeSafeUpstream;
use jev_core::adapter::{plain_ctx, Registry, RetryPolicy};
use jev_core::router::{MatchMode, RouteEdge};
use jev_core::upstream::JevError;
use jev_protocol::{Criteria, JevRequest, Question};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn edge() -> RouteEdge {
    RouteEdge {
        left: "jev".into(),
        r#match: MatchMode::Exact,
        right: "typesafe".into(),
        upstream_model: Some("jev-latest".into()),
        priority: 0,
        sticky: Default::default(),
        on_error: Default::default(),
    }
}

fn request() -> JevRequest {
    JevRequest {
        model: "jev".into(),
        state: serde_json::json!("hello"),
        questions: BTreeMap::from([(
            "q".into(),
            Question::Noul {
                instructions: "is this a greeting?".into(),
                criteria: Some(Criteria::Bool {
                    r#true: "yes".into(),
                    r#false: "no".into(),
                }),
            },
        )]),
        extensions: Default::default(),
        extra: Default::default(),
    }
}

struct FailOnceThenOk {
    status: u16,
    hits: Arc<AtomicU32>,
}

impl Respond for FailOnceThenOk {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.hits.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(self.status).set_body_string("rate limited")
        } else {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"answers": {}}))
        }
    }
}

#[tokio::test]
async fn retries_both_typesafe_rate_limit_statuses_then_returns_success() {
    for status in [429, 529] {
        let server = MockServer::start().await;
        let hits = Arc::new(AtomicU32::new(0));
        Mock::given(any())
            .respond_with(FailOnceThenOk {
                status,
                hits: hits.clone(),
            })
            .mount(&server)
            .await;

        let mut registry =
            Registry::with_retry(vec![edge()], RetryPolicy::new(2, Duration::from_millis(1)));
        registry.register(Box::new(
            TypeSafeUpstream::new_with_id(
                "typesafe".into(),
                format!("{}/v1/systemone", server.uri()),
                None,
            )
            .unwrap(),
        ));

        let response = registry.invoke(request(), plain_ctx()).await.unwrap();
        assert_eq!(response.upstream_calls, Some(2), "HTTP {status}");
        assert_eq!(hits.load(Ordering::SeqCst), 2, "HTTP {status}");
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }
}

#[tokio::test]
async fn unauthorized_typesafe_response_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let mut registry = Registry::with_retry(vec![edge()], RetryPolicy::new(3, Duration::ZERO));
    registry.register(Box::new(
        TypeSafeUpstream::new_with_id(
            "typesafe".into(),
            format!("{}/v1/systemone", server.uri()),
            None,
        )
        .unwrap(),
    ));

    let error = registry.invoke(request(), plain_ctx()).await.unwrap_err();
    assert!(matches!(error, JevError::Upstream { status: 401, .. }));
    assert!(!error.retryable());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
