# Jev-Switch 新版计划书

**版本**：v2（基于 DeepSeek 原始计划书 v1 + 本仓库 jev-decision-lab 实测数据 + jev-life broker 调研）
**撰写日期**：2026-09-22
**前置文档**：[`01-Jev-Switch-原始计划书-DeepSeek.md`](./01-Jev-Switch-原始计划书-DeepSeek.md)

---

## 零、与 v1 的关键差异（先看这里）

| 维度 | v1（DeepSeek 原始） | v2（本版） | 理由 |
|---|---|---|---|
| 上游数量 | "4 条原始需求" 隐含"多上游" | **明确 5 类上游**（按协议形态+部署位置） | 用户明确"5 类"而非"5 个"；按类别分组才能做 capability 收敛 |
| LLM→Jev 转接 | 列为 M4/M5 后续 | **列为"定制提供商"一等公民** | jev-life 的 broker 已经实现且成熟；不是后续，而是 5 类中的一类 |
| 技术栈 | TS/Node 或 Rust 二选一 | **Rust + axum（后端）+ React（前端），未来 Tauri** | 借鉴 sys1 的 Rust/axum；前端借鉴 cc-switch 的 React + shadcn/ui |
| 形态 | "类似 LM Studio 的 HTTP 代理" | **本地 daemon（system service / Tauri 桌面应用），跨平台** | 与 LM Studio / CC Switch 同生态位；Windows + macOS + Linux |
| 测试 5 provider × 4 version × 12 题 | 未提及 | **已实测，作为 baseline** | 见 `runs/integrated-benchmark-data.json` 与本仓库 `life-series-benchmark/` |
| GitHub 仓库 | 未提及 | **已建 `ARCJ137442/jev-decision-lab`**（私有 → 计划转公开） |  |

---

## 一、5 类上游的定义（不是 5 个）

**按"协议形态 + 部署位置"分类，而不是按 provider 实体分类**。这与 jev-life 的 `CAPABILITIES` 表按协议族索引的设计一致。

| # | 类别 | 协议形态 | 部署 | 代表实现 | capability 索引 |
|---|---|---|---|---|---|
| **C1** | **TypeSafe 云端官方** | Jev 原生 | 云 | `https://api.typesafe.ai/v1/systemone` | `type=choice/score/noul` 全支持 + `confidence` |
| **C2** | **第三方 Jev 透传网关** | Jev 变体 | 云 | OpenRouter `/api/alpha/decisions`、Vercel AI Gateway `/v4/ai/evaluation-model` | 协议层 ≠ 一致；按各家实测建 capability 表 |
| **C3** | **本地 LLM-Jev broker** | LLM 协议 + structured JSON / tool calling | 本地 | LM Studio jev-api（Qwen + JSON）、jev-life broker | 模拟 Jev 输出；prompt 模板可控 |
| **C4** | **本地独立 Jev 兼容服务**（"本地代理"） | Jev 原生 | 本地 | LLM2Jev（`POST /v1/systemone`，prefill-only logits）、LM Studio jev-api（OpenAI JSON）、sys1（Rust） | 二元/标量决策（prefill-only）或通用 Jev；**作独立 HTTP 服务跑，Jev-Switch 当 C1/C5 一样调用** |
| **C5** | **本地真 Jev 模型** | Jev 原生 | 本地 | Laya（ModernBERT 421M）、sys1（Rust/axum + candle） | 完整 Jev 协议；最快（30ms） |

**注意**：

- **C2 不是"次等 C1"**。OpenRouter / Vercel 与 TypeSafe 协议不同（特别是 Vercel evaluation-model 没有 `noul` 只有 `boolean`，`confidence` 字段缺失）。把它们当 Jev 兼容层会导致 capability 漏洞。
- **C3 和 C4 不是竞争关系**：
  - **C3**：broker **嵌在 Jev-Switch 进程内**——用户填 LLM endpoint，Jev-Switch 用 prompt + JSON/tool calling 在自己进程内翻译。优点是不用启额外服务。缺点是 broker 配置跟着 Jev-Switch 走，密钥共享。
  - **C4**：**独立的 HTTP 服务**——LLM2Jev / LM Studio jev-api / sys1 各自起一个进程，暴露 `POST /v1/systemone`。Jev-Switch 把它们当 C1/C5 一样调用，不做翻译。优点是 LLM2Jev 可以独立升级、独立部署。缺点是用户要额外启进程。
  - 用户可以**同时配**C3 和 C4：C3 用本地 Qwen + JSON，C4 用 LLM2Jev + prefill-only。
