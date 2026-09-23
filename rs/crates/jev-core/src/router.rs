//! 模型路由 DAG（A4 · contracts/03 + 08 §3）
//!
//! - 节点 = 对外 model id ∪ 中间别名 ∪ 提供商 id；边 = [`RouteEdge`]（`[[routes]]`）
//! - [`Router::select`] 返回**有序候选**（priority 升序，稳定排序），支持 exact/prefix、
//!   多跳别名链、`RouteCtx::{sticky_key, exclude}`
//! - 加载配置时用 [`check_acyclic`] 检环，成环拒绝（DAG 约束，contracts/03 §4）
//! - 旧 `[router] "m" = "u"` 扁平表 = 单条 exact 边（由 daemon config 合并成 edges）
//!
//! 选路 API 契约位（contracts/03 §3）：
//! ```text
//! RouteCtx { sticky_key, exclude }
//! Candidate { upstream_id, upstream_model, priority, hops }
//! Router::select(model, ctx) -> Result<Vec<Candidate>, RouterError>
//! ```
//!
//! **契约 §3/§4 歧义与本实现的贴近方式（不改契约，只记录）**：
//! - §3 冻结 `select -> Vec<Candidate>`，Candidate 无 `on_error`/`sticky` 字段；§4 行为
//!   规则表又要求 failover 按候选读取 `on_error`、sticky 按边生效。
//!   实现：内部 [`Router::plan`] 返回 [`PlanItem`]（Candidate + 末跳边策略），
//!   `select` 是其按契约剥壳的视图；daemon failover 循环用 `plan` 取 `on_error`。
//! - 候选策略（`on_error` / `sticky`）取**末跳边**（指向上游的那条边）；
//!   `upstream_model` 沿路径叠加（路径上靠后的 Some 覆盖靠前的，全 None = 沿用入参 model）。
//! - `priority` 排序取末跳边的 priority。
//! - prefix 匹配：仅在**顶层**对入参 model 匹配；`left` 去掉尾部 `*` 后做前缀比较
//!   （`local/*` → 前缀 `local/`）；节点展开（多跳）只走 exact 边。
//! - 粘性：`note_success(sticky_key, upstream_id)` 记忆；同 key 下 sticky=session 的
//!   已记忆候选在 select 时提到首位（失败 failover 由顶层循环负责）。

use crate::upstream::{Capabilities, JevError, QuestionType, Upstream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

/// 上游逻辑 id（契约 `Candidate.upstream_id` / `RouteCtx.exclude` 元素类型）。
pub type UpstreamId = String;
/// 图节点 id（对外 model id / 别名 / 提供商 id）。
pub type NodeId = String;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("no upstream registered for model '{0}'")]
    UnknownModel(String),
    #[error("upstream '{0}' not registered in router")]
    UnknownUpstream(String),
    /// 多跳环（DAG 约束）——配置加载时拒绝（contracts/03 §4）。
    #[error("route graph contains a cycle: {0}")]
    Cycle(String),
}

/* ══════════════════════════════════════════════════════════════════
   边序列化（contracts/03 §2；与 `[[routes]]` 双向等价）
   ══════════════════════════════════════════════════════════════════ */

/// 边匹配模式：`exact`（默认）| `prefix`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Exact,
    Prefix,
}

/// 粘性策略：`none`（默认）| `session`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Sticky {
    #[default]
    None,
    Session,
}

/// 失败策略：`next`（默认，可重试错误试下一候选）| `fail`（首错即返）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    #[default]
    Next,
    Fail,
}

