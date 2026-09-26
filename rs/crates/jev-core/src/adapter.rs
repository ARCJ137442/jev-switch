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
RetryPolicy（A6 · 07 P1-3 / contracts/02 §3 分层允许项）
══════════════════════════════════════════════════════════════════ */

/// 同候选内重试策略。
///
/// - **顺序（写死）**：先同候选退避重试，耗尽后才交给 failover（`on_error=next`
///   跨候选）。`on_error=fail` **首错即返** —— 不做同候选重试也不 failover
///   （contracts/03 §4「第一次错误即返回」字面）。
/// - 仅 `JevError::retryable()` 为 true 的错误进入重试（429/5xx/Timeout/Network；
///   本地类上游 `retryable_status=[0]` → 恒 false → **不重试**）。
/// - 退避：第 n 次失败后 sleep `backoff_base * 2^(n-1)`（n 从 1 起，指数封顶防溢出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 单候选最大实发次数（含首次）；`1` = 不重试。
    pub max_attempts: u32,
    /// 首次退避基数。
    pub backoff_base: std::time::Duration,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, backoff_base: std::time::Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            backoff_base,
        }
    }

    /// 不重试（单发即交 failover）。
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            backoff_base: std::time::Duration::ZERO,
        }
    }

    /// 第 `attempt` 次（1-based）失败后的退避时长。
    pub fn backoff_after(&self, attempt: u32) -> std::time::Duration {
        let exp = attempt.saturating_sub(1).min(16); // 指数封顶 2^16，防溢出
        self.backoff_base.saturating_mul(1u32 << exp)
    }
}

impl Default for RetryPolicy {
    /// 生产默认：单候选最多 3 次实发，退避 200ms / 400ms 指数。
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_base: std::time::Duration::from_millis(200),
        }
    }
}

/* ══════════════════════════════════════════════════════════════════
Registry（contracts/02 §2：register / invoke 传参永久冻结）
══════════════════════════════════════════════════════════════════ */

/// 上游注册表 + 调度入口。
///
/// - `register`：装配期注册（编译期 trait 同构，Q1=A；非运行时热插拔）
/// - `invoke`：DAG 选路 + **同候选退避重试** + 按候选 failover（A4 daemon 循环的
///   内核化），`upstream_calls` 如实 = 实发次数
pub struct Registry {
    router: Router,
    retry: RetryPolicy,
}

/// Per-endpoint runtime policy. The frozen `invoke(req, ctx)` entry remains failover;
/// the daemon opts into this additive scheduler API when an endpoint config asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingStrategy {
    Failover,
    Race { timeout_ms: u64 },
    LoadBalance { weight_mode: String },
    Shadow { shadow_target: String },
}

/// Error returned by the additive traced invocation API. The existing `invoke`
/// signatures continue returning `JevError`; callers that need diagnostics can
/// retain this sanitized route trace without persisting upstream error bodies.
#[derive(Debug)]
pub struct InvocationFailure {
    pub error: JevError,
    pub route_trace: serde_json::Value,
}

impl InvocationFailure {
    fn new(
        requested_model: &str,
        attempts: Vec<serde_json::Value>,
        upstream_calls: u32,
        error: JevError,
    ) -> Self {
        let route_trace = serde_json::json!({
            "requested_model": requested_model,
            "outcome": "failed",
            "upstream_calls": upstream_calls,
            "attempts": attempts,
            "failure": {
                "kind": error_kind(&error),
                "http_status": error.http_status(),
                "retryable": error.retryable(),
                "upstream_status": match &error {
                    JevError::Upstream { status, .. } => Some(*status),
                    _ => None,
                }
            }
        });
        Self { error, route_trace }
    }
}

fn error_kind(error: &JevError) -> &'static str {
    match error {
        JevError::Upstream { .. } => "upstream_http",
        JevError::Timeout { .. } => "timeout",
        JevError::Network { .. } => "network",
        JevError::Capability { .. } => "capability_mismatch",
        JevError::BadResponse { .. } => "bad_upstream_response",
        JevError::Config { .. } => "routing_config",
        JevError::UnknownModel(_) => "unknown_model",
        JevError::UnknownUpstream(_) => "unknown_upstream",
    }
}

fn annotate_attempt_failure(attempt: &mut serde_json::Value, error: &JevError, decision: &str) {
    if let Some(fields) = attempt.as_object_mut() {
        fields.insert("outcome".into(), serde_json::json!("failed"));
        fields.insert("error_kind".into(), serde_json::json!(error_kind(error)));
        fields.insert("retryable".into(), serde_json::json!(error.retryable()));
        fields.insert("retry_decision".into(), serde_json::json!(decision));
        if let JevError::Upstream { status, .. } = error {
            fields.insert("upstream_status".into(), serde_json::json!(status));
        }
    }
}

impl Registry {
    /// 以路由边（`[[routes]]` 合并结果）建注册表；上游经 `register` 逐个挂载。
    /// 重试策略 = [`RetryPolicy::default`]（3 次 / 200ms 指数退避）。
    pub fn new(edges: Vec<RouteEdge>) -> Self {
        Self::with_retry(edges, RetryPolicy::default())
    }

    /// 自定义同候选重试策略（测试可传 [`RetryPolicy::no_retry`] 隔离 failover 语义）。
    pub fn with_retry(edges: Vec<RouteEdge>, retry: RetryPolicy) -> Self {
        Self {
            router: Router::new(edges, HashMap::new()),
            retry,
        }
    }

