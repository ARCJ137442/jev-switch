# Jev-Switch 功能特性契约与施工计划

> **作者**：Mimo-V2.6-Pro（对齐会话定稿）  
> **日期**：2026-09-23  
> **上游评审**：`07-REVIEW-v0.1.0-mvp.md`（P0/P1/P2 清单仍然有效）  
> **状态**：**Phase 0 契约正文已完成**（见 `contracts/`）。本文件是前后端唯一共识；与 `07-` 冲突时以本文件「对齐定稿」为准，细则以 `contracts/` 正文为准。  
> **读者**：后续开发本仓库的 Agent。先读本文件，再读 `07-`，再动代码。

---

## 0. 对齐定稿（五问五答，已锁死）

| # | 议题 | 决定 | 含义 |
|---|---|---|---|
| **Q1** | serde 式扩展的真实机制 | **A · 编译期 trait 同构** | 新提供商 = 外部 crate 实现冻结 trait，编进 bin；**内核代码与函数传参零改**。为可扩展性做准备。暂不做运行时 dylib 插件（B） |
| **Q2** | 向 CC Switch 靠的边界 | **先 A+B，稳定后再 C** | 交互范式（卡片/开关/密钥表单/探测）**和**视觉都向 CC Switch 靠；**文件仍是真值源**。M7 前后端测试稳定后再做 Tauri 桌面形态（C） |
| **Q3** | 施工顺序 | **契约 → 并行 AB → 双剑合璧** | ① 本契约 ② 后端 A ∥ 前端 B ③ 合流联调。禁止无契约直接写码 |
| **Q4** | 密钥落盘 | **b · 本地明文 + UI 密文** | **本地优先，相信开发者**：密钥明文进 providers.toml + `.gitignore`；**UI 呈现必须密文**（掩码 / 末 4 位）。真实值不进界面、不进日志、不进上下文 |
| **Q5** | UI 与 toml 冲突 | **a · 以文件为准** | toml 是真值源；UI 检测 mtime/hash 外部变化 → 提示「重载 / 覆盖」，不静默丢手改 |
| **Q6** | 密钥防偷 | **强化档** | Q4 默认明文 toml 不变，但 UI/日志/API 一律密文；上游 key 不出 daemon；借鉴 CC Switch / sub2api / newAPI（详见 §2.4 与 07- P1-6） |

**硬约束（全程）**
- 写入仅限 `jev-switch/`；不得碰 `~/Rime/`、`jev-life/`、`jev-rime/`
- **不擅自 `git push`**（拉取/提交/推送须用户下令）
- 密钥真实值不进上下文 / 日志 / UI 明文
- P0 未完成前不加第 5 个上游

---

## 1. 路由形态定稿：**模型路由 DAG**

> **正名（用户定稿）**：叫 **「模型路由 DAG」** 更准确且可拓展；**二部图只是其中最简单的一种形式**。  
> 数据模型、Router API、序列化 **一律按 DAG**；二部图仅是第一版默认的查看/编辑视图。

| 层 | 定稿 |
|---|---|
| **语义/数据模型** | **模型路由 DAG**：有向边携带 `priority` / `upstream_model` / `sticky` / `on_error`；节点 = 对外 model id、可选中间别名、提供商/上游。**可多跳**（最简时退化为两层） |
| **最简形式** | **二部图**（对外 model id ∥ 提供商，单层边）—— 适合快速接线，**不等于**路由模型 |
| **序列化** | `providers.toml` 的 `[[routes]]`（见 §3）；与图形界面双向等价。字段设计须能自然扩展到多跳（例如允许 `left`/`right` 引用同一批节点 id，而不是写死「左=model、右=provider」两张表） |
| **默认 UI** | 第一版用 **二部图连线题** 做查看与交互；实践后若表达不动（前缀 / 多跳 / 边上参数）则**升级通用 DAG 视图**，数据模型不变 |
| **两套语义** | 不同模型 → 自动路由（exact/prefix 命中）；相同模型 → 可切换（一源多出边，按 priority / sticky / failover） |
| **引擎要求** | `Router::select` 与 admin API 从第一天按 DAG 实现；**禁止**在 core 里写死「只有左右两列」 |

