# 从 jev-life 超越的设计点

> **目的**：jev-life（`ARCJ137442/jev-life`，已 1.0.0 上线）是一个**成熟且公开**的
> TypeScript 实现 —— 它的 broker、capability 矩阵、决策结果结构已被生产环境验证。
> 本文**承认先**（第一节列出 jev-life 做得好的地方），再**指明** Jev-Switch 必须
> 在哪些维度上青出于蓝（第二节 5 个具体能力 + Rust 代码片段）。
>
> **不重复**：上游 5 类定义与 capability 矩阵详见 `03-`；整体规划见 `02-`。
> 本文专注 **「为什么 jev-life 没做」+「Jev-Switch 怎么做」+「还有什么 open question」**。

---

## 一、jev-life 设计的优点（承认先）

读 `C:\Users\56506\AppData\Local\Temp\jev-life\src\shared\backend.ts`、
`llm-broker.ts`、`types.ts`，以及 `CLAUDE.md` 与 `DESIGN.md` 引用的注释密度，
可以提炼出 8 条**值得直接借鉴**的设计：

1. **5 类 DecisionBackend 抽象** —— `systemone / llm-json / llm-tool` 三种 `kind`，
   上层 `channels / core / decide` 一行不改。Jev-Switch 的 `Upstream trait` 直接照搬。

2. **两套协议族并列** —— `openai-compat / anthropic-compat`，靠 `LlmProtocol` 判别路径、
   认证头、请求体形状（`input_tokens` vs `prompt_tokens`）。

3. **三态 effort + capability 收敛** —— `LlmReasoningEffort | null | undefined`：
   省略 = `none`、显式 `null` = 不发字段、具体档位 → `capabilitiesOf()` 收敛。
   实测依据：「留空」是与 `none` 同档（3/3）的明确选择。

4. **broker 纯函数化** —— `toLlmRequest / fromLlmContent / extractContent /
   extractToolCalls / answersFromToolArguments / usageOf` 全不碰网络、不读 env，
   服务端代理与浏览器内调用**复用同一份纯函数**。

5. **决策结果三项一等输出** —— `latencyMs / upstreamCalls / costUsd` 本身就是测量结果，
   第一天就在返回值里。`costUsd = null`（不是 0）尤其值得抄。

6. **重试 + 超时手写** —— `startTimeout()` 用 `clearTimeout` 在 `finally` 里收尾，
   避免每次请求留一个闭包住的 `AbortController` 在堆上。

7. **CORS 文案分开提示** —— 「超时」与「连不上」分开报（前者是上游慢、后者是根本到不了）；
   CORS 单独给一段文案提示，因为浏览器刻意不告诉脚本。

8. **协议知识与循环逻辑分离** —— `ToolWire` 接口让循环主体（轮次上限、零进展检测、
   pending 记账、token 累加）只写一遍；协议差异（`openAiWire` / `anthropicWire`）只
   在「怎么拼历史」这一个方法上分岔。

> **结论**：上述 8 条里有 4 条**直接复制**（详见第四节），是 Jev-Switch 的"输入"。
> 但 jev-life 的设计目标不是"Jev 协议多上游网关"——下一节是它没做的事。

---

## 二、Jev-Switch 超越的 5 个能力

### 能力 1：多上游运行时切换（jev-life 是单 backend）

**jev-life 现状**：一个 `DecisionBackend` 绑一个 `ClientConfig`（`base + model + apiKey`）。`BACKENDS` 表存所有后端，但运行时只有一个被 `createSystemoneBackend()` 包出来。**没有路由层**——切换是用户**手动**在界面改下拉框。

**Jev-Switch 升级**：在 broker 之上加一层 **Router**，把单 backend 改造成**多 upstream + 策略 + 熔断**：