- **C5 不与 C1-C4 冲突**。C5 是真 Jev 模型，可作为最快的本地 fallback。

---

## 二、5 类上游的 capability 矩阵（实测数据）

| 类别 | choice | score | noul | confidence | 稳定延迟 | 备注 |
|---|---|---|---|---|---|---|
| **C1** TypeSafe 云 | ✅ | ✅ | ✅ | ✅ | 1.5-3s | 标准；waitlist + $0.042/M input |
| **C2** OpenRouter Jev | ✅ | ✅ | ✅ | ✅ | 1-12s | **必须 IPv4**（Cloudflare IPv6 不可达） |
| **C2** Vercel AI Gateway | ✅ | ✅ | `boolean` 不是 `noul` | ❌ missing | 1-3s | **必须翻译层**：`noul→boolean`，confidence 用其它信号估 |
| **C3** LM Studio jev-api | ✅ | ✅ | ✅ | ✅ | 6s (4B) / 10s+ (27B) | **慢但免费**；适合本地大模型 |
| **C3** jev-life broker | ✅ | ✅ | ✅（语义对齐） | ✅（无 native confidence，用 prompt 约束） | 取决于上游 LLM | 已实测 0-100% 通过率（依 effort 开关） |
| **C4** **LLM2Jev**（独立 HTTP 服务） | ✅ | ✅ | ✅ | ✅（prefill logits → 概率） | 取决于本地 LLM | **独立 HTTP 服务（`POST /v1/systemone`）**，与 C1/C5 同协议；Jev-Switch 当普通 upstream 调用；133 stars；2026-09-22 新增多模态 |
| **C4** LM Studio jev-api | ✅ | ✅ | ✅ | ✅ | 6-10s | 替代路径：单进程 `py -3.11 jev_api_server.py --provider lmstudio` |
| **C4** sys1 (Rust/candle) | ✅ | ✅ | ✅ | ✅ | 本地推理 | 0 star；可作实现参考 |
| **C5** Laya 本地 | ✅ | ✅ | ✅ | ✅ | **30ms** | 最快；英文 checkpoint 中文降级 |

**实测依据**：本仓库 `runs/integrated-benchmark-data.json`（5 provider × 4 version × 12 题）+ `life-series-benchmark/results/`（80 篇 × 14 标签）。

---

## 二点五、"本地代理"提供商类别（C4 的独立化）

**为什么 C4 必须独立于 broker**：

LLM2Jev（133 stars，活跃维护，2026-09-22 新增多模态）已经暴露 **`POST /v1/systemone`** HTTP API——完全兼容 Jev 协议。这意味着：

- LLM2Jev **不是** LLM→Jev 转接库（jeva-life broker 那种）
- LLM2Jev **是** 独立的 Jev-shaped HTTP 服务（与 TypeSafe 云、Laya 本地同接口）

**Jev-Switch 的处理**：

```rust
// C4 是 systemone kind（与 C1/C5 同协议），不需要 broker
Upstream::new_systemone("http://127.0.0.1:8000", "llm2jev");

// 用户启动 LLM2Jev:
//   python examples/sglang_inference.py --model-path /path/to/model
// Jev-Switch 通过 standard /v1/systemone 调用，不需要特殊 capability 适配。
```

**桥接模式**（Jev-Switch 的核心价值之一）：

Jev-Switch 不只调用 C4，还可以**自身代理** C1/C2 的响应给 C4 调用方——这是 v1 DeepSeek 计划书没明确但关键的"桥接"能力：

```
                       Jev-Switch (127.0.0.1:11435)
                              │
   ┌──────────────────────────┼──────────────────────────┐
   │                          │                          │
   ▼                          ▼                          ▼
上游 (C1-C5)              上游 (C1-C5)               上游 (C1-C5)
TypeSafe                  LLM2Jev 本地              sys1 本地
Vercel                    LM Studio                Laya

调用方 (Agent / 应用)
   │
   ▼
Jev-Switch 入口 ──── 任一上游 ──── 用户不用关心后端
```

**桥接场景**：