**为什么以 DAG 为正名**：二部图表达不了「`jev` → 别名 `jev-fast` → vercel」这类中间层，也表达不了前缀折叠后的派生 id；先定 DAG 语义再套最简 UI，是 Q1「可扩展性优先」在路由层的同构要求。

## 2. 六份契约（阶段 0 交付物）→ **已写入 `docs/contracts/`**

> **Phase 0 已完成**：六份正文见 [`contracts/00-INDEX.md`](./contracts/00-INDEX.md)。下列清单已全部落入正文；实现阶段只勾「验收/实现」项。
> 覆盖对照：`01-协议`→2.1 · `02-扩展点`→2.2 · `03-路由`→2.3 · `04-密钥`→2.4 · `05-HTTP`→2.5 · `06-前端交互`→2.6。

### 2.1 协议契约（对齐 jev-life / TypeSafe）→ `contracts/01-协议契约.md`
- [ ] 内核形状以 TypeSafe `/v1/systemone` 为真值
- [ ] `criteria` **必填** + 三形态强类型（map / list / `{true,false}`）
- [ ] `Answer` 判别联合；**布尔答案无 `confidence`**
- [ ] 布尔双键：`noul` 与 `probability` 同时保留；读取顺序 `probability > probabilities["true"] > noul > boolean→0.95/0.05`
- [ ] `Score.score: f64`；choice/score 的 `probabilities`/`confidence` 必填
- [ ] `Usage` 强类型（`input_tokens`/`inputTokens` 双拼写 + `reasoning_tokens`）
- [ ] 补 `upstream_calls` / `latency_ms` / `cost_usd: Option<f64>`（**null ≠ 0**）
- [ ] 导出 `noul_probability()`
- [ ] 导出 JSON Schema 或 ts-rs 生成物，**UI 不得手写第二份类型**

### 2.2 扩展点契约（serde 同构 · Q1=A）→ `contracts/02-扩展点契约.md`
- [ ] 冻结 trait 签名（冻结后改签名 = 破坏性变更）：

```rust
pub trait ProtocolAdapter: Send + Sync {
    fn outgoing(&self, req: SystemOneRequest) -> SystemOneRequest { req }
    fn incoming(&self, raw: &[u8], ctx: &IncomingCtx) -> Result<SystemOneResponse, JevError>;
}
pub trait UpstreamAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn evaluate(&self, req: SystemOneRequest) -> Result<SystemOneResponse, JevError>;
}
```

- [ ] 厂商 DTO **只活在 adapter crate**，禁止进 `jev-protocol`
- [ ] 内核 API 永远是 `registry.register(Box<dyn UpstreamAdapter>)` + `invoke(&req)`，**传参永不因新厂商而改**
- [ ] **黄金测试**：外部 crate 自定义怪异字段（如 `{"verdict":"yes","odds":0.8}`）→ 实现两 trait → **零改内核**注册 → `POST /v1/systemone` 换 model 即命中
- [ ] **红线**：加提供商若需改 core 的 match / 函数传参 / 往 Response 塞厂商字段 → **设计失败，打回**

### 2.3 路由契约（模型路由 DAG · §1）→ `contracts/03-路由契约.md`
- [ ] `[[routes]]` 字段（DAG 边）：`left` / `match=exact|prefix` / `right` / `upstream_model` / `priority` / `sticky` / `on_error=next|fail`；命名可再开放 `from`/`to` 以支持多跳
- [ ] `Router::select(req, ctx) -> Vec<Candidate>`（有序候选，不是单 Upstream）
- [ ] 同模型多候选 failover；`upstream_calls` 如实计数
- [ ] 文件 ↔ DAG UI 双向等价（改任意一边，另一边一致）
- [ ] 参考：`sub2api` `CompositeModelRoute` + `SelectAccountForModelWithExclusions`

