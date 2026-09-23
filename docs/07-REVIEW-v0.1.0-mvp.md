# Jev-Switch 全方位评审 — v0.1.0-mvp

**作者（Author）：Mimo-V2.6-Pro**

> **作者**：Mimo-V2.6-Pro  
> **评审时间**：2026-09-23（增补 v1.1：UI 配置 / DAG 路由管理（默认二部图视图）/ serde 式扩展 / frontend-design）  
> **对齐结论**：Q1=A 编译期 trait；Q2=A+B 先、稳定后 C；Q3=契约→并行 AB→合流；Q4=b 明文 toml+UI 密文；Q5=a 文件为准。详见 `08-CONTRACT-功能特性契约与施工计划.md`。  
> **评审对象**：`ARCJ137442/jev-switch` @ `821e304`（tag `v0.1.0-mvp`）  
> **评审身份**：综合评审者（架构 / 协议保真 / 可集成性 / 成熟度 / 与 Jev 生态对齐）  
> **读者**：后续开发本仓库的 Agent。本文是**意见与约束**，不是施工图。施工图请回看 `02-`–`06-`，但**以本文的 P0/P1 纠偏为准**。  
> **评审基线**：`jev-life/src/shared/types.ts`（Jev 协议事实标准）、`docs/04-` 的 workspace 三 crate 设计、sub2api 的 `CompositeModelRoute` + `SelectAccountForModel` 调度模型、以及 TypeSafe 官方 API 形状。

---

## 0. 执行摘要（给赶时间的 Agent）

| 维度 | 评分 | 一句话 |
|---|---|---|
| MVP 目标达成 | ✅ **真达成了** | 4 条「做到」标准端到端通过（`docs/PROGRESS.md`），Vercel + Laya 双上游 + 翻译层 + UI 可跑 |
| 可集成性 | ❌ **不可集成** | 只有 `main.rs`、无 `lib.rs`、模块私有 —— 其他 Rust 应用**无法** `use jev_switch::*` |
| 模块化 | ⚠️ **设计有、落地无** | `docs/04-` 已画出 protocol/core/daemon 三 crate，M0 明确「简化: 不拆」；现在卡在单 crate 私有模块 |
| 协议保真 | ⚠️ **能用但偏松** | 与 jev-life TS 版有 12 处形状差异；`BooleanAnswer.probability` 键在归一化层**丢失** |
| 正确性 | ❌ **有实锤 bug** | `translate.rs` 注释说「用 probability」，实现读的是 `boolean` 并硬编码 0.95/0.05 |
| 路由能力 | ⚠️ **仅静态映射** | `model → upstream` 一对一；**没有**「同模型多供应商可切换 / 不同模型自动路由」；目标是**模型路由 DAG**；二部图是最简形式 / 默认编辑器（§3.6） |
| 成熟度 | 🟡 **MVP 刚封存** | 11 commits / 2 天 / tag `v0.1.0-mvp`；缺 lib、缺根 README、缺 LICENSE 文件、缺 examples、缺集成测试 |

**总裁决**：  
> **v0.1.0-mvp 作为「证明路由+翻译能跑通的 demo」是合格的；作为「可被其他 Rust 应用集成的 Jev 协议/路由基础设施」不合格。下一版的第一优先级不是加第 5 个上游，而是：拆 lib / 拆 workspace / 修 probability bug / 路由升级为二部图接线（文件 + 图形化）/ 提供商配置进 UI（学 CC Switch）/ serde 式零内核修改扩展点。前端交互统一走 `/frontend-design` 向 CC Switch 靠。**

---

## 1. 本次相对上一轮评估的增量

拉取结果：`c3180ac..821e304`，**2 个新 commit + 1 个 tag**。

| Commit | 内容 | 评价 |
|---|---|---|
| `37b4242` fix(ui): React app 渲染崩溃 root=0 修复 | `normalizeModels()` 兼容 `{models}`/`{data}`；`App.tsx` Array 防御；`main.tsx` `RootErrorBoundary` | **好**。根因分析写得很扎实（`m.data` 为 undefined → `serverModels.length` 抛错 → React 18 unmount）。这是**契约漂移**的症状，不是 UI 手滑 —— 见 §4.3 |
| `821e304` docs: PROGRESS v0.1.0-mvp 进度快照 | `docs/PROGRESS.md` 181 行 | **好**。可复现步骤、实测数据（Laya `auth: 0.7608, confidence: 0.4782` @ 170-200ms）、M1+ ROI 排序都有 |
| tag `v0.1.0-mvp` | 封存说明 | **好**。有意识地打里程碑 |

**没有变化的（上一轮已指出、本轮仍在的）**：
- 仍无 `lib.rs` / workspace
- 仍无根 `README.md` / `LICENSE` 文件
- `translate.rs` 的 `probability` bug **仍在**（机器验证：`build_jev_response_from_vercel` 中 `va.probability` 出现次数 = **0**）
- `docs/README.md` **仍过时**（自述「规划与文档仓库」、`03-/04-/05-` 状态标「待写」，实际早已写完且代码已落地）
- `SystemOneResponse` 仍无 `upstreamCalls` / `costUsd` / `latencyMs`
- 路由仍是 1:1 静态表

**PROGRESS.md 自我认知是准的**，它自己也把「全 5 类上游 / Router 加固 / OTel」排在 M1+。问题在于：**M1+ 的清单里漏了「拆 lib / 升级路由模型」这两件基础设施**，而它们才是「能不能被集成 / 能不能叫多上游路由器」的门槛。

---

## 2. 值得保留的优点（不要在重构里弄丢）

1. **4 条「做到」标准是端到端实测，不是自嗨**  
   `docs/PROGRESS.md:11-16` 有具体数值（Laya 概率 0.7608 / confidence 0.4782 / 170-200ms）。这种「先立可证伪标准再写代码」的纪律，与 jev-rime 的 TDD 红线同源，**必须保留**。

2. **`noul`↔`boolean` 调用方视角不可见**  
   调用方永远发 `type: noul`，网关翻译给 Vercel。这是 Jev-Switch 的核心产品价值（`docs/06` 验收 D8：两次 POST payload 一模一样）。**翻译层方向正确**，只是实现有 bug（§5.1）。

3. **Capability 表 + `noul_via_boolean` 标志**（`rs/src/upstream.rs:102-145`）  
   按能力而不是按厂商索引，继承 jev-life `capabilitiesOf` 的正确抽象。`supports()` 里「Vercel 经翻译层可收 noul」的短路是对的。

