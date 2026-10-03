# Roadmap Execution Plans

> **TL;DR**：这些是基于当前源码的执行状态和后续拆解，不是版本承诺。公开 `v0.7.0` 已包含 provider attempt 双视图、Dagre 节点排布和 Android 原生 JSON/剪贴板；`v0.7.1` 候选补跨层连线走廊。维护者只确认过 v0.6.3 Android 本机启停与 UI 连接，service/tile/通知、新文件/剪贴板和全链路性能仍有独立验收边界。
>
> **作者**：GPT-6 Luna xhigh（OpenAI Codex）  
> **AI 披露**：本文由 AI 协助整理，执行顺序和最终范围由项目维护者确认。

## 现状判断

当前代码和 v0.7.0 Release 已提供 TypeSafe 官方/兼容服务、OpenRouter 上游适配器、Provider 模型发现、Standalone、七页控制台、Dashboard 运行遥测、headless CLI、JSON 设置备份、真实 route activity 辉光、HUD 自动隐藏、provider attempt 双视图和 Android keepalive/tile 源码。维护者已确认桌面核心路径、官方 TypeSafe 调用、Android 初步调用及 v0.6.3 本机网关启停/连接；MSI/NSIS 原生交互、Android 真机 service/tile/通知、全链路性能和真实用户配置恢复仍须单独验收。

`abc793f` 已把 upstream attempt timing 合入主线：`TimedUpstream` 在 daemon 组合边界记录每次真实 adapter attempt（包括失败与重试），并提供会话级 `avg_upstream_latency_ms` 与 `upstream_attempts`。它是 transport + parsing 的 adapter processing time，不是纯网络耗时；不写 SQLite，也不与 gateway latency 相加。性能计划应直接复用这组数据，再补固定场景的 p50/p95/p99。

路线图与维护项按当前状态分为三组：

1. **已进入 Release**：Standalone、系统主题、命令面板 Phase 1、路由动画 Phase 1、GitHub Pages、OpenRouter 上游 adapter、CLI Phase 1。
2. **当前工作树小步改进**：高频 UI 图标化、Dashboard provider probes 并发和状态合并、共享 health poll、隐藏页 telemetry 调度。
3. **未完成的跨层基线/独立方向**：全链路性能 p50/p95/p99、TUI、Android 设备验收、服务发现、数据目录迁移和路由顾问。

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
| [Android APP 正式发布与验收计划](ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md) | 完整网关应用已稳定签名随 v0.7.0 发布；内置后端本机启停和 UI 连接仅有 v0.6.3 维护者设备证据；v0.7.0 补 Android 7–9 分享、原生 JSON/剪贴板与权限清单门禁，详见[权限审查](../verification/android-permission-audit-2026-10-03.md) | 原生 service/tile/通知、后台/OEM 回收、文件导出/剪贴板、热点 LAN 和跨 Android 版本行为仍需人工证据 |

## 共用交付门槛

- 每项计划先补契约/测试，再改 UI；不把 mock fixture 当作真实能力证明。
- 真实 key 只存在于 daemon 或本机受保护文件，不能进入日志、截图、Git 或公开文档。
- 桌面功能分为源码测试、候选包测试和真实 OS 人工验收三层，三者不互相冒充。
- 交付时更新对应验证记录，并写明未验证边界；未达到门槛的功能不得写入正式 Release 能力列表。
- 计划执行完后运行 Rust 默认/`ts-rs`、UI test/lint/build 和与职责相称的 Tauri 测试。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理，2026-09-30
