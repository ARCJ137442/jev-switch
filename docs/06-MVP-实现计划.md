# Jev-Switch MVP 实现计划 (M0)

**版本**: v1 (2026-09-23)
**前置文档**:
- [`02-Jev-Switch-新版计划书.md`](./02-Jev-Switch-新版计划书.md) v2.1 — 5 类上游 + 桥接模式
- [`03-上游类别与协议兼容矩阵.md`](./03-上游类别与协议兼容矩阵.md) — capability 表 + Vercel noul→boolean 翻译
- [`04-架构设计-from-sys1-借鉴.md`](./04-架构设计-from-sys1-借鉴.md) — Rust 后端 + workspace 三 crate + 13 条 axum 路由
- [`05-从-jev-life-超越的设计点.md`](./05-从-jev-life-超越的设计点.md) — 5 个能力 + 借鉴 vs 复制边界
- 实测依据: `../jev-decision-lab/ts/decision-bricks.ts` (4 Runtime 已 work) + `../jev-decision-lab/runs/integrated-benchmark-data.json` (5 provider × 4 version × 12 题)

**目标读者**: 主 Agent 通过 `/do-skill` 机械执行本文档。每一项必须**单一动作**且**可验证**。

---

## 〇、MVP 的"做到"标准（不是漂亮的，是真的）

下面 4 条由用户明确指定，必须**端到端跑通**：

1. **真实可做到路由的 Vercel 网关** — `POST http://127.0.0.1:8765/v1/systemone` → 转发到 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`
2. **真实可做到路由的本地 Laya 网关** — 同上路径 + `model: "laya-english"` → 转发到 `http://127.0.0.1:18765/v1/systemone`
3. **调用方无需适配格式** — 调用方只发 Jev 标准的 `type: noul`（Vercel 实际是 `boolean`），网关自动翻译
4. **调用方只需切换 `model` 字段** — `model: "laya-english"` 走 Laya；`model: "typesafe-ai/jev"` 走 Vercel。**调用方不知道也不需要知道目标上游的协议差异**

**端到端 smoke 测试通过 = MVP 完成**。

---

## 一、MVP 范围（IN / OUT）

### IN（M0 必做）

| 模块 | 范围 | 复用 |
|---|---|---|
| **M0.1** Rust workspace 骨架 | 单 crate `rs/` (简化: 不拆 workspace) + axum + tokio | 借鉴 sys1 `src/main.rs` |
| **M0.2** Protocol types | `SystemOneRequest` / `SystemOneResponse` / `DecisionQuestion` (含 `Boolean` 变体) | 04- §1.1 |
| **M0.3** `Upstream` trait | `async fn evaluate(&self, req: SystemOneRequest) -> Result<SystemOneResponse, JevError>` | 04- §2.4 |
| **M0.4** Capability table | 2 行表 (C2 Vercel + C5 Laya) | 03- §2.3 |
| **M0.5** 2 个 Upstream 实现 | C2 Vercel + C5 Laya | 直接对应 jev-decision-lab `createVercelGatewayRuntime` + `createLayaRuntime` |
| **M0.6** 简单 Router | 静态 `model → upstream` 映射（来自 config.toml），capability 不匹配直接 422 | 04- §2.4 |
| **M0.7** Vercel noul↔boolean 翻译层 | 03- §2.4 完整代码 | 03- §2.4 |
| **M0.8** Config.toml 加载 | `providers.toml` 含 2 行 provider + `[router] mapping` 段 | 04- §2.6 |
| **M0.9** axum HTTP server | `POST /v1/systemone` + `GET /health` + `GET /v1/models` (返回 provider 列表) | sys1 模式 |
| **M0.10** React UI (极简) | 单页: provider enable/disable + model 列表 + "Test" 按钮 | 04- §2.7 简化版 |
| **M0.11** 端到端 smoke | 见 §三.验收 | — |

### OUT（M0 不做）