```rust
// rs/src/upstream/mod.rs
#[async_trait]
pub trait Upstream: Send + Sync {
    async fn evaluate(&self, req: &DecisionRequest) -> Result<DecisionResult, JevError>;
    fn id(&self) -> &UpstreamId;
    fn kind(&self) -> UpstreamKind;                 // "systemone" | "llm-json" | "llm-tool" | "prefill-only"
    fn capabilities(&self) -> UpstreamCapabilities; // 见 03- 第二节
    async fn health(&self) -> HealthStatus;         // 实时：rate-limit / last-error / circuit
}

// 5 个具体实现 —— 每条只填 jev-life DecisionBackend 的"配置 + 协议"两个字段
pub struct SystemoneUpstream { /* C1/C2/C4/C5：原样代理 Jev-shaped HTTP */ }
pub struct LlmJsonUpstream   { /* C3 JSON 路径：调 broker.to_llm_request + from_llm_content */ }
pub struct LlmToolUpstream   { /* C3 tool 路径：调 broker.build_tool_request + 工具循环 */ }
pub struct PrefillOnlyUpstream { /* C4 prefill-only：用 logits 概率当作 confidence */ }
pub struct BrokerEmbedded    { /* broker 内嵌：复用 jev-life 的纯函数 */ }
```

```rust
// rs/src/router/mod.rs
pub struct RouterConfig {
    pub strategy: RouterStrategyKind,
    pub fallbacks: Vec<UpstreamId>,                 // 失败时按顺序试
    pub circuit_breaker: CircuitBreakerConfig,
}

#[async_trait]
pub trait RouterStrategy: Send + Sync {
    async fn select(
        &self,
        req: &DecisionRequest,
        health: &HealthRegistry,
    ) -> Option<UpstreamId>;
}

/// FailoverChain：按 priority 顺序，第一个 capability 匹配 + 健康 = 用
pub struct FailoverChain { fallbacks: Vec<UpstreamId> }

#[async_trait]
impl RouterStrategy for FailoverChain {
    async fn select(&self, req: &DecisionRequest, health: &HealthRegistry) -> Option<UpstreamId> {
        for id in &self.fallbacks {
            let up = health.get(id);
            if !up.is_healthy() { continue; }
            if !up.capabilities().supports_question_types(req.question_types()) { continue; }
            return Some(id.clone());
        }
        None
    }
}
```

```rust
// rs/src/router/circuit_breaker.rs —— 连续 N 次失败 → Open N 秒 → cooldown 后回 Closed
pub struct CircuitBreaker {
    state: Arc<RwLock<CbState>>,  // { consecutive_failures, opened_at: Option<Instant> }
    cfg: CircuitBreakerConfig,    // { max_failures, cooldown }
}
// is_open() / record_success() / record_failure() 三个方法
// （cooldown 后第一次失败立即重开，无需"半开"状态）
```

**4 种路由策略**：

| 策略 | 选谁 | 适用场景 |
|---|---|---|
| `FailoverChain`（默认） | priority 顺序，第一个健康且 capability 匹配 | 通用 |
| `RoundRobin` | 轮流 | 均匀分摊配额 |
| `WeightedRoundRobin` | 按 weight 比例轮 | 主备 + 长尾 |
| `LatencyBased` | 滑动 P50 最低 | 延迟敏感任务 |

**为什么 jev-life 没做**：单进程单 backend 是它的设计目标（每个浏览器标签一次决策），
加 Router 反而拖累启动；且路由带来的"上游漂移"会污染它的「按配置分组统计」原则。

---

### 能力 2：完整 OpenTelemetry 可观测性（jev-life 仅三项输出）

**jev-life 现状**

返回 `latencyMs / upstreamCalls / costUsd` 三项，**足够给单点测量**，但**不足以做
多上游对照**：没法回答"这次决策走了哪个分支"、"哪个上游超时了"、"失败前的
状态码序列是什么"。

**Jev-Switch 升级**：内置 OpenTelemetry trace + Prometheus metric。

