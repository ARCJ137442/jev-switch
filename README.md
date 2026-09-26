# jev-switch

[![CI](https://github.com/ARCJ137442/jev-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/ARCJ137442/jev-switch/actions/workflows/ci.yml)

Jev Switch 是一个轻量的 **Jev 协议模型网关**：把对外服务入口连接到可复用的上游接入配置，由可编辑路由图决定实际调用路径。Rust/axum daemon 承担协议转换、路由和转发；模型推理由上游服务执行。交付形态包括 React 控制台、Tauri 桌面应用与 Docker 服务。

![Jev Switch Tauri dashboard](docs/images/jev-switch-dashboard.png)

## 特性

- **两类调用入口**：对外服务入口由外部模型 ID、上游路由与策略组成；上游接入配置保存地址、凭据和该账号可用的模型。一个上游配置可被多个服务入口复用。
- **双态运行**：`local` 默认仅监听 loopback 并免调用 token；`cloud` 默认对外监听并要求调用 token，管理操作另走管理员会话。两种模式都可路由到本机、LAN 或云端上游；`local` 不代表离线。详见 [部署说明](docs/deployment.md) 与 [范围基线](docs/12-路线收缩与三线作战计划.md)。
- **Jev 原生接口**：`POST /v1/systemone` 使用 Jev 请求/响应形状；Vercel 等适配器负责上游方言转换。网关不加载模型权重。
- **可编辑调用路由 DAG**：支持对外入口、路由节点和提供商模型端口之间的多跳与分支，并配置候选优先级和失败处理；简单直连只是 DAG 的一种形式。
- **演练场横向比较**：可比较多个对外入口、直接上游模型或混合目标；结果分别呈现实际路径、耗时、usage 与错误。
- **密钥边界**：上游 key 留在 daemon；管理 API 只返回脱敏状态，不提供明文读回接口。不要把真实 key 提交到仓库或聊天。
- **四页控制台**：Dashboard、Providers、Routing、Playground；当前提供简体中文和英文，语言注册表可扩展。
- **契约驱动**：Rust `ts-rs` 生成前端类型到 `ui/src/generated/`；协议及路由不变量见 `docs/contracts/`。

## 架构

Rust workspace（`rs/Cargo.toml`）+ React 控制台（`ui/`）+ Tauri 壳（`src-tauri/`）：

| crate | 职责 | 依赖边界 |
|---|---|---|
| `jev-protocol` | 协议内核类型：`JevRequest`/`JevResponse`/`Answer` 判别联合、`noul_probability()`、feature `ts-rs` 类型导出 | **零 IO**：仅 serde / serde_json |
| `jev-core` | Router DAG（select/plan/failover）、冻结 trait `ProtocolAdapter`/`UpstreamAdapter` + `Registry`、`RetryPolicy`、redact | **无** axum / reqwest |
| `jev-adapters` | Vercel + Laya 上游实现、VercelProtocol 翻译、厂商 DTO | 厂商方言**只活在本 crate** |
| `jev-switch-daemon` | axum HTTP、配置/数据库、管理 API、静态 UI 托管 | 组装协议、路由与适配器 |
| `ui/` | React 控制台：Dashboard / Providers / Routing / Playground | 开发端口 5173；类型由 `ts-rs` 生成 |
| `src-tauri/` | Windows 桌面壳与随包 sidecar | MSI / NSIS |

```text
调用者：网关地址 + 调用 token + 对外模型 ID
   │
   ▼
对外服务入口（独立路由集合与策略）
   │
   ▼
可编辑调用路由 DAG
   │
   ▼
提供商接入配置（地址 + 凭据 + 模型端口）
   │
   ▼
上游适配器（当前内置 Vercel、Laya） -> 上游模型服务
```

## API 端点总表

HTTP surface 按调用、管理、事件与统计分组；端点随管理能力演进，不固定为早期 README 中的“8 个”。默认端口为 `11435`：local 默认 loopback，cloud 默认 `0.0.0.0`；可显式配置监听地址。开发 UI 的 CORS origin 为 `127.0.0.1:5173` 与 `localhost:5173`。

| Method | Path | 形状要点 |
|---|---|---|
| 调用 | `/health`, `/v1/models`, `/v1/systemone` | 健康身份、可公开模型、Jev 原生推理请求 |
| 提供商与路由 | `/v1/admin/providers*`, `/v1/admin/routes` | 接入配置、探测/直调与路由图管理 |
| 服务入口 | `/v1/admin/endpoints*`, `/v1/admin/config/default_strategy` | 对外模型 ID、路由策略与启停 |
| 访问与观测 | `/v1/admin/tokens*`, `/v1/admin/stats`, `/v1/admin/events*`, `/v1/stats/my`, `/v1/events/my*` | 调用权限、统计、历史与事件流 |
| 运行管理 | `/v1/admin/mode`, `/v1/admin/listen`, `/v1/admin/status`, `/v1/admin/config/*` | 模式/监听管理、TOML 导入导出与存储状态 |

完整 DTO、鉴权边界与错误语义以当前 [HTTP 契约](docs/contracts/05-HTTP契约.md)、[入口网关修订](docs/contracts/07-入口网关修订.md)和后端实现为准。

## 配置

配置文件位置由 `JEV_SWITCH_CONFIG` 指定，未设置时使用用户配置目录中的 `providers.toml`。首次启动时读取 TOML 并导入 SQLite；之后 SQLite 统一快照是运行配置权威来源。外部编辑提示漂移，需通过控制台显式导入；也可导出 TOML 作为备份。`JEV_SWITCH_DATA_DIR` 可指定 SQLite 数据目录。

仓库示例 [`rs/providers.example.toml`](rs/providers.example.toml) 配有 Vercel 和 Laya 的地址、模型路由及 `AI_GATEWAY_API_KEY` 环境变量引用。真实 key 不应写进 Git；提供商页可管理接入配置并只显示脱敏状态。

## 快速开始

```powershell
# 1. 终端 A：先设置环境，再启动后端（默认 local：127.0.0.1:11435）
$env:JEV_SWITCH_CONFIG="rs\providers.example.toml"
# 若要使用 Vercel，启动 daemon 前取消下一行注释并替换为真实 key
# $env:AI_GATEWAY_API_KEY="<your-key>"
cargo run --manifest-path rs/Cargo.toml

# 2. 终端 B：启动前端
npm run dev --prefix ui    # http://127.0.0.1:5173
```

前端可通过 `http://127.0.0.1:5173` 连接本机 daemon。生产构建由 daemon 同源托管 UI。Docker 默认使用 cloud 模式；先为管理密码和公开调用 Token 配置本机 `.env`，然后启动：

```bash
docker compose up -d
```

若要在 Docker 部署中调用 Vercel 上游，也需在 `.env` 配置 `AI_GATEWAY_API_KEY`。

## 测试与门禁

| 门禁 | 命令 | 预期 |
|---|---|---|
| Rust workspace 单测与集成 | `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline` | 按当前工作树运行；不要复用历史固定计数 |
| Rust + ts-rs | `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked --offline` | CI 同时检查生成物无漂移 |
| UI 测试 / 类型检查 / 构建 | `npm test --prefix ui` / `npm run lint --prefix ui` / `npm run build --prefix ui` | 当前 `lint` 脚本执行 TypeScript 检查（`tsc --noEmit`）；使用当前 `ui/package.json` 脚本 |
| GitHub Actions | CI / Release workflows | 状态以对应提交的 workflow 结果为准；本地构建不代表远端 CI 或发布通过 |

## 文档

从 [当前入口网关计划](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) 和 [目标重评](docs/OPUS5-GOAL-REASSESSMENT.md) 开始，再按需查看 [文档索引](docs/README.md)、[契约目录](docs/contracts/00-INDEX.md)、[发版手册](docs/RELEASE.md) 与[最新联调记录](docs/verification/2026-09-24-entry-gateway-integration.md)。历史计划和验收仅证明各自版本与覆盖范围，不代表当前整版验收。

## 开发说明

本项目由 AI 编码代理（MiMo、Claude Code 会话）协作构建，文档级作者逐件署名见各文件头与 [docs/README](docs/README.md) 索引；此披露同时满足 awesome-jev 等列表的 AI 辅助投稿规则。

## License

Licensed under **MIT OR Apache-2.0**：

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

## 当前验收状态

**状态截至 2026-09-26 17:45（北京时间）。** P1–P4 产品/Web 能力已实现并分项验证。便携版 `jev-26` 本地 Laya 请求返回 HTTP 200，AppData 中有成功记录和安全 route trace。只读 AppData 核对为 2 个提供商、9 条路由、7 个公开入口和 26 条调用历史。调用历史以请求 ID、入口、路由、状态、耗时和 token 用量为主，原始 JSON 折叠显示。用户提供的六张截图覆盖 Dashboard、Providers、Routing 入口/DAG 与双入口 Playground；Dashboard 已通过安全筛选，原图字节一致地复制到 README。其余截图继续作为本机验收资料保存：Providers 图含脱敏 key 片段和真实上游地址，Playground 图含演示输入，不直接公开。

P5 已有真实 Laya/Vercel 路由与演练场调用、请求历史、窗口几何及原生 WebView 布局测试证据。当前源码的 Docker 镜像已构建并通过 cloud 模式探活、UI 同源访问、未认证管理请求拒绝，以及替换容器后的提供商快照恢复测试。17:41 同批 Windows MSI、NSIS 和便携候选已生成，构建清单中的壳、daemon、UI 资源和安装包哈希均已复核；该新候选尚未冷启动，旧便携实例仍占用单实例端口。用户此前确认的托盘关窗/恢复只适用于当时实例；新候选的托盘点击、完整退出后重启与 AppData 恢复仍待核实。当前工具中没有 Tauri MCP，Computer Use 也不能操作原生托盘；隔离生命周期测试不替代实机点击。其他五张原图保存在本机忽略目录，过期设计预览不作当前发布截图。工作树变更尚未逐项审阅，也未提交；P5/P6 仍未完成，其他页面的安全公开截图、提交、远端 CI 与 Release workflow 验证待收口。完整状态与证据边界见[实施计划](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)及[目标重评](docs/OPUS5-GOAL-REASSESSMENT.md)。