- 其他 5 类上游 (C1 TypeSafe 云 / C2 OpenRouter / C2 LM Studio / C2 sys1 / C3 broker / C4 LLM2Jev)
- OTel / Prometheus / 完整 trace (M4)
- CLI (M5)
- Tauri (M7)
- CircuitBreaker / 复杂路由策略 (M2-M3 高级部分)
- capability 探测 / health check 自动摘除
- 多协议 (CORS 浏览器场景) — MVP 仅 localhost
- trace redact / 隐私配置

**理由**: MVP 验证"切换模型即可"这条核心 demo 价值。功能完整性留给 M1-M7。

---

## 二、文件清单（按主 Agent 执行顺序）

所有路径相对 `H:\A137442\Develop\AI\Jev\Jev-Switch\rs\`。

### 阶段 1: 项目脚手架 (M0.1)

```
rs/
├── Cargo.toml                              ← 新建
├── .gitignore                              ← 新建 (target/)
└── src/
    └── main.rs                             ← 新建 ("hello world" axum server on 8765)
```

**`Cargo.toml` 关键依赖**（参考 04- §2.3）:

```toml
[package]
name = "jev-switch"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
toml = "0.8"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**`main.rs` 目标**:
- 监听 `127.0.0.1:8765`
- `GET /health` → `200 OK` "jev-switch MVP"
- `cargo run` 启动后 curl 验证

**验收**:
- `cargo build` 通过（无 warning）
- `curl http://127.0.0.1:8765/health` 返回 `200 OK jev-switch MVP`

### 阶段 2: Protocol types (M0.2)

**新建 `src/protocol.rs`** — 完整代码在 04- §1.1 末尾。**关键决策**:
- `DecisionQuestion` 用 `#[serde(tag = "type", rename_all = "lowercase")]` 枚举
- 4 个变体: `Choice` / `Score` / `Noul` / `Boolean`（Vercel 适配）
- 复用 sys1 的 serde derive 模式

**验收**:
- `cargo test` 一个 unit test：构造 Jev 标准 request → serde_json round-trip
- 构造 Vercel `boolean` request → deserialize 成功，`type == "boolean"`

### 阶段 3: Upstream trait + Capability (M0.3 + M0.4)

**新建 `src/upstream.rs`**:

```rust
use crate::protocol::SystemOneRequest;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum JevError {
    #[error("upstream returned {status}: {body}")] Upstream { status: u16, body: String },
    #[error("upstream timeout")] Timeout,
    #[error("network error: {0}")] Network(String),
    #[error("capability mismatch: {0}")] Capability(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionType { Choice, Score, Noul, Boolean }

pub struct Capabilities {
    pub question_types: &'static [QuestionType],
    pub has_confidence: bool,
    pub noul_via_boolean: bool,  // true 表示 noul 问题须翻译为 boolean
}

pub trait Upstream: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn evaluate(&self, req: SystemOneRequest) -> Result<Value, JevError>;
}
```

**Capability 表 (M0.4)** — 与 03- §2.3 对齐:

```rust
pub fn capabilities_of(id: &str) -> Capabilities {
    match id {
        "vercel" => Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Boolean],
            has_confidence: false,
            noul_via_boolean: true,
        },
        "laya" => Capabilities {
            question_types: &[QuestionType::Choice, QuestionType::Score, QuestionType::Noul],
            has_confidence: true,
            noul_via_boolean: false,
        },
        _ => panic!("unknown upstream id: {}", id),
    }
}
```

**验收**:
- `cargo test` unit: 验证 2 个 capability 表项正确

### 阶段 4: 2 个 Upstream 实现 (M0.5)

**新建 `src/upstream_vercel.rs`** + **`src/upstream_laya.rs`** + **`src/upstream/mod.rs`**

