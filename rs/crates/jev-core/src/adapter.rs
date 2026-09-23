//! 冻结扩展点 + Registry（A5 · contracts/02 §2 **逐字**）
//!
//! # 签名冻结
//!
//! ```text
//! ProtocolAdapter::{outgoing, incoming, dialect}
//! UpstreamAdapter::{id, capabilities, evaluate}
//! Registry::{register(Box<dyn UpstreamAdapter>), invoke(req, ctx)}
//! ```
//!
//! **变更以上任何签名 = 破坏性版本 + 迁移说明**（contracts/02 §2 冻结条款）。
//! `register` / `invoke` 的传参**永不因新厂商而改**（serde 同构性合同）。
//!
//! # 与契约字面的两处偏差（实现按最贴近字面修复，此处备案）
//!
//! 1. `Registry::invoke`：契约 §2 写的是同步 `fn`，但同段冻结的
//!    `UpstreamAdapter::evaluate` 是 `async_trait` —— 同步 `invoke` 无法在无执行器的
//!    `jev-core`（无 tokio 依赖）里 await。唯一自洽读法 = `async fn invoke`，
//!    **参数列表与返回类型逐字不变**。
//! 2. `invoke` 错误类型须为 `JevError`（Result 形状冻结），而「无匹配边 404」
//!    属路由错误 —— `JevError` 扩充 `UnknownModel` / `UnknownUpstream` 两变体
//!    （`http_status()=404`，`retryable()=false`），不改任何既有变体。
//!
//! 分层铁律（§3）：本文件禁止出现具体 URL / 厂商 `match` —— 方言 id 字符串只在
//! adapter 实现的 `dialect()` 里。

use crate::router::{OnError, RouteCtx, RouteEdge, Router, RouterError};
use crate::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use std::collections::{BTreeSet, HashMap};

/* ══════════════════════════════════════════════════════════════════
   冻结 trait 1 · ProtocolAdapter（contracts/02 §2 原文）
   ══════════════════════════════════════════════════════════════════ */

/// 入站归一化上下文。
///
/// - `request`：outgoing 之后的请求（用于按 qid 还原语义）
/// - `original`：调用方原始请求
/// - `status`：上游 HTTP 状态
pub struct IncomingCtx<'a> {
    pub request: &'a JevRequest,
    pub original: &'a JevRequest,
    pub status: u16,
}

/// 方言适配 —— 签名**冻结**（变更 = 破坏性版本）。
///
/// serde 同构对照（contracts/02 §1）：给厂商 `impl ProtocolAdapter`
/// ≈ serde 的 `impl Serialize`；内核不关心对面是谁。
pub trait ProtocolAdapter: Send + Sync {
    /// 出站方言化。默认恒等。
    fn outgoing(&self, req: JevRequest) -> JevRequest {
        req
    }
    /// 入站归一化：厂商原始 bytes → 标准 [`JevResponse`]。
    fn incoming(&self, raw: &[u8], ctx: &IncomingCtx<'_>) -> Result<JevResponse, JevError>;
    /// 本适配器的方言 id（`vercel_boolean` / `typesafe` / …）。
    fn dialect(&self) -> &'static str;
}

/* ══════════════════════════════════════════════════════════════════
   冻结 trait 2 · UpstreamAdapter（contracts/02 §2 原文）
   ══════════════════════════════════════════════════════════════════ */

/// 上游适配 —— 签名**冻结**（`async_trait` 保证 dyn-compatible）。
///
/// 能力**注册制**（07 P2）：`capabilities()` 一律由 adapter 自报，
/// 内核不再按 id 硬编码查表。
#[async_trait::async_trait]
pub trait UpstreamAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError>;
}

/* ══════════════════════════════════════════════════════════════════
   Registry（contracts/02 §2：register / invoke 传参永久冻结）
   ══════════════════════════════════════════════════════════════════ */

