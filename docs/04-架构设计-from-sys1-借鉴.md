# Jev-Switch 架构设计 — 借鉴 sys1 与本仓库实测

**版本**：v1（2026-09-23）  
**前置文档**：[02- v2.1](./02-Jev-Switch-新版计划书.md)（5 类上游 + 桥接 + M1-M7）、[03-](./03-上游类别与协议兼容矩阵.md)（capability + 翻译层）、[`../jev-decision-lab/ts/decision-bricks.ts`](../jev-decision-lab/ts/decision-bricks.ts)、[`../jev-decision-lab/runs/integrated-benchmark-data.json`](../jev-decision-lab/runs/integrated-benchmark-data.json)（trace JSON 兼容目标）

**目标**：把 02- 规划与 03- capability 矩阵落地为 Rust 后端 + React 前端 + 未来 Tauri 的具体代码与目录结构。

---

## 一、借鉴 sys1 的部分

sys1（`github.com/alvarobartt/sys1`，Rust + axum + candle）是 Jev 生态唯一的"纯 Rust 实现"。它是 Jev-Switch 的直接架构蓝本，但**只在两层借鉴**——HTTP 层与 serde 类型层；推理层**完全跳过**（Jev-Switch 不做推理，推理在 C5 上游里）。

### 1.1 axum 路由 + serde 类型 — 直接照搬

sys1 入口模式（来自 `src/main.rs`）：`#[derive(Deserialize)] struct SystemOneRequest {...}` → `async fn evaluate_handler(State(model): State<Arc<Model>>, Json(req): ...) -> Result<Json<SystemOneResponse>, AppError>` → `Router::new().route("/v1/systemone", post(evaluate_handler)).with_state(state)`。

**Jev-Switch 改造点**：

| sys1 模式 | Jev-Switch 改造 | 理由 |
|---|---|---|
| `State<Arc<Model>>` 单状态 | `State<AppState>` 多 upstream + router + broker | sys1 只一个推理；Jev-Switch 要路由 |
| 直接 `Json(req)` | `Json(req)` + 协议兼容层预处理 | 03- 文档说明 Vercel `noul→boolean` 翻译 |
| handler 直接返回 | handler 异步 + IntoResponse | 让 capability 缺失字段可以补齐 |
| 无 admin 路由 | 加 `/admin/*` 系列 | M5 CLI 与 UI 共用 |

**serde 类型**（与 sys1 共享 schema）：

```rust
// crates/protocol/src/systemone.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneRequest {
    pub model: String,
    pub state: serde_json::Value,
    pub questions: BTreeMap<String, DecisionQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DecisionQuestion {
    Choice  { instructions: String, criteria: BTreeMap<String, String> },
    Score   { instructions: String, criteria: Vec<String> },
    Noul    { instructions: String, criteria: BTreeMap<String, String> },  // true/false
    Boolean { instructions: String, criteria: BTreeMap<String, String> },  // Vercel 适配
}
```

### 1.2 candle 推理 — 完全跳过

sys1 用 candle 加载本地 ModernBERT 做 30ms 推理。Jev-Switch **不重复造轮子**：

- **不引入 candle**：推理在 C5 上游（用户可选 Laya 或 sys1 本地部署）
- **不维护权重文件**：权重加载、tokenizer 配置都不进 Jev-Switch 仓库
- **唯一借鉴**：`candle-nn` 的 `Config` trait 思路——配置 schema 也用 `Deserialize` 驱动，但 schema 是 config.toml 而非 model.safetensors

**取舍**：自带推理 = 单进程但与 sys1 重复、权重分发麻烦；**采纳** "调度 + sys1/Laya 上游" = 推理升级独立、daemon < 50MB，代价是依赖用户启本地服务。

---

## 二、Rust 后端架构

### 2.1 模块树（在 02- v2.1 §4.2 基础上扩展）