/// 一条模型路由 DAG 边（`[[routes]]` 元素 / 旧 `[router]` 扁平表的等价展开）。
///
/// 字段与 contracts/03 §2 表逐一对齐；默认值：`match=exact`、`priority=0`、
/// `sticky=none`、`on_error=next`、`upstream_model=空`（发上游前不改写 model）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteEdge {
    /// 边起点：对外 id / 别名 / 前缀模式。
    pub left: String,
    /// `exact` | `prefix`（默认 exact）。
    #[serde(rename = "match", default)]
    pub r#match: MatchMode,
    /// 边终点：provider id **或**下游节点 id。
    pub right: String,
    /// 发给上游前的 model 改写；空 = 沿用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    /// 同 `left` 下越小越优先（默认 0）。
    #[serde(default)]
    pub priority: i32,
    /// 默认 `none`。
    #[serde(default)]
    pub sticky: Sticky,
    /// 默认 `next`。
    #[serde(rename = "on_error", default)]
    pub on_error: OnError,
}

impl RouteEdge {
    /// 旧式 `[router] "m" = "u"` 的等价边：单条 exact、默认策略（08 §3 兼容条款）。
    pub fn from_flat(model: &str, upstream: &str) -> Self {
        Self {
            left: model.to_string(),
            r#match: MatchMode::Exact,
            right: upstream.to_string(),
            upstream_model: None,
            priority: 0,
            sticky: Sticky::None,
            on_error: OnError::Next,
        }
    }

    /// 该边是否以 `model` 为入参命中（exact 相等 / prefix 前缀）。
    pub fn matches(&self, model: &str) -> bool {
        match self.r#match {
            MatchMode::Exact => self.left == model,
            MatchMode::Prefix => {
                let pat = self.left.strip_suffix('*').unwrap_or(&self.left);
                !pat.is_empty() && model.starts_with(pat)
            }
        }
    }
}

/* ══════════════════════════════════════════════════════════════════
   选路 API（contracts/03 §3）
   ══════════════════════════════════════════════════════════════════ */

/// 选路上下文：粘性键 + 排除集（sub2api `SelectAccountForModelWithExclusions`
/// 的 `excludedIDs` 对应物）。
#[derive(Debug, Clone, Default)]
pub struct RouteCtx {
    pub sticky_key: Option<String>,
    pub exclude: BTreeSet<UpstreamId>,
}

/// 有序候选（契约冻结四字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub upstream_id: UpstreamId,
    /// 发上游前替换进 `JevRequest.model` 的值（边未配置时 = 调用方原 model）。
    pub upstream_model: String,
    pub priority: i32,
    /// 审计用路径：命中的第一个 right 起，直到（含）终端上游 id。
    pub hops: Vec<NodeId>,
}

/// [`Router::plan`] 的单项：候选 + 末跳边失败策略 + 末跳边粘性。
///
/// 供 daemon failover 读取 `on_error`；`select` 仅投影出 [`Candidate`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    pub candidate: Candidate,
    pub on_error: OnError,
    pub sticky: Sticky,
}

pub struct Router {
    edges: Vec<RouteEdge>,
    upstreams: HashMap<UpstreamId, Box<dyn Upstream>>,
    /// sticky_key → 上次成功候选（session 粘性记忆）。
    sticky: std::sync::Mutex<HashMap<String, UpstreamId>>,
}

