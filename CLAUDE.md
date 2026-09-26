# Jev-Switch — 项目规则

## 产品边界

Jev-Switch 是 Jev 原生模型网关，提供 React 控制台、Tauri 桌面壳和 Docker 部署。基本数据流是：

```text
调用者（网关地址 + 可选调用 Token + 对外模型/入口 ID）
  → 服务入口与路由策略
  → 可编辑调用路由 DAG
  → 提供商接入配置（地址 + 上游 key + 模型）
  → 上游适配器（当前内置 Vercel、Laya）
```

- `local` 描述网关运行位置与默认监听方式，不代表离线；它仍可调用远程上游。云机器上的 `127.0.0.1` 指云机器自身。
- 对外服务入口与上游提供商配置是两种不同对象；不要混淆网关调用 Token 和供应商 API key。
- 仅提供 Jev 原生 `/v1/systemone` 接口；不实现 OpenAI/Anthropic 聊天格式兼容入口。
- 当前内置上游适配器为 Vercel 与 Laya；TypeSafe 官方 API 和 OpenRouter 尚未实现，属于 [最高优先级路线图事项](ROADMAP.md)。不要将计划写成当前功能。
- 上游 key 只由 daemon 持有；不得输出、写入公开文档、日志、测试产物或提交。管理 API 仅返回掩码状态。

## 开发与交付

- 开始跨层功能前先看 [当前实施计划](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)、[HTTP 契约](docs/contracts/05-HTTP契约.md)和[入口网关修订](docs/contracts/07-入口网关修订.md)，再按涉及范围读其他契约。
- [ROADMAP.md](ROADMAP.md) 记录未排期的未来方向，不代表当前发布包含或承诺了这些能力。
- Rust workspace 位于 `rs/`，React UI 位于 `ui/`，Tauri 桌面壳位于 `src-tauri/`。`ts-rs` 的 UI 类型来自 Rust 导出。
- 保持改动局部化；验证与改动职责相称。不要用历史测试计数或旧版运行报告替代当前工作树的结果。
- GitHub 仓库 About 简介保持“英文一句话 | 中文一句话介绍”的双语格式；每次发版或调整产品定位时核对其内容与 README/当前能力一致。当前简介由仓库维护者在 GitHub 设置中维护，不是代码版本字段。
- commit、push、对外发布需用户明确授权；未经授权不得操作远端项目状态。

## 常用命令

| 任务 | 命令 |
|---|---|
| 后端测试 | `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline` |
| Rust 生成类型门禁 | `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --locked --offline` |
| UI 测试/类型检查/构建 | `npm test --prefix ui` · `npm run lint --prefix ui` · `npm run build --prefix ui` |
| UI 开发服务 | `npm run dev --prefix ui`（默认 `http://127.0.0.1:5173`） |
| Tauri 测试 | 先 `npm run build --prefix ui`，再 `cargo test --manifest-path src-tauri/Cargo.toml --locked` |
| 本地 daemon | 设置 `$env:JEV_SWITCH_CONFIG="rs\providers.example.toml"` 后运行 `cargo run --manifest-path rs/Cargo.toml` |
| 健康检查 | `curl http://127.0.0.1:11435/health` |

版本号与完整发版步骤见 [docs/RELEASE.md](docs/RELEASE.md)。当前正式 GitHub Release 为 v0.1.0；新版本号须先经用户确认，并同步发布元数据。

## 文档入口

- 产品决策、当前施工状态与验收边界：[docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)
- 未来迭代方向：[ROADMAP.md](ROADMAP.md)
- 文档导航与历史资料说明：[docs/README.md](docs/README.md)
- 契约目录：[docs/contracts/00-INDEX.md](docs/contracts/00-INDEX.md)
- 发布流程：[docs/RELEASE.md](docs/RELEASE.md)
- 原目标恢复与评估：[docs/OPUS5-GOAL-REASSESSMENT.md](docs/OPUS5-GOAL-REASSESSMENT.md)