**Vercel 实现要点** (M0.7 翻译层在这里):
1. `req.questions` 遍历，遇到 `Noul` 翻译为 `Boolean` (改 `type`，保留 `criteria`)
2. POST 到 `https://ai-gateway.vercel.sh/v4/ai/evaluation-model`
3. Headers: `Authorization: Bearer <AI_GATEWAY_API_KEY>` + 4 个 `ai-*` header
4. Body: `{"state": ..., "questions": ..., "model": "typesafe-ai/jev"}` (model hardcode)
5. Response `boolean` answer → 翻译回 `noul` (true → 0.95, false → 0.05)，补 `confidence = max(probabilities)`
6. 注: Vercel 不报 `usage`，写 `null` (借鉴 jev-decision-lab `null != 0` 原则)

**Laya 实现要点**:
1. POST 到 `http://127.0.0.1:18765/v1/systemone` (基地址从 config 读)
2. Headers: `Content-Type: application/json`
3. Body: 原样转发（不翻译）
4. Response: 原样返回

**两个文件都包含**:
- `pub struct VercelUpstream { base, api_key, http }`
- `pub struct LayaUpstream { base, http }`
- `#[async_trait::async_trait] impl Upstream for VercelUpstream` — **注意**: 不用 async_trait，直接用 `async fn` in trait (Rust 1.75+)

**API key 来源**: `Upstream::new_vercel(api_key: String)`，从 config 读 env 变量
**HTTP client**: 共用一个 `reqwest::Client` (在 `AppState` 里)

**验收**:
- `cargo build` 通过
- unit test mock 一个 `httpmock` server，验证 VercelUpstream 翻译层把 `noul` → `boolean`，并把 response `boolean` → `noul`
- unit test mock 验证 LayaUpstream 原样透传

### 阶段 5: Router (M0.6)

**新建 `src/router.rs`**:

```rust
use std::collections::HashMap;
use crate::upstream::Upstream;

pub struct Router {
    /// model id → upstream id
    mapping: HashMap<String, String>,
    upstreams: HashMap<String, Box<dyn Upstream>>,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("no upstream registered for model {0}")] UnknownModel(String),
    #[error("upstream {0} not registered")] UnknownUpstream(String),
}

impl Router {
    pub fn new(mapping: HashMap<String, String>, upstreams: HashMap<String, Box<dyn Upstream>>) -> Self { ... }
    pub fn route(&self, model: &str) -> Result<&dyn Upstream, RouterError> {
        let upstream_id = self.mapping.get(model).ok_or_else(|| RouterError::UnknownModel(model.to_string()))?;
        self.upstreams.get(upstream_id).map(|b| b.as_ref()).ok_or_else(|| RouterError::UnknownUpstream(upstream_id.clone()))
    }
    pub fn list_models(&self) -> Vec<String> { self.mapping.keys().cloned().collect() }
}
```

**验收**:
- unit test: `Router::route("laya-english")` 返回 Laya
- unit test: `Router::route("typesafe-ai/jev")` 返回 Vercel
- unit test: `Router::route("unknown")` 报错

### 阶段 6: Config (M0.8)

**新建 `src/config.rs`** + `config.example.toml`

**`config.example.toml` 内容**:

```toml
[providers.vercel]
kind = "vercel"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key_env = "AI_GATEWAY_API_KEY"
enabled = true

[providers.laya]
kind = "laya"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true

[router]
"laya-english" = "laya"
"typesafe-ai/jev" = "vercel"
```

**`src/config.rs`**:
- `pub struct Config { providers: HashMap<String, ProviderConfig>, router: RouterConfig }`
- `impl Config { pub fn load(path: &Path) -> Result<Self> }`
- `ProviderConfig::enabled: bool` — 加载时若 `false` 跳过
- API key 从 `api_key_env` 读 env 变量

**验收**:
- 加载 `config.example.toml` 成功
- unit test: 缺 env 变量时返回明确错误

### 阶段 7: HTTP server (M0.9)

**重写 `src/main.rs`** 整合所有模块:

```rust
use axum::{routing::{get, post}, Router, Json, extract::State};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    router: Arc<Router>,
    config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载 config
    let config = Config::load(&std::path::Path::new("config.example.toml"))?;
    tracing_subscriber::fmt::init();
    
    // 2. 构造 2 个 upstream
    let mut upstreams: HashMap<String, Box<dyn Upstream>> = HashMap::new();
    if let Some(p) = config.providers.get("vercel") {
        if p.enabled {
            upstreams.insert("vercel".to_string(), Box::new(VercelUpstream::new(p.base.clone(), p.read_api_key()?)?));
        }
    }
    if let Some(p) = config.providers.get("laya") {
        if p.enabled {
            upstreams.insert("laya".to_string(), Box::new(LayaUpstream::new(p.base.clone())?));
        }
    }
    
    // 3. 构造 Router
    let router = Router::new(config.router.mapping.clone(), upstreams)?;
    
    // 4. axum routes
    let state = AppState { router: Arc::new(router), config: Arc::new(config) };
    let app = Router::new()
        .route("/v1/systemone", post(evaluate_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(models_handler))
        .with_state(state);
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8765").await?;
    tracing::info!("jev-switch MVP listening on http://127.0.0.1:8765");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn evaluate_handler(State(state): State<AppState>, Json(req): Json<SystemOneRequest>) -> Result<Json<SystemOneResponse>, JevError> {
    let upstream = state.router.route(&req.model).map_err(|e| JevError::Capability(e.to_string()))?;
    let resp = upstream.evaluate(req).await?;
    Ok(Json(serde_json::from_value(resp).map_err(|e| JevError::Network(e.to_string()))?))
}
```

**验收**:
- `cargo run` 启动后日志显示 listening on 8765
- `curl http://127.0.0.1:8765/health` → 200
- `curl http://127.0.0.1:8765/v1/models` → JSON list of ["laya-english", "typesafe-ai/jev"]

### 阶段 8: React UI (M0.10)

**新建 `ui/` (Vite + React + TypeScript + shadcn/ui 简化)**:

```
ui/
├── package.json
├── vite.config.ts
├── index.html
└── src/
    ├── main.tsx
    ├── App.tsx
    └── api.ts
```

**`App.tsx` 极简设计**:
- 顶部下拉框: model 选择 (从 `/v1/models` 拉)
- 中间: provider 状态卡片 (Vercel ✅/❌ / Laya ✅/❌) + 启用/禁用开关
- 底部: "Test with sample noul" 按钮 → 调 `/v1/systemone` 显示响应

**API client (`api.ts`)**:
- `GET /v1/models` → 拉可用 model
- `GET /health` → provider 健康
- `POST /v1/systemone` → 测试请求

**验收**:
- `npm run dev` 启动后浏览器打开 `http://localhost:5173`
- 看到 2 个 provider 卡片
- 切换 model 下拉框 → 点击 Test 按钮 → 看到响应

### 阶段 9: 端到端 smoke (M0.11)

**新建 `tests/smoke.sh`**:

```bash
#!/bin/bash
# 前提: Laya 服务在 18765 (python jev_laya_server.py), Vercel API key 已 export

set -e

# 1. 启动 jev-switch
cargo run --release &
SWITCH_PID=$!
sleep 3

# 2. health
curl -sf http://127.0.0.1:8765/health
echo ""

# 3. 路由到 Laya
echo "--- Laya ---"
curl -sf -X POST http://127.0.0.1:8765/v1/systemone \
  -H "Content-Type: application/json" \
  -d '{"model":"laya-english","state":{"test":true},"questions":{"q":{"type":"noul","instructions":"is this a test?","criteria":{"true":"yes","false":"no"}}}}' | jq .

# 4. 路由到 Vercel
echo "--- Vercel ---"
curl -sf -X POST http://127.0.0.1:8765/v1/systemone \
  -H "Content-Type: application/json" \
  -d '{"model":"typesafe-ai/jev","state":{"test":true},"questions":{"q":{"type":"noul","instructions":"is this a test?","criteria":{"true":"yes","false":"no"}}}}' | jq .

kill $SWITCH_PID
```