4. **`JevError::retryable` + `http_status()` 映射**（`rs/src/upstream.rs:54-79`）  
   429/5xx → 对外 503 + `retryable: true`，语义清楚。缺的是**真的去 retry**（§5.3）。

5. **UI 契约漂移的修复方式值得表扬**  
   `normalizeModels()` 不是硬改成某一边，而是**两边都接受**。但请注意：这是**止血**，根治要靠协议层统一（§4.3）。

6. **文档层次完整**（01 需求 → 02 规划 → 03 兼容矩阵 → 04 架构 → 05 差异化 → 06 施工 → PROGRESS 快照）。在同类仓库里文档密度罕见。

---

## 3. 架构评审：向「可集成 + 模块化」演进

### 3.1 P0：只有 `main` 没有 `lib` —— 集成性为零

**现状**（`rs/src/main.rs:12-18`）：

```rust
mod config;
mod protocol;
mod router;
mod translate;
mod upstream;
mod upstream_laya;
mod upstream_vercel;
```

**事实**：
- 无 `rs/src/lib.rs`
- 无 `Cargo.toml` 的 `[lib]` 段
- 所有 `pub` 只在 crate 内可见，对外是 binary
- 仓库 private、未发 crates.io

**后果**（可验证的）：
1. `jev-rime` 无法 `jev-switch = { path = "../jev-switch/rs" }` 然后 `use jev_switch::protocol::SystemOneRequest`
2. 任何想「在自己进程里做 Jev 决策」的 Rust 应用只能：(a) 复制源码，或 (b) 起 daemon 再 HTTP 调
3. 单元测试只能测本 crate 内部；**外部集成测试写不了**
4. `docs/04-` 设计里的「`protocol` 与 sys1 共享（未来可独立 publish）」**完全落空**

**这正是用户指出的点，完全成立。** Jev-Switch 的名字承诺「路由器/基础设施」，基础设施的最低门槛是**可被依赖**。

### 3.2 P0：模块化 —— 路由核心 vs 供应商适配 vs 数据转换 必须三层分离

用户的方向是对的。对照现状：

| 应有层次 | 职责 | 现状 | 差距 |
|---|---|---|---|
| **协议/数据层** | TypeSafe API 形状为基准、可扩展的 serde 类型 + 编解码 | `protocol.rs` 扁平可选字段 + `Value` 逃生舱 | 见 §4；缺「基于 TypeSafe 但可扩展」的稳定形态 |
| **路由核心层** | model→upstream 解析、策略（failover / sticky / weighted）、capability 匹配、熔断 | `router.rs` 仅 `HashMap<String,String>` 1:1 查表 | 无策略、无同模型多候选、无 health |
| **供应商适配层** | 各网关的 HTTP + 字段翻译 | `upstream_vercel.rs` / `upstream_laya.rs`，翻译塞在 `translate.rs` 且**硬编码 vercel** | 适配与翻译未模块化；`translate::normalize_request_for_vercel` 名字就写死了厂商 |
| **API 路由层**（daemon） | axum 路由、鉴权、CORS、admin | `main.rs` 一个文件干了启动+装配+HTTP+错误映射 | 与 `docs/04-` 的 `daemon/http` 分层不符 |

**建议的 crate 切分**（与 `docs/04-§2.1` 对齐，略作修正以满足「可集成」）：

```
rs/                              # cargo workspace
├── crates/
│   ├── jev-protocol/            # lib —— 纯 serde 类型 + noul/boolean 判别 + 编解码
│   │   └── src/lib.rs           #   零 tokio/reqwest；未来可独立 publish
│   ├── jev-core/                # lib —— 路由核心 + Upstream trait + capability + 翻译策略
│   │   └── src/
│   │       ├── router.rs        #   RouterStrategy: Exact | FailoverChain | StickySession | Weighted
│   │       ├── upstream.rs      #   trait Upstream + Capabilities
│   │       ├── translate.rs     #   trait ProtocolAdapter（不再写死 vercel）
│   │       └── error.rs
│   ├── jev-adapters/            # lib —— 具体调度商/网关适配（可按 feature 裁剪）
│   │   └── src/
│   │       ├── vercel_gateway.rs
│   │       ├── typesafe.rs
│   │       ├── openrouter.rs
│   │       ├── laya.rs
│   │       └── llm2jev.rs
│   └── jev-switch-daemon/       # bin —— axum + config + admin；依赖上述三个 lib
│       └── src/main.rs
└── examples/                    # 被依赖方怎么用 jev-protocol / jev-core 的可运行样例
```

**可集成性的验收标准（请开发 Agent 照此自测）**：

```bash
# 在一个全新 crate 里：
cargo add --path ../jev-switch/crates/jev-protocol
# 且以下代码能编译：
use jev_protocol::{SystemOneRequest, DecisionQuestion, Criteria};
```

**做不到这条，M1 的「加 3-4 个 Upstream」只是在错误的骨架上堆肉。**

### 3.3 数据转换层：学 serde，但以 TypeSafe API 为基准做「可扩展改造」

用户要求：「数据转换层学习 serde（有一个基于 TypeSafe API 但经过可扩展性改造的数据结构）」。理解为：

- **内核形态**以 TypeSafe 官方 `/v1/systemone` 的 request/response 为**唯一真值**
- **扩展点**显式化，而不是像现在这样用 `serde_json::Value` 和全 `Option` 打洞

**对照 jev-life `types.ts` 与当前 `protocol.rs`，建议的形态**：

```rust
// jev-protocol —— 内核：对齐 TypeSafe / jev-life
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Choice { instructions: String, criteria: Criteria },  // Criteria::Map
    Score  { instructions: String, criteria: Criteria },  // Criteria::List
    Noul   { instructions: String, criteria: Criteria },  // Criteria::Bool { true, false }
}

#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Choice { choice: String, probabilities: BTreeMap<String, f64>, confidence: f64 },
    Score  { score: f64,          probabilities: BTreeMap<String, f64>, confidence: f64 },
    Noul   {                      // 布尔族：概率本身即置信度，无 confidence 字段
        #[serde(flatten)] prob: BooleanProbability,   // noul? | probability? 双键
    },
}

/// 扩展点 1：判别值的方言。默认 TypeSafe/OpenRouter/AI-ML = noul；Vercel = boolean
pub trait NoulDialect {
    fn discriminator(&self) -> &'static str;  // "noul" | "boolean"
}

/// 扩展点 2：上游私有字段走 #[serde(flatten)] extra，不污染内核字段名
pub struct SystemOneResponse {
    pub model: Option<String>,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Option<Usage>,          // 强类型 Usage，不是 Value
    pub upstream_calls: Option<u32>,   // jev-life 语义：工具循环 N 次
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>, // providerMetadata 等
}
```

