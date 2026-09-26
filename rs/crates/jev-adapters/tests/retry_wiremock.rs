//! A6 · P1-3 硬标准验收（07-REVIEW §7 P1-3）：
//! **wiremock 先 429 后 200 → 客户端 200 且 `upstream_calls == 2`**
//!
//! 另含跨候选顺序验收：同候选重试**耗尽**后才 failover 到下一候选。
//!
//! 全程随机端口（wiremock Server 自分配）——不占 11435，B 线可并行 curl。

use jev_adapters::upstream_vercel::VercelUpstream;
use jev_core::adapter::{plain_ctx, Registry, RetryPolicy, UpstreamAdapter};
use jev_core::router::{MatchMode, RouteEdge};
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{Criteria, JevRequest, JevResponse, Question};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/* ── 夹具 ─────────────────────────────────────────────────── */

fn edge(left: &str, right: &str, priority: i32) -> RouteEdge {
    RouteEdge {
        left: left.to_string(),
        r#match: MatchMode::Exact,
        right: right.to_string(),
        upstream_model: None,
        priority,
        sticky: Default::default(),
        on_error: Default::default(),
    }
}

fn noul_request(model: &str) -> JevRequest {
    let mut q = BTreeMap::new();
    q.insert(
        "q".to_string(),
        Question::Noul {
            instructions: "is greeting?".into(),
            criteria: Criteria::Bool {
                r#true: "yes".into(),
                r#false: "no".into(),
            },
        },
    );
    JevRequest {
        model: model.to_string(),
        state: serde_json::json!("hi"),
        questions: q,
    }
}

/// wiremock 上的「Vercel 方言」200 响应（boolean + probability ——
/// 走 VercelProtocol::incoming 归一为标准 JevResponse）。
fn vercel_ok_template() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(&serde_json::json!({
        "answers": { "q": { "type": "boolean", "probability": 0.73, "boolean": true } },
        "model": "typesafe-ai/jev"
    }))
}

/// 第 1 次 429、之后 200（P1-3 字面：一次 429 一次 200）。
struct FlakyFirst429 {
    hits: Arc<AtomicU32>,
}

impl Respond for FlakyFirst429 {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        if self.hits.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(429).set_body_string("rate limited")
        } else {
            vercel_ok_template()
        }
    }
}

/// 恒 429（跨候选测试：同候选重试耗尽后必须换候选）。
struct Always429;

impl Respond for Always429 {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        ResponseTemplate::new(429).set_body_string("rate limited")
    }
}

/* ── 次候选 fake（进程内，非 wiremock） ─────────────────────── */

struct OkLaya {
    calls: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl UpstreamAdapter for OkLaya {
    fn id(&self) -> &str {
        "laya"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            question_types: &[QuestionType::Noul],
            has_confidence: true,
            has_usage: false,
            noul_via_boolean: false,
            retryable_status: &[0], // 本地类：不重试
        }
    }
    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(JevResponse {
            model: Some(req.model),
            answers: BTreeMap::new(),
            usage: None,
            upstream_calls: Some(1),
            latency_ms: None,
            cost_usd: None,
            extra: BTreeMap::new(),
        })
    }
}

/// vercel 双能力表（与 VERCEL_CAPABILITIES 一致 —— 集成测试只依赖公开 trait/DTO 路径）。
fn vercel_upstream_at(base: String) -> VercelUpstream {
    VercelUpstream::new(base, "test-key".to_string()).expect("vercel upstream")
}

/* ── 验收 ①（P1-3 硬标准）：429 → 200，客户端 200，upstream_calls==2 ── */

#[tokio::test]
async fn wiremock_429_then_200_client_200_upstream_calls_2() {
    let server = MockServer::start().await; // 随机端口
    let hits = Arc::new(AtomicU32::new(0));
    Mock::given(any())
        .respond_with(FlakyFirst429 { hits: hits.clone() })
        .mount(&server)
        .await;

    let mut reg = Registry::with_retry(
        vec![{
            let mut e = edge("jev", "vercel", 10);
            e.upstream_model = Some("typesafe-ai/jev".into());
            e
        }],
        RetryPolicy::new(3, Duration::from_millis(1)), // 极短基数防 flake
    );
    reg.register(Box::new(vercel_upstream_at(format!(
        "{}/evaluate",
        server.uri()
    ))));

    let resp = reg
        .invoke(noul_request("jev"), plain_ctx())
        .await
        .expect("P1-3 客户端 200");

    // 客户端拿到 200 + 标准 JevResponse（probability 0.73 归一）
    assert_eq!(resp.upstream_calls, Some(2), "upstream_calls == 2");
    let jev_protocol::Answer::Noul(n) = &resp.answers["q"] else {
        panic!("expected noul answer");
    };
    assert!((jev_protocol::noul_probability(n) - 0.73).abs() < 1e-12);

    // wiremock 侧确实收到 2 次请求（1×429 + 1×200）
    let received = server.received_requests().await.expect("recorded");
    assert_eq!(received.len(), 2, "wiremock 实收 2 发");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/* ── 验收 ②（A6 顺序）：同候选重试耗尽 → 才跨候选 ──────────── */

#[tokio::test]
async fn retry_exhausted_then_failover_to_next_candidate() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(Always429)
        .mount(&server)
        .await;

    let mut reg = Registry::with_retry(
        vec![
            {
                let mut e = edge("jev", "vercel", 10);
                e.upstream_model = Some("typesafe-ai/jev".into());
                e
            },
            {
                let mut e = edge("jev", "laya", 30);
                e.upstream_model = Some("laya-english".into());
                e
            },
        ],
        RetryPolicy::new(2, Duration::from_millis(1)), // 同候选预算 2
    );
    reg.register(Box::new(vercel_upstream_at(format!(
        "{}/evaluate",
        server.uri()
    ))));
    let laya_calls = Arc::new(AtomicU32::new(0));
    reg.register(Box::new(OkLaya {
        calls: laya_calls.clone(),
    }));

    let resp = reg
        .invoke(noul_request("jev"), plain_ctx())
        .await
        .expect("failover ok");

    // 同候选 2 发（耗尽）+ 跨候选 1 发 = 3
    let received = server.received_requests().await.expect("recorded");
    assert_eq!(received.len(), 2, "vercel 同候选恰 2 发（重试耗尽）");
    assert_eq!(laya_calls.load(Ordering::SeqCst), 1, "耗尽后才跨到 laya");
    assert_eq!(resp.upstream_calls, Some(3), "2 + 1 如实计数");
    assert_eq!(resp.model.as_deref(), Some("laya-english"));
}