```
rs/
├── Cargo.toml                         # workspace: protocol / core / daemon
├── crates/
│   ├── protocol/src/systemone.rs      # 与 sys1 共享 serde
│   ├── jev-switch-core/src/
│   │   ├── upstream/                  # trait + 5 类实现（systemone / llm_json / llm_tool / prefill_only / capabilities）
│   │   ├── router/                    # 策略 + 熔断
│   │   ├── broker/                    # ProtocolAdapter + 借鉴 jev-life
│   │   ├── compat/                    # Vercel 翻译层
│   │   └── observability/             # Otel + Prometheus
│   └── jev-switch-daemon/src/
│       ├── main.rs / cli.rs           # clap 子命令
│       └── http/                      # handler / health / admin
├── tests/                             # 集成测试
└── examples/{config.toml, bench_life_series.rs}
```

**workspace 拆分理由**：`protocol` 与 sys1 共享（未来可独立 publish）；`core` 不依赖 tokio/reqwest（业务规则纯函数测试）；`daemon` 才依赖 axum/reqwest。

### 2.2 Cargo.toml 关键依赖

```toml
[workspace]
members = ["crates/protocol", "crates/jev-switch-core", "crates/jev-switch-daemon"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }; serde_json = "1"
thiserror = "1"; anyhow = "1"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }
axum = "0.7"; hyper = "1"
tower = { version = "0.5", features = ["timeout", "limit", "load-shed"] }
tower-http = { version = "0.6", features = ["trace", "cors", "request-id"] }
config = { version = "0.14", default-features = false, features = ["toml"] }
clap = { version = "4", features = ["derive", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
opentelemetry = "0.24"
opentelemetry_sdk = { version = "0.24", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.17", features = ["grpc-tonic"] }
prometheus = "0.13"
indicatif = "0.17"

[dev-dependencies]
wiremock = "0.6"; proptest = "1"
```

**取舍**：`rustls-tls` 而非 `native-tls`（跨平台一致）；`tower` 的 `limit` + `load-shed` 防 C4 本地故障拖垮 daemon；`clap` 的 `env` feature 让子命令从 `JEV_SWITCH_CONFIG` 读配置路径。

### 2.3 完整的 axum 路由表

| 方法 | 路径 | handler | 用途 |
|---|---|---|---|
| `POST` | `/v1/systemone` | `handler::systemone` | 主入口：Jev 协议请求（C1-C5 全部） |
| `POST` | `/v1/chat/completions` | `handler::openai_compat` | OpenAI 兼容入口（v0 实验） |
| `GET`  | `/v1/models` | `handler::list_models` | 列出可达上游 + capability |
| `GET`  | `/health` | `health::liveness` | liveness probe |
| `GET`  | `/ready` | `health::readiness` | readiness：所有 upstream health |
| `GET`  | `/metrics` | `health::prometheus` | Prometheus exposition |
| `GET`  | `/admin/providers` | `admin::list_providers` | 列出所有配置 upstream |
| `GET`  | `/admin/providers/:id` | `admin::get_provider` | 单个 upstream 详情 + health |
| `POST` | `/admin/upstream/switch` | `admin::switch_upstream` | 切主上游（运行时） |
| `POST` | `/admin/upstream/test` | `admin::test_upstream` | 跑一次 health probe |
| `GET`  | `/admin/traces` | `admin::list_traces` | 列 trace 文件 |
| `GET`  | `/admin/traces/:id` | `admin::get_trace` | 重放一次请求 |
| `GET`  | `/admin/health/dashboard` | `admin::health_dashboard` | JSON 健康仪表 |

**handler::systemone 签名**（核心 30 行）：

```rust
// crates/jev-switch-daemon/src/http/handler.rs
pub async fn systemone(
    State(state): State<AppState>,
    Json(req): Json<SystemOneRequest>,
) -> Result<axum::response::Response, AppError> {
    // 1. 协议兼容预处理（Vercel 路径在 router 选择时已分流，这里只做最终归一）
    let normalized = state.compat_layer.normalize(req)?;

    // 2. Router 选择上游（含熔断 + capability 收敛）
    let upstream_id = state.router.select(&normalized, &state.health).await?;

    // 3. 调用上游（带 trace span）
    let result = state.upstreams.get(&upstream_id).unwrap()
        .evaluate(&normalized).await?;

    // 4. 协议收敛（Vercel boolean→noul 等）
    let jev_resp = state.compat_layer.denormalize(result, &normalized)?;

    // 5. 写 trace + metric
    state.observability.record(&upstream_id, &normalized, &jev_resp);

    Ok(Json(jev_resp).into_response())
}
```