```rust
// rs/src/observability/trace.rs —— decision.root → router.select → upstream.evaluate
// → fallback.triggered 的层级化 span，附带 routing.selected_upstream /
// routing.health_snapshot / response.status / response.usage 事件
pub struct RouterSpan {
    pub trace_id: TraceId,
    pub span_id: SpanId,
}

impl RouterSpan {
    pub fn decision_root(req: &DecisionRequest) -> Self { /* ... */ }
    pub fn record_route(&self, primary: &UpstreamId, fallbacks: &[UpstreamId]) { /* event */ }
    pub fn record_upstream_attempt(&self, up: &UpstreamId) -> UpstreamSpan { /* child span */ }
    pub fn record_fallback_triggered(&self, from: &UpstreamId, to: &UpstreamId, reason: &JevError) { /* event */ }
}
```

**trace 文件落地**（参考 jev-decision-lab `life-series-benchmark/articles.json` gitignore 的经验）：

```
~/.jev-switch/
├── config.toml
├── traces/{date}/{trace_id}.json     # 可重放
└── metrics/prometheus-snapshot.prom
```

```rust
// rs/src/observability/redact.rs —— 默认 redact title/excerpt，保留 question.type/instructions
pub struct Redactor { pub redact_title: bool, pub redact_state: bool }

impl Redactor {
    pub fn redact(&self, body: &serde_json::Value) -> serde_json::Value {
        // benchmark 数据可能含敏感标题 → 默认 redact；
        // 保留 question.type/instructions 让 trace 可读懂
        if self.redact_title { /* 用 "<title: redacted, len=N>" 占位 */ }
        body
    }
}
```

**Prometheus 指标**（`/metrics` 端点）：

```
# HELP jev_decision_total Total decisions routed
# TYPE jev_decision_total counter
jev_decision_total{outcome="success",upstream="typesafe"} 142
jev_decision_total{outcome="fallback",upstream="vercel"} 3

# HELP jev_decision_latency_seconds Decision wall-clock latency
# TYPE jev_decision_latency_seconds histogram
```

**为什么 jev-life 没做**：jev-life 是浏览器内单点决策，加 OTel 会引 100KB SDK；它把"测量"放在 `costUsd` 这一栏做，够用。

---

### 能力 3：config.toml + CLI 路由（jev-life 是硬编码 BACKENDS 表）

**jev-life 现状**

`BACKENDS` 表是**编译期常量**（`src/server/server.ts` 与 `api/_upstream.ts` 里硬编码
的 `if (id === "vercel")` 等）。新增后端要改源码、重部署。

**Jev-Switch 升级**：完整 TOML schema + CLI 子命令。

```toml
# ~/.jev-switch/config.toml — 默认 Providers
[providers.typesafe]
kind = "systemone"                           # 见 03- 第二节 "5 个具体实现"
base = "https://api.typesafe.ai/v1/systemone"
api_key_env = "TYPESAFE_API_KEY"
priority = 1
timeout_ms = 60_000

[providers.openrouter]
kind = "systemone"
base = "https://openrouter.ai/api/alpha/decisions"
api_key_env = "OPENROUTER_API_KEY"
priority = 2
# jev-decision-lab 实测：必须 IPv4（Cloudflare IPv6 不可达）

[providers.vercel]
kind = "systemone"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key_env = "VERCEL_AI_GATEWAY_API_KEY"
priority = 3
noul_via_boolean = true                      # 翻译层：noul → boolean

[providers.laya]
kind = "systemone"
base = "http://127.0.0.1:18765/v1/systemone"
priority = 4

[providers.lm_studio_4b]
kind = "llm-json"                            # C3 broker JSON 路径
protocol = "openai"
base = "http://127.0.0.1:1234/v1/chat/completions"
model = "qwen/qwen3-4b-2507"
api_key_env = "LM_STUDIO_API_KEY"
priority = 5

[routing]
strategy = "failover-chain"
fallbacks = ["typesafe", "openrouter", "vercel", "laya", "lm_studio_4b"]
circuit_breaker = { max_failures = 3, cooldown_s = 60 }

[observability]
trace_dir = "~/.jev-switch/traces"
redact_title = true
redact_state = false                          # 默认 redact title，state 可选
metrics_port = 9090
```