/// 上游注册表 + 调度入口。
///
/// - `register`：装配期注册（编译期 trait 同构，Q1=A；非运行时热插拔）
/// - `invoke`：DAG 选路 + 按候选 failover（A4 daemon 循环的内核化），
///   `upstream_calls` 如实 = 实发次数
pub struct Registry {
    router: Router,
}

impl Registry {
    /// 以路由边（`[[routes]]` 合并结果）建注册表；上游经 `register` 逐个挂载。
    pub fn new(edges: Vec<RouteEdge>) -> Self {
        Self {
            router: Router::new(edges, HashMap::new()),
        }
    }

    /// 注册上游 —— 传参永不因新厂商而改（冻结）。
    pub fn register(&mut self, up: Box<dyn UpstreamAdapter>) {
        self.router.register(up);
    }

    /// 路由引擎视图（`/v1/models` 等只读装配用）。
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// 执行一次 Jev 决策 —— 签名冻结（参数/返回类型 = contracts/02 §2 字面；
    /// `async` 见模块文档偏差备案 1）。
    ///
    /// 行为（contracts/03 §4）：
    /// - 无匹配边 / 全悬空 → [`JevError::UnknownModel`] / [`JevError::UnknownUpstream`]（404）
    /// - capability 不匹配 → 跳过该候选（不计入失败语义、不实发）
    /// - `on_error=next` 且 `JevError::retryable()` → 试下一候选；
    ///   `fail` / 不可重试 → 第一错即返
    /// - 全败 → 返回最后错误（调用方按 `http_status()` 映射）
    /// - 成功 → `upstream_calls = 实发次数`
    pub async fn invoke(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
    ) -> Result<JevResponse, JevError> {
        let plan = match self.router.plan(&req.model, &ctx) {
            Ok(p) if p.is_empty() => {
                // 命中边但候选全被 exclude 过滤 → 无可用上游（404）
                return Err(JevError::UnknownModel(req.model.clone()));
            }
            Ok(p) => p,
            Err(RouterError::UnknownModel(m)) => return Err(JevError::UnknownModel(m)),
            Err(RouterError::UnknownUpstream(u)) => return Err(JevError::UnknownUpstream(u)),
            Err(RouterError::Cycle(c)) => {
                return Err(JevError::Config {
                    upstream_id: req.model.clone(),
                    message: c,
                })
            }
        };

        let qts: Vec<QuestionType> = req.questions.values().map(|q| q.question_type()).collect();

        let mut upstream_calls: u32 = 0;
        let mut last_err: Option<JevError> = None;
        let mut cap_skip: Option<JevError> = None;

        for item in &plan {
            let Some(upstream) = self.router.upstream(&item.candidate.upstream_id) else {
                last_err = Some(JevError::Config {
                    upstream_id: item.candidate.upstream_id.clone(),
                    message: "upstream disappeared after select".into(),
                });
                continue;
            };

            // capability：不匹配 = 跳过（不实发、不计失败语义）
            if let Some(qt) = qts.iter().find(|qt| !upstream.capabilities().supports(**qt)) {
                if cap_skip.is_none() {
                    cap_skip = Some(JevError::Capability {
                        upstream_id: upstream.id().to_string(),
                        detail: format!(
                            "model '{}' requires question type '{}' but upstream '{}' cannot handle it",
                            req.model,
                            qt.as_str(),
                            upstream.id()
                        ),
                    });
                }
                continue;
            }

            // upstream_model 改写后发上游
            let mut attempt_req = req.clone();
            attempt_req.model = item.candidate.upstream_model.clone();

            upstream_calls += 1; // 实发计数（capability 跳过不计）
            match upstream.evaluate(attempt_req).await {
                Ok(mut resp) => {
                    resp.upstream_calls = Some(upstream_calls); // 如实 = 实发次数
                    return Ok(resp);
                }
                Err(e) => {
                    let retry_next = e.retryable() && item.on_error == OnError::Next;
                    if retry_next {
                        last_err = Some(e);
                        continue; // on_error=next → 下一候选
                    }
                    return Err(e); // fail 首错即返 / 不可重试即返
                }
            }
        }

        if let Some(e) = last_err {
            return Err(e);
        }
        if let Some(e) = cap_skip {
            return Err(e); // 全部 capability 跳过 → 422
        }
        Err(JevError::UnknownModel(req.model.clone()))
    }
}