### 2.4 关键 trait 定义

```rust
// crates/jev-switch-core/src/upstream/mod.rs
#[async_trait::async_trait]
pub trait Upstream: Send + Sync {
    fn id(&self) -> &UpstreamId;
    fn kind(&self) -> &'static str;              // "systemone" | "llm-json" | ...
    fn capabilities(&self) -> &UpstreamCapabilities;
    fn health(&self) -> HealthStatus;
    async fn evaluate(&self, req: &SystemOneRequest) -> Result<DecisionResult, JevError>;
}

#[derive(Debug, Clone)]
pub struct DecisionResult {
    pub upstream_id: UpstreamId,
    pub response: SystemOneResponse,
    pub latency_ms: u64,
    pub trace_events: Vec<TraceEvent>,
}
```

```rust
// crates/jev-switch-core/src/router/mod.rs
#[async_trait::async_trait]
pub trait RouterStrategy: Send + Sync {
    async fn select(&self, req: &SystemOneRequest, health: &HealthRegistry)
        -> Option<UpstreamId>;
}
```

```rust
// crates/jev-switch-core/src/upstream/capabilities.rs（03- §2.3 CAPABILITIES 表）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoulDiscriminator { Noul, Boolean }
#[derive(Debug, Clone)]
pub struct UpstreamCapabilities {
    pub supports_question_types: &'static [QuestionType],
    pub supports_confidence: bool,
    pub supports_usage: bool,
    pub noul_discriminator: NoulDiscriminator,
    pub retryable_status: &'static [u16],
}
impl UpstreamCapabilities {
    pub fn supports(&self, qt: QuestionType) -> bool {
        match (qt, self.noul_discriminator) {
            (QuestionType::Noul, NoulDiscriminator::Boolean) => false,  // Vercel
            (q, _) => self.supports_question_types.contains(&q),
        }
    }
}

// 三态 effort 收敛（03- §四）
pub fn resolve_effort(desired: Option<Option<EffortLevel>>, cap: &UpstreamCapabilities)
    -> Option<EffortLevel> {
    match desired {
        None => Some(EffortLevel::None),
        Some(None) => None,
        Some(Some(level)) if cap.supports_effort(level) => Some(level),
        Some(Some(_)) => None,
    }
}
```

---

## 三、协议兼容层实现（对应 03- 文档）

### 3.1 Vercel `noul→boolean` 翻译器

```rust
// crates/jev-switch-core/src/compat/vercel.rs
use crate::protocol::{SystemOneRequest, SystemOneResponse, DecisionAnswer, DecisionQuestion};

/// 入口：noul → boolean（Vercel 不识别 noul 字段名）
pub fn vercel_normalize(req: SystemOneRequest) -> SystemOneRequest {
    let mut out = req;
    for q in out.questions.values_mut() {
        if matches!(q, DecisionQuestion::Noul { .. }) {
            let (instructions, criteria) = match q {
                DecisionQuestion::Noul { instructions, criteria } => (instructions.clone(), criteria.clone()),
                _ => unreachable!(),
            };
            *q = DecisionQuestion::Boolean { instructions, criteria };
        }
    }
    out
}

/// 出口：Vercel boolean → Jev noul（true → 0.95 / false → 0.05）
pub fn vercel_denormalize(resp: SystemOneResponse, original_q: &SystemOneRequest) -> SystemOneResponse {
    let mut out = resp;
    for (qid, ans) in out.answers.iter_mut() {
        let was_noul = matches!(original_q.questions.get(qid), Some(DecisionQuestion::Noul { .. }));
        if was_noul && let Some(b) = ans.boolean {
            let prob = if b { 0.95 } else { 0.05 };
            ans.noul = Some(prob);
            ans.probabilities = Some(BTreeMap::from([
                ("true".into(), prob), ("false".into(), 1.0 - prob),
            ]));
            ans.confidence = ans.confidence.or(Some(prob));
        }
    }
    out
}
```

### 3.2 协议差异收敛层（入口）

