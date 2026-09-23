# jev-switch

[![CI](https://github.com/ARCJ137442/jev-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/ARCJ137442/jev-switch/actions/workflows/ci.yml)

本地 **Jev 协议多上游路由器** —— Jev 原生入口 × 多上游切换 × 桥接模式。Rust（axum）后端 + React 控制台，目标形态含 Tauri 桌面。

## 特性

- **双态模式（local ⇄ cloud）**：本地态免 token 即装即用（CC Switch / LM Studio 式）；云端态以 Bearer 调用 token + 独立管理密码变身中转站（sub2api / newapi 式）——**一套内核，本地可路由、云端可中转**，聚合官方 / Vercel 等为一个 Jev API。上游拓扑不区分：以内核所在位置解析地址（LAN 内 Laya 自然直连）。详见 [docs/12](docs/12-路线收缩与三线作战计划.md) 与 [docs/deployment.md](docs/deployment.md)
- **8 个 HTTP 端点**（health / models / systemone + 5 个 admin），全 JSON、统一错误体 `{error, upstream, retryable}`
- **Jev 原生**：`POST /v1/systemone` 即 TypeSafe/jev-life 形状；`criteria` 必填、布尔族无 `confidence`、`noul`/`probability` 双键保留（`docs/contracts/01`）
- **多上游模型路由 DAG**：`[[routes]]` exact/prefix、同模型多候选 failover、priority/sticky/on_error、加载与 PUT 双重检环（`docs/contracts/03`）
- **翻译层**：调用方永远发 `type: noul`，Vercel 方言 `boolean` 的出/入站转换在 adapter 内完成 —— 切换 `model` 即可换上游
- **密钥防偷**：上游 key 只活在 daemon；API 只回 `api_key_masked`、**无读回明文接口**；错误/日志统一 redact（`docs/contracts/04`）
- **契约驱动**：六份 `docs/contracts/01–06` 定形状，TS 类型由 Rust ts-rs 生成（`ui/src/generated/`），UI 不手写第二份

## 架构

四 crate cargo workspace（`rs/Cargo.toml`，default-members = daemon）+ 独立 `ui/`：

| crate | 职责 | 依赖边界 |
|---|---|---|
| `jev-protocol` | 协议内核类型：`JevRequest`/`JevResponse`/`Answer` 判别联合、`noul_probability()`、feature `ts-rs` 类型导出 | **零 IO**：仅 serde / serde_json |
| `jev-core` | Router DAG（select/plan/failover）、冻结 trait `ProtocolAdapter`/`UpstreamAdapter` + `Registry`、`RetryPolicy`、redact | **无** axum / reqwest |
| `jev-adapters` | Vercel + Laya 上游实现、VercelProtocol 翻译、厂商 DTO | 厂商方言**只活在本 crate** |
| `jev-switch-daemon` | axum HTTP（lib `build_app` + bin `jev-switch`）、config 加载、admin API | 组装以上三个 |
| `ui/` | React 三页控制台：Providers / Routing（二部图连线）/ Playground | 5173；类型全部来自 `ui/src/generated/` |

```text
调用方 / UI(127.0.0.1:5173)
   │   HTTP JSON（契约 docs/contracts/05）
   ▼
jev-switch-daemon  127.0.0.1:11435
   ├─ jev-protocol   协议类型（零 IO）
   ├─ jev-core       Router DAG + Registry + retry + redact
   └─ jev-adapters   Vercel / Laya（noul↔boolean 翻译层）
          │
          ▼
   https://ai-gateway.vercel.sh …   /   http://127.0.0.1:18765 …（可换任意上游）
```

## API 端点总表

监听 `127.0.0.1:11435`（loopback 天然仅本机）；CORS 白名单 `127.0.0.1:5173` / `localhost:5173` 两 origin；配置走 `$JEV_SWITCH_CONFIG`。

| Method | Path | 形状要点 |
|---|---|---|
| GET | `/health` | `{"status":"ok","version":"0.1.0"}` |
| GET | `/v1/models` | `{object:"list",data:[{id,object,upstream}],upstreams:[…]}`（**已过滤不可路由**；无 Vercel key 时 data 仅 `laya-english` 类可路由项） |
| POST | `/v1/systemone` | Jev 协议；缺 criteria→400、未知 model→404(`upstream:null`)、capability 无候选→422、上游 retryable→503 |
| GET | `/v1/admin/providers` | `{providers:[{id,kind,base,enabled,api_key_masked,api_key_set}]}`——**零明文**（无 `api_key` 字段） |
| PUT | `/v1/admin/providers` | `{providers:[{id,kind,base,enabled,api_key?}]}` 整表替换；api_key 省略=保留/空串=清除；响应回 masked 全表 |
| GET | `/v1/admin/routes` | `{routes:[RouteEdge…]}`（旧 `[router]` 已展开的运行时边表） |
| PUT | `/v1/admin/routes` | 整表替换；**检环先于落盘**→400 `{"error":"路由配置存在环 (cycle): a -> b -> a",…}`；right 非法→400（文案含「既不是已注册 provider」）；成功→热替换+写 toml（`[router]` 段移除） |
| POST | `/v1/admin/providers/{id}/probe` | `{ok,latency_ms,status,error}`；未知 id→404；HTTP GET 连通探测（无 auth 零计费）；不可达→`ok:false,status:0` |

错误语义速查：未知 model 404 · capability 不匹配 422 · 上游 429/5xx（retryable）503 · 协议非法 400 · 上游反序列化失败 502。行为要点：Laya 等本地类上游不重试不 failover（429 透传非 503）；`upstream_calls` = 实发次数；boolean 入站归一 noul（响应 `type:"noul"`）。

## 配置

真值源：`$JEV_SWITCH_CONFIG`，缺省 `~/.jev-switch/providers.toml`（相对路径按进程 CWD 解析；Unix 期望 0600，Windows 降级告警）。

```toml
[providers.vercel]
kind = "vercel"
base = "https://ai-gateway.vercel.sh/v4/ai/evaluation-model"
api_key = "sk-…"                  # 明文优先（0600 文件）；UI/API 只出 masked
# api_key_env = "AI_GATEWAY_API_KEY"  # 兼容保留：明文缺省时回退读环境变量
enabled = true

[providers.laya]
kind = "laya"
base = "http://127.0.0.1:18765/v1/systemone"
enabled = true

# 旧式扁平表（兼容保留）：每条 = 单条 match=exact 边（priority=0）
[router]
"laya-english" = "laya"
"typesafe-ai/jev" = "vercel"

# 新式模型路由 DAG 边（正式形态；与图形界面双向等价）
[[routes]]
left = "jev"
match = "exact"                # exact | prefix（默认 exact）
right = "vercel"
upstream_model = "typesafe-ai/jev"
priority = 10                  # 同 left 下越小越优先
sticky = "session"             # none | session（默认 none）
on_error = "next"              # next | fail（默认 next）

[[routes]]
left = "jev"
right = "laya"
upstream_model = "laya-english"
priority = 30                  # 双候选：Vercel 429/5xx 自动 failover 到 Laya

[[routes]]
left = "local/*"
match = "prefix"               # POST {"model":"local/qwen"} → laya
right = "laya"
priority = 40
```

- 合并顺序：`[router]` 展开在前、`[[routes]]` 在后；**加载时合并图检环，含环直接拒绝启动**
- `PUT /v1/admin/routes` 为整表替换：成功后移除 `[router]` 段并热替换运行时边表（toml 往返**不保留注释**）
- 示例：[`rs/providers.example.toml`](rs/providers.example.toml)

## 快速开始

```bash
# 1. 后端（监听 127.0.0.1:11435）
$env:JEV_SWITCH_CONFIG="rs\providers.example.toml"; cargo run --manifest-path rs/Cargo.toml

# 2. 前端
npm run dev --prefix ui    # http://127.0.0.1:5173

# 3. 可选：本地上游 Laya（18765）
E:\venvs\laya\Scripts\python.exe E:\tmp\jev_laya_server.py --port 18765
```

或 **Docker 单命令部署**（含前端同源托管与 cloud 态鉴权，细节见 [docs/deployment.md](docs/deployment.md)）：

```bash
docker compose up -d        # JEV_SWITCH_MODE=cloud + token/密码经 compose env 注入
```

端到端冒烟（会自起停 daemon，**勿与第 1 步并跑**）：

```bash
powershell -File scripts/smoke.ps1    # Windows 主用（也兼容 pwsh）
bash scripts/smoke.sh                 # Linux CI / Git Bash
```

## 测试与门禁

| 门禁 | 命令 | 预期 |
|---|---|---|
| workspace 单测+集成 | `cargo test --manifest-path rs/Cargo.toml --workspace` | **142/142** |
| ts-rs 生成物 | `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs` | **165/165**（CI 另做生成物 diff 门禁） |
| 黄金测试（仓外 `my_lab_adapter` 零改内核） | `powershell -File rs/scripts/golden-test.ps1` | **3/3** |
| 端到端 smoke（11 步 / 9 断言） | `powershell -File scripts/smoke.ps1` 或 `bash scripts/smoke.sh` | **PASS 9/9, SKIP z**（Laya/Vercel 不可达只 warning/SKIP） |
| UI 类型检查 / 构建 | `npm run lint --prefix ui` / `npm run build --prefix ui` | 绿 |
| CI | GitHub Actions（push/PR → main） | [badge](https://github.com/ARCJ137442/jev-switch/actions/workflows/ci.yml) 全绿 |

## 文档

完整索引见 **[docs/README.md](docs/README.md)**。
阅读顺序：`08-CONTRACT`（定稿） → `contracts/00-INDEX` → `contracts/01–06` → `07-REVIEW`（意见清单） → 历史 `01–06`。验收报告：`docs/10-v0.5.0-验收报告.md`。

## 开发说明

本项目由 AI 编码代理（MiMo、Claude Code 会话）协作构建，文档级作者逐件署名见各文件头与 [docs/README](docs/README.md) 索引；此披露同时满足 awesome-jev 等列表的 AI 辅助投稿规则。

## License

Licensed under **MIT OR Apache-2.0**：

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

当前状态：`v0.1.0-mvp` 已封存 → Phase 0 六契约 + 并行 A/B（workspace / 协议对齐 / DAG / admin / UI 三页）已合流；测试与 smoke/CI 门禁齐备，处 **v0.5.0-stable 前夕**（终局 CDP 实测后打 tag，由 P2-8 执行）。