1. **Agent A 想用 Jev，但只认 TypeSafe** → Jev-Switch 把 TypeSafe 的回包原样透传
2. **Agent A 想用 Laya 但只懂 TypeSafe 协议** → Jev-Switch 把 Laya 当 C5 透明调用
3. **Agent A 在不同任务用不同上游** → Jev-Switch 按路由策略（failover / round-robin / latency）调度
4. **LLM2Jev 升级不影响 Agent** → Jev-Switch 把 LLM2Jev 当 C4 调用，LLM2Jev 内部升级时 Agent 无感

**这与 LM Studio / CC Switch 在 LLM 生态的角色一致**——它们也是"协议入口 + 多上游切换"，Jev-Switch 在 Jev 生态做同样的事。

---

## 三、青出于蓝的 5 个新增能力（jev-life 没解决）

jev-life 的 broker 设计**非常成熟**（5 类 backend × 2 套协议 × 三态 effort × 工具循环），但它的设计目标不是"Jev 协议多上游网关"。Jev-Switch 必须解决 jev-life 没解决的 5 个问题：

### 3.1 多上游运行时切换（jev-life 是单 backend）

**jev-life 现状**：一个 `DecisionBackend` 绑一个 cfg（base + key），运行时通过 `BACKENDS` 表切换。

**Jev-Switch 升级**：在 broker 之上加一层 **Router**：

```
DecisionRequest → Router → [Primary, Secondary, ...] → DecisionBackend → Backend → upstream
                  ↓ 失败时
                  fallback chain 触发，自动选下一个上游
```

Router 抽象：

```rust
trait Upstream {
    async fn evaluate(&self, req: &DecisionRequest) -> Result<DecisionResult, JevError>;
    fn kind(&self) -> &'static str;          // "systemone" | "llm-json" | ...
    fn capabilities(&self) -> Capabilities;   // choice/score/noul + reasoning_effort 支持范围
    fn health(&self) -> HealthStatus;       // 实时健康：rate-limit、last-error、circuit-breaker
}

struct RouterConfig {
    strategy: RoundRobin | WeightedRoundRobin | FailoverChain | LatencyBased,
    fallbacks: Vec<UpstreamId>,            // 失败时按顺序尝试
    circuit_breaker: CircuitBreakerConfig, // 触发熔断的条件（连续 N 次 5xx → 短路 N 秒）
}
```

### 3.2 可观测性（jev-life 仅 `latencyMs/upstreamCalls/costUsd`）

**Jev-Switch 升级**：内置 OpenTelemetry：

```
traceparent: 00-{trace_id}-{span_id}-01
  ├─ span: router.select_upstream
  ├─ span: upstream.evaluate
  │   ├─ event: request.body (redacted by config)
  │   ├─ event: response.status
  │   ├─ event: response.latency
  │   └─ event: error (if any)
  └─ span: fallback.triggered (if upstream failed)
```

每个请求输出：trace JSON 到 `~/.jev-switch/traces/{date}/{trace_id}.json`，可重放。

### 3.3 配置文件 + CLI（jev-life 是硬编码 BACKENDS 表）

**Jev-Switch 配置**：`~/.jev-switch/config.toml`（YAML 兼容）

```toml
# 默认 Providers
[providers.typesafe]
kind = "systemone"
base = "https://api.typesafe.ai/v1/systemone"
api_key_env = "TYPESAFE_API_KEY"
priority = 1

[providers.openrouter]
kind = "systemone"
base = "https://openrouter.ai/api/alpha/decisions"
api_key_env = "OPENROUTER_API_KEY"
priority = 2

[providers.vercel]
kind = "systemone"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key_env = "VERCEL_AI_GATEWAY_API_KEY"
priority = 3
noul_via_boolean = true   # 翻译层：type=noul → type=boolean

[providers.laya]
kind = "systemone"
base = "http://127.0.0.1:18765/v1/systemone"
priority = 4

# LLM-Jev 转接（broker 内嵌）
[providers.lm_studio_4b]
kind = "llm-json"
protocol = "openai"
base = "http://127.0.0.1:1234/v1/chat/completions"
model = "qwen/qwen3-4b-2507"
priority = 5

[providers.lm_studio_8b]
kind = "llm-json"
protocol = "openai"
base = "http://127.0.0.1:1234/v1/chat/completions"
model = "qwen/qwen3-8b"
priority = 6

# 路由策略
[routing]
strategy = "failover-chain"        # 失败时按 priority 顺序 fallback
fallbacks = ["typesafe", "openrouter", "vercel", "laya", "lm_studio_4b"]
circuit_breaker = { max_failures = 3, cooldown_s = 60 }

# 可观测性
[config.observability]
trace_dir = "~/.jev-switch/traces"
metrics_port = 9090               # Prometheus 端点
```