**关键要求**：
1. **`criteria` 必填 + 强类型**（jev-life 实测：缺失 = 上游 400）。当前 `Option<Value>` 会把错误推迟到运行时。
2. **`Answer` 用判别联合**，不要扁平全 Option。布尔答案**没有** `confidence`，这是一条不变量，现在完全丢失。
3. **`BooleanProbability` 同时保留 `noul` 与 `probability` 双键**（jev-life `noulProbability()` 的做法）。当前 `DecisionAnswer` 归一化后只剩 `noul`，**丢掉 Vercel 真实键名 `probability`**。
4. **翻译层变成 `ProtocolAdapter` trait**，按上游注册，而不是 `translate.rs` 文件名写死 vercel：

```rust
pub trait ProtocolAdapter {
    fn outgoing(&self, req: SystemOneRequest) -> SystemOneRequest;   // noul → boolean 等
    fn incoming(&self, raw: Value, req: &SystemOneRequest) -> SystemOneResponse;
}
```

### 3.4 API 路由层：学 sub2api 的「一个网关，两套路由语义」

用户要求：「API 路由层学习 sub2api（一个网关，不同的模型→自动路由，相同模型→可切换路由）」。

sub2api（`Wei-Shaw/sub2api`，Go）的可借鉴点，精确到源码：

**A. `CompositeModelRoute`（公开模型名 → 上游模型 + 平台）**  
`backend/ent/schema/composite_model_route.go`：

| 字段 | 含义 | 对 Jev-Switch 的映射 |
|---|---|---|
| `public_model` | 客户端看到的模型名或前缀 | 我们调用方写的 `model` 字段 |
| `match_type` | `exact` / `prefix` | 支持 `typesafe-ai/*` 这类前缀族 |
| `target_platform` | 具体供应商平台 | `vercel` / `laya` / `typesafe` / `openrouter` |
| `upstream_model` | 真正发给上游的模型名；空 = 沿用 public_model | 例如 `typesafe-ai/jev` → Vercel 的 `ai-model-id` |
| `endpoint` | `any` / `messages` / ... 路由作用域 | 可映射到 question-type 范围 |
| `priority` | 同匹配强度下数值小者优先 | 同模型多候选的默认序 |
| `enabled` | 软开关 | 对应 `providers.*.enabled` |

**B. 调度：`SelectAccountForModel`（粘性会话 + 优先级 + 模型映射 + 排除集）**  
`backend/internal/service/gateway_scheduling.go`：

```go
SelectAccountForModelWithExclusions(ctx, groupID, sessionHash, requestedModel, excludedIDs)
// 1. composite group → resolveCompositeRouteDecision → 改写 platform + upstreamModel
// 2. 按粘性会话 / 优先级 / 模型支持过滤候选
// 3. 负载感知排序
// 4. 失败可带 excludedIDs 重选（failover）
```

**映射到用户说的两套语义**：

| 语义 | sub2api 做法 | Jev-Switch 应做 |
|---|---|---|
| **不同模型 → 自动路由** | `public_model` / prefix → `target_platform` + `upstream_model` | `model: "laya-english"` 自动去 Laya；`model: "typesafe-ai/jev"` 自动去 Vercel；**并支持前缀族**（`model: "local/*"` → 本地池） |
| **相同模型 → 可切换路由** | 同一 `public_model` 在 group 内多 account；`priority` + `sessionHash` 粘性 + `excludedIDs` failover | 同一个逻辑模型（如 `jev`）可配置 `[routes.jev] candidates = [{upstream, priority}, ...]`；调用方**可选** sticky key；429/5xx 时按 priority 换下一个 |

**当前差距（实测）**：`rs/src/router.rs:36-44`

```rust
pub fn route(&self, model: &str) -> Result<&dyn Upstream, RouterError> {
    let upstream_id = self.mapping.get(model).ok_or(...)?;  // 1:1，无候选列表
    self.upstreams.get(upstream_id)...
}
```

`providers.example.toml` 也是 1:1：

```toml
[router]
"laya-english" = "laya"
"typesafe-ai/jev" = "vercel"
```

**建议的 config 形态（兼容现有 1:1，向后兼容扩展）**：

```toml
# 旧写法继续有效（等价于 exact + 单候选）
"laya-english" = "laya"

# 新写法：同模型多候选 = 可切换路由
[routes.jev]
match = "exact"                 # 或 "prefix"
candidates = [
  { upstream = "vercel",  upstream_model = "typesafe-ai/jev", priority = 10 },
  { upstream = "typesafe", upstream_model = "jev-latest",     priority = 20 },
  { upstream = "laya",    upstream_model = "laya-english",    priority = 30 },
]
sticky = "session"              # none | session | header:x-jev-sticky
on_error = "next"               # next | fail（按 priority 换 / 直接失败）
```

**路由核心与调度商适配的边界**（用户强调的点，写死给后续 Agent）：

```
jev-core::Router
  输入: SystemOneRequest + RouteContext { sticky_key, exclude: Set<UpstreamId> }
  输出: Vec<Candidate>   // 有序候选，不是单个 Upstream
  只认识: UpstreamId + Capabilities + priority
  不认识: HTTP、API key、厂商 URL、字段翻译

jev-adapters::*
  输入: 已经过 ProtocolAdapter::outgoing 的请求
  输出: 原始 JSON（翻译在 incoming 回调）
  只认识: 自己厂商的 URL / header / 字段方言
  不认识: 优先级、粘性、failover
```

---


### 3.5 P1 — 提供商配置进 UI（学 CC Switch）

**用户指令（必须执行）**：后续提供商配置应**尽可能在 UI 中直接完成**，形态向 **CC Switch**（`farion1231/cc-switch`）靠拢。

**现状**：提供商只活在 `rs/providers.example.toml` + 环境变量（`AI_GATEWAY_API_KEY`）。UI 里的 `PROVIDERS` 是**写死的展示常量**（`ui/src/api.ts:170-185`），不能增删改，`enabled` 字段只是摆设。用户改一个上游要停进程、改 toml、export key、重启。