```rust
// rs/src/config/mod.rs
#[derive(Deserialize)]
pub struct Config {
    pub providers: BTreeMap<String, ProviderConfig>,
    pub routing: RoutingConfig,
    pub observability: ObservabilityConfig,
}

#[derive(Deserialize)]
pub struct ProviderConfig {
    pub kind: UpstreamKind,
    pub base: String,
    pub model: Option<String>,
    pub api_key_env: Option<String>,       // 从环境读，**绝不落盘**
    pub priority: u32,
    pub timeout_ms: Option<u64>,
    pub protocol: Option<LlmProtocol>,     // 仅 llm-json / llm-tool
    pub noul_via_boolean: Option<bool>,    // 仅 systemone（Vercel 适配）
}

// CLI（clap derive）—— up / status / trace / bench / config validate / probe
// 完整命令见第二节 5 的入口代码
```

**与 jev-decision-lab `ts/decision-bricks.ts` 的 4 Runtime 关系**：4 Runtime（`local-qwen / openrouter / vercel / laya`）是 TS 原型；Jev-Switch 的 5 个 Rust `Upstream` 实现是把它们一比一翻译 + 加 capability 收敛 + 加熔断 + 加 trace。**TS 评测数据继续作为 Rust 实现的回归基线**。

**为什么 jev-life 没做**：Vercel serverless 部署下，启动时读 `process.env` 就够了；硬编码表的好处是**编译期就能查到所有配置错误**。但本地 daemon 形态下，**用户改配置不应该要求重编译**。

---

### 能力 4：真 Jev 协议 provider + 翻译层（jev-life 只 proxy 到 Vercel）

**jev-life 现状**

`BACKENDS` 表里**所有非 systemone 后端都是 broker 包装的 LLM**——它的核心定位是
"LLM 转 Jev"，不代理其他 Jev-shaped 上游（OpenRouter / Vercel / LLM2Jev / Laya）。

```ts
// jev-life/shared/types.ts:99
export function normalizeQuestionTypes<T>(body: T, upstream: string): T {
  const target = noulDiscriminator(upstream);  // Vercel → "boolean"
  // ... 只动布尔族 questions[*].type
}
```

`noulDiscriminator()` + `normalizeQuestionTypes()` 是 jev-life **唯一的翻译层**——
它只把 `noul ↔ boolean` 互相转换，**不**回答 Vercel 缺 `confidence` 怎么办、
不回答 `probabilities` 怎么从 boolean 派生。

**Jev-Switch 升级**：完整翻译层 + 真 Jev 协议原生支持。

```rust
// rs/src/compat/noul.rs —— noul ↔ boolean 翻译
pub fn jev_noul_to_vercel_boolean(q: &JevNoulQuestion) -> VercelBooleanQuestion {
    VercelBooleanQuestion {
        r#type: "boolean",
        instructions: q.instructions.clone(),
        criteria: q.criteria.clone(),   // {true, false} 字段
    }
}

pub fn vercel_boolean_to_noul(vercel_ans: &VercelBooleanAnswer) -> JevNoulAnswer {
    // Vercel 丢掉原始概率（只保留 true/false）—— 补救：true → 0.95, false → 0.05
    // 这是一个工程妥协（不是真概率），但 03- 实测 71.4% partial 准确率证明可接受
    let p = if vercel_ans.boolean { 0.95 } else { 0.05 };
    JevNoulAnswer {
        noul: p,
        probabilities: { "true": p, "false": 1.0 - p },
    }
}
```