```rust
// crates/jev-switch-core/src/compat/mod.rs
pub struct CompatLayer { vercel_enabled: bool }

impl CompatLayer {
    pub fn normalize(&self, req: SystemOneRequest) -> Result<SystemOneRequest, JevError> {
        if self.vercel_enabled { Ok(vercel_normalize(req)) } else { Ok(req) }
    }
    pub fn denormalize(&self, result: DecisionResult, original: &SystemOneRequest)
        -> Result<SystemOneResponse, JevError> {
        if self.vercel_enabled && result.upstream_id.as_str() == "vercel" {
            Ok(vercel_denormalize(result.response, original))
        } else { Ok(result.response) }
    }
}
```

**取舍**：翻译只在选到 Vercel 上游后才启用（避免给 C1/C4 加无谓开销）；`confidence` 用 0.95/0.05 而非原始概率——Vercel boolean 类型**真的不给概率**，这是已知损失（03- §2.4 提到）。

---

## 四、Broker 借鉴 jev-life

`../jev-life/src/shared/llm-broker.ts` 的核心抽象是**纯函数 + 协议族索引**。Rust 移植后变成 trait 而非 switch：

```rust
// crates/jev-switch-core/src/broker/mod.rs
#[async_trait::async_trait]
pub trait ProtocolAdapter: Send + Sync {
    fn id(&self) -> &'static str;  // "openai" | "anthropic"
    fn build_request(&self, sys: &SystemOneRequest, effort: Option<EffortLevel>) -> LlmRequest;
    fn extract_content(&self, raw: &LlmRawResponse) -> Result<String, BrokerError>;
    fn extract_tool_calls(&self, raw: &LlmRawResponse) -> Result<Vec<ToolCall>, BrokerError>;
    fn map_to_jev(&self, content: &str, tool_calls: &[ToolCall], req: &SystemOneRequest)
        -> Result<SystemOneResponse, BrokerError>;
}
```

### 4.1 5 类 broker 抽象（按 C3 上游 LLM 协议族切分）

| broker 抽象 | 协议族 | 上游举例 | 何时启用 |
|---|---|---|---|
| `OpenAiJsonBroker` | OpenAI + JSON mode | LM Studio jev-api、OpenAI API | `kind = "llm-json"` + `protocol = "openai"` |
| `OpenAiToolBroker` | OpenAI + tool calling | OpenAI 函数调用 | 同上 + `tool_calling = true` |
| `AnthropicJsonBroker` | Anthropic Messages + JSON | Anthropic API | `protocol = "anthropic"` |
| `AnthropicToolBroker` | Anthropic + tool use | Anthropic tool use | 同上 + `tool_calling = true` |
| `PrefillOnlyBroker` | prefill logits | LLM2Jev（**实际不需要**，走 C4 systemone） | 历史占位 |

### 4.2 与 jev-life 关键差异

| 维度 | jev-life broker | Jev-Switch broker |
|---|---|---|
| 协议族 | 1 套（OpenAI 兼容） | **2 套**（OpenAI + Anthropic） |
| Tool loop | 有 | 有（含零进展检测） |
| Effort 三态 | 有 | 有（直接搬 `resolve_effort`） |
| 配置来源 | 硬编码 BACKENDS 表 | `config.toml` 动态加载 |
| 错误分类 | 简单 | 区分 `EmptyContent` / `BudgetExhausted` / `ToolStuck` |

### 4.3 jev-life 纯函数 → Rust 纯函数对照

```typescript
// jev-life: src/shared/llm-broker.ts
export function buildLlmRequest(backendId, decision, effort): LlmRequest {}
export function extractLlmContent(backendId, raw): string {}
export function callClampEffort(desired, backendId): Effort {}
```

```rust
// Jev-Switch: crates/jev-switch-core/src/broker/openai.rs
impl ProtocolAdapter for OpenAiJsonBroker {
    fn build_request(&self, sys: &SystemOneRequest, effort: Option<EffortLevel>) -> LlmRequest {
        let prompt = self.build_prompt(sys);  // 借鉴 jev-life 的 prompt 模板
        LlmRequest {
            model: self.model.clone(),
            messages: vec![LlmMessage::system(prompt.system), LlmMessage::user(prompt.user)],
            response_format: Some(ResponseFormat::Json { schema: jev_json_schema(sys) }),
            reasoning_effort: effort.and_then(|e| match_effort_to_openai(e)),
            temperature: Some(0.0),
        }
    }
    // ... extract_content / extract_tool_calls / map_to_jev
}
```