**目标形态（CC Switch 式）**：

| 能力 | 要求 | 验收 |
|---|---|---|
| 提供商卡片列表 | 名称 / kind / base URL / 可用性 / 启停开关 | UI 可见，开关即时生效或一键重载 |
| 密钥管理 | 在 UI 粘贴 API key；后端落 `keyring` 或本地加密文件，**不进 git、不进日志** | 粘贴后能跑通一次真实调用；`git status` 无密钥 |
| 增删提供商 | 表单或「粘贴 toml/json」两种入口 | 新增一个 mock provider 无需改 Rust 源码 |
| 健康探测 | 每卡片 `Probe` 按钮 + 最近延迟/最近错误 | 停掉 Laya 后卡片变红 |
| 配置持久化 | 写 `~/.jev-switch/providers.toml`（或 sqlite）；启动加载 | 重启 daemon 后 UI 配置仍在 |
| 双向同步 | toml 手改 → UI 刷新可见；UI 改 → toml 一致 | 单一事实源，禁止 UI 一份、toml 一份 |

**硬约束**：
- 密钥**永远**只存在服务端；UI 只收「已配置 / 末 4 位」回显。
- 配置 schema 与 `jev-core` 的 `ProviderConfig` **同一份**（serde derive），禁止 UI 再手写第三份类型。
- **前端与交互设计必须走 `/frontend-design` skill，视觉与交互向 CC Switch 靠**（见 §3.7）。

---

### 3.6 P1 — **模型路由 DAG**（配置文件 + 图形化两种方式；二部图 = 最简形式）

**用户指令（必须执行）**：路由配置要**跟电路接线一样简单**。两种等价入口：

1. **配置文件**（工程师/可 diff/可进 git）
2. **图形化增减连线**（默认给人用）—— **用 DAG 的方式呈现与管理模型路由**

**形态说明（2026-09-23 定稿 · 后续修订）**：

- **正名：「模型路由 DAG」** —— 更准确、更可拓展。这是**数据模型与管理语义**的主形态：有向、可多跳（对外 id → 别名 → 上游）、边上带 priority / sticky / on_error。
- **二部图 = 模型路由 DAG 的最简形式**（两层：对外 model id ∥ 提供商，无中间别名层）。它是**合适的查看与交互起点**，不是路由模型本身。
- **实践条款**：第一版 UI 用二部图连线题验证交互；若前缀节点 / 多跳别名 / 边上参数表达不动，**升级到通用 DAG 视图**，`[[routes]]` 数据模型不变、可向后兼容扩展（如允许 `left` 出现在右侧成为中间节点）。
- 引擎与 API 层**从第一天就按 DAG 写**（不要按二部图写死两层）；只有编辑器默认长成二部图。

> 术语表：**模型路由 DAG** = 路由模型（主词）；**二部图** = 其最简形式 / 默认编辑器视图。

```
  对外暴露的 model id          提供商 / upstream
  ┌─────────────────┐          ┌─────────────────┐
  │ jev             │──────────│ vercel          │
  │                 │─────┐    │  (priority=10)  │
  ├─────────────────┤     │    ├─────────────────┤
  │ laya-english    │     └────│ laya            │
  ├─────────────────┤          │  (priority=30)  │
  │ local/*         │──────────├─────────────────┤
  │                 │          │ typesafe        │
  └─────────────────┘          └─────────────────┘
        左列（public_model）          右列（providers）
```

**交互规则（写进前端需求）**：
- 左列：对外暴露的 model id（可加前缀型 id，如 `local/*`）
- 右列：已配置的提供商/上游实例（来自 §3.5 的卡片）
- **连线 = 一条路由**；一根线可携带浮层参数：`priority` / `upstream_model` 别名 / `sticky` / `on_error`
- 左一右多 = **相同模型可切换**（同 model 多候选）
- 右一左多 = **同一提供商服务多个对外 id**
- 拖拽新增节点、点线删除、连线交叉可读（二部图只左右连线，天然无环）
- 与配置文件**实时双向**：拖线 → toml 更新；改 toml → 图上连线变

**数据模型（文件与 UI 共用）**：

```toml
# —— 右列：提供商（CC Switch 式卡片的数据源）——
[providers.vercel]
kind = "vercel-gateway"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key_ref = "keyring:jev-switch/vercel"   # 不落明文
enabled = true

# —— 左右之间的连线：二部图的边 ——
[[routes]]
left  = "jev"                 # 左节点：对外 model id（支持 prefix:）
match = "exact"               # exact | prefix
right = "vercel"              # 右节点：provider id
upstream_model = "typesafe-ai/jev"   # 线上的别名标签
priority = 10
sticky = "session"
on_error = "next"

[[routes]]
left  = "jev"
match = "exact"
right = "laya"
upstream_model = "laya-english"
priority = 30
```

**渲染建议**：第一版按二部图连线题做 —— SVG/Canvas 连线层 + DOM 节点（节点可交互，连线用贝塞尔）；**不要**一上来引入完整图编辑器框架。设计与交互交付走 `/frontend-design`。若实践证明二部图不够用，再评估通用 DAG 视图；**路由数据模型（`[[routes]]`）保持稳定，只换视图**。

**验收**：
- [ ] UI 拖一条 `jev ↔ laya` 线后，toml 出现对应 `[[routes]]` 项，且 `POST /v1/systemone {model:"jev"}` 能落到 Laya
- [ ] 删线后路由立即不可用
- [ ] 同一 left 连两个 right，停掉 priority 高的那个，请求落到另一个（与 §P1-1 failover 打通）
- [ ] 配置文件手改后 UI 图无需重启即刷新

---

### 3.7 P0 设计原则 — serde 式「零内核修改」扩展新提供商

**用户指令（必须执行）**：Rust 侧学 serde 的另一点——**适配新提供商时，开发者自行编写数据结构，像实现 `Serialize`/`Deserialize` 那样挂上，即可在内核中使用；内核代码乃至函数传参都不需要改。**

这比 §3.3 的「分层」更狠：不只是拆文件，而是**开放闭合**。对照 serde：

