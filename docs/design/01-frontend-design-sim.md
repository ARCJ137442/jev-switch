# Jev-Switch · 前端与交互设计（frontend-design 流程模拟稿）

> **作者**：Mimo-V2.6-Pro  
> **日期**：2026-09-23  
> **性质**：**模拟 `/frontend-design` skill 的流程、产出物与设计要求**写成的综合设计稿（按指示**未真实调用**该 skill）。  
> **上游契约**：`docs/contracts/06-前端交互契约.md`、`03-路由契约.md`、`04-密钥与防偷.md`、`05-HTTP契约.md`  
> **对齐**：Q2 = **先 A+B**（交互范式 + 视觉向 **CC Switch** 靠），稳定后再 **C**（Tauri）  
> **读者**：前端 B 阶段的实现 Agent。**照此施工**；与契约冲突时契约优先。

---

## 0. 模拟的 skill 流程（做了什么）

`/frontend-design` 类 skill 的典型管线（本稿按此走完）：

```text
① 设计简报     从用户/契约提炼目标、约束、非目标
② 参考解构     CC Switch 与现状 UI：借什么 / 不借什么
③ 设计原则     反 AI 套路、可生产、可扩展
④ 设计系统     字色阶、密度、组件原子、状态
⑤ 信息架构     三页 + 全局壳
⑥ 逐页规格     布局、组件、状态机、空/错/载
⑦ 关键交互     模型路由 DAG · 二部图连线题
⑧ 可访问性     键盘、对比度、焦点、aria
⑨ 实现约束     代码形态、性能、与 API 对齐
⑩ 验收清单     可勾选的 DoD
```

---

## 1. 设计简报（Brief）

| 项 | 内容 |
|---|---|
| **产品** | Jev-Switch —— 本地 Jev 协议多上游路由器的控制台 |
| **用户** | 开发者 / 本地单人（信任开发者，但密钥暴露面必须密文） |
| **目标** | ① 一眼看清提供商健康 ② 像接电路一样配置模型路由 ③ 快速试一次 Jev 决策 |
| **成功标准** | 新用户 < 3 分钟配好一个上游并跑通一次 Run Jev；改路由 < 10 秒完成一次拖线 |
| **约束** | 文件 = 真值源（Q5）；UI 密文密钥（Q4/Q6）；Web 优先，Tauri 后置（Q2） |
| **非目标** | 多租户、移动优先、暗色主题（本期）、i18n 全量（预留 zh/en 键） |

---

## 2. 参考解构：CC Switch 借什么 / 不借什么

| 维度 | 借（A+B） | 不借 / 延后 |
|---|---|---|
| **交互范式** | 提供商**卡片**、启停**开关**、密钥**表单**、**Probe/体检**、一键切换启用 | 系统托盘、原子改写 Claude Code 配置文件 |
| **视觉** | 工具向、信息密度高、状态灯号、列表可扫读 | 拟物插画、营销 hero、大段介绍文案 |
| **桌面形态** | — | Tauri 壳、全局快捷键、单实例锁 → **阶段 C** |
| **配置权威** | — | UI 即真值源 → **拒绝**（Q5 文件为准） |

**现状 UI（jevplayground 黑白等宽）的处理**：  
Playground 页**保留**极简黑白等宽 DNA；Providers / Routing 采用「CC Switch 工具密度 × 现有单色细线」的融合，而不是两套皮肤。

---

## 3. 设计原则（反 AI 套路 · 可生产）

1. **工具感 > 展示感** —— 界面是仪表盘不是落地页；无「✨」「🚀」、无渐变彩虹、无假 AI 光晕。
2. **单色为主，语义色只表状态** —— 默认黑/白/灰；绿=健康、琥珀=降级、红=错误；禁用装饰性彩色。
3. **等宽数字与 id** —— 延迟、概率、token、model id 一律 `tabular` + mono。
4. **边框分隔，不用阴影堆卡片** —— 沿用现有 `border-border` 1px 纪律。
5. **状态永远可见** —— 启停、健康、同步冲突（外部改 toml）必须在 UI 呈现，禁止静默。
6. **密文默认** —— 任何密钥呈现先想「截图会不会泄露」。
7. **一等公民：失败** —— 空态/错误态/加载态与成功态同级设计。
8. **可键盘完成主路径** —— 配路由、跑决策不强制鼠标（连线编辑器提供表单备选）。

---

## 4. 设计系统（Tokens）

### 4.1 色

| Token | 值 | 用途 |
|---|---|---|
| `bg` | `#fafafa` | 页面底 |
| `panel` | `#ffffff` | 面板 |
| `ink` | `#0a0a0a` | 主文字 / 主 CTA |
| `inkMuted` | `#525252` | 次要文字 |
| `inkSubtle` | `#a3a3a3` | 标签、占位 |
| `border` | `#e5e5e5` | 1px 分隔 |
| `ok` | `#059669` | 启用 / 健康 |
| `warn` | `#d97706` | 降级 / 外部修改 |
| `danger` | `#dc2626` | 错误 / 熔断 |
| `edge` | `#0a0a0a` | DAG 选中边 |
| `edgeIdle` | `#d4d4d4` | DAG 默认边 |

