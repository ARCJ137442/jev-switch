# Jev-Switch — 项目规则

## 产品边界

Jev-Switch 是 Jev 原生模型网关，提供 React 控制台、Tauri 桌面壳和 Docker 部署。基本数据流是：

```text
调用者（网关地址 + 可选调用 Token + 对外模型/入口 ID）
  → 服务入口与路由策略
  → 可编辑调用路由 DAG
  → 提供商接入配置（地址 + 上游 key + 模型）
  → 上游适配器（当前内置 Vercel、Laya、TypeSafe SystemOne、OpenRouter）
```

- `local` 描述网关运行位置与默认监听方式，不代表离线；它仍可调用远程上游。云机器上的 `127.0.0.1` 指云机器自身。
- 对外服务入口与上游提供商配置是两种不同对象；不要混淆网关调用 Token 和供应商 API key。
- 仅提供 Jev 原生 `/v1/systemone` 接口；不实现 OpenAI/Anthropic 聊天格式兼容入口。
- 当前源码内置上游适配器为 Vercel、Laya、TypeSafe SystemOne 与 OpenRouter；OpenRouter 是上游转换 adapter，不是普通 OpenAI/Anthropic 对外入口；TypeSafe 本地使用仍要求 SystemOne-compatible HTTP endpoint。正式发布边界以 README、docs/RELEASE.md 和对应核验记录为准。
- 上游 key 只由 daemon 持有；不得输出、写入公开文档、日志、测试产物或提交。管理 API 仅返回掩码状态。

## 开发与交付

- 开始跨层功能前先看 [当前实施计划](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)、[HTTP 契约](docs/contracts/05-HTTP契约.md)和[入口网关修订](docs/contracts/07-入口网关修订.md)，再按涉及范围读其他契约。
- [ROADMAP.md](ROADMAP.md) 记录未排期的未来方向，不代表当前发布包含或承诺了这些能力。
- [路线图执行计划总表](docs/design/ROADMAP-EXECUTION-PLANS.md)及其专项计划用于把近期候选拆成可执行任务；计划不等于 Release 承诺。
- [下一轮用户反馈迭代计划](docs/design/USER-FEEDBACK-ITERATION-PLAN-NEXT.md)记录近期 UI/交互工作包和验收边界；开始相关实现时更新任务状态，不能把计划当作当前能力或 Release 承诺。
- Android APP 是面向没有 Termux 用户的正式 APK 渠道，维护优先级次于 Windows。APK 必须包含 Android daemon 核心；当前实现首次打开即启动本机网关，只有用户明确停止后才在下次打开时保持关闭。APP、通知和 Quick Settings tile 共用启停状态机。正式包使用 Jev-Switch 专属稳定签名密钥，不能复用其他应用的 key；service/tile/通知/网络/生命周期等具体能力仍按真实设备证据陈述。详见 [Android 构建与验收计划](docs/design/ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md)。
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

版本号与完整发版步骤见 [docs/RELEASE.md](docs/RELEASE.md)。当前源码版本为 `0.7.1`；版本字段（含 Android 配置）、发行产物、README 与核验记录必须保持一致。近期发布、Android 文件/剪贴板与路由 DAG 的自动/人工边界见[交接与核验](docs/verification/v0.7.1-handoff-2026-10-03.md)及[Android 权限审查](docs/verification/android-permission-audit-2026-10-03.md)。

## 文档入口

- 产品决策、当前施工状态与验收边界：[docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)
- 未来迭代方向：[ROADMAP.md](ROADMAP.md)
- 文档导航与历史资料说明：[docs/README.md](docs/README.md)
- 契约目录：[docs/contracts/00-INDEX.md](docs/contracts/00-INDEX.md)
- 发布流程：[docs/RELEASE.md](docs/RELEASE.md)
- 原目标恢复与评估：[docs/OPUS5-GOAL-REASSESSMENT.md](docs/OPUS5-GOAL-REASSESSMENT.md)
- 下一轮用户反馈计划：[docs/design/USER-FEEDBACK-ITERATION-PLAN-NEXT.md](docs/design/USER-FEEDBACK-ITERATION-PLAN-NEXT.md)
