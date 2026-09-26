# Jev-Switch 文档索引

> **本仓库**：`https://github.com/ARCJ137442/jev-switch`（private）  
> **定位**：**Jev-Switch = 模型调用入口之间进行可配置转换的轻量网关**（Rust 内核 + React 控制台，支持本地/云端方向与 Tauri/Docker 交付）
> **当前核实状态（2026-09-26 11:51，北京时间）**：P1–P4 产品与 Web 能力已实现并分项验；正式 AppData 有公开 Laya/Vercel 成功记录，直连上游持久追踪已加入当前源码与测试，但仍待当前运行版验证。Windows 当前批次 MSI、NSIS、便携程序已生成，WiX 引用的壳、daemon、HTML/JS/CSS 哈希逐项与便携输入一致；最新便携目录为 `E:\tmp\jev-switch-portable-final-20260926-115041-986`。Jev 未运行，11435 空闲，Laya `18767` 仍运行，AppData 17 条历史保留。环境没有 Docker CLI，故未重跑 Docker 验收；便携冷启动、真实调用追踪、Tauri WebView/托盘与 P6 最新截图、发布收尾仍待完成。详见[联调记录](verification/2026-09-24-entry-gateway-integration.md)；历史结论只适用于各自版本与覆盖范围。
> **当前施工入口**：[入口网关认知对齐、决策与实施计划](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)。便携运行时需整体保留壳、sidecar 与 UI 资源，配置仍从正式 AppData 读取；可用 `scripts/build-windows-release.ps1` 重建 MSI/NSIS 与便携包。总计划 §3/§3.1 记录决定及八条批注，§7/§8 和文末最新增量记录当前验收状态。

## 核心定位（一句话）

> **对外入口卡片 → 可交互调用路由 DAG → 提供商复合卡片内的模型端口**。两端都是模型调用入口，网关负责可配置转换；演练场支持两类入口横比。

## 文档列表