### 4.2 字

| 用途 | 字体 | size |
|---|---|---|
| UI 正文 | Inter / system | 13–14px |
| 标签/eyebrow | JetBrains Mono · uppercase · tracking-widest | 10px |
| id / URL / JSON | Mono | 12–13px |
| 数字 | Mono + `tabular-nums` | 12–13px |

### 4.3 密度与形状

- 圆角：`0`（按钮/输入/面板）—— 与现网一致；CC Switch 的圆角**不**照搬
- 间距：4px 基准；卡片内边 `12×16`
- 高度：控件 32px；紧凑行 40px
- 焦点：`outline: 1px solid ink; outline-offset: 2px`（无光晕）

---

## 5. 信息架构

```text
┌──────────────────────────────────────────────────────────┐
│  [J] Jev-Switch    Providers · Routing · Playground       │
│      ● daemon ok          masked-key hint    v0.1.0      │
├──────────────────────────────────────────────────────────┤
│  ⚠ 配置已在外部修改    [重载] [覆盖]          ← 冲突横幅  │
├──────────────────────────────────────────────────────────┤
│                                                          │
│   <Page = Providers | Routing | Playground>              │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  endpoint 127.0.0.1:11435 · /v1/systemone ·  file=truth  │
└──────────────────────────────────────────────────────────┘
```

| 页 | 路由（建议） | 职责 |
|---|---|---|
| Providers | `/#/providers` | 上游 CRUD + 密钥 + Probe |
| Routing | `/#/routing` | **模型路由 DAG** 编辑（默认二部图） |
| Playground | `/#/playground`（默认） | 试跑 Jev 决策 |

全局槽位：**冲突横幅**、toast、daemon 状态灯。

---

## 6. 页规格

### 6.1 Providers

**布局**：左窄「工具栏」+ 右侧卡片流（1 列移动 / 2 列 ≥1024px）

**卡片结构**

```text
┌──────────────────────────────────────────────┐
│ ● vercel                    [Probe]  [ENABLED│OFF]
│ vercel-gateway                               │
│ https://ai-gateway.vercel.sh/v4/ai/…         │
│ key  sk-****a1b2              [Replace key] │
│ last  42ms · 2h ago    ▁▂▃  sparkline(可选) │
└──────────────────────────────────────────────┘
```

| 元素 | 行为 |
|---|---|
| 状态点 | `ok/warn/danger/subtle`；仅语义色 |
| `ENABLED` 开关 | 即时 `PUT`；失败 toast + 状态回滚 |
| `Replace key` | 内联表单，**只出密文占位** `••••••••`；无 Show 明文 |
| `Probe` | 调 `POST /v1/admin/providers/{id}/probe`；按钮 loading；结果写卡片 footer |
| `+ Add provider` | 顶部工具条；表单 or「粘贴 toml 片段」Tab |
| 删除 | 二次确认（打字 id）；**不**清 toml 备份注释 |

**状态机**：`idle → loading → ok | error`；开关 `optimistic + rollback`。

**空态**：「还没有提供商」+ 贴 toml 引导 + 指向 providers.example.toml。

### 6.2 Routing（模型路由 DAG）

**布局**：上方工具条 + 中央图画布 + 右侧「边检视器」抽屉

#### 主视图：二部图连线题（最简 DAG）

```text
   对外 model id                         提供商
┌─────────────┐                        ┌─────────────┐
│ jev     [•] │━━━━━━━━━━━━━━━━━━━━━━━▶│ vercel  [•] │  p=10
│             │━━━━━━━━━┐              ├─────────────┤
├─────────────┤         │              │ laya    [•] │  p=30
│ local/* [~] │━━━━━━━━─┼─────────────▶├─────────────┤
├─────────────┤         │              │ typesafe[•] │
│ jev-fast[a] │◀────────┘  (别名节点)   └─────────────┘
└─────────────┘
```

| 交互 | 规格 |
|---|---|
| 拖线 | 从左节点锚点拖到右节点；松手建边 + 打开边浮层 |
| 删边 | 点线 → 浮层 [Delete]；或选中 + `Backspace` |
| 边浮层 | `priority` / `upstream_model` / `sticky` / `on_error` 四字段表单 |
| 多候选 | 同一 left 多条出边；线上标 `p=10` 等 badge；画布按 priority 纵向错开 |
| 前缀 | `local/*` 节点 badge `PREFIX` |
| 多跳 | `right` 可为中间节点（二部图表达吃力时：**升级通用 DAG 视图**，数据模型不变） |
| 环 | `PUT` 前本地检环；服务端 400 时标红涉及边 |
| 键盘备选 | Tab 可聚焦节点/边；`Enter` 开边表单；提供「表单编辑全部 routes」等价面板（无障碍 + 无鼠标） |
| 同步 | 任何变更 debounce 400ms 后 `PUT /v1/admin/routes`；失败保留脏标记 `UNSAVED` |

**节点卡**

- 左：model id（mono）+ 类型 badge（`MODEL` / `PREFIX` / `ALIAS`）
- 右：provider 名 + 启用灯
- 选中：1px `ink` 外框

