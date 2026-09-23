# Jev-Switch 文档索引

> **本仓库**：`https://github.com/ARCJ137442/jev-switch`（private）  
> **定位**：**Jev-Switch = 本地 Jev 协议多上游路由器**（Rust 后端 + React 前端 + 未来 Tauri）  
> **当前阶段**：`v0.1.0-mvp` 已封存 → **Phase 0 契约正文已完成** → 待并行后端 A / 前端 B

## 核心定位（一句话）

> **Jev 原生入口 × 多上游切换 × 桥接模式** —— 与 LM Studio / CC Switch 同生态位。

## 文档列表

| 文件 | 内容 | 状态 |
|---|---|---|
| `01-Jev-Switch-原始计划书-DeepSeek.md` | DeepSeek 原始计划书（4 条需求 + 调研 + MVP + 风险） | baseline |
| `02-Jev-Switch-新版计划书.md` | v2.1：5 类上游 + 桥接模式 + M1–M7 | 现行规划 |
| `03-上游类别与协议兼容矩阵.md` | C1–C5 字段矩阵 + Vercel noul→boolean | 已写 |
| `04-架构设计-from-sys1-借鉴.md` | Rust workspace 三 crate + React + Tauri | 已写 |
| `05-从-jev-life-超越的设计点.md` | 相对 jev-life 的 5 个超越点 | 已写 |
| `06-MVP-实现计划.md` | M0 十一步施工清单 | 已执行（见 PROGRESS） |
| `PROGRESS.md` | `v0.1.0-mvp` 进度快照与实测 | 封存 |
| **`07-REVIEW-v0.1.0-mvp.md`** | **全方位评审**（作者 **Mimo-V2.6-Pro**） | **定稿** |
| **`08-CONTRACT-功能特性契约与施工计划.md`** | **对齐定稿 Q1–Q6 + 施工计划**（作者 **Mimo-V2.6-Pro**） | **定稿** |
| `09-用户叙事评估报告-MiMo.md` | 10 叙事 vs v0.1.0 匹配度 + 三标记（openai-compat 不做 / fallback·shadow 待讨论 / 端口 11435 已做）（作者 **MiMo mimo-v2.6-flash**） | 已存档 |
| **`contracts/00-INDEX.md`** | **六份契约正文入口**（作者 **Mimo-V2.6-Pro**） | **Phase 0 完成** |
| `contracts/01-协议契约.md` | Jev 内核类型与不变量 | 定稿 |
| `contracts/02-扩展点契约.md` | serde 同构：冻结 trait + 黄金测试 | 定稿 |
| `contracts/03-路由契约.md` | **模型路由 DAG**（二部图 = 最简形式） | 定稿 |
| `contracts/04-密钥与防偷.md` | 明文 toml + 防偷红线 | 定稿 |
| `contracts/05-HTTP契约.md` | `/v1/*` 形状 + 错误体 + CORS | 定稿 |
| `contracts/06-前端交互契约.md` | CC Switch 范式 + 三页 IA | 定稿 |
| **`design/01-frontend-design-sim.md`** | **前端与交互设计稿**（模拟 `/frontend-design` 流程，作者 Mimo-V2.6-Pro） | 定稿 |

## 阅读顺序（开发 Agent）

1. `08-CONTRACT`（对齐定稿，不可再开）
2. `contracts/00-INDEX` → 01–06 正文
3. `07-REVIEW`（为何这么定 + P0/P1/P2）
4. `02`–`06` 历史规划（冲突以 07/08/contracts 为准）
5. `PROGRESS`（MVP 实测基线）

## 对齐定稿速查

| Q | 决定 |
|---|---|
| Q1 | serde 同构 = **编译期 trait**；内核与传参零改 |
| Q2 | 先 **A+B**（交互+视觉靠 CC Switch），稳定后再 **C**（Tauri） |
| Q3 | **契约 → 并行 AB → 双剑合璧** |
| Q4 | 密钥明文 toml + `.gitignore`；**UI 必须密文** |
| Q5 | **以文件为准**；外部修改 → 重载/覆盖 |
| Q6 | 防偷强化：上游 key 不出 daemon；无读回明文 API |

## 路由形态

**模型路由 DAG**（主词，可多跳、边上带 priority/sticky/on_error）；  
**二部图 = 其中最简单的形式**，作默认查看/编辑视图，实践后再定是否升级通用 DAG 视图。详见 `contracts/03-路由契约.md`。

## 代码布局

```
rs/     Rust 后端（MVP：protocol / translate / upstream / router / axum daemon）
ui/     React UI（Playground 已可跑；Providers/Routing 页待 B 阶段）
docs/   本目录
```

## 与生态

- 协议真值：TypeSafe + `jev-life/src/shared/types.ts`
- 路由参考：`Wei-Shaw/sub2api`
- 桌面形态参考：`farion1231/cc-switch`
- 种子数据：`jev-decision-lab`
- 参考实现：`alvarobartt/sys1`、`Yinsongxu/LLM2Jev`

— 索引维护：Mimo-V2.6-Pro