```rust
// rs/src/compat/mod.rs —— Vercel 完整翻译（confidence 从 probabilities 推断）
pub fn vercel_to_jev(
    vercel: &VercelResponse,
    req: &DecisionRequest,
) -> JevResponse {
    let answers = vercel.answers.iter().map(|(k, v)| {
        let jev_answer = match v {
            VercelAnswer::Choice { choice, probabilities } => JevAnswer::Choice {
                choice: choice.clone(),
                probabilities: probabilities.clone(),
                // ★ **关键翻译**：Vercel 不给 confidence，从 probabilities 推
                // 实测依据：本仓库 life-series-benchmark 80 篇
                confidence: probabilities.values().cloned().fold(0.0_f64, f64::max),
            },
            VercelAnswer::Score { score, probabilities } => JevAnswer::Score {
                score: *score,
                probabilities: probabilities.clone(),
                confidence: probabilities.values().cloned().fold(0.0_f64, f64::max),
            },
            VercelAnswer::Boolean(b) => {
                let noul = vercel_boolean_to_noul(b);
                JevAnswer::Noul { noul: noul.noul, probabilities: noul.probabilities }
            }
        };
        (k.clone(), jev_answer)
    }).collect();
    // Vercel 不报 usage —— 给 null 而不是 0（与 jev-life 同条纪律）
    JevResponse { model: Some("vercel-jev"), answers, usage: None }
}
```

**C5 Laya 真 Jev 集成**（不需要 broker —— 与 C1/C4 同协议）：

```rust
// rs/src/upstream/systemone.rs —— Laya 直接走 systemone 协议，无翻译
// HTTP POST {base}/v1/systemone，body = 标准 JevRequest，回包 = 标准 JevResponse
// 唯一差异：中文题目要降级（英文 checkpoint）—— capability 漂移问题
impl Upstream for SystemoneUpstream {
    async fn evaluate(&self, req: &DecisionRequest) -> Result<DecisionResult, JevError> {
        self.post_json(&self.endpoint("/v1/systemone"), req).await
    }
}
```

**broker 借鉴 + 增量**：

| jev-life 纯函数 | Jev-Switch 处理 |
|---|---|
| `toLlmRequest` / `toAnthropicRequest` / `fromLlmContent` / `extractContent` / `extractToolCalls` / `answersFromToolArguments` / `answersFromToolInput` | **直接移植**（算法即知识） |
| `capabilitiesOf` / `clampEffort` / `resolveEffort` | **直接移植**，表扩大（5 类 × 2 协议族 = 10 条） |
| `unwrapFence` / `clamp01` / `usageOf` / `anthropicUsageOf` / `answerTool` | **直接移植** |
| `callJev` / `callOnce` / `runWithRetry`（HTTP + 重试编排） | **不移植**（自写 reqwest + tower，但算法同源） |
| `BACKENDS` 表 / `estimateCost` | **不移植**（换 config.toml；按 provider 分建价目） |

**为什么 jev-life 没做**：它的设计目标就是「给任意 LLM 套上 Jev 壳」——
OpenRouter / Vercel 这种"已经是 Jev"的网关不在它的视野里。

---

### 能力 5：本地 daemon + 跨平台 + Tauri（jev-life 是 Vercel serverless）

**jev-life 现状**：Vercel 三条 Serverless + 本地 Node 静态服务器；GitHub Pages 那条完全没服务端，broker 跑在浏览器内。

**Jev-Switch 升级**：M1 本地 daemon → M6 React Web 控制面板 → M7 Tauri 桌面。

**Jev-Switch 升级**：M1 本地 daemon → M6 React Web 控制面板 → M7 Tauri 桌面。