    /// 注册上游 —— 传参永不因新厂商而改（冻结）。
    pub fn register(&mut self, up: Box<dyn UpstreamAdapter>) {
        self.router.register(up);
    }

    /// Replace the configured providers after an admin config write without changing
    /// the frozen `register`/`invoke` signatures or restarting the daemon.
    pub fn replace_upstreams(&self, upstreams: Vec<Box<dyn UpstreamAdapter>>) {
        self.router.replace_upstreams(upstreams);
    }

    /// 路由引擎视图（`/v1/models` 等只读装配用）。
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// 热替换路由边表（A7 admin `PUT /v1/admin/routes` 整表替换、无重启）。
    /// 已注册上游与重试策略不动；调用方先 [`crate::router::check_acyclic`] 校验。
    pub fn replace_edges(&self, edges: Vec<RouteEdge>) {
        self.router.replace_edges(edges);
    }

    /// 当前重试策略。
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry
    }

    /// 执行一次 Jev 决策 —— 签名冻结（参数/返回类型 = contracts/02 §2 字面；
    /// `async` 见模块文档偏差备案 1）。
    ///
    /// 行为（contracts/03 §4 + 07 P1-3）：
    /// - 无匹配边 / 全悬空 → [`JevError::UnknownModel`] / [`JevError::UnknownUpstream`]（404）
    /// - capability 不匹配 → 跳过该候选（不计入失败语义、不实发）
    /// - **顺序**：`on_error=fail` 首错即返；`next` 时先同候选按 [`RetryPolicy`]
    ///   退避重试，耗尽 → 下一候选；不可重试错误 → 即返
    /// - 全败 → 返回最后错误（调用方按 `http_status()` 映射）
    /// - 成功 → `upstream_calls = 实发次数`
    pub async fn invoke(&self, req: JevRequest, ctx: RouteCtx) -> Result<JevResponse, JevError> {
        self.invoke_with_strategy(req, ctx, RoutingStrategy::Failover)
            .await
    }

    pub async fn invoke_with_strategy(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
        strategy: RoutingStrategy,
    ) -> Result<JevResponse, JevError> {
        self.invoke_with_strategy_traced(req, ctx, strategy)
            .await
            .map_err(|failure| failure.error)
    }

    /// Invoke with a safe route trace retained on both success and failure.
    /// Trace entries contain route metadata and error classifications only;
    /// upstream response bodies and request content are never copied into it.
    pub async fn invoke_with_strategy_traced(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
        strategy: RoutingStrategy,
    ) -> Result<JevResponse, InvocationFailure> {
        let name = match &strategy {
            RoutingStrategy::Failover => "failover",
            RoutingStrategy::Race { .. } => "race",
            RoutingStrategy::LoadBalance { .. } => "load_balance",
            RoutingStrategy::Shadow { .. } => "shadow",
        };
        let result = match strategy {
            RoutingStrategy::Failover => self.invoke_failover_traced(req, ctx).await,
            RoutingStrategy::Race { timeout_ms } => {
                self.invoke_race_traced(req, ctx, timeout_ms).await
            }
            RoutingStrategy::LoadBalance { weight_mode } => {
                self.invoke_load_balance_traced(req, ctx, &weight_mode)
                    .await
            }
            RoutingStrategy::Shadow { shadow_target } => {
                self.invoke_shadow_traced(req, ctx, &shadow_target).await
            }
        };
        match result {
            Ok(mut response) => {
                if let Some(trace) = response
                    .extra
                    .get_mut("route_trace")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    trace
                        .entry("strategy")
                        .or_insert_with(|| serde_json::Value::String(name.into()));
                    trace.insert(
                        "accounting".into(),
                        serde_json::json!({
                            "usage_cost_scope":"selected_upstream_response_only",
                            "provider_latency_scope":"selected_upstream_response_only",
                            "additional_attempt_usage_cost":"not_aggregated"
                        }),
                    );
                }
                Ok(response)
            }
            Err(mut failure) => {
                if let Some(trace) = failure.route_trace.as_object_mut() {
                    trace.insert("strategy".into(), serde_json::json!(name));
                    trace.insert(
                        "accounting".into(),
                        serde_json::json!({
                            "usage_cost_scope":"selected_upstream_response_only",
                            "provider_latency_scope":"selected_upstream_response_only",
                            "additional_attempt_usage_cost":"not_aggregated"
                        }),
                    );
                }
                Err(failure)
            }
        }
    }

    async fn invoke_failover_traced(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
    ) -> Result<JevResponse, InvocationFailure> {
        let plan = match self.router.plan(&req.model, &ctx) {
            Ok(p) if p.is_empty() => {
                // 命中边但候选全被 exclude 过滤 → 无可用上游（404）
                return Err(InvocationFailure::new(
                    &req.model,
                    Vec::new(),
                    0,
                    JevError::UnknownModel(req.model.clone()),
                ));
            }
            Ok(p) => p,
            Err(RouterError::UnknownModel(m)) => {
                return Err(InvocationFailure::new(
                    &req.model,
                    Vec::new(),
                    0,
                    JevError::UnknownModel(m),
                ))
            }
            Err(RouterError::UnknownUpstream(u)) => {
                return Err(InvocationFailure::new(
                    &req.model,
                    Vec::new(),
                    0,
                    JevError::UnknownUpstream(u),
                ))
            }
            Err(RouterError::Cycle(c)) => {
                return Err(InvocationFailure::new(
                    &req.model,
                    Vec::new(),
                    0,
                    JevError::Config {
                        upstream_id: req.model.clone(),
                        message: c,
                    },
                ));
            }
        };

        let qts: Vec<QuestionType> = req.questions.values().map(|q| q.question_type()).collect();

        let mut upstream_calls: u32 = 0;
        let mut last_err: Option<JevError> = None;
        let mut cap_skip: Option<JevError> = None;
        let mut route_attempts = Vec::<serde_json::Value>::new();

        for item in &plan {
            let Some(upstream) = self.router.upstream(&item.candidate.upstream_id) else {
                route_attempts.push(serde_json::json!({
                    "provider_id": item.candidate.upstream_id,
                    "upstream_model": item.candidate.upstream_model,
                    "hops": item.candidate.hops,
                    "priority": item.candidate.priority,
                    "outcome": "skipped",
                    "reason": "upstream_unavailable"
                }));
                last_err = Some(JevError::Config {
                    upstream_id: item.candidate.upstream_id.clone(),
                    message: "upstream disappeared after select".into(),
                });
                continue;
            };

            // capability：不匹配 = 跳过（不实发、不计失败语义）
            if let Some(qt) = qts
                .iter()
                .find(|qt| !upstream.capabilities().supports(**qt))
            {
                route_attempts.push(serde_json::json!({
                    "provider_id": item.candidate.upstream_id,
                    "upstream_model": item.candidate.upstream_model,
                    "hops": item.candidate.hops,
                    "priority": item.candidate.priority,
                    "outcome": "skipped",
                    "reason": "capability_mismatch",
                    "required_question_type": qt.as_str()
                }));
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

            // ── 同候选内：退避重试（A6）──
            let mut attempt: u32 = 0;
            loop {
                attempt += 1;
                upstream_calls += 1; // 实发计数（capability 跳过不计）
                route_attempts.push(serde_json::json!({
                    "provider_id": item.candidate.upstream_id.clone(),
                    "upstream_model": item.candidate.upstream_model.clone(),
                    "hops": item.candidate.hops.clone(),
                    "priority": item.candidate.priority,
                    "attempt": attempt,
                    "outcome": "in_flight"
                }));
                match upstream.evaluate(attempt_req.clone()).await {
                    Ok(mut resp) => {
                        route_attempts.last_mut().expect("attempt was recorded")["outcome"] =
                            serde_json::json!("succeeded");
                        resp.upstream_calls = Some(upstream_calls); // 如实 = 实发次数
                        resp.extra.insert(
                            "route_trace".into(),
                            serde_json::json!({
                                "requested_model": req.model.clone(),
                                "selected_provider": item.candidate.upstream_id.clone(),
                                "selected_model": item.candidate.upstream_model.clone(),
                                "selected_hops": item.candidate.hops.clone(),
                                "attempts": route_attempts,
                                "upstream_calls": upstream_calls
                            }),
                        );
                        return Ok(resp);
                    }
                    Err(e) => {
                        let retry_decision = if item.on_error == OnError::Fail {
                            "stop_by_candidate_policy"
                        } else if e.retryable() && attempt < self.retry.max_attempts {
                            "retry_same_candidate"
                        } else if e.retryable() {
                            "next_candidate"
                        } else {
                            "stop_non_retryable"
                        };
                        annotate_attempt_failure(
                            route_attempts.last_mut().expect("attempt was recorded"),
                            &e,
                            retry_decision,
                        );
                        // on_error=fail：第一次错误即返回（不同候选也不换、同候选也不重试）
                        if item.on_error == OnError::Fail {
                            return Err(InvocationFailure::new(
                                &req.model,
                                route_attempts,
                                upstream_calls,
                                e,
                            ));
                        }
                        // 可重试 && 同候选还有预算 → 退避后再试**同一**候选
                        if e.retryable() && attempt < self.retry.max_attempts {
                            let delay = self.retry.backoff_after(attempt);
                            if !delay.is_zero() {
                                tokio::time::sleep(delay).await;
                            }
                            continue;
                        }
                        // 重试耗尽 / 不可重试：
                        if e.retryable() {
                            last_err = Some(e);
                            break; // on_error=next → 下一候选（跨候选 failover）
                        }
                        return Err(InvocationFailure::new(
                            &req.model,
                            route_attempts,
                            upstream_calls,
                            e,
                        )); // 不可重试 → 即返
                    }
                }
            }
        }

        if let Some(e) = last_err {
            return Err(InvocationFailure::new(
                &req.model,
                route_attempts,
                upstream_calls,
                e,
            ));
        }
        if let Some(e) = cap_skip {
            return Err(InvocationFailure::new(
                &req.model,
                route_attempts,
                upstream_calls,
                e,
            )); // 全部 capability 跳过 → 422
        }
        Err(InvocationFailure::new(
            &req.model,
            route_attempts,
            upstream_calls,
            JevError::UnknownModel(req.model.clone()),
        ))
    }

    fn plan_for(
        &self,
        model: &str,
        ctx: &RouteCtx,
    ) -> Result<Vec<crate::router::PlanItem>, JevError> {
        match self.router.plan(model, ctx) {
            Ok(plan) if !plan.is_empty() => Ok(plan),
            Ok(_) => Err(JevError::UnknownModel(model.into())),
            Err(RouterError::UnknownModel(model)) => Err(JevError::UnknownModel(model)),
            Err(RouterError::UnknownUpstream(upstream)) => Err(JevError::UnknownUpstream(upstream)),
            Err(RouterError::Cycle(cycle)) => Err(JevError::Config {
                upstream_id: model.into(),
                message: cycle,
            }),
        }
    }

    async fn invoke_load_balance_traced(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
        mode: &str,
    ) -> Result<JevResponse, InvocationFailure> {
        let plan = self
            .plan_for(&req.model, &ctx)
            .map_err(|error| InvocationFailure::new(&req.model, Vec::new(), 0, error))?;
        let qts: Vec<_> = req.questions.values().map(|q| q.question_type()).collect();
        let capable: Vec<_> = plan
            .into_iter()
            .filter(|item| {
                self.router
                    .upstream(&item.candidate.upstream_id)
                    .is_some_and(|up| qts.iter().all(|qt| up.capabilities().supports(*qt)))
            })
            .collect();
        if capable.is_empty() {
            return self.invoke_failover_traced(req, ctx).await;
        }
        let selected = weighted_index(&capable, mode).ok_or_else(|| {
            InvocationFailure::new(
                &req.model,
                Vec::new(),
                0,
                JevError::Config {
                    upstream_id: req.model.clone(),
                    message: format!("unsupported load_balance weight_mode '{mode}'"),
                },
            )
        })?;
        let item = capable[selected].clone();
        self.invoke_selected_traced(req, item).await
    }

    async fn invoke_selected_traced(
        &self,
        req: JevRequest,
        item: crate::router::PlanItem,
    ) -> Result<JevResponse, InvocationFailure> {
        let upstream = self
            .router
            .upstream(&item.candidate.upstream_id)
            .ok_or_else(|| {
                InvocationFailure::new(
                    &req.model,
                    Vec::new(),
                    0,
                    JevError::UnknownUpstream(item.candidate.upstream_id.clone()),
                )
            })?;
        let mut upstream_req = req.clone();
        upstream_req.model = item.candidate.upstream_model.clone();
        let mut attempts = Vec::new();
        let mut upstream_calls = 0;
        for attempt in 1..=self.retry.max_attempts.max(1) {
            upstream_calls += 1;
            attempts.push(serde_json::json!({"provider_id":item.candidate.upstream_id,"upstream_model":item.candidate.upstream_model,"hops":item.candidate.hops,"priority":item.candidate.priority,"attempt":attempt,"outcome":"in_flight"}));
            match upstream.evaluate(upstream_req.clone()).await {
                Ok(mut response) => {
                    attempts.last_mut().expect("attempt was recorded")["outcome"] =
                        serde_json::json!("succeeded");
                    response.upstream_calls = Some(upstream_calls);
                    response.extra.insert("route_trace".into(), serde_json::json!({"requested_model":req.model,"selected_provider":item.candidate.upstream_id,"selected_model":item.candidate.upstream_model,"selected_hops":item.candidate.hops,"attempts":attempts,"upstream_calls":upstream_calls}));
                    return Ok(response);
                }
                Err(error) if error.retryable() && attempt < self.retry.max_attempts => {
                    annotate_attempt_failure(
                        attempts.last_mut().expect("attempt was recorded"),
                        &error,
                        "retry_same_candidate",
                    );
                    let delay = self.retry.backoff_after(attempt);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
                Err(error) => {
                    annotate_attempt_failure(
                        attempts.last_mut().expect("attempt was recorded"),
                        &error,
                        "stop",
                    );
                    return Err(InvocationFailure::new(
                        &req.model,
                        attempts,
                        upstream_calls,
                        error,
                    ));
                }
            }
        }
        Err(InvocationFailure::new(
            &req.model,
            attempts,
            upstream_calls,
            JevError::UnknownModel(req.model.clone()),
        ))
    }

    async fn invoke_race_traced(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
        timeout_ms: u64,
    ) -> Result<JevResponse, InvocationFailure> {
        let plan = self
            .plan_for(&req.model, &ctx)
            .map_err(|error| InvocationFailure::new(&req.model, Vec::new(), 0, error))?;
        let qts: Vec<_> = req.questions.values().map(|q| q.question_type()).collect();
        let mut tasks = tokio::task::JoinSet::new();
        let started_calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<serde_json::Value>::new()));
        let mut skipped = None;
        for item in plan {
            let Some(upstream) = self.router.upstream(&item.candidate.upstream_id) else {
                attempts
                    .lock()
                    .expect("race attempt log lock")
                    .push(serde_json::json!({
                        "provider_id": item.candidate.upstream_id,
                        "upstream_model": item.candidate.upstream_model,
                        "hops": item.candidate.hops,
                        "priority": item.candidate.priority,
                        "outcome": "skipped",
                        "reason": "upstream_unavailable"
                    }));
                continue;
            };
            if let Some(qt) = qts
                .iter()
                .find(|qt| !upstream.capabilities().supports(**qt))
            {
                skipped.get_or_insert_with(|| JevError::Capability {
                    upstream_id: upstream.id().into(),
                    detail: format!(
                        "model '{}' requires unsupported question type '{}'",
                        req.model,
                        qt.as_str()
                    ),
                });
                attempts
                    .lock()
                    .expect("race attempt log lock")
                    .push(serde_json::json!({
                        "provider_id": item.candidate.upstream_id,
                        "upstream_model": item.candidate.upstream_model,
                        "hops": item.candidate.hops,
                        "priority": item.candidate.priority,
                        "outcome": "skipped",
                        "reason": "capability_mismatch",
                        "required_question_type": qt.as_str()
                    }));
                continue;
            }
            let mut upstream_req = req.clone();
            upstream_req.model = item.candidate.upstream_model.clone();
            let dispatched = started_calls.clone();
            let attempt_log = attempts.clone();
            tasks.spawn(async move {
                let attempt_id = dispatched.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                attempt_log
                    .lock()
                    .expect("race attempt log lock")
                    .push(serde_json::json!({
                        "provider_id":item.candidate.upstream_id,
                        "upstream_model":item.candidate.upstream_model,
                        "hops":item.candidate.hops,
                        "priority":item.candidate.priority,
                        "attempt":attempt_id,
                        "outcome":"in_flight"
                    }));
                let result = upstream.evaluate(upstream_req).await;
                if let Some(entry) = attempt_log
                    .lock()
                    .expect("race attempt log lock")
                    .iter_mut()
                    .find(|entry| {
                        entry.get("attempt").and_then(serde_json::Value::as_u64)
                            == Some(u64::from(attempt_id))
                    })
                {
                    match &result {
                        Ok(_) => entry["outcome"] = serde_json::json!("succeeded"),
                        Err(error) => {
                            annotate_attempt_failure(entry, error, "race_candidate_failed")
                        }
                    }
                }
                (item, result)
            });
        }
        if tasks.is_empty() {
            let attempts = attempts.lock().expect("race attempt log lock").clone();
            return Err(InvocationFailure::new(
                &req.model,
                attempts,
                0,
                skipped.unwrap_or_else(|| JevError::UnknownModel(req.model.clone())),
            ));
        }
        let duration = std::time::Duration::from_millis(timeout_ms.max(1));
        let raced = tokio::time::timeout(duration, async {
            let mut last_error = None;
            while let Some(joined) = tasks.join_next().await {
                let Ok((item, result)) = joined else { continue };
                match result {
                    Ok(response) => {
                        return Ok((response, item));
                    }
                    Err(error) => last_error = Some(error),
                }
            }
            Err(last_error
                .or(skipped)
                .unwrap_or_else(|| JevError::UnknownModel(req.model.clone())))
        })
        .await;
        tasks.abort_all();
        let mut attempts = attempts.lock().expect("race attempt log lock").clone();
        for attempt in &mut attempts {
            if attempt.get("outcome").and_then(serde_json::Value::as_str) == Some("in_flight") {
                attempt["outcome"] = serde_json::json!("cancelled");
                attempt["reason"] = serde_json::json!("race_ended_before_completion");
            }
        }
        attempts.sort_by_key(|attempt| {
            attempt
                .get("attempt")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX)
        });
        let calls = started_calls.load(std::sync::atomic::Ordering::SeqCst);
        match raced {
            Ok(Ok((mut response, item))) => {
                response.upstream_calls = Some(calls);
                response.extra.insert(
                    "route_trace".into(),
                    serde_json::json!({
                        "requested_model":req.model,
                        "selected_provider":item.candidate.upstream_id,
                        "selected_model":item.candidate.upstream_model,
                        "selected_hops":item.candidate.hops,
                        "attempts":attempts,
                        "upstream_calls":calls,
                        "strategy":"race"
                    }),
                );
                Ok(response)
            }
            Ok(Err(error)) => Err(InvocationFailure::new(&req.model, attempts, calls, error)),
            Err(_) => Err(InvocationFailure::new(
                &req.model,
                attempts,
                calls,
                JevError::Timeout {
                    upstream_id: req.model.clone(),
                },
            )),
        }
    }

    async fn invoke_shadow_traced(
        &self,
        req: JevRequest,
        ctx: RouteCtx,
        target: &str,
    ) -> Result<JevResponse, InvocationFailure> {
        let target_item = if let Some(upstream) = self.router.upstream(target) {
            Some((upstream, req.model.clone(), vec![target.to_owned()]))
        } else {
            self.router
                .plan(target, &RouteCtx::default())
                .ok()
                .and_then(|plan| plan.into_iter().next())
                .and_then(|item| {
                    self.router
                        .upstream(&item.candidate.upstream_id)
                        .map(|upstream| {
                            (upstream, item.candidate.upstream_model, item.candidate.hops)
                        })
                })
        };
        if let Some((upstream, model, hops)) = target_item {
            let mut shadow_req = req.clone();
            shadow_req.model = model.clone();
            // Do not claim an extra call until the detached task has entered its
            // evaluate future. It continues polling that future immediately after
            // acknowledging the start.
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let future = upstream.evaluate(shadow_req);
                let _ = started_tx.send(());
                let _ = future.await;
            });
            let _ = started_rx.await;
            let mut primary = match self.invoke_failover_traced(req.clone(), ctx).await {
                Ok(response) => response,
                Err(mut failure) => {
                    if let Some(trace) = failure.route_trace.as_object_mut() {
                        trace.insert("shadow".into(), serde_json::json!({"target":target,"model":model,"hops":hops,"dispatched":true,"usage_cost":"not_collected"}));
                        let calls = trace
                            .get("upstream_calls")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0)
                            .saturating_add(1);
                        trace.insert("upstream_calls".into(), serde_json::json!(calls));
                    }
                    return Err(failure);
                }
            };
            let previous = primary.upstream_calls.unwrap_or(1);
            primary.upstream_calls = Some(previous.saturating_add(1));
            if let Some(trace) = primary
                .extra
                .get_mut("route_trace")
                .and_then(serde_json::Value::as_object_mut)
            {
                trace.insert("shadow".into(), serde_json::json!({"target":target,"model":model,"hops":hops,"dispatched":true,"usage_cost":"not_collected"}));
                trace.insert(
                    "upstream_calls".into(),
                    serde_json::json!(previous.saturating_add(1)),
                );
            }
            Ok(primary)
        } else {
            Err(InvocationFailure::new(
                &req.model,
                Vec::new(),
                0,
                JevError::Config {
                    upstream_id: req.model.clone(),
                    message: format!("shadow target '{target}' is not routable"),
                },
            ))
        }
    }
}