---

## 五、可观测性（otel 实战）

### 5.1 trace span 设计

```rust
// crates/jev-switch-core/src/observability/trace.rs
pub async fn trace_evaluate<U: Upstream>(upstream: &U, req: &SystemOneRequest)
    -> Result<DecisionResult, JevError>
{
    let tracer = global::tracer("jev-switch");
    let mut span = tracer.start("upstream.evaluate");
    span.set_attribute("upstream.id", upstream.id().to_string());
    span.set_attribute("upstream.kind", upstream.kind());
    span.set_attribute("request.questions.count", req.questions.len());
    let result = upstream.evaluate(req).await;
    if let Ok(r) = &result {
        span.set_attribute("response.latency_ms", r.latency_ms as i64);
    }
    span.end();
    result
}
```

**span 树**（与 02- v2.1 §3.2 对齐）：`router.select_upstream` → `compat.normalize` → `upstream.evaluate`（含 `request.body` / `response.status` / `response.latency_ms` 事件） → 可选 `fallback.triggered`。

### 5.2 Prometheus metric 命名

```rust
// crates/jev-switch-core/src/observability/metric.rs
pub static REQUESTS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!("jev_switch_requests_total", "Total requests per upstream",
        &["upstream", "kind", "status"]).unwrap()  // success | error | fallback
});
pub static LATENCY_SECONDS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!("jev_switch_request_duration_seconds", "Request latency",
        &["upstream", "kind"], vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]).unwrap()
});
pub static CAPABILITY_MISSES: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!("jev_switch_capability_misses_total", "Routed-miss count",
        &["upstream", "missing"]).unwrap()  // e.g. "vercel/missing=noul"
});
```

### 5.3 与 jev-decision-lab trace JSON 兼容

`integrated-benchmark-data.json` 是 TS 原型产出的 trace JSON 数组。Jev-Switch trace 文件 schema 必须**字段对齐**，以便 M5 benchmark 子命令直接复用历史数据：

```json
{
  "trace_id": "0af7651916cd43dd8448eb211c80319c",
  "provider": "vercel",
  "started_at": "2026-09-22T13:45:01.123Z",
  "elapsed_ms": 1247,
  "request":  { "model": "...", "state": "...", "questions": { ... } },
  "response": { "answers": { ... } },
  "error":    null,
  "spans": [
    { "name": "router.select_upstream", "duration_us": 320, "attributes": {...} },
    { "name": "upstream.evaluate",      "duration_us": 1240000, "attributes": {...} }
  ]
}
```

**输出位置**：`~/.jev-switch/traces/{date}/{trace_id}.json`（02- §3.2）。  
**redact 策略**（借鉴 02- §七 #5）：trace 文件中 `questions[*].instructions` 与 `state` 默认 redact；`config.observability.full_trace = true` 才落原文。

---

## 六、配置与 CLI

### 6.1 config.toml 完整 schema

```toml
# ~/.jev-switch/config.toml
schema_version = "1"

[server]
listen = "127.0.0.1:8765"
ui_listen = "127.0.0.1:8766"
admin_listen = "127.0.0.1:8767"   # /admin/* + /metrics 单独绑定
cors_origins = ["http://127.0.0.1:8766"]

[providers.typesafe]   # priority=1, systemone, TYPESAFE_API_KEY, timeout_ms=10000
[providers.openrouter] # priority=2, systemone, OPENROUTER_API_KEY, ipv4_only=true (Cloudflare IPv6 不可达)
[providers.vercel]     # priority=3, systemone, VERCEL_AI_GATEWAY_API_KEY, noul_via_boolean=true → 触发 compat/vercel.rs
[providers.laya]       # priority=4, systemone, no_retry=true (本地)
[providers.llm2jev]    # priority=5, systemone, no_retry=true (本地)
[providers.qwen_4b_broker]   # priority=6, kind=llm-json, protocol=openai, base=http://127.0.0.1:1234/v1/chat/completions, model=qwen/qwen3-4b-2507, effort_default=low
[providers.claude_haiku_broker] # priority=7, kind=llm-tool, protocol=anthropic, model=claude-haiku-4-5

[routing]
strategy = "failover-chain"
fallbacks = ["typesafe", "openrouter", "vercel", "laya", "llm2jev", "qwen_4b_broker"]
circuit_breaker = { max_failures = 3, cooldown_s = 60, half_open_after_s = 30 }

[observability]
trace_dir = "~/.jev-switch/traces"
metrics_port = 9090
otel_endpoint = ""                # 空 = 不发 OTLP
log_level = "info"
full_trace = false                # true = 落原文 questions.instructions

[cli]
default_provider = "typesafe"
```