| 文件 | 内容 | 状态 |
|---|---|---|
| **[design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md](design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md)** | **本轮用户决策、两侧入口定义、卡片粒度、DAG、比较范围、实施路线与验收门槛** | **当前施工入口；P1–P4 已验，P5/P6 未收口** |
| [OPUS5-GOAL-REASSESSMENT.md](OPUS5-GOAL-REASSESSMENT.md) | 原会话目标恢复、源码/运行核查与缺口证据 | 评估完成；不是产品完成声明 |
| [design/SERVICE-ENDPOINT-CONFIG-PLAN.md](design/SERVICE-ENDPOINT-CONFIG-PLAN.md) | 对外入口 CRUD、持久化、实时生效及策略专项 | 当前工作树已实现并分项验证；最新版安装验收待 P5 |
| [design/PLAYGROUND-COMPARISON-PLAN.md](design/PLAYGROUND-COMPARISON-PLAN.md) | 两类入口横比、目标身份、执行边界、布局与验收 | 多目标执行/直连持久追踪已实现；最新版 UI 验收待 P5 |
| `01-Jev-Switch-原始计划书-DeepSeek.md` | DeepSeek 原始计划书（4 条需求 + 调研 + MVP + 风险） | baseline |
| `02-Jev-Switch-新版计划书.md` | v2.1：5 类上游 + 桥接模式 + M1–M7 | 历史愿景，不能整体重启 |
| `03-上游类别与协议兼容矩阵.md` | C1–C5 字段矩阵 + Vercel noul→boolean | 已写 |
| `04-架构设计-from-sys1-借鉴.md` | Rust workspace 三 crate + React + Tauri | 已写 |
| `05-从-jev-life-超越的设计点.md` | 相对 jev-life 的 5 个超越点 | 已写 |
| `06-MVP-实现计划.md` | M0 十一步施工清单 | 已执行（见 PROGRESS） |
| `PROGRESS.md` | `v0.1.0-mvp` 进度快照与实测 | 封存 |
| **`07-REVIEW-v0.1.0-mvp.md`** | **全方位评审**（作者 **Mimo-V2.6-Pro**） | **定稿** |
| **`08-CONTRACT-功能特性契约与施工计划.md`** | **对齐定稿 Q1–Q6 + 施工计划**（作者 **Mimo-V2.6-Pro**） | **定稿** |
| `09-用户叙事评估报告-MiMo.md` | 10 叙事 vs v0.1.0 匹配度 + 三标记（openai-compat 不做 / fallback·shadow 待讨论 / 端口 11435 已做）（作者 **MiMo mimo-v2.6-flash**） | 已存档 |
| `10-v0.5.0-验收报告.md` | 全量 DoD 复跑 + **P2-7 CDP 12/12**（第九节 + 截图归档 `design/screenshots-v0.5.0/`） | 历史版本验收 |
| `11-提供商与模型接口路由.md` | 三元组、复合卡片内多模型及历史实证 | 同地址多账号旧限制已被本轮裁决更新 |
| `12-路线收缩与三线作战计划.md` | **双态战略（本地可路由·云端可中转）** + 冻结清单 + UI/Docker/Tauri 三线计划 | 范围基线，结合后来批准专项阅读 |
| **[contracts/00-INDEX.md](contracts/00-INDEX.md)** | **六份基线契约与本轮入口网关修订的入口** | **历史基线保留；修订实施中** |
| `contracts/01-协议契约.md` | Jev 内核类型与不变量 | 定稿 |
| `contracts/02-扩展点契约.md` | serde 同构：冻结 trait + 黄金测试 | 定稿 |
| `contracts/03-路由契约.md` | **模型路由 DAG**（二部图 = 最简形式） | 定稿 |
| `contracts/04-密钥与防偷.md` | 明文 toml + 防偷红线 | 定稿 |
| `contracts/05-HTTP契约.md` | `/v1/*` 形状 + 错误体 + CORS | 定稿 |
| `contracts/06-前端交互契约.md` | CC Switch 范式 + 三页 IA | 定稿 |
| [contracts/07-入口网关修订.md](contracts/07-入口网关修订.md) | 接入配置、统一持久化、入口 API、直接上游调用及 UI 修订 | 当前工作树实现与回归已完成；最新安装版运行验收待 P5 |
| **`design/01-frontend-design-sim.md`** | **前端与交互设计稿**（模拟 `/frontend-design` 流程，作者 Mimo-V2.6-Pro） | 定稿 |
| `design/UI-REDESIGN-v2.md` | 历史 UI 导航与视觉草图 | 冲突以最新计划为准 |
| `design/ROUTING-INTERACTION-SPEC-v2.md` | **交互 DAG 精确接线规范**（磁吸、拖拽、端口、撤销） | 已补两端语义，交互待完整验收 |
| `design/I18N-DESIGN.md` | **国际化与可扩展语言列表** | 基础机制存在，覆盖与扩展待验收 |
| `RELEASE.md` | **发版手册**（四处版本号门禁 / 流水线结构 / 一次性 Windows 最终验收） | workflow 已配置便携 ZIP；本轮候选包尚未发布 |
| `design/IMPLEMENTATION-PLAN.md` | 旧 SSE、权限、统计实施计划 | 专项目标保留，按最新计划重验 |
| `E2E-VERIFICATION-REPORT.md` | 旧端到端验证声明 | 不能替代当前版本实测，结合目标重评阅读 |
| `I18N-GUIDE.md` | **国际化开发规范**（贡献者指南）（作者 Claude Opus 4.8） | 已完成 |

## 阅读顺序（开发 Agent）

1. `design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md`（最新用户决策、范围与施工顺序）
2. `OPUS5-GOAL-REASSESSMENT.md`（原始目标、当前缺口与运行证据）
3. `design/SERVICE-ENDPOINT-CONFIG-PLAN.md`、`design/PLAYGROUND-COMPARISON-PLAN.md`、路由交互与国际化专项
4. `contracts/00-INDEX` → 01–06 基线与 07 修订，再按需查 `08-CONTRACT`（保留未变更的不变量，后续批准的修订优先）
5. `12-路线收缩与三线作战计划.md`（范围基线，后续明确授权优先）
6. `07-REVIEW`、历史验收与 `02`–`06`（了解原因与演变，不作当前完成证明）
7. `PROGRESS.md`（MVP 历史实测）

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
ui/     React 控制台；入口配置、完整 DAG 与两类比较按最新计划补齐
docs/   本目录
```

## 与生态

- 协议真值：TypeSafe + `jev-life/src/shared/types.ts`
- 路由参考：`Wei-Shaw/sub2api`
- 桌面形态参考：`farion1231/cc-switch`
- 种子数据：`jev-decision-lab`
- 参考实现：`alvarobartt/sys1`、`Yinsongxu/LLM2Jev`

— 原索引：Mimo-V2.6-Pro；本轮决策与状态索引更新：GPT-6 Astra / Codex（2026-09-24）