| serde | Jev-Switch 应有 |
|---|---|
| `#[derive(Serialize, Deserialize)]` 挂到用户类型 | `#[derive(JevUpstream)]` 或 `impl Upstream + impl ProtocolAdapter` |
| 用户类型任意，内核用泛型/trait 调用 | 新提供商的 request/response 方言类型任意，内核只见 `SystemOneRequest/Response` |
| 加新类型 **不改 serde 源码** | 加新提供商 **不改 jev-core / jev-protocol 一行** |
| 调用点 `serde_json::to_string(&x)` 不关心 x 是什么 | 调用点 `router.invoke(&req)` 不关心对面是 Vercel 还是 Laya |
| 传参不变 | `fn invoke(req: &SystemOneRequest) -> Result<SystemOneResponse>` 永不变签名 |

**落地机制（三选一或组合，推荐 A+D）**：

**A. trait + 注册表（运行时插件）—— 与 serde 的「实现 trait」同构**

```rust
// jev-core：内核只认这两个 trait，签名冻结
pub trait UpstreamAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn evaluate(&self, req: SystemOneRequest) -> Result<SystemOneResponse, JevError>;
}

pub trait ProtocolAdapter: Send + Sync {
    /// 出站方言化（如 noul → boolean）。默认恒等。
    fn outgoing(&self, req: SystemOneRequest) -> SystemOneRequest { req }
    /// 入站归一化。内核只调这个，不感知厂商字段。
    fn incoming(&self, raw: &[u8], ctx: &IncomingCtx) -> Result<SystemOneResponse, JevError>;
}

// jev-adapters 或用户自己的 crate：
pub struct NotionJevAdapter { /* 用户自定义任意字段结构 */ }
impl ProtocolAdapter for NotionJevAdapter { /* … */ }
impl UpstreamAdapter for NotionJevAdapter { /* … */ }

// 注册：内核 API 永远是这个，传参不变
registry.register(Box::new(NotionJevAdapter::from_config(cfg)?));
```

**B. serde 自定义类型直通**：厂商 DTO 全部由适配器自己 `#[derive(Deserialize)]`，**永不**进入 `jev-protocol`。内核边界只做一次 `incoming -> SystemOneResponse`。这样新厂商的脏字段、异体命名（如 Vercel 的 `probability` vs `boolean`）**不会污染**内核类型。

**C. feature flag / 动态库（后置）**：`jev-adapters/vercel` 可 cargo feature 裁剪；更远期 cdylib 插件。M1 不做动态库。

**D. 宏/派生（可选甜点）**：`#[derive(JevProvider)]` 自动生成注册入口。M2 再说，先保证 trait 边界干净。

**验收（serde 同构性的黄金测试）**：
- [ ] 在 **jev-switch 仓库之外** 写一个 `my_lab_adapter` crate，只依赖 `jev-core` + `jev-protocol`
- [ ] 自定义一套怪异字段（例如 `{"verdict":"yes","odds":0.8}`）
- [ ] **不修改 jev-core / jev-protocol / daemon 任何一行**
- [ ] `registry.register(...)` 后 `POST /v1/systemone` 换 model 即可命中，响应已是标准 `SystemOneRequest/Response` 形状
- [ ] 全程 `invoke` 函数签名零改动

**红线**：若某次「加提供商」需要改 `jev-core` 的 match、改 `Router::route` 参数、或往 `SystemOneResponse` 加厂商专有字段 —— **视为设计失败，打回**。厂商差异只允许活在 adapter 的 DTO 里。

---

### 3.8 前端/交互设计的强制流程

**用户指令（必须执行）**：后续前端与交互设计**统一走 `/frontend-design` skill**，产出「综合的前端与交互设计」，**视觉与交互向 CC Switch 靠**。

| 域 | 设计目标 | 交付物 |
|---|---|---|
| 提供商管理页 | CC Switch 式卡片 + 开关 + 密钥表单 | `/frontend-design` 设计稿 + 组件清单 |
| 路由接线页 | **模型路由 DAG**；默认编辑器 = 二部图（最简形式，§3.6） | 交互原型（拖线/删线/线上浮层参数；数据层按 DAG） |
| 调试台 | 保留现有 jevplayground 风双栏（Input/Output + Run Jev） | 微调即可，不推翻 |
| 信息架构 | 三页：Providers / Routing / Playground | 导航与空态/错误态/加载态全量定义 |

**要求给 frontend Agent 的话术**：调用 `/frontend-design`，输入是本评审 §3.5–§3.6 + CC Switch 的卡片/开关/密钥交互范式，输出是可直接施工的设计说明；**不要**让 Agent 凭空发明一套与 CC Switch 无关的视觉语言。


## 4. 协议保真度评审（vs jev-life `types.ts`）

基准：`/jev-life/src/shared/types.ts`（生产验证过的事实标准）。

### 4.1 逐字段对照（浓缩；完整版见上一轮评估）

| 字段 | Jev-Switch 现状 | jev-life TS | 一致? | 处置 |
|---|---|---|---|---|
| Request 名 | `SystemOneRequest` | `JevRequest` | 名不同 | 可保留 `SystemOne*` 别名，但 lib 导出两个名字 |
| `state` | `Value` + default | `unknown` 必填 | 否 | 必填 |
| `Question.type` | 4 值 tagged enum | 同 4 值 | **是** | 保留 |
| `Question.instructions` | `String` | `string` | **是** | 保留 |
| `Question.criteria` | `Option<Value>` | 必填 `Criteria` 三形态 | **否** | 改必填 + 强类型 |
| `Answer` 建模 | 扁平全 Option | 判别联合 3 分支 | **否** | 改联合 |
| `BooleanAnswer.probability` | **归一化后无此字段** | `probability?: number` | **否** | **P1 必须补** |
| `BooleanAnswer.noul` | `Option<f64>` | `noul?: number` | 是 | 保留 |
| `BooleanAnswer.confidence` | 有（Option） | **明确无** | **否** | 删掉布尔分支的 confidence |
| `ScoreAnswer.score` | `Value` | `number` | **否** | `f64` |
| `Choice/Score.probabilities/confidence` | Option | 必填 | **否** | 必填 |
| `usage` | `Value` | 双拼写 + reasoning_tokens | **否** | 强类型 `Usage` |
| `upstreamCalls` | **缺失** | `upstreamCalls?: number` | **否** | 补（工具循环计量依赖它） |
| `providerMetadata` | 仅 VercelResponse | `JevResponse` 一等 | **否** | `extra` flatten 或正式字段 |
| `latencyMs` / `costUsd` | **缺失** | jev-life 一等输出 | **否** | 补（`costUsd = null` 不是 0） |
| `noulProbability()` | 缺失 | 有 | **否** | 补到 jev-protocol |