**CLI**：

```bash
jev-switch up typesafe              # 切到 TypeSafe
jev-switch up --interactive         # TUI（ratatui）选上游
jev-switch status                   # 健康仪表
jev-switch trace <id>               # 重放一次请求
jev-switch bench                    # 跑内置 benchmark（80 篇 life-series）
```

### 3.4 真 Jev 协议 provider + 翻译层（jev-life 只有 proxy）

**关键发现**：C2 (Vercel) 的 `type: noul` 实际上**不是 noul**——是 `boolean`（Jev 协议 spec 三个 type 是 `choice/score/boolean`，**没有 noul**）。但 jev-life broker 的 `fromLlmContent` 按 question 的 type 推断（`upstreamTypeFor`），所以**避开**了这个差异。

Jev-Switch 不能这么懒——用户可能调任一上游，要做明确翻译：

```rust
// vercel-specific translation layer
fn vercel_to_jev(vercel_response: VercelResponse, questions: &Questions) -> JevResponse {
    // vercel 用 boolean 表达 "noul true/false"
    // 我们映射回 noul: 0.95 if true else 0.05
    // 然后 confidence 字段缺失：从 probabilities 算 max prob 当 confidence
    // 实测依据：本仓库 life-series-benchmark
}
```

### 3.5 本地 daemon + 跨平台（jev-life 是 Vercel serverless）

**Jev-Switch 形态**：

| 阶段 | 形态 | 用户场景 |
|---|---|---|
| M1 | **本地 daemon + 命令行** | `jev-switch` 起服务监听 11435；用户用 curl / 应用调 |
| M2 | + Web 控制面板（React + Vite） | `http://127.0.0.1:8766` 控制面板，可视化切换、查 trace |
| M3 | + **Tauri 桌面应用**（参考 cc-switch） | 跨平台：Windows / macOS / Linux；系统托盘一键切换 |

**与 LM Studio / CC Switch 对位**：

| 项目 | 形态 | 核心区别 |
|---|---|---|
| LM Studio | 本地 .app / .exe | OpenAI 兼容 API；不是 Jev |
| CC Switch | Tauri 桌面 | LLM API 配置管理；不是 Jev |
| **Jev-Switch** | 本地 daemon → Tauri | **Jev 多上游 + 本地决策 + 可观测** |

---

## 四、技术栈迁移：TS → Rust + React + (Tauri)

### 4.1 为什么从 TS 迁 Rust

| 选项 | 论据 |
|---|---|
| **Rust + axum（首选）** | sys1 已证明可行；单二进制分发；启动快（< 50ms）；适合 daemon |
| TS / Node（保留 TS 部分） | jev-life / jev-decision-lab 已写；快速迭代；但 npm 依赖分发麻烦 |
| Go | jev-accounts-hub 模式；中庸 |

**采用 Rust + axum + React 前端 + Tauri 后续**。

**TS 代码保留作为参考与原型**（在 jev-decision-lab / life-series-benchmark/），Rust 代码在 `rs/` 目录新建。

### 4.2 项目结构（Rust）