### 2.4 配置与密钥契约（Q4=b · Q5=a · Q6 防偷强化）→ `contracts/04-密钥与防偷.md`
- [ ] 真值源 = `~/.jev-switch/providers.toml`（或 `JEV_SWITCH_CONFIG`）
- [ ] 密钥默认档：**明文 toml + `.gitignore` + 文件权限 0600**（信任开发者）
- [ ] **UI 只出密文**：掩码或末 4 位；真实值禁止回传给前端、禁止进 `console`/tracing/错误体
- [ ] **上游 API key 只存在于 daemon**；浏览器拿到的只能是 `api_key_masked`；**无读回明文 API**
- [ ] 错误/日志统一 redact（上游错误体可能回显 key）
- [ ] 升级档（M2+，可配置）：`secrets.enc` 口令加密或 OS keyring；Tauri 阶段对齐 CC Switch 桌面密钥方案
- [ ] **调研借鉴**：CC Switch / sub2api / newAPI 的 token 存放与脱敏各 ≥1 条落地做法 → `docs/contracts/04-密钥与防偷.md`
- [ ] **已知可借鉴做法（Phase 0 验证后定稿）**：
  - *newAPI*：**上游 key（channel）与下发 key（issued）分离** —— 客户端只拿签发 key，上游 token 不出网关；另有 `SESSION_SECRET` / `CRYPTO_SECRET` 做会话与落盘加密。Jev-Switch 对应：**上游 API key 永不出 daemon**，浏览器只拿 masked；M2+ 加密档用独立 `CRYPTO_SECRET`。
  - *sub2api*：集中保管订阅 token + 代理转发 + 粘性会话。对应：调试/Playground 一律走 daemon 转发，禁止浏览器直连上游。
  - *CC Switch*：桌面本地配置文件 + 可选系统 keyring / 文件权限。对应：`providers.toml` 0600；Tauri 阶段对齐其 keyring。
  - 业界通用：**禁止「读回明文」API**、日志 redact、审计。
- [ ] UI 启动与保存前比对 toml mtime/hash；外部修改 → 横幅「配置已在外部修改」+ **重载 / 覆盖** 二选一（默认重载）
- [ ] 单一事实源：禁止 UI 内嵌第二份 `PROVIDERS` 常量当权威（展示也从 `/v1/models` 或 admin 配置 API 拉）

### 2.5 HTTP 契约（消灭 `37b4242` 类漂移）→ `contracts/05-HTTP契约.md`
- [ ] `POST /v1/systemone` → `SystemOneResponse`
- [ ] `GET /v1/models` → **定死一种**形状（建议 OpenAI 风 `object/data`，或 `{models,upstreams}`，二选一写进契约并生成 TS）
- [ ] `GET /health`
- [ ] 配置管理 API（新增，供 UI）：`GET/PUT /v1/admin/providers`、`GET/PUT /v1/admin/routes` —— 仅 localhost
- [ ] 错误体统一 `{error, upstream, retryable}`
- [ ] CORS：M1 起 origin 白名单，禁止长期 `very_permissive()`

### 2.6 前端交互契约（Q2=A+B · 走 `/frontend-design`）→ `contracts/06-前端交互契约.md`
- [ ] **强制**调用 `/frontend-design` 产出综合前端与交互设计；视觉与交互向 **CC Switch** 靠
- [ ] 信息架构三页：**Providers** / **Routing** / **Playground**
- [ ] Providers：CC Switch 式卡片 + 启停开关 + 密钥表单（**密文**）+ Probe 按钮 + 最近延迟/错误
- [ ] Routing：**模型路由 DAG** 管理；默认编辑器 = **二部图**（最简形式：左 model id、右 provider，连线带 priority/alias/sticky）；实践条款见 §1
- [ ] Playground：保留 jevplayground 黑白双栏 + `Run Jev ↗`，只微调不推翻
- [ ] 空态 / 错误态 / 加载态 / 外部修改横幅全量定义

---

## 3. 路由与配置数据模型（契约正文级草案）