**验收**:
- `bash tests/smoke.sh` 跑通
- Laya 响应: `noul: 0.xx` (0-1 概率)
- Vercel 响应: `noul: 0.xx` (翻译自 `boolean: true/false`，**不是 0/1 也不带 confidence 缺失字段**)
- 两次响应**调用方发送的 payload 一模一样**

---

## 三、验收 (Definition of Done)

| # | 标准 | 验证方式 |
|---|---|---|
| **D1** | `cargo build --release` 通过，无 warning | 终端 |
| **D2** | `cargo test` 全部通过 | 终端 |
| **D3** | `cargo run` 启动后 `curl /health` 返回 200 | 终端 |
| **D4** | `curl /v1/models` 返回 `["laya-english", "typesafe-ai/jev"]` | 终端 |
| **D5** | `bash tests/smoke.sh` 跑通 | 终端 |
| **D6** | Laya 响应含 `noul: 0.xx` (0-1) | smoke 输出 |
| **D7** | Vercel 响应含 `noul: 0.xx` (翻译自 boolean) | smoke 输出 |
| **D8** | 调用方两次 POST 的 payload 字段**完全相同** | smoke 输出 diff |
| **D9** | `npm run dev` 启动 React UI，浏览器可见 2 provider 卡片 | 浏览器 |
| **D10** | UI 点 Test 按钮后看到响应 | 浏览器 |

**D1-D8 = 后端 MVP 完成**; D9-D10 = 前端 MVP 完成**; 全部通过 = MVP 整体完成。

---

## 四、风险与缓解

| 风险 | 缓解 |
|---|---|
| Vercel 持续 429 (rate limit) | smoke 脚本重试 3 次；如持续失败，记录为"已知问题"，不影响 MVP 整体验收 |
| Laya 服务未启动 (本机) | smoke 脚本里 `curl 18765/health` 预检；如不通，输出 warning 继续 Vercel 部分 |
| `reqwest::Client` 共享但 trait 不能 clone | 在 `AppState` 里放 `Arc<reqwest::Client>`；upstream 在 `evaluate` 内用 `client.clone()` 取 |
| `async fn in trait` 编译失败（Rust < 1.75） | Cargo.toml `rust-version = "1.75"` 显式要求；不达标时报错 |
| undici 依赖问题（已用 Rust 不涉及） | N/A |
| 翻译层漏字段（Vercel 缺 confidence） | `serde_json::Value` 在边界用，类型只在协议 crate 严格；UI 显示 "confidence: missing" |
| React UI 卡 `npm install` | 锁版本（pin Vite 5 + React 18）；提供 `package-lock.json` |
| Laya zero-shot 准确率低（实测 17.5%） | MVP **不**关心准确率，只关心"路由+翻译+接口一致"；准确率提升是 M1 之后 |

---

## 五、给主 Agent 的执行清单

主 Agent 使用 `/do-skill` 时按以下顺序执行（每步可独立验证）:

```
□ 阶段 1: 脚手架       (D1)
  → 创建 rs/Cargo.toml, src/main.rs, .gitignore
  → cargo build + curl /health
□ 阶段 2: protocol.rs   (D2 partial)
  → 添加 src/protocol.rs
  → cargo test 一个 round-trip 测试
□ 阶段 3: upstream.rs trait + capabilities_of  (D2 partial)
  → 添加 src/upstream.rs
  → cargo test capability 表
□ 阶段 4: vercel + laya  (D2)
  → 添加 src/upstream/vercel.rs (含 noul→boolean 翻译)
  → 添加 src/upstream/laya.rs
  → cargo test (用 wiremock mock HTTP)
□ 阶段 5: router.rs    (D2)
  → 添加 src/router.rs
  → cargo test 路由表
□ 阶段 6: config.rs    (D2)
  → 添加 src/config.rs + config.example.toml
  → cargo test
□ 阶段 7: main.rs 整合  (D3, D4)
  → 替换 main.rs 整合所有模块
  → cargo run + curl /health, /v1/models
□ 阶段 8: ui/         (D9)
  → 创建 ui/ Vite + React + TS
  → 写 App.tsx, api.ts
  → npm run dev + 浏览器验证
□ 阶段 9: smoke.sh    (D5, D6, D7, D8)
  → 创建 tests/smoke.sh
  → bash tests/smoke.sh 验证
□ 阶段 10: 写 README   (D10)
  → 创建 README.md 说明如何启动 MVP
  → 记录已知限制
□ 阶段 11: 提交
  → git init (rs/) + 首次 commit
  → 不推送到 GitHub（待 MVP 验证后再说）
```