```
rs/
├── Cargo.toml
├── src/
│   ├── main.rs                       # daemon 入口 + CLI 解析
│   ├── lib.rs                        # Jev-Switch 库
│   ├── upstream/
│   │   ├── mod.rs                    # Upstream trait
│   │   ├── systemone.rs              # C1, C2 系统（Jev-shaped）
│   │   ├── llm_json.rs               # C3 LLM broker (JSON 路径)
│   │   ├── llm_tool.rs               # C3 LLM broker (tool 路径)
│   │   ├── prefill_only.rs           # C4 prefill-only 适配
│   │   └── capabilities.rs            # capability 矩阵（按协议族索引）
│   ├── router/
│   │   ├── mod.rs                    # Router + 策略
│   │   ├── round_robin.rs
│   │   ├── weighted.rs
│   │   ├── failover_chain.rs         # 默认
│   │   ├── latency_based.rs
│   │   └── circuit_breaker.rs
│   ├── broker/
│   │   ├── mod.rs                    # 借鉴 jev-life broker 纯函数设计
│   │   ├── openai.rs                 # OpenAI 协议族
│   │   ├── anthropic.rs              # Anthropic 协议族
│   │   ├── prompt.rs                 # 系统提示词模板
│   │   ├── effort.rs                 # 三态 effort + capability 收敛
│   │   ├── tool_loop.rs              # 工具循环（含零进展检测）
│   │   └── errors.rs                 # BrokerError 区分空 content / 预算烧光
│   ├── observability/
│   │   ├── mod.rs
│   │   ├── trace.rs                  # OpenTelemetry trace span
│   │   ├── metric.rs                 # Prometheus metrics
│   │   └── log.rs                    # 结构化 log
│   ├── config/
│   │   ├── mod.rs                    # config.toml 解析
│   │   └── secret.rs                 # API key 从 env 读，绝不落盘
│   ├── http/
│   │   ├── mod.rs                    # axum 路由
│   │   ├── handler.rs                # POST /v1/systemone
│   │   ├── health.rs                 # GET /health, /ready, /metrics
│   │   └── admin.rs                  # GET /admin/providers, POST /admin/up
│   └── compat/
│       ├── mod.rs                    # Vercel evaluation-model 翻译
│       └── noul.rs                   # boolean→noul 0.95/0.05
├── tests/                            # 单元 + 集成
│   ├── broker/
│   ├── router/
│   └── upstream/
└── examples/                          # 示例配置 + 启动脚本
    ├── config.toml
    └── bench_life_series.rs          # 跑 life-series-benchmark (80 篇)

ui/                                  # React 前端（Vite + shadcn/ui）
├── package.json
├── src/
│   ├── App.tsx                       # 主面板
│   ├── ProvidersPanel.tsx
│   ├── TraceViewer.tsx
│   └── MetricsPanel.tsx
└── ...

tauri-app/                            # M3：Tauri 桌面应用
└── (scaffold from tauri init)
```

### 4.3 Cargo.toml 关键依赖

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "stream"] }
hyper = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
opentelemetry = "0.24"
opentelemetry-otlp = "0.17"
prometheus = "0.13"
config = "0.14"             # toml 解析
clap = { version = "4", features = ["derive"] }  # CLI
anyhow = "1"
thiserror = "1"