```rust
// rs/src/main.rs —— 本地 daemon 入口
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::load_from_toml("~/.jev-switch/config.toml")?;
    let router = Router::new(cfg.routing, cfg.providers).await?;
    let app = axum::Router::new()
        .route("/v1/systemone", post(handler::evaluate))
        .route("/health", get(handler::health))
        .route("/metrics", get(handler::metrics))
        .route("/admin/providers", get(handler::list_providers))
        .route("/admin/up/:id", post(handler::switch_to))
        .layer(TraceLayer::new_for_otel())      // OTel 自动注入
        .with_state(AppState { router });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8765").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

**形态对比**：

| 项目 | 部署 | 启动 | 分发 |
|---|---|---|---|
| jev-life | Vercel + GitHub Pages | 冷启动 100ms+ | `git push` |
| **Jev-Switch M1** | **本地 daemon** | < 50ms（Rust） | `cargo install` |
| **Jev-Switch M7** | **Tauri 桌面** | 即开即用 | `.exe / .dmg / .AppImage` |
| LM Studio / CC Switch | 本地 .app / Tauri | 即开即用 | 单文件下载 |

**借鉴 cc-switch 的 Tauri 2 + React 模式**（`farion1231/cc-switch`）：`src-tauri/` 包装
axum 进 tauri runtime + `ui/` 用 React + Vite + shadcn/ui（Providers / TraceViewer /
Metrics 三页）。配置文件位置约定（与 LM Studio / CC Switch 一致）：
- Windows：`%USERPROFILE%\.jev-switch\config.toml`
- macOS / Linux：`~/.jev-switch/config.toml`

**为什么 jev-life 没做**：Vercel serverless 形态的好处是**零安装、零运维**；
代价是延迟高（冷启动 100ms+）、密钥集中、无法离线。这与 Jev-Switch 的
"本地优先 + 可观测"定位正好相反。

---

## 三、能力 1+2+3+4+5 的相互关系（M1-M7 切分）

```
能力 4（broker 纯函数 + 翻译层）     ← M3（依赖 M1 的 Upstream trait 形状）
能力 1（Router + 策略 + 熔断）       ← M2（依赖 M1）
能力 2（OTel + Prometheus）          ← M4（依赖 M1-M3 的 hook 点）
能力 3（config.toml + CLI）          ← M5（依赖 M1-M3 + M4 的 trace）
能力 5（daemon + Tauri）             ← M1 启 daemon，M6 React，M7 Tauri
```

| 编号 | 包含能力 | 独立交付 | 必须同时 |
|---|---|---|---|
| **M1** | 4 + 1（骨架） | ✅ 单跑 5 类 upstream | - |
| **M2** | 1（路由 + 熔断） | ✅ 加在 M1 上 | 依赖 M1 |
| **M3** | 4（broker 纯函数 + 翻译） | ✅ 单跑 broker | 依赖 M1（借用 Upstream trait 形状） |
| **M4** | 2（OTel + Prometheus） | ✅ 加 trace / metrics | 依赖 M1-M3（要 hook 路由 + 上游 + broker） |
| **M5** | 3（config.toml + CLI） | ✅ 单跑 CLI | 依赖 M1-M4 |
| **M6** | 5（React 前端） | ✅ 单跑 UI | 依赖 M4-M5（要 trace JSON + metrics） |
| **M7** | 5（Tauri 桌面） | ✅ 单跑桌面 | 依赖 M6 |

**关键耦合**：

- **M1 是地基**：`Upstream trait + Capabilities + JevError` 三类型定下后，M2-M5 都基于它们
- **M2 与 M3 互不依赖**：路由可独立用 mock upstream 验证；broker 可独立于路由验证
- **M4 必须在 M1-M3 后做**：trace 要 hook 的点都来自这三层
- **M5 与 M4 松耦合**：M5 可无 trace JSON 也跑（trace 留空）

---

## 四、借鉴 vs 复制的边界

**复制**（算法即知识，跨语言有效）：

- `shared/types.ts` 协议最小集 → `rs/src/types.rs`
- `normalizeQuestionTypes` 算法（noul↔boolean 重写为 Rust）
- `toLlmRequest / toAnthropicRequest / fromLlmContent / extractContent / extractToolCalls` 算法
- `clampEffort / resolveEffort` 三态语义（关键交互约束）
- `clamp01 / unwrapFence / usageOf / anthropicUsageOf` 小工具
- `answerTool` JSON Schema（协议事实）
- `DecisionResult` 三项一等输出（`latencyMs / upstreamCalls / costUsd = null`）
- `JevError.retryable / quotaExhausted` 字段语义
- `backoffDelay`（指数退避 + 抖动 + 30s 上限）
- `startTimeout` 语义（finally clear）
- `isRetryableStatus / isQuotaError`（408/429/5xx + 文案关键词）
- `BACKENDS` "按协议族索引"思路 → `CAPABILITIES` 常量表
- `runToolLoop` 零进展检测 + `maxRounds = pending.size + 4`
- `UsageTally` 三栏累加（input/output/reasoning）
- `DESIGN.md` 第七节"中间那层必须真的兼容"作为设计原则

**不取**（架构形态不同）：

- `createSystemoneBackend / createLlmBackend` 架构（Jev-Switch 用 Router 替代）
- `BACKENDS` 硬编码表（Jev-Switch 用 config.toml）
- `api/_upstream.ts / src/server/server.ts`（Vercel serverless vs 本地 daemon 两种哲学）

**借鉴 vs 复制的判断标准**：

1. **纯函数 → 复制**（算法即知识）
2. **HTTP/IO → 不复制**（语言/运行时不同）
3. **配置/状态 → 不复制**（架构形态不同）
4. **错误分类 / 重试策略 → 复制**（语义是经验结晶）
5. **注释里的"踩过的坑" → 复制到 Rust 注释**（跨语言有效）

---

## 五、可独立审阅点（10 条）

每条三段式：「为什么 jev-life 没做」+「Jev-Switch 怎么做」+「open question」。

### 1. Router 抽象的位置

- **没做原因**：jev-life 把 backend 选择放在**调用方**（`channels.ts` 与 `api.ts` 显式传 id）。
- **Jev-Switch**：在 `evaluate()` 前加 `Router::select()`，失败时按 fallback 链重试（用 jev-life 的 `backoffDelay`）。
- **open**：失败时**同步重试**（尾延迟 = 所有上游之和）还是**返回 503 让客户端重试**？建议同步 + trace 标 `fallback.triggered`。

### 2. Circuit Breaker 的"开/半开/关"

- **没做原因**：单 backend 没意义。
- **Jev-Switch**：见第二节 1 的代码（Closed/Open 两态，cooldown 后回 Closed）。
- **open**：是否要"半开"探测？建议保留两态，靠"cooldown 后第一次失败立即重开"代替。

### 3. trace 默认 redact 策略

- **没做原因**：jev-life raw response 原样返前端（含 reasoning_content）。
- **Jev-Switch**：默认 redact title/excerpt，保留 question.type/instructions；用 `<state: redacted, keys=[..]>` 占位。
- **open**：state redact 默认开还是关？游戏棋盘不敏感，life-series title 是真实新闻。建议默认开，按 provider/用户级开关。

### 4. capability 表的"硬编码 vs 启动时 probe"

- **没做原因**：jev-life 的 `CAPABILITIES` 表是编译期常量（`agnes / openai-compat / anthropic-compat` 三条）。
- **Jev-Switch**：M1 硬编码 `CAPABILITIES`，M5 加 `jev-switch probe <provider>` 实测回写。
- **open**：probe 与硬编码冲突时以谁为准？建议 probe 只是探测工具，不动表 —— 表是"我们的知识"。

### 5. CLI 与 daemon 的 IPC

- **没做原因**：jev-life 是浏览器直连。
- **Jev-Switch**：daemon 监听 `127.0.0.1:8765`，CLI 是 HTTP 客户端；Tauri 进程内复用 axum Router。
- **open**：要不要 Unix Domain Socket？M1 用 TCP loopback 简单，M7 Tauri 进程内复用绕开。

### 6. broker 内的"工具循环 vs JSON 路径"选择

- **没做原因**：jev-life 让**调用方**选（`llm.callPolicy = "json" | "tool"`）。
- **Jev-Switch**：调用方仍可选，但 Router 按 upstream capability 自动降级（选 tool 但上游不支持 → 降级 JSON）。
- **open**：降级时是否标 `policy.degraded`？建议标 —— 让用户看到"我想用 tool 但被降级了"。

### 7. config.toml 的"分层 + 环境变量注入密钥"

- **没做原因**：jev-life 走 Vercel env，浏览器侧密钥为空。
- **Jev-Switch**：`api_key_env = "TYPESAFE_API_KEY"`，daemon 启动时 `std::env::var()`，绝不写进 trace/日志。
- **open**：密钥轮换？M5 加 `jev-switch config rotate`，轮换 = 重新读 env + 刷新内存副本。

### 8. Tauri 单实例 + 系统托盘

- **没做原因**：jev-life 是浏览器，多实例无所谓。
- **Jev-Switch**：Tauri 2 `tauri-plugin-single-instance` + `SystemTray`，托盘点 = 弹出 ProvidersPanel + 显示当前健康。
- **open**：托盘要不要"一键 bench"？建议不加 —— 80 篇跑 30 分钟，托盘动作不该这么重。

### 9. Capability 漂移检测

- **没做原因**：单 backend + 编译期常量 = 漂移就重编译。
- **Jev-Switch**：M5 加 `probe` 命令 + 定期后台 probe（6h）+ 漂移检测时 trace 标 `capability.drift_detected`。
- **open**：漂移时自动禁用 upstream？建议不禁用 —— Vercel 偶尔字段问题，重启就好。

### 10. React 前端 trace 可视化

- **没做原因**：jev-life trace = 当前请求 raw 包。
- **Jev-Switch**：TraceViewer 按 span 层级（参考 Jaeger UI），可重放（POST 同 req body 给 daemon）。
- **open**：重放要不要"原样上游 + 同一 trace_id"？建议新 trace_id + 保留"重放来源"链接 —— 避免 trace 链污染。

---

## 六、可独立 fork 的子任务（供未来 subAgent 推进）

1. **`rs/src/types.rs` 的 Jev 协议最小集移植** — 从 jev-life `shared/types.ts` 移植 `Question / Answer / JevRequest / JevResponse / Criteria / NoulType`；输出 `rs/src/types.rs`（~100 行）+ property tests for `noulDiscriminator / normalizeQuestionTypes / noulProbability`；依赖：无。
2. **`rs/src/broker/` 的纯函数移植** — 移植 `toLlmRequest / toAnthropicRequest / fromLlmContent / extractContent / extractToolCalls / answersFromToolArguments / answersFromToolInput / clampEffort / resolveEffort / capabilitiesOf / clamp01 / unwrapFence / usageOf / anthropicUsageOf`；输出 ~500 行 + proptest 200+ 条；依赖：任务 1。
3. **`rs/src/upstream/` 的 5 个 Upstream 实现** — `SystemoneUpstream / LlmJsonUpstream / LlmToolUpstream / PrefillOnlyUpstream / BrokerEmbedded`；输出 ~600 行 + wiremock 集成测试；依赖：任务 1+2。
4. **`rs/src/router/` 的 4 种策略 + CircuitBreaker** — `RouterStrategy` trait + 4 impl + `CircuitBreaker`；输出 ~400 行 + property tests；依赖：任务 3。
5. **`rs/src/compat/` 的 Vercel 翻译层 + noul↔boolean** — `vercel_to_jev / jev_to_vercel / noul_via_boolean`；输出 ~150 行 + 对照 jev-decision-lab 21/80 篇 life-series 数据；依赖：任务 1。

---

## 七、参考链接

- jev-life broker：`C:\Users\56506\AppData\Local\Temp\jev-life\src\shared\backend.ts`
- jev-life 纯函数 broker / 协议最小集：`...\shared\llm-broker.ts` / `...\shared\types.ts`
- 本仓库 `02-` v2.1：5 类上游定义；`03-`：capability 矩阵 + 翻译层细节
- jev-decision-lab `runs/integrated-benchmark-data.json`（5×4×12 高考题）+ `life-series-benchmark/`（80 篇）
- sys1（`alvarobartt/sys1`）—— Router HTTP 模式；CC Switch（`farion1231/cc-switch`）—— Tauri 形态

---

## 接下来要做的事

1. `rs/` 下 `cargo init --lib` + axum + tokio + reqwest
2. 按编号完成第六节 5 个子任务（可多 subAgent 并行）
3. 写 `docs/04-架构设计-from-sys1-借鉴.md`
4. M1 启 5 类 upstream + capability 矩阵，wiremock 集成测试