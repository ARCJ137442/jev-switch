# Roadmap Execution Plans

> **TL;DR**：这些是基于当前源码的执行状态和后续拆解，不是版本承诺。`v0.8.0` 候选已保留 provider attempt 双视图、Dagre 节点及跨层边走廊、Android 原生 JSON/剪贴板，并增加只读 Graph API、DAG 节点模型和 Dashboard Phase 1 布局编辑。维护者已确认 v0.7.1 Android 真机主要能力；后续 APK 回归、OEM 差异和全链路性能仍按版本独立记录。
>
> **作者**：GPT-6 Luna xhigh（OpenAI Codex）  
> **AI 披露**：本文由 AI 协助整理，执行顺序和最终范围由项目维护者确认。

## 现状判断

当前代码和 v0.8.0 候选已提供 TypeSafe 官方/兼容服务、OpenRouter 上游适配器、Provider 模型发现、Standalone、七页控制台、Dashboard 运行遥测、headless CLI、JSON 设置备份、真实 route activity 辉光、HUD 自动隐藏、provider attempt 双视图、DAG 节点只读投影和 Dashboard Phase 1 布局编辑。维护者已确认桌面核心路径和 v0.7.1 Android 真机主要路径；Graph API 写入、专用页面布局、全链路性能和跨设备差异仍须单独验收。

`abc793f` 已把 upstream attempt timing 合入主线：`TimedUpstream` 在 daemon 组合边界记录每次真实 adapter attempt（包括失败与重试），并提供会话级 `avg_upstream_latency_ms` 与 `upstream_attempts`。它是 transport + parsing 的 adapter processing time，不是纯网络耗时；不写 SQLite，也不与 gateway latency 相加。性能计划应直接复用这组数据，再补固定场景的 p50/p95/p99。

路线图与维护项按当前状态分为三组：

1. **已进入 Release**：Standalone、系统主题、命令面板 Phase 1、路由动画 Phase 1、GitHub Pages、OpenRouter 上游 adapter、CLI Phase 1。
2. **当前工作树并行主线**：DAG GraphDocument/策略历史闭环，以及 Dashboard 第一阶段组件布局编辑器。
3. **后续收口/独立方向**：Windows 证据与性能基线、Android 跨设备回归、服务发现、数据目录迁移、本地模型提供商生命周期、模型画像与属性路由、TUI、全链路性能 p50/p95/p99 和路由顾问。

本地模型提供商生命周期的详细设计已落盘到[本地模型提供商生命周期计划](LOCAL-MODEL-LIFECYCLE-PLAN.md)，通用 Agent 配置面见[配置能力注册表计划](CONFIG-CAPABILITY-REGISTRY-PLAN.md)：0.10.x 已完成结构化 argv、readiness、startup_check、按候选 on_demand 门控、provider 互斥、所有权快照、独立审计、主机命令开关和 Provider 高级 UI；scope 写入、跨 daemon 进程组回收、真实服务和平台验收仍待实现，尚未进入正式 Release。配置写入已落地[原子配置写入计划](ATOMIC-CONFIG-WRITE-PLAN.md)的路由事务和入口边操作；版本冲突与更大范围 Graph transaction 仅在确有需求时再排期。

模型画像与属性路由的需求已归档到[模型画像与属性路由计划](MODEL-PROFILES-ATTRIBUTE-ROUTING-PLAN.md)：它与 Capabilities、lifecycle、DAG/failover 分层，先做静态/动态 × 自动/人工证据和确定性硬过滤，不自动放宽隐私/模态约束。

Android 方向已从“可行性调研”进入“完整网关应用实现”：先在 GitHub Actions 准备 Android 工具链、编译 daemon library 并产出可下载 APK artifact，再安排模拟器/真机启停与生命周期验收。没有本机 Android SDK 不阻塞 Phase A；没有真机证据不扩大为稳定 Android 支持。远程/局域网控制台只能作为可选模式，不能替代本机内核。

## 状态矩阵

| 方向 | 当前状态 | 保留的未完成边界 |
|---|---|---|
| [Standalone 精确候选验收](ROADMAP-PLAN-STANDALONE-ACCEPTANCE.md) | v0.2.0 随 Release；该精确候选 cold start/cache/health 已验证 | 托盘 OS 菜单、多硬件退出计时及未覆盖的干净账户行为 |
| [跟随系统主题](ROADMAP-PLAN-SYSTEM-THEME.md) | light/dark/system 已随 v0.2.0 发布；webview UI gate 通过 | 最新 Tauri 原生 system-change screenshot 未单独重验 |
| [命令面板 Phase 1](ROADMAP-PLAN-COMMAND-PALETTE.md) | 导航/search/keyboard Phase 1 已随 v0.2.0 发布 | 新业务动作、使用频率排序及原生快捷键验收 |
| [路由图实时动效](ROADMAP-PLAN-ROUTING-LIVE-ANIMATION.md) | trace 驱动的单边 success/failure/retry Phase 1 已随 v0.2.0 发布 | 并发 trace 可辨识、暂停设置与 Tauri 实际调用画面 |
| [GitHub Pages 首页](ROADMAP-PLAN-GITHUB-PAGES.md) | v0.2.0 页面已部署；v0.5.0 网站文案、Release metadata 和主题/语言切换已同步 | 推送/部署更新及移动端正式截图 |
| [OpenRouter adapter](ROADMAP-PLAN-OPENROUTER.md) | adapter 已随 v0.2.0 发布并通过离线/WireMock 门禁 | 未使用 live key 验证账户能力/费用 |
| 图标/性能首轮 | v0.5.0 已实现高频 UI 图标、健康探测并发/共享状态和隐藏页遥测暂停，并通过 UI tests/build、Windows 原生隔离和桌面自动门禁；品牌图标和跨硬件基线仍未完成 | 安装器、跨硬件和全链路 daemon/SQLite/UI 性能基线仍在本轮之后 |
| [Android APP 正式发布与验收计划](ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md) | 完整网关应用已稳定签名随 v0.7.0 发布；维护者已确认 v0.7.1 真机启停、通知、tile、后台生命周期、局域网、文件/剪贴板和图标/签名主要路径 | 后续 APK 回归、OEM/Android 版本差异和跨设备性能仍需按设备记录；不把单设备证据泛化为所有设备 |
| [DAG 节点引擎与三类用户视图](DAG-NODE-ENGINE-ARCHITECTURE-PLAN.md) | GraphDocument/GraphNode 纯领域模型与校验已在工作树实现，入口/内部/提供商映射测试通过 | 尚未接入 daemon snapshot、HTTP API 或现有 Router；不是当前 Release 承诺 |
| [Dashboard 组件系统与布局编辑](DASHBOARD-COMPONENT-SYSTEM-PLAN.md) | Dashboard registry、布局 normalize、本地存储、编辑草稿、排序/显隐/宽度、撤销重做和设置入口已在工作树实现 | 尚未做自由拖拽、跨页面布局和 Release 集成；不是当前 Release 承诺 |

## 共用交付门槛

- 每项计划先补契约/测试，再改 UI；不把 mock fixture 当作真实能力证明。
- 真实 key 只存在于 daemon 或本机受保护文件，不能进入日志、截图、Git 或公开文档。
- 桌面功能分为源码测试、候选包测试和真实 OS 人工验收三层，三者不互相冒充。
- 交付时更新对应验证记录，并写明未验证边界；未达到门槛的功能不得写入正式 Release 能力列表。
- 计划执行完后运行 Rust 默认/`ts-rs`、UI test/lint/build 和与职责相称的 Tauri 测试。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理，2026-09-30