### 4.2 与 docs/03 的自我不一致

`docs/03-§2.1` 写的标准协议里 **request 的 `type` 只有 `choice|score|noul`**，**没有 `boolean`**；而 `protocol.rs` 的 `DecisionQuestion` 把 `Boolean` 做成一等变体。  
这在「网关内部表示」上说得通，但在「对外 lib 形态」上会造成：调用方不知道该发 `noul` 还是 `boolean`。

**裁决**：对外 `Question` 只暴露 `choice|score|noul`（语义层）；`boolean` 是**方言**，只在 `ProtocolAdapter::outgoing` 之后出现。这与 jev-life `normalizeQuestionTypes` 的设计一致——「客户端只发语义，不猜上游怎么拼」。

### 4.3 UI↔Rust 契约漂移（刚被 `37b4242` 止血的那件事）

| 字段 | `ui/src/api.ts` | `rs/src/protocol.rs` / `main.rs` | 风险 |
|---|---|---|---|
| `/v1/models` | `ModelsResponse { object, data: ModelInfo[] }` | `{ models: [{model, upstream}], upstreams: [...] }` | **已崩过一次** |
| `SystemOneResponse.model` | **必填** `string` | `Option<String>` | 反序列化宽松度不一致 |
| `SystemOneResponse` 额外字段 | `upstream?`, `latency_ms?` | **无** | UI 假装有测量字段，后端不报 |
| `SystemOneAnswer` | 无 `type` 字段 | 有 `type: Option<String>` | UI 靠猜字段有无来分型 |
| `criteria` | 分变体强类型 | `Option<Value>` | UI 比后端还严 |

**根治方案**：`jev-protocol` 导出 JSON Schema / 或 `typescript` 绑定（`ts-rs` / `typeshare`），UI 类型从 Rust 生成，禁止手写第二份。短期至少：把 `ModelsResponse` 在 Rust 侧定死，UI 不要 `normalize` 两种形状——**归一化应该在协议层做一次，不是在每个客户端做一次**。

---

## 5. 正确性缺陷（按严重度）

### 5.1 P0 — `probability` 翻译 bug（数据可信度）

**位置**：`rs/src/translate.rs`

**文件头声称**（L5, L41-42, L57-58）：
- 「Vercel 实际还返回 `probability`，**用它**」
- 「优先级：probability > boolean（probability 含更多信息）」

**实现现实**：
- `build_jev_response_from_vercel` 的 boolean 路径（L107-110）：
  ```rust
  let prob = if va.boolean.unwrap_or(false) { 0.95 } else { 0.05 };
  // 注释甚至写「上游确实只返回 boolean（不返回 probability）」← 与自己文件头矛盾
  ```
  **机器验证**：全文 `va.probability` 出现 **0** 次。
- `denormalize_response_from_vercel`（L59）：
  ```rust
  let probability: f64 = if let Some(p) = ans.boolean {  // p: Option<bool> 解成 bool
  ```
  变量名叫 `probability`，解的是 `boolean: Option<bool>`。后面的 `.and_then(|p| p.get("true"))` 指望 `probabilities["true"]`，但 boolean 路径**从未把 Vercel 的 `probability` 写进 `probabilities`**。
- 单测 `vercel_probability_field_used_directly`（L294-313）测的是「手动塞进 `probabilities["true"]=0.73`」，**绕过了真实 Vercel 形态**（真实是 `{"type":"boolean","probability":0.69}`，见 jev-life 注释）。**测试给了虚假安全感**。

**影响**：所有经 Vercel 的布尔决策，概率被硬编码成 0.95/0.05。对「校准概率」类实验（如 jev-rime 的短语决策）是**致命的**——你测到的不是 Jev 的概率，是翻译层的常数。

**修复要求**：
1. `BooleanAnswer` / `DecisionAnswer` 保留 `probability: Option<f64>` 键
2. 优先级：`probability` → `probabilities["true"]` → `noul` → 由 `boolean` 推 0.95/0.05（**仅作最后回退**）
3. 补一个**真实形态**的单测：
   ```json
   {"answers":{"q":{"type":"boolean","probability":0.69}}}
   ```
   期望 `noul ≈ 0.69`，**不是** 0.95。

### 5.2 P1 — 无重试/退避，却把 429 标成 retryable

`JevError::retryable` + `capabilities.retryable_status` 都在，但 `systemone_handler` **一次失败就返回**。  
429 被映射成对外 503 + `retryable: true`，等于「告诉客户端可以重试，但网关自己不重试」。

**要求**：`jev-core` 提供 `RetryPolicy { max_attempts, backoff, retryable }`，由 Router 在多候选间 failover，或单候选内退避重试。本地 Laya（`retryable_status: &[0]`）明确不重试——这点设计是对的。

### 5.3 P1 — `usage` 不建模 → 预算/成本守卫做不了

`SystemOneResponse.usage: Option<Value>`。jev-life 认真处理了 `inputTokens`/`input_tokens` 双拼写 + `completion_tokens_details.reasoning_tokens`。  
下游 `jev-rime` 有「单次实验 ≤ ¥1」红线——**没有 usage 强类型就算不了钱**。

### 5.4 P2 — 其他

| 项 | 问题 |
|---|---|
| CORS `very_permissive()` | MVP 可接受；M1 必须 origin 白名单（自己已在 commit message 承认） |
| IPv4 | 注释承认 Cloudflare IPv6 问题，未实装 `family=4`（PROGRESS 也记了「未实装」） |
| `capabilities_of` 硬编码 | 每加一个上游要改 match；应改为 adapter 注册制 |
| 无 `check_capability` 的 handler 调用 | `Router::check_capability` 是 `#[allow(dead_code)]`，**路由时不校验 capability** |
| 无根 README / LICENSE 文件 | `Cargo.toml` 写了 `MIT OR Apache-2.0` 但仓库无 LICENSE，GitHub license=null |
| `docs/README.md` 过时 | 仍说「规划与文档仓库」「03-05 待写」 |
| 无 `examples/`、无 `tests/` 目录、无 CI | 不可验证「别人怎么用」 |
| PROGRESS 里的 Windows 路径 | `H:\A137442\...`、`E:\venvs\...` 与本机 Termux 路径不一致；复现步骤要双路径 |

---

## 6. 成熟度评估