```toml
# ── 右列：提供商（CC Switch 式卡片数据源；密钥明文 + gitignore）──
[providers.vercel]
kind = "vercel-gateway"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key = "…"                 # 明文；UI 只显示密文
enabled = true

[providers.laya]
kind = "systemone-native"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true

# ── 模型路由 DAG 边：对外 model id → 提供商（二部图 = 最简形式）──
# 图形界面与本段双向等价；字段可扩展到多跳别名
[[routes]]
left  = "jev"                 # 对外 model id
match = "exact"               # exact | prefix
right = "vercel"              # provider id
upstream_model = "typesafe-ai/jev"
priority = 10
sticky = "session"            # none | session
on_error = "next"             # next | fail

[[routes]]
left  = "jev"
match = "exact"
right = "laya"
upstream_model = "laya-english"
priority = 30

[[routes]]
left  = "local/*"
match = "prefix"
right = "laya"
priority = 40
```

**旧式 `[router] "model" = "upstream"` 保持兼容**，等价于单候选 `exact` 边。

---

## 4. 施工计划（Q3=契约 → 并行 AB → 合流）

```text
阶段 0  契约正文 docs/contracts/01–06       ✅ 2026-09-23 完成（Mimo-V2.6-Pro）
        ↓
阶段 1  并行 AB
        ├─ 后端 A：P0-1 workspace+lib → P0-2 probability bug → P0-3 协议
        │          → P1-1 DAG 路由 → P1-2/5 ProtocolAdapter+黄金测试 → P1-3 重试
        └─ 前端 B：/frontend-design → Providers 页 → Routing DAG/二部图 → Playground 微调
        ↓
阶段 2  双剑合璧：联调 + examples + tests + smoke
        ↓ 测试稳定
阶段 3  （后置）Q2=C：Tauri 桌面形态对齐 CC Switch 整体体验
```

### 后端 A 验收（摘自 `07-REVIEW` §7，仍然有效）
- P0-1：外部 `cargo add --path .../jev-protocol` 后 `use jev_protocol::SystemOneRequest` 可编译
- P0-2：`{"type":"boolean","probability":0.69}` → `noul≈0.69`（**不是** 0.95）
- P0-3：与 jev-life `types.ts` 可 diff，差异仅 Rust 命名
- P1-1：同一 `model: "jev"` 配双候选，停 Vercel 自动落 Laya
- P1-5：外部 adapter 零改内核（黄金测试）
- P1-3：wiremock 一次 429 一次 200 → 客户端 200 且 `upstream_calls==2`

### 前端 B 验收
- 提供商卡片可启停 / 粘贴密钥（密文回显）/ Probe 变红变绿
- 拖线 → toml 出现 `[[routes]]` → `POST` 路由正确；删线立即失效
- toml 外部修改 → UI 横幅 → 重载后图与列表一致
- `/frontend-design` 交付物归档进 `docs/design/`

---

## 5. 成熟度目标

| 里程碑 | 定义 |
|---|---|
| `v0.1.0-mvp` | 已封存（demo 合格） |
| `v0.2.0-contract` | **已达成**：六份契约正文合入 `docs/contracts/` |
| `v0.3.0-lib` | P0-1/2/3 完成，可被 path-dep |
| `v0.4.0-router` | **模型路由 DAG** + Providers UI + 黄金测试通过 |
| `v0.5.0-stable` | 合流联调、测试稳定 → 具备做 C（Tauri）的前置 |

---

## 6. 与生态的关系（不变）

- 协议真值：TypeSafe + `jev-life/src/shared/types.ts`
- 路由/调度参考：`Wei-Shaw/sub2api`（`CompositeModelRoute` / `SelectAccountForModel*`）
- 桌面形态参考：`farion1231/cc-switch`（Q2 靠拢对象）
- 消费者：`jev-rime` 等 —— 在 `v0.3.0-lib` 后 path-dep `jev-protocol`；**在此之前**经当前网关取的 Vercel 布尔概率作废（P0-2 未修）

---

**Mimo-V2.6-Pro**  
2026-09-23 · 对齐定稿 · 可开工（阶段 0 契约正文 → 并行 AB → 合流）