---

## 六、附: 关键技术参考 (避免主 Agent 重新设计)

| 主题 | 参考位置 | 用途 |
|---|---|---|
| Jev 标准 protocol 类型 | [`04-架构设计-from-sys1-借鉴.md` §1.1](./04-架构设计-from-sys1-借鉴.md) | 直接移植 |
| Vercel 翻译层 | [`03-上游类别与协议兼容矩阵.md` §2.4](./03-上游类别与协议兼容矩阵.md) | 完整 Rust 代码 |
| Capability 表 (C2 Vercel + C5 Laya) | [`03-上游类别与协议兼容矩阵.md` §2.3](./03-上游类别与协议兼容矩阵.md) | 直接抄 |
| Laya / Vercel HTTP 客户端代码 | `../jev-decision-lab/ts/decision-bricks.ts` (TS) | 逐行翻译成 Rust (reqwest 语法) |
| 实测 5 provider 矩阵 | `../jev-decision-lab/runs/integrated-benchmark-data.json` | 验证 router 选路 (Vercel vs Laya) 后的响应 |

---

## 七、MVP 不做什么（明确划线）

| 不做 | 原因 |
|---|---|
| C1 TypeSafe 云 | waitlist 限制 |
| C2 OpenRouter | 与 C2 Vercel 同类，留 M1+ 拓展 |
| C3 broker (LLM-Jev 翻译) | M1+ 拓展 |
| C4 LLM2Jev / LM Studio / sys1 | M1+ 拓展；MVP 仅 C2 + C5 |
| OTel / Prometheus | M4 |
| CLI | M5 |
| Tauri | M7 |
| Circuit Breaker / 健康自动摘除 | 简化为手动 `enabled = false` |
| 多 Router 策略 (WeightedRoundRobin 等) | MVP 仅"model→upstream"静态映射 |
| Capability 探测 | MVP 硬编码 capability；不探测 |
| Trace 记录 (OpenTelemetry / 本地 trace JSON) | M4 |
| CORS / 浏览器直连 | MVP 仅 localhost 服务于 localhost |
| 复杂错误重试 (指数退避) | MVP 直接 fail-fast；M1+ 加重试 |
| LLM token 计量 / cost 估算 | M4 |

---

## 八、MVP 完成后的下一步

1. **M1 拓展上游类型**: 加 C1 TypeSafe + C2 OpenRouter + C4 LLM2Jev
2. **M2 Router 高级**: 加 WeightedRoundRobin + LatencyBased + Circuit Breaker
3. **M3 broker 抽象**: 把 jev-life `llm-broker.ts` 移植为 C3 Rust broker
4. **M4 可观测性**: OTel + Prometheus + trace JSON 兼容 `runs/integrated-benchmark-data.json`
5. **M5 CLI**: clap 子命令 up / status / trace / bench
6. **M6 React UI 完整版**: 4 页面 (Providers / Trace / Metrics / Settings)
7. **M7 Tauri**: 桌面应用打包

每步可独立 fork subAgent 推进。

---

**End of M0 plan. Main agent 接到此文件后用 /do-skill 顺序执行 §五 的 11 个阶段。**
