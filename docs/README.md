# Jev-Switch 文档索引

> **TL;DR：**当前正式版为 `v0.4.1`；v0.4.0 的便携/Standalone 桌面核心路径已有维护者人工验收，v0.4.1 新增 UI/运行时和 Android arm64 构建已自动验证，安装器、Android 安装/真机生命周期和全链路性能仍有独立边界。先读入口网关实施计划；未排期的未来方向见 [ROADMAP.md](../ROADMAP.md)。
>
> **本轮索引修订：**GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理，2026-09-30。

> **2026-09-30 adapter 状态：**当前源码和 `v0.4.1` Release 包含 TypeSafe 官方、TypeSafe-compatible SystemOne 与 OpenRouter 上游；官方 TypeSafe `/v1/systemone` 和 OpenRouter `/api/v1/chat/completions` 均已完成协议/离线核对。OpenRouter 仍无 live key 证据，按对应核验边界保留。
>
> **TypeSafe 实测：**2026-09-29 使用隔离 daemon 和用户本地密钥文件完成一笔官方 API 调用，HTTP 200；未记录密钥。范围与边界见[官方 API 实测](verification/typesafe-official-live-2026-09-29.md)。
>
> **本仓库**：[`ARCJ137442/jev-switch`](https://github.com/ARCJ137442/jev-switch)（public）<br>
> **定位**：**Jev-Switch = 模型调用入口之间进行可配置转换的轻量网关**（Rust 内核 + React 控制台，支持本地/云端方向与 Tauri/Docker 交付）
> **历史发布快照（截至 2026-09-27）**：当时最新正式版为 [v0.1.0 Release](https://github.com/ARCJ137442/jev-switch/releases/tag/v0.1.0)，包含 Windows MSI、NSIS、便携 ZIP 与 Docker 镜像。对应 Release gate 的 Rust、UI、Tauri 与版本检查通过；验收证据覆盖 2 家上游、9 条路由、7 个公开入口和 34 条调用记录。用户提供的 Tauri 截图整理为公开安全图册，并确认当时安装版“关窗留托盘、点菜单恢复”通过；这些材料不证明 Release 候选的精确 build identity、托盘 Exit 或退出后恢复。当前版本与能力以本文后续的 2026-09-30 产品基线、[发布手册](RELEASE.md)及对应核验记录为准。历史结论只适用于各自版本与覆盖范围。
> **当前产品基线与维护阶段（2026-09-30）**：正式版为 v0.4.1；v0.4.0 的便携/Standalone 桌面核心路径和官方 TypeSafe 调用已有维护者人工确认，v0.4.1 新增 UI/运行时和 Android arm64 APK 构建已自动验证。MSI/NSIS 新包、Android 安装/真机 service、TUI、OpenRouter live、真实用户配置恢复和跨设备性能仍按各自边界处理。Release/CLI 状态见[发布手册](RELEASE.md)、[CLI 文档](CLI.md)、[CLI 验证记录](verification/cli-phase1-2026-09-30.md)、[0.4.0 桌面核验记录](verification/v0.4.0-release-candidate-2026-09-30.md)和[0.4.1 发布核验记录](verification/v0.4.1-release-candidate-2026-09-30.md)。

## 核心定位（一句话）

> **对外入口卡片 → 可交互调用路由 DAG → 提供商复合卡片内的模型端口**。两端都是模型调用入口，网关负责可配置转换；演练场支持两类入口横比。

## 文档列表

| 文件 | 内容 | 状态 |
|---|---|---|
| **[../ROADMAP.md](../ROADMAP.md)** | **未来功能方向、优先级和可验收边界** | OpenRouter 已发布；其他方向按表内状态推进 |
| [design/ROADMAP-EXECUTION-PLANS.md](design/ROADMAP-EXECUTION-PLANS.md) | 基于当前源码的近期路线图执行拆解、文件入口、ICE 和验收门槛 | 计划草案；不代表 Release 承诺 |
| [verification/typesafe-systemone-adapter-2026-09-28.md](verification/typesafe-systemone-adapter-2026-09-28.md) | TypeSafe 官方与本地-compatible SystemOne adapter 的当前实现、wiremock 证据和边界 | 源码/离线契约验证；不代表已发布或已做真实官方推理 |
| [verification/typesafe-official-live-2026-09-29.md](verification/typesafe-official-live-2026-09-29.md) | TypeSafe 官方 key 的隔离单次实测，Noul/Choice/Score 与 usage | 当前源码直连 provider；不代表 Standalone/公开入口 E2E |
| [verification/standalone-lmstudio-e2e-2026-09-29.md](verification/standalone-lmstudio-e2e-2026-09-29.md) | 较早 Standalone 候选的冷启动、缓存、五页 WebView 与 LM Studio 本地 SystemOne UI 路由 trace | 该记录只属于文档顶部候选哈希；真实系统托盘点击和第二次双击聚焦仍待人工确认 |
| [verification/typesafe-model-discovery-2026-09-29.md](verification/typesafe-model-discovery-2026-09-29.md) | TypeSafe `/v1/models` 实测与 daemon/UI 自动模型发现 | 管理时目录/鉴权检查；不替代推理调用 |
| [verification/runtime-telemetry-2026-09-30.md](verification/runtime-telemetry-2026-09-30.md) | Dashboard 当前 daemon 会话遥测、流量曲线、adapter attempt latency 与资源卡的实现和验证边界 | Phase 1/2 已落地；Tauri WebView 精确资源仍未提供 |
| [verification/openrouter-adapter-2026-09-30.md](verification/openrouter-adapter-2026-09-30.md) | OpenRouter 官方 OpenAPI 核对、adapter 实现、模型目录和 WireMock 验证边界 | v0.2.0–v0.4.0 已发布；真实 key/live call 未验证 |
| [verification/system-theme-2026-09-30.md](verification/system-theme-2026-09-30.md) | light/dark/system 首帧解析、系统切换和主题控件回归 | 源码与 UI 门禁通过；原生系统主题截图仍待验收 |
| [verification/v0.2.0-human-acceptance-matrix-2026-09-30.md](verification/v0.2.0-human-acceptance-matrix-2026-09-30.md) | v0.2.0 Standalone、主题、OpenRouter、Pages、命令面板、路由动效、遥测和关闭性能的人类验收矩阵 | **v0.2.0 历史发布验收范围**；不得当作 v0.4.0 当前清单 |
| [verification/windows-release-candidate-2026-09-29.md](verification/windows-release-candidate-2026-09-29.md) | v0.1.0/v0.2.0 Windows MSI、NSIS、Portable 与 Standalone 候选哈希和点时验证 | 历史候选证据；当前资产见 v0.4.0 Release |
| [verification/recovery-regression-2026-09-28.md](verification/recovery-regression-2026-09-28.md) | Windows 重启后的 Kev、Laya、Jev Switch 恢复、隔离 target 回归与小样本 Rime smoke | 恢复核对；不替代完整 benchmark |
| [verification/jevk5-4b-lmstudio-2026-09-28.md](verification/jevk5-4b-lmstudio-2026-09-28.md) | JevK5-4B GGUF 经 LM Studio bridge 注册、路由与清理核对 | 实验性 one-hot 兼容路径；不代表原生 logits 或生产校准 |
| [USER-JOURNEYS.md](USER-JOURNEYS.md) | 当前基线、已发布体验与路线图用户旅程 | 前六节为历史构想；v0.2 旅程标已发布能力与边界；v0.3 旅程含 CLI/性能首轮体验 |
| **[design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)** | **用户决策、两侧入口定义、卡片粒度、DAG、比较范围、实施路线与验收边界** | **当前产品边界；v0.4.0 桌面核心路径已人工验收；UI 图标/性能范围见专项记录** |
| [design/USER-FEEDBACK-ITERATION-PLAN.md](design/USER-FEEDBACK-ITERATION-PLAN.md) | 本轮用户反馈的实施顺序、UI 原则、数据/安全边界和验收门槛 | 当前迭代入口；各 Phase 不代表 Release 承诺 |
| [OPUS5-GOAL-REASSESSMENT.md](OPUS5-GOAL-REASSESSMENT.md) | 原会话目标恢复、源码/运行核查与缺口证据 | 评估完成；不是产品完成声明 |
| [design/SERVICE-ENDPOINT-CONFIG-PLAN.md](design/SERVICE-ENDPOINT-CONFIG-PLAN.md) | 对外入口 CRUD、持久化、实时生效及策略专项 | 历史专项设计；当前实现状态以主计划和对应版本验收记录为准 |
| [design/PLAYGROUND-COMPARISON-PLAN.md](design/PLAYGROUND-COMPARISON-PLAN.md) | 两类入口横比、目标身份、执行边界、布局与验收 | 历史专项设计；当前实现状态以主计划和对应版本验收记录为准 |
| `01-Jev-Switch-原始计划书-DeepSeek.md` | DeepSeek 原始计划书（4 条需求 + 调研 + MVP + 风险） | 历史基线 |
| `02-Jev-Switch-新版计划书.md` | v2.1：5 类上游 + 桥接模式 + M1–M7 | 历史愿景，不作为当前范围 |
| `03-上游类别与协议兼容矩阵.md` | C1–C5 字段矩阵 + Vercel noul→boolean | 历史矩阵；当前适配范围见 README/ROADMAP |
| `04-架构设计-from-sys1-借鉴.md` | Rust workspace 三 crate + React + Tauri | 历史架构草案 |
| `05-从-jev-life-超越的设计点.md` | 相对 jev-life 的 5 个超越点 | 历史设计方向 |
| `06-MVP-实现计划.md` | M0 十一步施工清单 | 已封存；执行记录见 PROGRESS |
| `PROGRESS.md` | `v0.1.0-mvp` 进度快照与实测 | 封存的 MVP 历史快照 |
| **`07-REVIEW-v0.1.0-mvp.md`** | **全方位评审**（作者 **Mimo-V2.6-Pro**） | **历史评审记录** |
| **`08-CONTRACT-功能特性契约与施工计划.md`** | **对齐定稿 Q1–Q6 + 施工计划**（作者 **Mimo-V2.6-Pro**） | **历史契约/施工计划；最新边界见当前主计划** |
| `09-用户叙事评估报告-MiMo.md` | 10 叙事 vs v0.1.0 匹配度 + 三标记（openai-compat 不做 / fallback·shadow 待讨论 / 端口 11435 已做）（作者 **MiMo mimo-v2.6-flash**） | 历史评估 |
| `10-v0.5.0-验收报告.md` | 全量 DoD 复跑 + **P2-7 CDP 12/12**（第九节 + 截图归档 `design/screenshots-v0.5.0/`） | 历史版本验收 |
| `11-提供商与模型接口路由.md` | 三元组、复合卡片内多模型及历史实证 | 历史设计与证据；现行语义见契约和主计划 |
| `12-路线收缩与三线作战计划.md` | **双态战略（本地可路由·云端可中转）** + 冻结清单 + UI/Docker/Tauri 三线计划 | 历史范围基线，后续明确决定优先 |
| [design/IMPLEMENTATION-PLAN.md](design/IMPLEMENTATION-PLAN.md) | 早期完整实施草案与当时的计划假设 | 历史参考，已由当前主计划和发版手册取代；其中旧版本/tag 指令不可执行 |
| **[contracts/00-INDEX.md](contracts/00-INDEX.md)** | **六份基线契约与本轮入口网关修订的入口** | **历史基线保留；最新实现状态见当前主计划** |
| `contracts/01-协议契约.md` | Jev 内核类型与不变量 | 定稿 |
| `contracts/02-扩展点契约.md` | serde 同构：冻结 trait + 黄金测试 | 定稿 |
| `contracts/03-路由契约.md` | **模型路由 DAG**（二部图 = 最简形式） | 定稿 |
| `contracts/04-密钥与防偷.md` | 明文 toml + 防偷红线 | 定稿 |
| `contracts/05-HTTP契约.md` | `/v1/*` 形状 + 错误体 + CORS | 定稿 |
| `contracts/06-前端交互契约.md` | CC Switch 范式 + 三页 IA | 定稿 |
| [contracts/07-入口网关修订.md](contracts/07-入口网关修订.md) | 接入配置、统一持久化、入口 API、直接上游调用及 UI 修订 | 契约参考；v0.1.0 的实现和验收边界见当前主计划 |
| `design/01-frontend-design-sim.md` | 早期前端与交互设计模拟稿（作者 Mimo-V2.6-Pro） | 历史视觉参考；当前页面结构以主计划和截图为准 |
| `design/02-竞标方案.md` | 早期 A/B/C 视觉方案比较 | 历史预览，不代表当前 UI |
| `design/DATABASE-MIGRATION-PLAN.md` | 初版 SQLite 迁移方案（作者 Claude Opus 4.8） | 旧状态“待实施”已过期；迁移现状以主计划和契约为准 |
| `design/ICON-SYSTEM-DESIGN.md` | 初版图标系统方案（作者 Claude Opus 4.8） | 原稿状态为历史；当前 Lucide 应用范围和未覆盖页面见 UI 图标/性能核验 |
| `design/ROUTING-INTERACTION-SPEC-v2.md` | **交互 DAG 精确接线规范**（磁吸、拖拽、端口、撤销） | 设计规范参考；原稿阶段状态已过期，当前实现/验收以主计划为准 |
| `design/I18N-DESIGN.md` | **国际化与可扩展语言列表**（作者 Claude Opus 4.8；GPT-6 Luna 实施记录） | 2026-09-24 状态快照；当前已提供语言见 README，新增语言方向见 ROADMAP |
| `RELEASE.md` | **发版手册**（版本门禁 / 流水线结构 / Windows 与 CLI 产物） | `v0.4.1` 已发布 Windows、Linux/Windows CLI 与 Docker 资产；桌面核心人工证据沿用 v0.4.0 |
| [screenshots.md](screenshots.md) | Dashboard 活动、Routing 入口/DAG 与 Playground 产品截图 | 用户提供的 v0.1.0 历史 UI 图册，不替代当前版截图 |
| `I18N-GUIDE.md` | **国际化开发规范**（贡献者指南）（作者 Claude Opus 4.8） | 已完成 |
| [verification/termux-build-deployment-2026-09-27.md](verification/termux-build-deployment-2026-09-27.md) | **Termux / Android arm64 构建与 daemon 烟测历史** | 点时源码固定到 `f1670e7`；后续 CLI 已实现，见 CLI 文档 |
| [CLI.md](CLI.md) | **Termux/SSH/headless CLI Phase 1**：状态、模型、调用、路由与事件读取 | v0.4.0 独立 Rust 客户端；TUI 暂未包含 |
| [verification/cli-phase1-2026-09-30.md](verification/cli-phase1-2026-09-30.md) | CLI 自动测试与 local daemon smoke | v0.4.0 实现证据；Termux 真机/cloud 会话仍未验证 |
| [verification/ui-icon-performance-2026-09-30.md](verification/ui-icon-performance-2026-09-30.md) | 导航/Dashboard/Routing 图标化、共享 health poll、并发 provider probe 和 telemetry 后台暂停 | 当前未发布工作树验证；本机 fixture 数据不代表跨设备结论 |
| [verification/v0.4.0-release-candidate-2026-09-30.md](verification/v0.4.0-release-candidate-2026-09-30.md) | v0.4.0 自动化、Windows 原生隔离核验与维护者人工桌面验收 | 安装器/Android/全链路性能仍未覆盖 |
| [verification/android-apk-build-2026-09-30.md](verification/android-apk-build-2026-09-30.md) | Android arm64 unsigned APK 本机构建到 Gradle/AAPT 产物的核验 | 构建成功；未安装、未启动模拟器、未做 service/tile/后台验收 |
| [verification/v0.4.1-release-candidate-2026-09-30.md](verification/v0.4.1-release-candidate-2026-09-30.md) | v0.4.1 自动门禁、Android 构建证据与人工验收引导 | 自动验证完成；新桌面包/Android 安装和真机生命周期待人工验收 |
| [design/ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md](design/ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md) | Android APP 实验性完整网关计划：daemon library、FlClash 风格 service/action/通知/tile、普通移动端与生命周期验收边界 | 启停原型在工作树；尚无 Android APK、原生 service/tile 或真机证据 |

## 阅读顺序（开发 Agent）

1. `design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md`（最新用户决策、范围与施工顺序）
2. `OPUS5-GOAL-REASSESSMENT.md`（原始目标、当前缺口与运行证据）
3. `../ROADMAP.md`（未来候选，不作为当前版本能力承诺）
4. 路由交互与国际化专项；服务入口和 Playground 两份专项文档仅作历史设计参考
5. `contracts/00-INDEX` → 01–06 基线与 07 修订，再按需查 `08-CONTRACT`（保留未变更的不变量，后续批准的修订优先）
6. `07-REVIEW`、历史验收与 `02`–`06`（需要追溯决策演变时阅读，不作当前范围或完成证明）
7. `PROGRESS.md`（MVP 历史实测）

旧设计稿、会话产物和专项计划用于解释设计演变，不作为当前能力或待办状态的来源。`12-路线收缩与三线作战计划.md` 保留为 2026-09-23 的范围收缩决策记录；后续明确授权和最新产品决策优先。当前实施与验收以主计划及对应版本记录为准；未来候选只记在 `../ROADMAP.md`。保留的历史记录均按各自标注的时间点和适用范围理解，不代表现行效力。

## 早期对齐定稿速查（历史）

下表保留早期裁决。当前配置权威来源与 TOML/SQLite 迁移需按最新计划 P1 统一，不能据此保留两套互不生效的配置。

| Q | 决定 |
|---|---|
| Q1 | serde 同构 = **编译期 trait**；内核与传参零改 |
| Q2 | 先 **A+B**（交互+视觉靠 CC Switch），稳定后再 **C**（Tauri） |
| Q3 | **契约 → 并行 AB → 双剑合璧** |
| Q4 | 密钥明文 toml + `.gitignore`；**UI 必须密文** |
| Q5 | **以文件为准**；外部修改 → 重载/覆盖 |
| Q6 | 防偷强化：上游 key 不出 daemon；无读回明文 API |

## 路由形态

**调用路由 DAG**：左侧对外入口卡片，中间可编辑路由节点，右侧提供商复合卡片内的模型端口；从左到右呈现并支持多跳、分支与精确接线。二部图是简单特例，不是最终能力上限。详见最新计划与 `contracts/03-路由契约.md`。

## 代码布局

```
rs/     Rust workspace：protocol / core / adapters / daemon
ui/     React 五页控制台：仪表盘、提供商、入口、路由 DAG 与演练场
docs/   本目录
```

## 与生态

- 协议真值：TypeSafe + `jev-life/src/shared/types.ts`
- 路由参考：`Wei-Shaw/sub2api`
- 桌面形态参考：`farion1231/cc-switch`
- 种子数据：`jev-decision-lab`
- 参考实现：`alvarobartt/sys1`、`Yinsongxu/LLM2Jev`

— 原索引：Mimo-V2.6-Pro；本轮维护与 AI 披露：GPT-6 Luna xhigh（OpenAI Codex，2026-09-27）