impl Router {
    pub fn new(edges: Vec<RouteEdge>, upstreams: HashMap<String, Box<dyn Upstream>>) -> Self {
        Self {
            edges,
            upstreams,
            sticky: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 旧扁平 `model → upstream` 映射的兼容构造（每条 = 单 exact 边）。
    pub fn from_flat(
        mapping: HashMap<String, String>,
        upstreams: HashMap<String, Box<dyn Upstream>>,
    ) -> Self {
        let mut edges: Vec<RouteEdge> = mapping
            .iter()
            .map(|(m, u)| RouteEdge::from_flat(m, u))
            .collect();
        // 稳定顺序：按 left 排，保证旧表行为与 HashMap 迭代序无关
        edges.sort_by(|a, b| a.left.cmp(&b.left));
        Self::new(edges, upstreams)
    }

    /* ── 选路 ──────────────────────────────────────────────── */

    /// 选路主入口（供 handler failover）：有序计划项（含末跳边 `on_error`/`sticky`）。
    ///
    /// - 无匹配边 → [`RouterError::UnknownModel`]（顶层转 404）
    /// - 匹配边全部悬空（right 既非已注册上游、又无出边）→ [`RouterError::UnknownUpstream`]
    /// - `ctx.exclude` 过滤；sticky=session + `ctx.sticky_key` 命中记忆 → 提到首位
    pub fn plan(
        &self,
        model: &str,
        ctx: &RouteCtx,
    ) -> Result<Vec<PlanItem>, RouterError> {
        let mut out: Vec<PlanItem> = Vec::new();
        let mut dangling: Vec<NodeId> = Vec::new();
        let mut any_match = false;

        for e in &self.edges {
            if e.matches(model) {
                any_match = true;
                let mut path = Vec::new();
                self.resolve(e, model, Vec::new(), None, &mut path, &mut out, &mut dangling);
            }
        }

        if !any_match {
            return Err(RouterError::UnknownModel(model.to_string()));
        }
        if out.is_empty() {
            if let Some(d) = dangling.first() {
                return Err(RouterError::UnknownUpstream(d.clone()));
            }
            // 命中边但零候选：仅可能被 exclude 全过滤（由调用方决定 404/跳过）
            return Ok(Vec::new());
        }

        // exclude：排除集过滤（不计入失败语义，由调用方语义决定）
        out.retain(|it| !ctx.exclude.contains(&it.candidate.upstream_id));

        // 同 left/跨边 priority 升序；稳定排序保留配置序
        out.sort_by_key(|it| it.candidate.priority);

        // sticky：同 key 粘住上次成功的 sticky=session 候选（提到首位）
        if let Some(key) = &ctx.sticky_key {
            let remembered = self
                .sticky
                .lock()
                .unwrap()
                .get(key)
                .cloned();
            if let Some(up) = remembered {
                if let Some(pos) = out.iter().position(|it| {
                    it.sticky == Sticky::Session && it.candidate.upstream_id == up
                }) {
                    let it = out.remove(pos);
                    out.insert(0, it);
                }
            }
        }

        Ok(out)
    }

    /// 契约形状的选路（contracts/03 §3 字面）：有序候选，不含策略字段。
    pub fn select(
        &self,
        model: &str,
        ctx: &RouteCtx,
    ) -> Result<Vec<Candidate>, RouterError> {
        Ok(self
            .plan(model, ctx)?
            .into_iter()
            .map(|p| p.candidate)
            .collect())
    }

    /// 记录 sticky_key 的成功候选（session 粘性写入口）。
    /// HTTP 层当前无 sticky key 入口（handler 传 None），供单测/未来 admin 用。
    pub fn note_success(&self, sticky_key: &str, upstream_id: &str) {
        self.sticky
            .lock()
            .unwrap()
            .insert(sticky_key.to_string(), upstream_id.to_string());
    }

    /// 沿一条命中边展开到终端上游（多跳：right 指向别名节点时递归）。
    /// `path` 为路径级防环（配置加载已拒环；运行时再兜底）。
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        &self,
        edge: &RouteEdge,
        origin_model: &str,
        hops_so_far: Vec<NodeId>,
        overlay: Option<String>,
        path: &mut Vec<NodeId>,
        out: &mut Vec<PlanItem>,
        dangling: &mut Vec<NodeId>,
    ) {
        let right = edge.right.clone();
        if path.contains(&right) {
            return; // 运行时兜底：配置本应已拒环
        }
        path.push(right.clone());

        let mut hops = hops_so_far;
        hops.push(right.clone());
        // upstream_model 沿路径叠加：靠后的 Some 覆盖靠前的
        let overlay = edge.upstream_model.clone().or(overlay);

        if self.upstreams.contains_key(&right) {
            out.push(PlanItem {
                candidate: Candidate {
                    upstream_id: right.clone(),
                    upstream_model: overlay.unwrap_or_else(|| origin_model.to_string()),
                    priority: edge.priority,
                    hops,
                },
                on_error: edge.on_error,
                sticky: edge.sticky,
            });
            path.pop();
            return;
        }

        // 别名节点：按 exact left == right 展开（prefix 只在顶层对 model 匹配）
        let children: Vec<RouteEdge> = self
            .edges
            .iter()
            .filter(|e| e.left == right && e.r#match == MatchMode::Exact)
            .cloned()
            .collect();
        if children.is_empty() {
            dangling.push(right);
            path.pop();
            return;
        }
        for child in &children {
            self.resolve(
                child,
                origin_model,
                hops.clone(),
                overlay.clone(),
                path,
                out,
                dangling,
            );
        }
        path.pop();
    }

    /* ── 兼容 / 装配视图 ─────────────────────────────────────── */

    /// 兼容包装：取 `select` 第一候选对应的 upstream（旧 1:1 语义）。
    pub fn route(&self, model: &str) -> Result<&dyn Upstream, RouterError> {
        let cands = self.select(model, &RouteCtx::default())?;
        let first = cands
            .into_iter()
            .next()
            .ok_or_else(|| RouterError::UnknownModel(model.to_string()))?;
        self.upstream(&first.upstream_id)
            .ok_or_else(|| RouterError::UnknownUpstream(first.upstream_id))
    }

    /// 按 id 取已注册上游（handler 逐候选调用）。
    pub fn upstream(&self, id: &str) -> Option<&dyn Upstream> {
        self.upstreams.get(id).map(|b| b.as_ref())
    }

    /// 所有对外可见的 model id（全部边的 left 去重排序；含 prefix 模式）。
    pub fn list_models(&self) -> Vec<String> {
        let mut v: Vec<String> = self.edges.iter().map(|e| e.left.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// 全部边（admin/审计视图）。
    pub fn edges(&self) -> &[RouteEdge] {
        &self.edges
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

    /// 校验 capability：题型不被**第一候选**上游支持 → 422 语义（旧行为保留）。
    /// failover 路径的逐候选校验由 handler/`plan` 循环自行做（跳过 ≠ 失败）。
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
            RouterError::Cycle(c) => JevError::Config {
                upstream_id: model.to_string(),
                message: c,
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

/* ══════════════════════════════════════════════════════════════════
   检环（DAG 约束 —— 配置加载时调用）
   ══════════════════════════════════════════════════════════════════ */

/// 对全部边构成的 left→right 有向图做 DFS 三色检环；发现环 →
/// [`RouterError::Cycle`]（携带环路径）。配置加载时必须调用（contracts/03 §4）。
pub fn check_acyclic(edges: &[RouteEdge]) -> Result<(), RouterError> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        adj.entry(e.left.as_str()).or_default().push(e.right.as_str());
    }

    #[derive(Clone, Copy, PartialEq)]
    enum St {
        Gray,
        Black,
    }
    let mut state: HashMap<&str, St> = HashMap::new();
    let mut stack: Vec<&str> = Vec::new();

    fn dfs<'a>(
        n: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        state: &mut HashMap<&'a str, St>,
        stack: &mut Vec<&'a str>,
    ) -> Result<(), RouterError> {
        match state.get(n) {
            Some(St::Black) => return Ok(()),
            Some(St::Gray) => {
                let start = stack.iter().position(|x| *x == n).unwrap_or(0);
                let mut path = stack[start..].to_vec();
                path.push(n);
                return Err(RouterError::Cycle(path.join(" -> ")));
            }
            None => {}
        }
        state.insert(n, St::Gray);
        stack.push(n);
        if let Some(rs) = adj.get(n) {
            for r in rs {
                dfs(r, adj, state, stack)?;
            }
        }
        stack.pop();
        state.insert(n, St::Black);
        Ok(())
    }

    for e in edges {
        dfs(e.left.as_str(), &adj, &mut state, &mut stack)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::SystemOneRequest;

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

    fn ups(ids: &[(&str, &'static [QuestionType])]) -> HashMap<String, Box<dyn Upstream>> {
        ids.iter().map(|(id, qts)| (id.to_string(), mock(id, *qts))).collect()
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

    /* ── 兼容回归（旧 1:1 行为） ───────────────────────────── */

    #[test]
    fn route_known_model() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let r = Router::from_flat(mapping, ups(&[("u-a", &[QuestionType::Noul])]));
        let up = r.route("m-a").unwrap();
        assert_eq!(up.id(), "u-a");
    }

    #[test]
    fn route_unknown_model() {
        let r = Router::from_flat(HashMap::new(), HashMap::new());
        assert!(matches!(r.route("nope"), Err(RouterError::UnknownModel(_))));
    }

    #[test]
    fn route_known_model_unknown_upstream() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "ghost".into());
        let r = Router::from_flat(mapping, HashMap::new());
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
        let r = Router::from_flat(mapping, HashMap::new());
        assert_eq!(r.list_models(), vec!["alpha", "zeta"]);
    }

    #[test]
    fn capability_check_passes() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let r = Router::from_flat(mapping, ups(&[("u-a", &[QuestionType::Noul])]));
        r.check_capability("m-a", &[QuestionType::Noul]).unwrap();
    }

    #[test]
    fn capability_check_fails() {
        let mut mapping = HashMap::new();
        mapping.insert("m-a".into(), "u-a".into());
        let r = Router::from_flat(
            mapping,
            ups(&[("u-a", &[QuestionType::Choice, QuestionType::Score])]),
        );
        let err = r.check_capability("m-a", &[QuestionType::Noul]).unwrap_err();
        assert_eq!(err.http_status(), 422);
    }

    /* ── A4：DAG select ─────────────────────────────────────── */

    #[test]
    fn select_dual_candidates_ordered_by_priority() {
        // 验收 ①（选路侧）：同 model "jev" 双候选，p10 在 p30 前
        let mut e1 = edge("jev", "vercel", 10);
        e1.upstream_model = Some("typesafe-ai/jev".into());
        let mut e2 = edge("jev", "laya", 30);
        e2.upstream_model = Some("laya-english".into());
        let r = Router::new(
            vec![e2.clone(), e1.clone()], // 配置序故意打乱
            ups(&[
                ("vercel", &[QuestionType::Choice, QuestionType::Boolean]),
                ("laya", &[QuestionType::Noul]),
            ]),
        );
        let c = r.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].upstream_id, "vercel");
        assert_eq!(c[0].upstream_model, "typesafe-ai/jev");
        assert_eq!(c[0].priority, 10);
        assert_eq!(c[0].hops, vec!["vercel"]);
        assert_eq!(c[1].upstream_id, "laya");
        assert_eq!(c[1].upstream_model, "laya-english");
        assert_eq!(c[1].priority, 30);
    }

    #[test]
    fn select_prefix_local_star_hits() {
        // 验收 ②：local/* prefix 命中
        let pe = RouteEdge {
            left: "local/*".into(),
            r#match: MatchMode::Prefix,
            right: "laya".into(),
            upstream_model: None,
            priority: 40,
            sticky: Sticky::None,
            on_error: OnError::Next,
        };
        let r = Router::new(vec![pe], ups(&[("laya", &[QuestionType::Noul])]));

        let c = r.select("local/qwen", &RouteCtx::default()).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].upstream_id, "laya");
        // 无 upstream_model → 沿用调用方原 model
        assert_eq!(c[0].upstream_model, "local/qwen");
        assert_eq!(c[0].hops, vec!["laya"]);

        // 前缀外不命中
        assert!(matches!(
            r.select("global/x", &RouteCtx::default()),
            Err(RouterError::UnknownModel(_))
        ));
    }