### 6.2 CLI 子命令（clap derive）

```rust
// crates/jev-switch-daemon/src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jev-switch", version, about)]
pub struct Cli {
    #[arg(long, env = "JEV_SWITCH_CONFIG", default_value = "~/.jev-switch/config.toml")]
    pub config: String,
    #[command(subcommand)] pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 启动 daemon（默认前台）
    Up      { #[arg(long)] interactive: bool },
    /// 健康仪表
    Status  { #[arg(long)] watch: bool },
    /// 重放一次请求
    Trace   { trace_id: String, #[arg(long)] replay: bool },   // 真发请求 vs 仅查看
    /// 跑内置 benchmark
    Bench   { #[arg(long, default_value = "life-series")] suite: String,
              #[arg(long)] upstream: Option<String> },
    /// 配置校验（不起 daemon）
    Config  { #[command(subcommand)] action: ConfigCmd },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    Validate,                                 // 仅 toml schema 校验
    Schema,                                   // 输出 JSON Schema
    Migrate { from: String, to: String },
}
```

**关键实现**：`jev-switch config validate` 必须在 daemon 启动前完成——这是 M5 的硬要求，能拦截 80% 的 schema 错误。

---

## 七、前端 React + shadcn/ui

### 7.1 页面结构

```
ui/                                # React 19 + Vite + shadcn/ui
├── package.json
├── vite.config.ts
├── src/
│   ├── App.tsx                    # 路由（TanStack Router）
│   ├── routes/
│   │   ├── ProvidersPanel.tsx     # 主页：上游列表 + 切换
│   │   ├── TraceViewer.tsx        # 列表 + 单条 trace 详情
│   │   ├── MetricsPanel.tsx       # Prometheus 图表（prom-client 直连）
│   │   └── Settings.tsx           # config.toml 在线编辑
│   ├── components/                # ProviderCard / TraceSpanTree / UpstreamSwitchDialog / HealthBadge
│   └── lib/{api.ts, types.ts}     # fetch 封装（→ 127.0.0.1:8767 admin API）
└── index.html
```

### 7.2 与 Rust 后端的 API 契约

UI 只调 `/admin/*` 系列（`admin_listen` 端口），**不直连 `/v1/systemone`**——避免 CORS 与密钥泄露：

| UI 调用 | 后端 | 返回 |
|---|---|---|
| `GET /admin/providers` | `admin::list_providers` | `Vec<ProviderInfo>` |
| `POST /admin/upstream/switch` | `admin::switch_upstream` | `{ ok: bool }` |
| `GET /admin/health/dashboard` | `admin::health_dashboard` | `{ uptime_s, providers, circuit_breakers }` |
| `GET /admin/traces?date=...` | `admin::list_traces` | `Vec<{trace_id, started_at, upstream, elapsed_ms}>` |
| `GET /admin/traces/:id` | `admin::get_trace` | 完整 trace JSON（与 §5.3 同 schema） |
| `GET /metrics` | `health::prometheus` | Prometheus exposition（MetricsPanel 直连） |

**shadcn/ui 选用组件**：`Card`、`Tabs`、`Table`、`Badge`、`Dialog`、`Sheet`、`ScrollArea`、`Toast`（sonner）。

---

## 八、未来 Tauri 包装（M3）

### 8.1 借鉴 cc-switch 的 Tauri 2 + React 模式

cc-switch 是 LLM 路由的 Tauri 桌面应用，参考其结构：

