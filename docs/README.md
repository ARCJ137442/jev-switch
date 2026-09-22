# Jev-Switch 文档索引

> 本仓库 `H:\A137442\Develop\AI\Jev\Jev-Switch` 是 **Jev-Switch** 的**规划与文档仓库**。代码与原型在同级的 `jev-decision-lab/`（`H:\A137442\Develop\AI\Jev\jev-decision-lab\`）。

## 文档列表

| 文件 | 内容 | 状态 |
|---|---|---|
| `01-Jev-Switch-原始计划书-DeepSeek.md` | DeepSeek 起草的 Jev-Switch 原始计划书（4 条需求 + 调研快照 + 战略判断 + MVP + 风险） | baseline |
| `02-Jev-Switch-新版计划书.md` | 经过深度调研、与本仓库已有代码对齐后的新版计划书（v2.1） | 现行版 |
| `03-上游类别与协议兼容矩阵.md` | 5 类上游（TypeSafe 云 / 第三方网关 / LLM-broker / 本地独立服务 / 真 Jev 模型）的协议兼容差异 | 待写 |
| `04-架构设计-from-sys1-借鉴.md` | 借鉴 sys1 (Rust/axum) + 我们的 jev-decision-lab 实测，Rust 后端 + React 前端的具体设计 | 待写 |
| `05-从-jev-life-超越的设计点.md` | 我们的目标比 jev-life 原 TypeScript 网关青出于蓝的具体技术点 | 待写 |

## 核心定位（一句话）

> **Jev-Switch = 本地运行的 Jev 协议多上游路由器，与 LM Studio / CC Switch 同生态位，独特价值是 "Jev 原生入口 × 多上游切换 × 桥接模式"。**

## 阅读顺序

1. 先读 `01-` 原始计划书（baseline，DeepSeek 著）
2. 再读 `02-` 新版计划书（v2.1，含 5 类上游定义 + 桥接模式）
3. 具体技术决策看 `03-/04-/05-` 主题文档（待写）

## 与 jev-decision-lab 的关系

- **Jev-Switch**（本仓库）：规划与文档
- **jev-decision-lab**（同级）：**种子原型 + 调研数据**
  - TS 实现 4 Runtime（local-qwen / openrouter / vercel / laya）
  - 实测 5 provider × 4 version × 12 题
  - 实测 80 篇 × 14 标签 life-series benchmark
  - 已有 7 commits + GitHub 私有仓库
- **后续**：jev-decision-lab 的 Rust 重写 + UI 在 `Jev-Switch/rs/` 和 `Jev-Switch/ui/`

## 生态参考

- `alvarobartt/sys1`（Rust + axum 单后端 Jev）— 借鉴目标
- `Yinsongxu/LLM2Jev`（Python prefill-only，133 stars）— C4 实现
- `antTing/jev-accounts-hub`（Go TypeSafe 单 provider 多账户）— 互补
- `ARCJ137442/jev-life`（TypeScript LLM-Jev broker）— broker 设计参考
- `farion1231/cc-switch`（Tauri LLM 路由）— 桌面应用形态参考
- `heyjunpenn/awesome-jev`（640 项生态列表）