**边**

- 默认 `edgeIdle` 1.5px；hover/选中 `edge` 2px
- 贝塞尔曲线；箭头向右
- badge：`p=10`、`alias→jev-fast`、`sticky:session`

**空态**：「还没有路由」+ 示例 toml 片段 +「一键导入 example」。

### 6.3 Playground（保留 DNA，按契约 01 微调）

```text
┌────────── INPUT ──────────┬───────── OUTPUT ─────────┐
│ state        [JSON]       │  answers                  │
│ questions    [JSON]       │  ┌ q ───────────────────┐ │
│ model  [jev ▼]            │  │ type noul            │ │
│ ~12 tokens · 1 question   │  │ noul    0.96         │ │
│                           │  │ probs   t 0.96 / f…  │ │
│                           │  └──────────────────────┘ │
│                           │  usage  in 42 · out 0     │
│                           │  upstream_calls 1 · 42ms  │
└───────────────────────────┴───────────────────────────┘
                 [ Run Jev ↗ ]  黑色主按钮
```

| 要点 | 规格 |
|---|---|
| Answer 分型 | Choice / Score / **Noul（无 confidence 行）** 按契约 01 |
| 双键 | Vercel 回 `probability` 时显示 `prob 0.69` 与 `noul 0.69` 来源标注 |
| 计量 | `usage` 优先；本地 `chars/4` 必须标 **估算**；`cost_usd` null 显示 `—` |
| 示例 chips | 保留 4 个（`examples.ts`） |

---

## 7. 全局状态与文案

| 状态 | 视觉 | 文案（zh 为主，键可 i18n） |
|---|---|---|
| loading | 按钮内 spinner / 行骨架 | `Running…` / `Saving…` |
| 外部修改 | 横幅 `warn` 描边 | `配置已在外部修改` → `重载`（默认）/ `覆盖` |
| 密钥已保存 | toast `ok` | `密钥已保存，仅显示掩码` |
| 上游 429 | 输出面板 `danger` | `上游限流（retryable）→ 已切换候选 / 已返回 503` |
| 环检测失败 | 边标红 | `路由成环，已拒绝写入` |
| daemon 挂 | Header 灯红 | `daemon unreachable` |

**禁止文案**：Emoji 装饰、「出错了哦～」、泄露 key 的 error 回显（必须 redact）。

---

## 8. 可访问性

| 项 | 要求 |
|---|---|
| 对比度 | 正文 ≥ 4.5:1；状态色与文字同用加标签，不单靠颜色 |
| 焦点 | 可见 focus ring；Tab 序 = 视觉序 |
| 连线编辑 | 必须有**表单等价路径**（路由表编辑） |
| aria | 开关 `aria-checked`；卡片状态 `aria-live=polite` |
| 快捷键 | `⌘/Ctrl+Enter` = Run Jev；`Delete` = 删选中边 |

---

## 9. 实现约束（给前端 B）

1. **类型单一来源**：从 Rust/JSON Schema 生成 TS（契约 05）；禁止再手写 `ModelsResponse`。
2. **API**：仅契约 05 端点；admin 仅 127.0.0.1。
3. **图编辑第一版**：SVG 边 + DOM 节点；**不引入** 重型图编辑器；实践条款见契约 03（二部图 → 可升级 DAG）。
4. **组件目录建议**

```text
ui/src/
  app/Shell.tsx           # 壳 + 冲突横幅 + 路由
  pages/ProvidersPage.tsx
  pages/RoutingPage.tsx
  pages/PlaygroundPage.tsx
  components/providers/ProviderCard.tsx, KeyForm.tsx, ProbeButton.tsx
  components/routing/BipartiteCanvas.tsx, EdgeInspector.tsx, RouteTableForm.tsx
  components/playground/…
  styles/tokens.css
```

5. **视觉验收**：截图对比——工具密度向 CC Switch，单色细线向 jevplayground；融合而非双皮肤。
6. **设计交付物**：本稿 + 后续 Figma/HTML 原型（若有）归档本目录。

---

## 10. 验收清单（DoD）

- [x] 设计简报 / 原则 / tokens / IA / 三页规格 / 状态文案 / a11y / 实现约束成文
- [ ] `/frontend-design` 真调用产物（若后续再跑）与本稿 diff，冲突以契约为准
- [ ] Providers：卡片 + 开关 + 密文密钥 + Probe
- [ ] Routing：二部图拖线 ↔ toml 双向；键盘表单备选
- [ ] Playground：Noul 无 confidence；估算标注；`—` 表 null
- [ ] 冲突横幅重载/覆盖
- [ ] 无明文 key 进 DOM/console（契约 04）

---

## 11. 与契约映射

| 本稿 § | 契约 |
|---|---|
| §2 CC Switch 解构 | 06 §1 |
| §6.1 Providers | 06 §3 · 04-密钥 |
| §6.2 Routing | 03-路由 · 06 §4 |
| §6.3 Playground | 01-协议 · 06 §5 |
| §7 冲突横幅 | 04 §3 · 05 §admin |
| §9 类型/API | 05 |

— **模拟流程与设计稿作者：Mimo-V2.6-Pro**