/* ══════════════════════════════════════════════════════════════════
   内部小工具（failover 中的 exclude 收集等）
   ══════════════════════════════════════════════════════════════════ */

/// 便捷构造：空 `exclude` + 无 sticky 的调用上下文（HTTP 层当前入口）。
pub fn plain_ctx() -> RouteCtx {
    RouteCtx {
        sticky_key: None,
        exclude: BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{MatchMode, Sticky};
    use crate::upstream::Capabilities;
    use jev_protocol::{Answer, Criteria, NoulAnswer, NoulKind, Question};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    struct Handle {
        calls: AtomicU32,
        seen: Mutex<Vec<String>>,
    }

    /// 测试用上游：按脚本返回成功 JevResponse 或错误。
    struct FakeUp {
        id: String,
        cap: Capabilities,
        mode: Mode,
        handle: std::sync::Arc<Handle>,
    }

    enum Mode {
        Ok,
        Err429,
        Err400,
        BadResponse,
    }

    #[async_trait::async_trait]
    impl UpstreamAdapter for FakeUp {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Capabilities {
            self.cap.clone()
        }
        async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
            self.handle.calls.fetch_add(1, Ordering::SeqCst);
            self.handle.seen.lock().unwrap().push(req.model.clone());
            match self.mode {
                Mode::Ok => Ok(JevResponse {
                    model: Some(req.model),
                    answers: BTreeMap::new(),
                    usage: None,
                    upstream_calls: Some(1),
                    latency_ms: None,
                    cost_usd: None,
                    extra: BTreeMap::new(),
                }),
                Mode::Err429 => Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 429,
                    body: "rate limited".into(),
                    retryable: true,
                }),
                Mode::Err400 => Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 400,
                    body: "bad".into(),
                    retryable: false,
                }),
                Mode::BadResponse => Err(JevError::BadResponse {
                    upstream_id: self.id.clone(),
                    message: "invalid jev json".into(),
                }),
            }
        }
    }

    use std::collections::BTreeMap;

    fn fake(
        id: &str,
        qts: &'static [QuestionType],
        mode: Mode,
    ) -> (Box<dyn UpstreamAdapter>, std::sync::Arc<Handle>) {
        let handle = std::sync::Arc::new(Handle {
            calls: AtomicU32::new(0),
            seen: Mutex::new(Vec::new()),
        });
        let cap = Capabilities {
            question_types: qts,
            has_confidence: false,
            has_usage: false,
            // Boolean 方言表驱动 → supports(Noul) 短路 true（与 daemon fake 一致）
            noul_via_boolean: qts.contains(&QuestionType::Boolean),
            retryable_status: &[408, 429, 500, 502, 503, 504],
        };
        (
            Box::new(FakeUp {
                id: id.to_string(),
                cap,
                mode,
                handle: handle.clone(),
            }),
            handle,
        )
    }

    fn edge(left: &str, right: &str, priority: i32) -> RouteEdge {
        RouteEdge {
            left: left.into(),
            r#match: MatchMode::Exact,
            right: right.into(),
            upstream_model: None,
            priority,
            sticky: Sticky::None,
            on_error: OnError::Next,
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

    /* ── 黄金路径：register → invoke 命中 ─────────────────────── */

    #[tokio::test]
    async fn register_then_invoke_hits_and_counts_calls() {
        let mut reg = Registry::new(vec![
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
        ]);
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let resp = reg.invoke(noul_request("jev"), plain_ctx()).await.expect("failover ok");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1);
        assert_eq!(resp.upstream_calls, Some(2)); // 实发 2 次
        // upstream_model 改写
        assert_eq!(h1.seen.lock().unwrap().as_slice(), &["typesafe-ai/jev"]);
        assert_eq!(h2.seen.lock().unwrap().as_slice(), &["laya-english"]);
    }

    #[tokio::test]
    async fn unknown_model_is_404_semantics() {
        let reg = Registry::new(vec![]);
        let err = reg.invoke(noul_request("nope"), plain_ctx()).await.unwrap_err();
        assert_eq!(err.http_status(), 404);
        assert!(!err.retryable());
        assert!(err.to_string().contains("nope"));
    }

    #[tokio::test]
    async fn capability_skip_then_success_counts_only_real_sends() {
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Mode::Err429); // 不支持 noul → 跳过
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);
        let resp = reg.invoke(noul_request("jev"), plain_ctx()).await.unwrap();
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0, "cap 跳过 = 零实发");
        assert_eq!(resp.upstream_calls, Some(1));
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn all_capability_skipped_is_422() {
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10)]);
        let (v1, h1) = fake("vercel", &[QuestionType::Choice], Mode::Ok);
        reg.register(v1);
        let err = reg.invoke(noul_request("jev"), plain_ctx()).await.unwrap_err();
        assert_eq!(err.http_status(), 422);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn on_error_fail_stops_at_first_error() {
        let mut reg = Registry::new(vec![
            RouteEdge {
                on_error: OnError::Fail,
                ..edge("jev", "vercel", 10)
            },
            edge("jev", "laya", 30),
        ]);
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);
        let err = reg.invoke(noul_request("jev"), plain_ctx()).await.unwrap_err();
        assert_eq!(err.http_status(), 503);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "fail 不试下一候选");
    }

    #[tokio::test]
    async fn non_retryable_stops_even_with_next() {
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::Err400);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);
        let err = reg.invoke(noul_request("jev"), plain_ctx()).await.unwrap_err();
        assert_eq!(err.http_status(), 400);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn bad_response_502_no_failover() {
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::BadResponse);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);
        let err = reg.invoke(noul_request("jev"), plain_ctx()).await.unwrap_err();
        assert_eq!(err.http_status(), 502);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0);
    }

    /* ── ProtocolAdapter 默认恒等 + 对象可用 ──────────────────── */

    struct EchoDialect;

    impl ProtocolAdapter for EchoDialect {
        fn incoming(&self, raw: &[u8], _ctx: &IncomingCtx<'_>) -> Result<JevResponse, JevError> {
            serde_json::from_slice(raw).map_err(|e| JevError::BadResponse {
                upstream_id: self.dialect().into(),
                message: e.to_string(),
            })
        }
        fn dialect(&self) -> &'static str {
            "echo"
        }
    }

    #[test]
    fn protocol_adapter_default_outgoing_is_identity() {
        let a = EchoDialect;
        let req = noul_request("m");
        let out = a.outgoing(req.clone());
        assert_eq!(out, req);
        assert_eq!(a.dialect(), "echo");
        // 对象安全：Box<dyn ProtocolAdapter> 可用（外部 crate 注册方言用）
        let boxed: Box<dyn ProtocolAdapter> = Box::new(EchoDialect);
        assert_eq!(boxed.dialect(), "echo");
    }

    #[test]
    fn incoming_ctx_fields_accessible() {
        let a = EchoDialect;
        let req = noul_request("m");
        let ctx = IncomingCtx {
            request: &req,
            original: &req,
            status: 200,
        };
        let resp = a
            .incoming(br#"{"answers":{}}"#, &ctx)
            .expect("valid jev response");
        assert!(matches!(resp.answers.get("q"), None)); // answers 空
        let _: &Answer = &Answer::Noul(NoulAnswer {
            kind: NoulKind::Noul,
            noul: None,
            probability: None,
        }); // 类型在场（编译期）
        assert_eq!(ctx.status, 200);
    }
}