```
tauri-app/
├── src-tauri/
│   ├── Cargo.toml             # 依赖 jev-switch-daemon（本地 path）
│   └── src/{main.rs, sidecar.rs}  # spawn + 管理 daemon 子进程
├── src/                       # = ui/ 的内容（pnpm workspace 引用）
└── package.json
```

**Tauri 2 核心角色**：Tauri shell（系统托盘 + 单实例锁）+ Tauri commands（UI ↔ Rust IPC）+ Sidecar（启停 `jev-switch-daemon`，pipe 日志到 UI）。

### 8.2 桌面应用形态 vs daemon 形态的差异

| 维度 | daemon（`jev-switch up`） | Tauri 桌面 |
|---|---|---|
| 用户入口 | `curl 127.0.0.1:8765/v1/systemone` | 系统托盘 + Webview UI |
| 启动方式 | `systemd` / `launchd` / Windows Service | `.app` / `.exe` |
| 进程模型 | 单进程 | Tauri 父 + daemon 子进程（sidecar） |
| UI 端口 | 8766 独立 | Tauri webview 内（无端口） |
| 适用场景 | 服务器 / WSL / headless | 桌面用户日常 |

**关键技术点**：Sidecar 自动启停（Tauri 启 spawn、退出 SIGTERM 等 5s）；单实例锁（`tauri-plugin-single-instance`）；API key 走 `KeyringStore` 不进前端；UI 经 commands 调 Rust 不直 HTTP（避免 CORS）。

**不重复造的部分**：React UI 在 `ui/` 与 `tauri-app/src/` 复用（pnpm workspace）；Rust 业务逻辑全在 `core` / `daemon`，Tauri 只做壳；配置 / trace / metric schema 不变。

---

## 九、对其他 Agent 的可审阅点（**04- 特有**，不与 02- / 03- 重复）

1. **sys1 借鉴边界**："HTTP + serde 借鉴，candle 不借鉴"。是否过度保守——是否应保留 candle 作 C5 fallback？
2. **workspace 三 crate 拆分**：`protocol` / `core` / `daemon`。是否需要更细（如 broker 独立），或反过来合并？
3. **`admin_listen` 与 `listen` 分离**：把 `/admin/*` + `/metrics` 绑到 8767。是否过度？单端口 + CORS 隔离够不够？
4. **Tauri sidecar vs 单进程**：选 sidecar。是否应改成 daemon 逻辑直接编进 Tauri Rust binary？
5. **trace redact 默认关闭原文**：默认 `full_trace = false`。是否应默认开启（调试友好但泄露数据）？
6. **broker 的 5 类切分**：4 实际 + Prefill 占位。是否合并 Anthropic JSON/Tool（Anthropic 没"纯 JSON mode"）？
7. **OpenAI 兼容入口 `/v1/chat/completions`**：v0 实验。若用户大量用 OpenAI SDK，是否提前到 M2？
8. **`trace --replay` 安全性**：真发请求烧上游额度。是否需 `--dry-run` 默认开，或 `JEV_SWITCH_REPLAY_CONFIRM` env 变量？
9. **schema_version = "1" 迁移**：引入但未设计迁移路径。M5 就建 `config migrate` 子命令，还是延后？
10. **`bench --suite life-series` 数据依赖**：数据在 `jev-decision-lab/life-series-benchmark/`。Cargo workspace 怎么引用同级目录（git submodule？workspace 嵌套？拷到 `crates/jev-switch-daemon/benches/data/`）？

---

## 十、参考链接

- **sys1**：[github.com/alvarobartt/sys1](https://github.com/alvarobartt/sys1) — Rust + axum + candle Jev 实现
- **cc-switch**：[github.com/farion1231/cc-switch](https://github.com/farion1231/cc-switch) — Tauri 参考
- **jev-life broker**：`../jev-life/src/shared/llm-broker.ts`
- **TypeSafe API**：[docs.typesafe.ai/api](https://docs.typesafe.ai/api)
- **02- v2.1 §4.2**：`./02-Jev-Switch-新版计划书.md` — Rust 模块树原始版
- **03- §2.4**：`./03-上游类别与协议兼容矩阵.md` — Vercel 翻译层字段级
- **同级 trace JSON**：`../jev-decision-lab/runs/integrated-benchmark-data.json`