| 阶段 | 判定 |
|---|---|
| 概念验证 (PoC) | ✅ 已越过 |
| MVP / demo | ✅ **正在此处**（tag `v0.1.0-mvp` 封存） |
| 可集成的库 | ❌ 未开始（无 lib） |
| 可扩展的网关 | ❌ 未开始（仅 2 上游、1:1 路由） |
| 生产级 | ❌ 远未达到 |

**活跃度**：11 commits / 约 2 天（2026-09-22 ~ 09-23）。爆发式、规划驱动。  
**测试**：约 20 个源内单测，**无**集成测试、无 smoke.sh（06- 计划里有，树里没有）、无 CI。  
**文档/代码比**：docs ~2300 行 vs rs ~1770 行 vs ui ~1400 行。文档先行是特色，但 `docs/README.md` 掉队。

---

## 7. 给后续开发 Agent 的意见清单（按 ROI，带验收标准）

> 下面每条都是**可独立派发**的任务。P0 未完成前，**不要**并行去加新上游。

### P0-1 拆 workspace + 导出 lib（解锁集成性）
- [ ] `crates/jev-protocol`（lib，零 IO 依赖）
- [ ] `crates/jev-core`（lib，路由 + trait + 翻译策略，不依赖 axum）
- [ ] `crates/jev-adapters`（lib，vercel/laya 先行；feature 门控）
- [ ] `crates/jev-switch-daemon`（bin，现有 main.rs 迁入）
- [ ] **验收**：新建临时 crate `cargo add --path .../jev-protocol` 后 `use jev_protocol::SystemOneRequest` 能编译
- [ ] **验收**：`cargo test --workspace` 全绿
- **禁止**：在拆分时顺手改协议字段名以外的行为（先搬家，再改语义）

### P0-2 修 `probability` bug（解锁数据可信度）
- [ ] `Answer` 布尔分支保留 `noul` + `probability` 双键
- [ ] 优先级 `probability > probabilities["true"] > noul > boolean→0.95/0.05`
- [ ] **验收**：单测喂 `{"type":"boolean","probability":0.69}` 得到 `noul≈0.69`
- [ ] **验收**：与 jev-life `noulProbability()` 对同一夹具结果一致

### P0-3 协议层对齐 jev-life（解锁跨项目复用）
- [ ] `criteria` 必填 + `Criteria` 三形态强类型
- [ ] `Answer` 判别联合；布尔无 `confidence`
- [ ] `Score.score: f64`；choice/score 的 `probabilities`/`confidence` 必填
- [ ] `Usage` 强类型（双拼写 + reasoning_tokens）
- [ ] 补 `upstream_calls` / `latency_ms` / `cost_usd: Option<f64>`（null ≠ 0）
- [ ] 导出 `noul_probability()`
- [ ] **验收**：JSON Schema 或 ts-rs 生成物与 `jev-life/src/shared/types.ts` 可 diff；差异仅限「Rust 命名风格」

### P1-1 路由升级为「模型自动路由 + 同模型可切换」（学 sub2api）
- [ ] `RouteTable` 支持 `exact|prefix` 匹配 + 多 `candidates`（upstream + upstream_model + priority）
- [ ] `sticky: none|session`，`on_error: next|fail`
- [ ] `Router::select(req, ctx) -> Vec<Candidate>`（返回有序候选，不是单 Upstream）
- [ ] handler 侧按候选 try-next；记录 `upstream_calls`
- [ ] **验收**：同一 `model: "jev"` 配 Vercel+Laya 两候选；停掉 Vercel 后自动落到 Laya
- [ ] **验收**：`model: "local/*"` 前缀能命中本地池
- [ ] 参考实现：`sub2api/backend/ent/schema/composite_model_route.go`、`backend/internal/service/gateway_scheduling.go::SelectAccountForModelWithExclusions`

### P1-2 翻译层模块化（`ProtocolAdapter`）
- [ ] `translate.rs` 拆为 trait + 每上游一个 impl；文件名不再写死厂商
- [ ] **验收**：新增一个 mock 上游（方言：`type: "bool"`）只需新增一个 adapter 文件 + 注册，不改 core

### P1-3 重试 / 退避
- [ ] `RetryPolicy` + 指数退避；429/5xx 走 policy；Laya 不重试
- [ ] **验收**：wiremock 返回一次 429 一次 200，客户端仍拿到 200，且 `upstream_calls == 2`

### P1-4 UI 提供商配置 + DAG 路由接线（学 CC Switch + `/frontend-design`）
- [ ] Providers 页：卡片 / 启停 / 密钥 / Probe / 增删（§3.5）
- [ ] Routing 页：**模型路由 DAG** 数据层 + 二部图（最简形式）连线编辑器，左 model id、右 provider，边带 priority/alias/sticky（§3.6）
- [ ] 文件与 UI 双向同步；密钥不进 git
- [ ] **前端设计强制走 `/frontend-design`**，向 CC Switch 靠（§3.8）
- [ ] **验收**：拖线 → toml 更新 → `POST /v1/systemone` 路由正确；删线立即失效
- [ ] **实践检验**：二部图编辑器跑通真实路由配置 2 周或 3 个真实场景后，书面记录「保留二部图最简视图 / 升级通用 DAG 视图」的结论（**模型路由 DAG 不变**）

### P1-5 serde 式零内核修改扩展点（开放闭合）
- [ ] 冻结 `UpstreamAdapter` / `ProtocolAdapter` trait 签名
- [ ] 厂商 DTO 全部留在 adapter 内，不进 `jev-protocol`
- [ ] **验收（黄金测试）**：外部 crate 自定义怪异字段适配器，**零改内核**注册后可用（§3.7）
- [ ] **红线**：加提供商若需改 core 的 match / 函数传参 / 往 Response 塞厂商字段 → 设计失败打回

### P1-6 密钥「防偷」强化（学 CC Switch / sub2api / newAPI）

Q4 已定「本地明文 toml + UI 密文」为默认档，但用户明确要求**防偷**必须认真做。借鉴三家「也要安全存 API token」的实践：

| 来源 | 可借鉴点（待 Phase 0 调研定稿） | 对 Jev-Switch 的含义 |
|---|---|---|
| **CC Switch**（Tauri） | 桌面端密钥存储 / 系统 keyring / 本地配置文件权限 | 本地 daemon 的文件权限 0600、目录 0700；Tauri 阶段再评估 OS keyring |
| **sub2api** | 服务端集中存 token、下发派生 key、代理转发（上游 token 永不出边界） | **上游 API key 永不到浏览器**；UI 只拿派生/掩码；调试请求经 daemon 转发 |
| **newAPI** | 中转网关的渠道密钥管理、脱敏回显、权限分层 | 密文回显格式、禁止导出明文的 API、admin 操作审计日志 |