[dev-dependencies]
wiremock = "0.6"            # mock HTTP 上游
proptest = "1"              # broker 模糊测试
```

---

## 五、MVP 切分（M1-M7）

| 编号 | 任务 | 时间预算 | 依赖 |
|---|---|---|---|
| **M1** | 5 类 Upstream trait + 5 个具体实现（C1-C5 各一个）+ capability 矩阵 | 2 周 | - |
| **M2** | Router + FailoverChain + CircuitBreaker | 1 周 | M1 |
| **M3** | Broker 纯函数（OpenAI/Anthropic 两协议族 + 三态 effort） | 1.5 周 | M1 |
| **M4** | 可观测性（OpenTelemetry trace + Prometheus metric） | 1 周 | M1-M3 |
| **M5** | config.toml + CLI（up/status/trace/bench） | 1 周 | M1-M3 |
| **M6** | React 前端（Providers / Trace Viewer / Metrics 三页） | 2 周 | M4-M5 |
| **M7** | Tauri 桌面应用打包（参考 cc-switch） | 1 周 | M6 |

**总计**：~10 周（单人）；含独立运行线与可中断。

---

## 七、给后续 Agent 的可审阅点

1. **5 类上游的协议兼容矩阵**：本仓库 `runs/integrated-benchmark-data.json` 是 C2-C5 的实测数据；C1 (TypeSafe) 未实测（waitlist），假设全支持。
2. **Vercel `noul→boolean` 翻译层**：实测见 `life-series-benchmark/`；0/42 partial 准确率是基于 human baseline 的，待 Vercel 100 篇完整数据后更新。
3. **broker 借鉴 jev-life**：`/c/Users/56506/AppData/Local/Temp/jev-life/src/shared/llm-broker.ts` 是参考实现——但只取纯函数（`toLlmRequest` / `fromLlmContent` / `extractContent` / `extractToolCalls`），HTTP 层自己写。
4. **Laya 的 prefill-only 等价物**：Laya 是真 Jev 模型（C5），不需要 broker；prefill-only 是 C4（用 logits 概率）——这两类不要混。
5. **trace 不应包含原文**：借鉴 jev-decision-lab `life-series-benchmark/articles.json` gitignore 的经验；trace JSON 里 title/excerpt 要按配置 redact。

---

## 八、对 DeepSeek 原始计划书的修正

| 章节 | v1 原文 | v2 修正 |
|---|---|---|
| 3.2 假设 B | "你不做，短期内不会有别人做" | **已不成立**：LLM2Jev 133 stars、sys1 已有、cc-switch 已覆盖 LLM 路由。Jev-Switch 差异化是"Jev 协议多上游"，但 LLM→Jev 已被 LLM2Jev 解决 |
| 3.3 7 天前置实验 | 发 Discussion 验证需求 | **建议保留但加 pivot**：本次实测 + jev-life broker 调研已提供"事后证据"。可以**先发 Discussion 同时启动 MVP** |
| 4.1 M1 技术栈 | TS/Node 或 Rust 二选一 | **确定 Rust + axum**（借鉴 sys1） |
| 4.1 M3 | "Laya 英文 checkpoint 不支持中文" | **不准确**：实测 Laya 在中文上 17.5%（14 选 1），有降级但不为 0 |
| 4.2 M4 LLM→Jev | "需要时可以配置" | **不延后**：是 5 类上游之一 |
| 5.2 | "TS 优先快速迭代" | **改为 Rust 优先**：sys1 已证明可行，单二进制分发对 daemon 形态至关重要 |

---

## 九、当前状态（2026-09-22）

- **仓库**：`H:\A137442\Develop\AI\Jev\jev-decision-lab`（GitHub: `ARCJ137442/jev-decision-lab`，已 7 commits）
- **已交付**：
  - `ts/decision-bricks.ts`：4 Runtime（local-qwen / openrouter / vercel / laya）原型
  - `life-series-benchmark/`：5 provider × 14 标签 benchmark 脚本 + HTML 报告
  - `runs/`：完整 5×4×12 高考题 benchmark 数据 + 可视化报告
  - `docs/01-Jev-Switch-原始计划书-DeepSeek.md`：DeepSeek 原始计划书
- **下一步**：
  1. **初始化 Rust 项目骨架**（`rs/` + Cargo.toml）
  2. **5 类 Upstream trait 设计文档**（`docs/03-上游类别与协议兼容矩阵.md`）
  3. **借鉴 jev-life broker 的纯函数 Rust 实现**（`rs/src/broker/`）

---

## 十、参考链接

- **本仓库**：[github.com/ARCJ137442/jev-decision-lab](https://github.com/ARCJ137442/jev-decision-lab)
- **jev-life broker 参考**：[github.com/ARCJ137442/jev-life/src/shared/llm-broker.ts](https://github.com/ARCJ137442/jev-life)
- **sys1 (Rust 单后端)**：[github.com/alvarobartt/sys1](https://github.com/alvarobartt/sys1)
- **LLM2Jev (prefill-only)**：[github.com/Yinsongxu/LLM2Jev](https://github.com/Yinsongxu/LLM2Jev)
- **jev-accounts-hub (Go 多账户)**：[github.com/antTing/jev-accounts-hub](https://github.com/antTing/jev-accounts-hub)
- **TypeSafe API**：[docs.typesafe.ai/api](https://docs.typesafe.ai/api)
- **CC Switch (Tauri LLM)**：[github.com/farion1231/cc-switch](https://github.com/farion1231/cc-switch)
- **awesome-jev**：[github.com/heyjunpenn/awesome-jev](https://github.com/heyjunpenn/awesome-jev)

---

**接下来要做的事**（按 ROI）：

1. 把 `ts/decision-bricks.ts` 的 4 Runtime 设计移植到 Rust trait（保留 jev-decision-lab 的协议兼容矩阵作为参考）
2. 写 `docs/03-上游类别与协议兼容矩阵.md`（详细 capability 表 + 实测数据）
3. 写 `docs/04-架构设计-from-sys1-借鉴.md`（Rust 后端 + React 前端的具体设计）
4. 写 `docs/05-从-jev-life-超越的设计点.md`（青出于蓝的 5 个具体技术点）

每个文档都可独立由其他 Agent 审阅。