fn weighted_index(plan: &[crate::router::PlanItem], mode: &str) -> Option<usize> {
    if plan.is_empty() {
        return None;
    }
    let weights: Vec<u64> = match mode {
        "equal" => vec![1; plan.len()],
        "priority" => {
            let max = plan
                .iter()
                .map(|item| item.candidate.priority as i64)
                .max()?;
            plan.iter()
                .map(|item| (max - item.candidate.priority as i64 + 1).max(1) as u64)
                .collect()
        }
        _ => return None,
    };
    let total: u64 = weights.iter().copied().sum();
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    let mut pick = hasher.finish() % total.max(1);
    for (index, weight) in weights.into_iter().enumerate() {
        if pick < weight {
            return Some(index);
        }
        pick -= weight;
    }
    Some(0)
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
        /// 恒 429（retryable=true —— vercel 类）。
        Err429,
        /// 第 1 次 429、之后成功（A6 同候选重试成功路径）。
        Err429OnceThenOk,
        /// 前 2 次 429、之后成功（A6 退避耗尽/failover 顺序路径）。
        Err429TwiceThenOk,
        /// 429 但按本地类映射 retryable=false（laya 语义 —— 不重试不 failover）。
        LocalErr429,
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
            let n = self.handle.calls.fetch_add(1, Ordering::SeqCst) + 1; // 1-based
            self.handle.seen.lock().unwrap().push(req.model.clone());
            let rate_limited = |retryable: bool| {
                Err(JevError::Upstream {
                    upstream_id: self.id.clone(),
                    status: 429,
                    body: "rate limited".into(),
                    retryable,
                })
            };
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
                Mode::Err429 => rate_limited(true),
                Mode::Err429OnceThenOk => {
                    if n == 1 {
                        rate_limited(true)
                    } else {
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
                Mode::Err429TwiceThenOk => {
                    if n <= 2 {
                        rate_limited(true)
                    } else {
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
                Mode::LocalErr429 => rate_limited(false),
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
        // A4 failover 语义隔离：钉 no-retry（同候选重试由 A6 专测覆盖）
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
            RetryPolicy::no_retry(),
        );
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::Err429);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let resp = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .expect("failover ok");
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1);
        assert_eq!(resp.upstream_calls, Some(2)); // 实发 2 次
                                                  // upstream_model 改写
        assert_eq!(h1.seen.lock().unwrap().as_slice(), &["typesafe-ai/jev"]);
        assert_eq!(h2.seen.lock().unwrap().as_slice(), &["laya-english"]);
    }

    #[tokio::test]
    async fn load_balance_dispatches_to_selected_route_only() {
        let mut registry = Registry::with_retry(
            vec![edge("jev", "a", 10), edge("jev", "b", 20)],
            RetryPolicy::no_retry(),
        );
        let (a, ah) = fake("a", &[QuestionType::Noul], Mode::Ok);
        let (b, bh) = fake("b", &[QuestionType::Noul], Mode::Ok);
        registry.register(a);
        registry.register(b);
        for _ in 0..100 {
            registry
                .invoke_with_strategy(
                    noul_request("jev"),
                    plain_ctx(),
                    RoutingStrategy::LoadBalance {
                        weight_mode: "equal".into(),
                    },
                )
                .await
                .unwrap();
        }
        let a_calls = ah.calls.load(Ordering::SeqCst);
        let b_calls = bh.calls.load(Ordering::SeqCst);
        assert_eq!(a_calls + b_calls, 100);
        assert!(
            a_calls > 0 && b_calls > 0,
            "both equally weighted candidates should be selected over 100 randomized calls"
        );
    }

    #[tokio::test]
    async fn race_reports_strategy_and_dispatches_candidate_set() {
        let mut registry = Registry::with_retry(
            vec![edge("jev", "a", 10), edge("jev", "b", 20)],
            RetryPolicy::no_retry(),
        );
        let (a, ah) = fake("a", &[QuestionType::Noul], Mode::Ok);
        let (b, bh) = fake("b", &[QuestionType::Noul], Mode::Ok);
        registry.register(a);
        registry.register(b);
        let response = registry
            .invoke_with_strategy(
                noul_request("jev"),
                plain_ctx(),
                RoutingStrategy::Race { timeout_ms: 1_000 },
            )
            .await
            .unwrap();
        assert_eq!(response.extra["route_trace"]["strategy"], "race");
        assert_eq!(response.upstream_calls, Some(2));
        assert_eq!(
            ah.calls.load(Ordering::SeqCst) + bh.calls.load(Ordering::SeqCst),
            2
        );
    }

    #[tokio::test]
    async fn race_cancels_losing_in_flight_candidate_and_counts_dispatched_calls() {
        struct Slow {
            started: std::sync::Arc<AtomicU32>,
            completed: std::sync::Arc<AtomicU32>,
        }
        #[async_trait::async_trait]
        impl UpstreamAdapter for Slow {
            fn id(&self) -> &str {
                "slow"
            }
            fn capabilities(&self) -> Capabilities {
                Capabilities {
                    question_types: &[QuestionType::Noul],
                    has_confidence: false,
                    has_usage: false,
                    noul_via_boolean: false,
                    retryable_status: &[],
                }
            }
            async fn evaluate(&self, _req: JevRequest) -> Result<JevResponse, JevError> {
                self.started.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                self.completed.fetch_add(1, Ordering::SeqCst);
                unreachable!("the race winner should cancel this candidate")
            }
        }

        let started = std::sync::Arc::new(AtomicU32::new(0));
        let completed = std::sync::Arc::new(AtomicU32::new(0));
        let mut registry = Registry::with_retry(
            vec![edge("jev", "slow", 0), edge("jev", "fast", 1)],
            RetryPolicy::no_retry(),
        );
        registry.register(Box::new(Slow {
            started: started.clone(),
            completed: completed.clone(),
        }));
        let (fast, fast_handle) = fake("fast", &[QuestionType::Noul], Mode::Ok);
        registry.register(fast);

        let response = registry
            .invoke_with_strategy(
                noul_request("jev"),
                plain_ctx(),
                RoutingStrategy::Race { timeout_ms: 1_000 },
            )
            .await
            .unwrap();
        assert_eq!(response.extra["route_trace"]["selected_provider"], "fast");
        assert_eq!(
            response.upstream_calls,
            Some(2),
            "both evaluate futures were dispatched before the fast candidate won"
        );
        assert_eq!(
            response.extra["route_trace"]["attempts"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            started.load(Ordering::SeqCst),
            1,
            "slow candidate entered its upstream future"
        );
        assert_eq!(
            completed.load(Ordering::SeqCst),
            0,
            "race aborts the losing upstream future"
        );
        assert_eq!(fast_handle.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn shadow_runs_target_without_changing_primary_response() {
        let mut registry =
            Registry::with_retry(vec![edge("jev", "primary", 10)], RetryPolicy::no_retry());
        let (primary, ph) = fake("primary", &[QuestionType::Noul], Mode::Ok);
        let (shadow, sh) = fake("shadow", &[QuestionType::Noul], Mode::Ok);
        registry.register(primary);
        registry.register(shadow);
        let response = registry
            .invoke_with_strategy(
                noul_request("jev"),
                plain_ctx(),
                RoutingStrategy::Shadow {
                    shadow_target: "shadow".into(),
                },
            )
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(ph.calls.load(Ordering::SeqCst), 1);
        assert_eq!(sh.calls.load(Ordering::SeqCst), 1);
        assert_eq!(response.upstream_calls, Some(2));
        assert_eq!(
            response.extra["route_trace"]["selected_provider"],
            "primary"
        );
        assert_eq!(response.extra["route_trace"]["shadow"]["dispatched"], true);
        assert_eq!(
            response.extra["route_trace"]["shadow"]["usage_cost"],
            "not_collected"
        );
    }

    #[tokio::test]
    async fn unknown_model_is_404_semantics() {
        let reg = Registry::new(vec![]);
        let err = reg
            .invoke(noul_request("nope"), plain_ctx())
            .await
            .unwrap_err();
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
        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
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
        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.http_status(), 503);
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1);
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "fail 不试下一候选");
    }

    #[tokio::test]
    async fn traced_failure_keeps_retry_and_failover_path_without_error_body() {
        let mut reg = Registry::with_retry(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            RetryPolicy::no_retry(),
        );
        let (first, first_handle) = fake("vercel", &[QuestionType::Noul], Mode::Err429);
        let (second, second_handle) = fake("laya", &[QuestionType::Noul], Mode::LocalErr429);
        reg.register(first);
        reg.register(second);

        let failure = reg
            .invoke_with_strategy_traced(
                noul_request("jev"),
                plain_ctx(),
                RoutingStrategy::Failover,
            )
            .await
            .unwrap_err();

        assert_eq!(failure.error.http_status(), 429);
        assert_eq!(first_handle.calls.load(Ordering::SeqCst), 1);
        assert_eq!(second_handle.calls.load(Ordering::SeqCst), 1);
        assert_eq!(failure.route_trace["outcome"], "failed");
        assert_eq!(failure.route_trace["upstream_calls"], 2);
        assert_eq!(failure.route_trace["attempts"].as_array().unwrap().len(), 2);
        assert_eq!(
            failure.route_trace["attempts"][0]["retry_decision"],
            "next_candidate"
        );
        assert_eq!(failure.route_trace["attempts"][0]["upstream_status"], 429);
        assert_eq!(failure.route_trace["attempts"][1]["provider_id"], "laya");
        assert_eq!(
            failure.route_trace["attempts"][1]["retry_decision"],
            "stop_non_retryable"
        );
        assert_eq!(failure.route_trace["failure"]["http_status"], 429);
        assert!(!failure.route_trace.to_string().contains("rate limited"));
    }

    #[tokio::test]
    async fn non_retryable_stops_even_with_next() {
        let mut reg = Registry::new(vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)]);
        let (v1, h1) = fake("vercel", &[QuestionType::Boolean], Mode::Err400);
        let (v2, h2) = fake("laya", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);
        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
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
        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
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

    /* ════════════════════════════════════════════════════════
    A6 · RetryPolicy（同候选退避 → 耗尽再跨候选）
    ════════════════════════════════════════════════════════ */

    #[test]
    fn retry_policy_backoff_formula() {
        let p = RetryPolicy::new(5, std::time::Duration::from_millis(100));
        assert_eq!(p.backoff_after(1), std::time::Duration::from_millis(100));
        assert_eq!(p.backoff_after(2), std::time::Duration::from_millis(200));
        assert_eq!(p.backoff_after(3), std::time::Duration::from_millis(400));
        // max_attempts 下限 1；no_retry 零退避
        assert_eq!(
            RetryPolicy::new(0, std::time::Duration::ZERO).max_attempts,
            1
        );
        assert_eq!(RetryPolicy::no_retry().max_attempts, 1);
        // 指数封顶防溢出
        let big = RetryPolicy::new(100, std::time::Duration::from_secs(1));
        let _ = big.backoff_after(99);
    }

    /// 同候选内重试成功：429 → 退避 → 200，**不触碰**次候选；upstream_calls=2。
    #[tokio::test(start_paused = true)]
    async fn same_candidate_retry_succeeds_before_failover() {
        let mut reg = Registry::with_retry(
            vec![edge("jev", "a", 10), edge("jev", "b", 30)],
            RetryPolicy::new(3, std::time::Duration::from_secs(1)),
        );
        let (v1, h1) = fake("a", &[QuestionType::Boolean], Mode::Err429OnceThenOk);
        let (v2, h2) = fake("b", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let resp = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .expect("retry then ok");
        assert_eq!(
            h1.calls.load(Ordering::SeqCst),
            2,
            "同候选：429 后重试 1 次成功"
        );
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0, "未耗尽 → 不跨候选");
        assert_eq!(resp.upstream_calls, Some(2), "如实 = 实发 2 次");
    }

    /// 顺序写死：**先同候选重试、耗尽后才跨候选** ——
    /// a 恒 429（max_attempts=2 → a 实发恰 2 次）后才轮到 b 成功；
    /// 终态计数 a=2, b=1 唯一蕴含顺序 [a, a, b]（b 成功即返回，之后不会再有 a）。
    #[tokio::test(start_paused = true)]
    async fn retry_exhausted_then_cross_candidate_order() {
        let mut reg = Registry::with_retry(
            vec![edge("jev", "a", 10), edge("jev", "b", 30)],
            RetryPolicy::new(2, std::time::Duration::from_secs(1)),
        );
        let (v1, h1) = fake("a", &[QuestionType::Boolean], Mode::Err429);
        let (v2, h2) = fake("b", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let resp = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .expect("failover after retry");
        assert_eq!(
            h1.calls.load(Ordering::SeqCst),
            2,
            "同候选重试至耗尽（预算 2）"
        );
        assert_eq!(h2.calls.load(Ordering::SeqCst), 1, "耗尽后才跨候选");
        assert_eq!(resp.upstream_calls, Some(3), "2 同候选 + 1 跨候选实发");
    }

    /// 退避时长按公式累加：虚拟时间下 10s + 20s = 30s（base=10s，两次失败后成功）。
    #[tokio::test(start_paused = true)]
    async fn backoff_uses_virtual_time_exponential_schedule() {
        let base = std::time::Duration::from_secs(10);
        let mut reg = Registry::with_retry(vec![edge("jev", "a", 10)], RetryPolicy::new(3, base));
        let (v1, h1) = fake("a", &[QuestionType::Boolean], Mode::Err429TwiceThenOk);
        reg.register(v1);

        let t0 = tokio::time::Instant::now();
        let resp = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .expect("3rd attempt ok");
        let elapsed = t0.elapsed();
        assert_eq!(h1.calls.load(Ordering::SeqCst), 3);
        assert_eq!(resp.upstream_calls, Some(3));
        assert_eq!(
            elapsed,
            base + base * 2,
            "第 1 次失败退避 base、第 2 次退避 2*base"
        );
    }

    /// 本地类上游（JevError::retryable()=false）：**不重试、不 failover**、
    /// 次候选零触碰；429 非 retryable → 透传 429（非 503）。
    #[tokio::test]
    async fn local_style_upstream_429_not_retried_not_failed_over() {
        let mut reg = Registry::with_retry(
            vec![edge("jev", "local", 10), edge("jev", "b", 30)],
            RetryPolicy::new(5, std::time::Duration::from_millis(1)), // 预算再大也不用
        );
        let (v1, h1) = fake("local", &[QuestionType::Noul], Mode::LocalErr429);
        let (v2, h2) = fake("b", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
        assert_eq!(h1.calls.load(Ordering::SeqCst), 1, "本地不重试");
        assert_eq!(
            h2.calls.load(Ordering::SeqCst),
            0,
            "retryable=false 也不 failover"
        );
        assert_eq!(
            err.http_status(),
            429,
            "非 retryable → 透传上游码（非 503）"
        );
        assert!(!err.retryable());
    }

    /// on_error=fail：首错即返 —— 即使策略给了重试预算也不重试（契约字面）。
    #[tokio::test(start_paused = true)]
    async fn on_error_fail_skips_same_candidate_retry_too() {
        let mut reg = Registry::with_retry(
            vec![
                RouteEdge {
                    on_error: OnError::Fail,
                    ..edge("jev", "a", 10)
                },
                edge("jev", "b", 30),
            ],
            RetryPolicy::new(3, std::time::Duration::from_secs(1)),
        );
        let (v1, h1) = fake("a", &[QuestionType::Boolean], Mode::Err429OnceThenOk);
        let (v2, h2) = fake("b", &[QuestionType::Noul], Mode::Ok);
        reg.register(v1);
        reg.register(v2);

        let err = reg
            .invoke(noul_request("jev"), plain_ctx())
            .await
            .unwrap_err();
        assert_eq!(
            h1.calls.load(Ordering::SeqCst),
            1,
            "fail：第一次错误即返回，零重试"
        );
        assert_eq!(h2.calls.load(Ordering::SeqCst), 0);
        assert_eq!(err.http_status(), 503);
    }
}