**契约级要求（不因 Q4=b 而降低）**：
- [ ] 上游密钥**只存在于 daemon 侧**；浏览器/日志/tracing/错误体/context 一律密文
- [ ] `GET` 配置类 API 只回 `api_key_masked`（如 `sk-…a1b2`），永不回明文
- [ ] `PUT` 允许写入新密钥；**无「读回明文」API**
- [ ] 文件权限：`providers.toml` 0600、`~/.jev-switch/` 0700；`.gitignore` 覆盖 `providers.toml` 与 `*.enc`
- [ ] 错误信息脱敏：上游 4xx body 可能回显 key —— 日志与 ErrorBody 统一 redact
- [ ] 可选加固（M2+）：`secrets.enc` / OS keyring 升级档，配置切换；默认档仍是明文 toml（信任开发者）
- [ ] **Phase 0 调研**：对照 CC Switch / sub2api / newAPI 源码或文档各写 ≥1 条可落地做法，写入 `docs/contracts/04-密钥与防偷.md`

**红线**：任何 UI/导出/截图友好特性若会暴露明文 token —— 拒绝实现。

### P2 杂项
- [ ] 根 `README.md`（中英可选）+ `LICENSE` 文件（与 Cargo.toml 一致）
- [ ] 修正 `docs/README.md` 过时表述；文档索引纳入 `PROGRESS.md` 与本文
- [ ] `examples/basic.rs` + `tests/integration_*.rs` + `tests/smoke.sh`（06- 计划里那份）
- [ ] CORS origin 白名单；IPv4 `family=4` 可选开关
- [ ] capability 由 adapter 注册，删掉 `capabilities_of` 的 match
- [ ] handler 真正调用 `check_capability`（或删掉这个死代码）
- [ ] 可观测性（M4）与 Tauri（M7）**继续后置**，不阻塞上述

---

## 8. 战略定位判断（Jev 生态里站哪）

| 项目 | 角色 | Jev-Switch 相对位置 |
|---|---|---|
| TypeSafe 官方 API | 协议真值源 | **对齐基准** |
| `jev-life` | TS 事实标准（协议形状 + broker） | **协议应向它对齐**；broker 可后置借鉴 |
| `jev-decision-lab` | 实测数据种子 | 上游依赖（benchmark 数据） |
| `jev-rime` | 实验消费者 | **应能 path-dep `jev-protocol`**；现在不能 |
| `sys1` / `LLM2Jev` | C4/C5 单上游服务 | 未来 adapter 目标 |
| `cc-switch` | 桌面切换器形态 | Tauri（M7）参考 |
| **sub2api** | 多供应商订阅配额网关 | **路由/调度模型参考**（CompositeModelRoute + sticky + failover） |
| **Jev-Switch** | **Jev 原生入口 × 多上游路由 × 桥接** | 应成为「Jev 界的 sub2api / LiteLLM」，但**先能被 Rust 集成** |

**不可替代价值**（维持 `docs/01/02` 的判断）：入口是 Jev 原生、出口是多上游、并对 Vercel 类方言做翻译。LiteLLM/OpenRouter 入口是 OpenAI 形状，TypeSafe 官方不会提供「切到 Laya」。  
**但价值兑现的前提**是 §3 的三层分离 + lib 导出，否则只是「一个能跑的 demo daemon」。

---

## 9. 与 jev-rime 的关系（附带结论）

- jev-rime **不应**把 Jev-Switch 当 crate 依赖（现在也做不到）
- jev-rime **应**在 P0-1/P0-3 完成后 path-dep `jev-protocol`，避免第三份协议声明
- 在 P0-2 完成前，任何经 Jev-Switch→Vercel 的**校准概率实验数据都应作废**（概率被硬编码 0.95/0.05）
- 直连 Vercel 的实验可以继续；**不要**经当前 MVP 网关取概率

---

## 10. 评审人声明

- 本文所有代码引用均可在 `@821e304` 复核；行号以该 commit 为准。
- 对 `translate.rs` 的「probability 未使用」结论为**机器验证**（全文搜索 `va.probability` = 0 次），不是印象。
- 对 sub2api 的引用来自 `Wei-Shaw/sub2api` 公开源码（`composite_model_route.go` / `gateway_scheduling.go` / `README_CN.md`），未做运行时验证。
- 评分是相对「**可集成的 Jev 协议/路由基础设施**」这一目标；若目标降为「个人 demo」，§0 表中「可集成性/模块化」两行可改为 N/A，其余结论不变。

**Mimo-V2.6-Pro**  
2026-09-23

---

## 附录 A：评审基线清单

| 基线 | 用途 |
|---|---|
| `jev-life/src/shared/types.ts` | Jev 协议事实标准 |
| `jev-life` DESIGN/CLAUDE 中的 capability / effort / costUsd 语义 | 扩展字段语义 |
| `docs/04-架构设计-from-sys1-借鉴.md` §2.1 | 本应存在的 workspace 三 crate |
| `docs/03-上游类别与协议兼容矩阵.md` | C1–C5 与字段矩阵 |
| `Wei-Shaw/sub2api` CompositeModelRoute + gateway_scheduling | 路由/调度模型 |
| TypeSafe `/v1/systemone` 形状（docs/03 §2.1） | 内核协议真值 |

## 附录 B：关键文件索引（@821e304）

| 路径 | 行数 | 角色 |
|---|---|---|
| `rs/src/protocol.rs` | 215 | 协议类型（待替换为 jev-protocol） |
| `rs/src/translate.rs` | 347 | Vercel 翻译（**含 P0 bug**） |
| `rs/src/upstream.rs` | 231 | Upstream trait + capability |
| `rs/src/upstream_vercel.rs` | 190 | Vercel 适配 |
| `rs/src/upstream_laya.rs` | 130 | Laya 适配 |
| `rs/src/router.rs` | 209 | 1:1 静态路由 |
| `rs/src/config.rs` | 172 | providers.toml |
| `rs/src/main.rs` | 262 | axum daemon（应瘦身为 bin） |
| `ui/src/api.ts` | 204 | UI 客户端（含 normalizeModels 止血） |
| `docs/PROGRESS.md` | 181 | MVP 快照 |
