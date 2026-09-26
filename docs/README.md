# Jev-Switch 文档索引

> **TL;DR：**当前正式版为 `v0.1.0`。先读入口网关实施计划；未排期的未来方向见 [ROADMAP.md](../ROADMAP.md)。
>
> **本轮索引修订：**GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理，2026-09-27。
>
> **本仓库**：[`ARCJ137442/jev-switch`](https://github.com/ARCJ137442/jev-switch)（public）<br>
> **定位**：**Jev-Switch = 模型调用入口之间进行可配置转换的轻量网关**（Rust 内核 + React 控制台，支持本地/云端方向与 Tauri/Docker 交付）
> **当前核实状态（2026-09-27）**：正式 [v0.1.0 Release](https://github.com/ARCJ137442/jev-switch/releases/tag/v0.1.0) 已由 tag 流水线发布，含 Windows MSI、NSIS、便携 ZIP 与 Docker 镜像。Release gate 的 Rust、UI、Tauri 与版本检查通过。验收证据覆盖 2 家上游、9 条路由、7 个公开入口和 34 条调用记录。Laya 不重复实测。你提供的 Tauri 截图已整理为公开安全图册，并确认当时安装版“关窗留托盘、点菜单恢复”通过；它不证明 Release 候选的精确 build identity、托盘 Exit 或退出后恢复。凭据配置和本机运行目录不纳入发布包。详见[计划](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)、[便携运行记录](verification/portable-runtime-acceptance-2026-09-26.md)及[截图图册](screenshots.md)。历史结论只适用于各自版本与覆盖范围。
> **当前产品基线与维护阶段**：网关实施及验收状态以[入口网关认知对齐与实施计划](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)为准；v0.1.0 已发布。本阶段工作为发布后公开文档校准和两份 Awesome PR 跟进，不继续实现未排期路线图功能。便携运行时需整体保留壳、sidecar 与 UI 资源，配置仍从正式 AppData 读取；可用 `scripts/build-windows-release.ps1` 重建 MSI/NSIS 与便携包。总计划 §3/§3.1 记录决定及八条批注，顶部发布后状态与对应证据节记录现行验收边界。

## 核心定位（一句话）

> **对外入口卡片 → 可交互调用路由 DAG → 提供商复合卡片内的模型端口**。两端都是模型调用入口，网关负责可配置转换；演练场支持两类入口横比。

## 文档列表

| 文件 | 内容 | 状态 |
|---|---|---|
| **[../ROADMAP.md](../ROADMAP.md)** | **未来功能方向、优先级和可验收边界** | TypeSafe 官方/OpenRouter 适配器为最高优先级；不代表当前已实现或已排期 |
| [USER-JOURNEYS.md](USER-JOURNEYS.md) | 当前基线说明与路线图各方向对应的具体用户旅程 | 早期场景已标为历史构想；第七、八节覆盖路线图中的未来体验提案，不表示已实现 |
| **[design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)** | **本轮用户决策、两侧入口定义、卡片粒度、DAG、比较范围、实施路线与验收门槛** | **当前施工入口；P1–P4 已验；P5 有用户截图与 Hide/Restore 证据，候选 Exit/恢复边界仍开放；P6 v0.1.0 已发布** |
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
| **`design/01-frontend-design-sim.md`** | **前端与交互设计稿**（模拟 `/frontend-design` 流程，作者 Mimo-V2.6-Pro） | 定稿 |
| `design/ROUTING-INTERACTION-SPEC-v2.md` | **交互 DAG 精确接线规范**（磁吸、拖拽、端口、撤销） | 设计规范参考；原稿阶段状态已过期，当前实现/验收以主计划为准 |
| `design/I18N-DESIGN.md` | **国际化与可扩展语言列表** | 基础机制存在，覆盖与扩展待验收 |
| `RELEASE.md` | **发版手册**（四处版本号门禁 / 流水线结构 / 一次性 Windows 最终验收） | `v0.1.0` 已发布 MSI/NSIS/便携 ZIP 与 Docker 镜像 |
| [screenshots.md](screenshots.md) | Dashboard 活动、Routing 入口/DAG 与 Playground 产品截图 | 公开筛选的 v0.1.0 界面图册 |
| `I18N-GUIDE.md` | **国际化开发规范**（贡献者指南）（作者 Claude Opus 4.8） | 已完成 |

## 阅读顺序（开发 Agent）

1. `design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md`（最新用户决策、范围与施工顺序）
2. `OPUS5-GOAL-REASSESSMENT.md`（原始目标、当前缺口与运行证据）
3. `../ROADMAP.md`（未来候选，不作为当前版本能力承诺）
4. 路由交互与国际化专项；服务入口和 Playground 两份专项文档仅作历史设计参考
5. `contracts/00-INDEX` → 01–06 基线与 07 修订，再按需查 `08-CONTRACT`（保留未变更的不变量，后续批准的修订优先）
6. `12-路线收缩与三线作战计划.md`（范围基线，后续明确授权优先）
7. `07-REVIEW`、历史验收与 `02`–`06`（了解原因与演变，不作当前完成证明）
8. `PROGRESS.md`（MVP 历史实测）

旧设计稿、会话产物和专项计划用于解释设计演变，不作为当前能力或待办状态的来源。当前实现与验收以主计划及对应版本记录为准；未来候选只记在 `../ROADMAP.md`。已从当前阅读路径撤下的文档仍可能留在仓库，清理状态以工作任务记录为准，不据此声称文件已删除。

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
ui/     React 四页控制台：仪表盘、提供商、路由与演练场
docs/   本目录
```

## 与生态

- 协议真值：TypeSafe + `jev-life/src/shared/types.ts`
- 路由参考：`Wei-Shaw/sub2api`
- 桌面形态参考：`farion1231/cc-switch`
- 种子数据：`jev-decision-lab`
- 参考实现：`alvarobartt/sys1`、`Yinsongxu/LLM2Jev`

— 原索引：Mimo-V2.6-Pro；本轮维护与 AI 披露：GPT-6 Luna xhigh（OpenAI Codex，2026-09-27）