    #[test]
    fn select_multi_hop_alias_chain() {
        // 验收 ③：jev → jev-fast（别名）→ vercel
        let mut via = edge("jev", "jev-fast", 5);
        via.on_error = OnError::Fail; // 中间边策略不影响候选（取末跳边）
        let mut last = edge("jev-fast", "vercel", 10);
        last.upstream_model = Some("typesafe-ai/jev".into());
        last.on_error = OnError::Next;
        let r = Router::new(
            vec![via, last],
            ups(&[("vercel", &[QuestionType::Boolean])]),
        );
        let c = r.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].upstream_id, "vercel");
        assert_eq!(c[0].hops, vec!["jev-fast", "vercel"]);
        assert_eq!(c[0].upstream_model, "typesafe-ai/jev");
        assert_eq!(c[0].priority, 10); // 末跳边 priority

        let plan = r.plan("jev", &RouteCtx::default()).unwrap();
        assert_eq!(plan[0].on_error, OnError::Next); // 末跳边策略
    }

    #[test]
    fn upstream_model_overlay_along_chain() {
        // 路径上游叠加：前边 Some、末边 None → 前边值存活；末边 Some → 覆盖
        let mut a = edge("jev", "alias", 1);
        a.upstream_model = Some("from-first-edge".into());
        let b = edge("alias", "u", 1); // None
        let r = Router::new(vec![a, b], ups(&[("u", &[QuestionType::Noul])]));
        let c = r.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c[0].upstream_model, "from-first-edge");

        let mut a2 = edge("jev", "alias", 1);
        a2.upstream_model = Some("first".into());
        let mut b2 = edge("alias", "u", 1);
        b2.upstream_model = Some("last".into());
        let r2 = Router::new(vec![a2, b2], ups(&[("u", &[QuestionType::Noul])]));
        let c2 = r2.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c2[0].upstream_model, "last");
    }

    #[test]
    fn select_exclude_filters_candidates() {
        let r = Router::new(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            ups(&[
                ("vercel", &[QuestionType::Boolean]),
                ("laya", &[QuestionType::Noul]),
            ]),
        );
        let mut ctx = RouteCtx::default();
        ctx.exclude.insert("vercel".into());
        let c = r.select("jev", &ctx).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].upstream_id, "laya");

        ctx.exclude.insert("laya".into());
        let c2 = r.select("jev", &ctx).unwrap();
        assert!(c2.is_empty()); // 全排除 → 空（顶层负责 404/跳过语义）
    }

    #[test]
    fn sticky_session_hoists_remembered_candidate() {
        // sticky=session + note_success：同 key 粘住上次成功候选
        // laya 边开 sticky=session
        let router = Router::new(
            vec![
                edge("jev", "vercel", 10),
                RouteEdge {
                    sticky: Sticky::Session,
                    ..edge("jev", "laya", 30)
                },
            ],
            ups(&[
                ("vercel", &[QuestionType::Boolean]),
                ("laya", &[QuestionType::Noul]),
            ]),
        );
        router.note_success("sess-1", "laya");
        let mut ctx = RouteCtx::default();
        ctx.sticky_key = Some("sess-1".into());
        let c = router.select("jev", &ctx).unwrap();
        // 记忆命中 laya（sticky=session 边）→ 提到首位，尽管 priority 30 > 10
        assert_eq!(c[0].upstream_id, "laya");
        assert_eq!(c[1].upstream_id, "vercel");

        // 不同 key 不粘
        let mut ctx2 = RouteCtx::default();
        ctx2.sticky_key = Some("sess-2".into());
        let c2 = router.select("jev", &ctx2).unwrap();
        assert_eq!(c2[0].upstream_id, "vercel");

        // 无 sticky_key 不粘
        let c3 = router.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c3[0].upstream_id, "vercel");
    }

    #[test]
    fn dangling_right_reports_unknown_upstream() {
        let r = Router::new(vec![edge("jev", "ghost", 1)], HashMap::new());
        assert!(matches!(
            r.select("jev", &RouteCtx::default()),
            Err(RouterError::UnknownUpstream(u)) if u == "ghost"
        ));
    }

    #[test]
    fn partial_dangling_keeps_resolved_candidates() {
        // vercel 悬空（未注册）但 laya 在 → 只回 laya（不可路由过滤的选路侧基础）
        let r = Router::new(
            vec![edge("jev", "vercel", 10), edge("jev", "laya", 30)],
            ups(&[("laya", &[QuestionType::Noul])]),
        );
        let c = r.select("jev", &RouteCtx::default()).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].upstream_id, "laya");
    }

    #[test]
    fn exact_and_prefix_both_contribute_candidates() {
        let r = Router::new(
            vec![
                edge("local/foo", "vercel", 10),
                RouteEdge {
                    r#match: MatchMode::Prefix,
                    ..edge("local/*", "laya", 20)
                },
            ],
            ups(&[
                ("vercel", &[QuestionType::Boolean]),
                ("laya", &[QuestionType::Noul]),
            ]),
        );
        let c = r.select("local/foo", &RouteCtx::default()).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].upstream_id, "vercel");
        assert_eq!(c[1].upstream_id, "laya");
    }

    /* ── 检环 ────────────────────────────────────────────────── */

    #[test]
    fn check_acyclic_accepts_dag() {
        // jev → jev-fast → vercel + 旁路：无环
        let edges = vec![
            edge("jev", "jev-fast", 5),
            edge("jev-fast", "vercel", 10),
            edge("jev", "laya", 30),
            RouteEdge {
                r#match: MatchMode::Prefix,
                ..edge("local/*", "laya", 40)
            },
        ];
        check_acyclic(&edges).unwrap();
    }

    #[test]
    fn check_acyclic_rejects_cycle() {
        // 验收 ④：a → b → a 成环
        let edges = vec![edge("a", "b", 1), edge("b", "a", 2)];
        let err = check_acyclic(&edges).unwrap_err();
        assert!(matches!(err, RouterError::Cycle(_)));
        assert!(err.to_string().contains("a -> b -> a") || err.to_string().contains("cycle"));
    }

    #[test]
    fn check_acyclic_rejects_self_loop() {
        let err = check_acyclic(&[edge("a", "a", 1)]).unwrap_err();
        assert!(matches!(err, RouterError::Cycle(_)));
    }

    #[test]
    fn check_acyclic_rejects_cycle_reachable_from_query_start() {
        // 环挂在别名链中间也拒绝
        let edges = vec![
            edge("jev", "x", 1),
            edge("x", "y", 1),
            edge("y", "x", 1),
        ];
        assert!(check_acyclic(&edges).is_err());
    }

    /* ── RouteEdge 默认值 / 序列化 ───────────────────────────── */

    #[test]
    fn route_edge_defaults_match_contract() {
        let e: RouteEdge = serde_json::from_value(serde_json::json!({
            "left": "jev", "right": "vercel"
        }))
        .unwrap();
        assert_eq!(e.r#match, MatchMode::Exact);
        assert_eq!(e.priority, 0);
        assert_eq!(e.sticky, Sticky::None);
        assert_eq!(e.on_error, OnError::Next);
        assert_eq!(e.upstream_model, None);

        let e2: RouteEdge = serde_json::from_value(serde_json::json!({
            "left": "jev", "match": "prefix", "right": "laya",
            "upstream_model": "laya-english", "priority": 30,
            "sticky": "session", "on_error": "fail"
        }))
        .unwrap();
        assert_eq!(e2.r#match, MatchMode::Prefix);
        assert_eq!(e2.sticky, Sticky::Session);
        assert_eq!(e2.on_error, OnError::Fail);
        assert_eq!(e2.upstream_model.as_deref(), Some("laya-english"));
    }

    #[test]
    fn plan_preserves_on_error_per_candidate() {
        let r = Router::new(
            vec![
                RouteEdge {
                    on_error: OnError::Fail,
                    ..edge("jev", "vercel", 10)
                },
                edge("jev", "laya", 30), // 默认 Next
            ],
            ups(&[
                ("vercel", &[QuestionType::Boolean]),
                ("laya", &[QuestionType::Noul]),
            ]),
        );
        let plan = r.plan("jev", &RouteCtx::default()).unwrap();
        assert_eq!(plan[0].on_error, OnError::Fail);
        assert_eq!(plan[1].on_error, OnError::Next);
    }
}
