# Jev-Switch UI 重构计划书
**版本**: v2.0 Blueprint  
**日期**: 2026-09-24  
**作者**: Claude Opus 4.8  
**状态**: 施工蓝图（Blueprint for Implementation）

---

## §1 设计理念

### 1.1 程序本质与对标

**程序定位**: 本地 Jev 协议路由 daemon 的**实时控制台**（Control Panel）

**不是什么**:
- ❌ 营销型着陆页（无需说服用户注册）
- ❌ 文档教学站（不是 tutorial）
- ❌ 数据分析平台（不是 BI dashboard）

**是什么**:
- ✅ 本地服务监控面板（类似 Docker Desktop）
- ✅ 流量路由配置工具（类似 Proxyman）
- ✅ 实时状态展示 + 快速操作入口（类似 ngrok Dashboard）

**对标物**（行业标杆）:
| 产品 | 借鉴点 | 避开点 |
|------|--------|--------|
| **Docker Desktop** | 容器状态一目了然、操作就地完成 | 避免过度简化（Docker 优先新手，我们优先效率） |
| **Proxyman** | 流量可视化、请求历史清晰 | 避免信息过载（Proxyman 面向调试，我们面向运维） |
| **LM Studio** | 模型健康灯 + 推理日志 | 避免过度装饰（LM Studio 偏消费级） |
| **sub2api**（用户提及） | 首屏集成关键信息、快速操作 | 避免功能堆砌（sub2api 可能过于密集） |

**设计锚点**（三句话定义）:
1. **打开即知状态**：谁在跑、健康不健康、流量走哪了 → 图形优先，文字补充
2. **就地快速操作**：toggle 切、inline-edit 改、拖拽建边 → 无二次确认框
3. **历史可追溯**：看得到过去 1 小时 failover 了 3 次 → 日志页必须有

---

### 1.2 核心设计原则（决策裁决依据）

遇到设计分歧时，按以下优先级裁决：

#### **P0 原则（不可妥协）**

1. **功能优先于美观**  
   如果一个设计让操作变慢/变复杂，无论多美都不用  
   例：mode 切换从三步简化为 toggle，即使 toggle 开关视觉平淡

2. **状态可见性优先于简洁**  
   关键状态（daemon 跑没跑、provider 健康度）必须一眼可见  
   不能为了"极简"把状态藏在二级菜单

3. **渐进披露优先于平铺**  
   说明性文字收进 tooltip/折叠区，界面只放操作和状态  
   但操作本身（按钮/开关/输入框）不能藏

#### **P1 原则（强烈建议）**

4. **就地操作优先于弹窗**  
   inline-edit > modal dialog  
   toggle > 确认框  
   拖拽建边 > 表单填写

5. **图形优先于文字**  
   健康灯（绿/黄/红）> "healthy" 文字  
   火花线趋势图 > "最近 10 次成功 8 次"  
   流量动画 > "当前有 3 条活跃连接"

6. **持久化优先于临时态**  
   Probe 结果存储，不因切页丢失  
   日志保留（至少内存保留 1000 条）  
   路由配置实时保存

#### **P2 原则（在不违背 P0/P1 前提下执行）**

7. **深色主题优先**  
   控制台标配，减少眼疲劳  
   浅色主题保留兼容，但设计时以深色为准

8. **精致但克制的动效**  
   动画服务于反馈和引导，不是装饰  
   页面加载、hover、focus 有动效，但 ≤300ms

9. **国际化友好**  
   避免"终端腔"美学（`font-mono uppercase tracking-widest`）对 CJK 的破坏  
   字号 ≥12px（10px 对中文不可读）

---

### 1.3 用户反馈的三条不满（原始需求）

**来源**: 用户 2026-09-23 亲口给出（`memory/ui-aesthetic-direction.md`）

1. **太多冗余说明文字**  
   问题：抓不住重点，没恪守渐进披露  
   解决：说明收进 tooltip/`?`气泡/折叠区

2. **缺动效与可视化图表**  
   问题：做不到"一图胜千言"  
   解决：参照 sub2api 首页，集成信息 + 快速操作

3. **该交互的写成字**  
   问题：本该程序干的事，写一堆字让人看懂再手动操作  
   解决：提升交互性（toggle/inline-edit/拖拽）

**验收标准**: 重构后用户打开界面，能在 5 秒内：
- [ ] 知道 daemon 是否在跑
- [ ] 知道当前 mode（local/cloud）
- [ ] 知道哪些 provider 健康/不健康
- [ ] 能一键切换 mode（无二次确认）
- [ ] 能看到最近 10 条请求流水

---

## §2 信息架构（IA）重构

### 2.1 旧架构问题

**当前导航**: `Home | Providers | Routing | Playground`（四页平铺）

**问题**:
1. **Home 定位模糊**: 目前 Home = 系统状态卡 + mode/listen 编辑，但：
   - 无全局健康概览（看不到 provider 健康灯）
   - 无流量历史（看不到最近请求）
   - 像"设置页"而非"首页"

2. **缺失日志页**: 无处查看：
   - 请求历史（哪个模型 → 哪个上游 → 耗时/成本）
   - failover 事件（vercel 挂了自动切 laya）
   - 错误记录（上游 503 / 密钥失效）

3. **Playground 埋太深**: 测试功能是高频操作，但在末尾

---

### 2.2 新架构设计

```
┌─ Dashboard (首页) ────────────────────────────────────────┐
│  - 全局状态卡（daemon/mode/listen/providers 健康）        │
│  - 实时流水（最近 10 条请求）                             │
│  - 快捷操作（mode toggle / + provider / 编辑路由）       │
└────────────────────────────────────────────────────────────┘
         ↓
┌─ Providers (提供商管理) ───────────────────────────────────┐
│  - 提供商卡片列表（健康监控 + 配置编辑）                  │
│  - + Add Provider 表单                                     │
│  - Probe 按钮 + 火花线历史                                │
└────────────────────────────────────────────────────────────┘
         ↓
┌─ Routes (路由拓扑) ────────────────────────────────────────┐
│  - BipartiteCanvas 可视化编辑                             │
│  - 左列模型 ← 边 → 右列提供商                            │
│  - EdgeInspector 浮层 + RouteTableForm 表格降级           │
└────────────────────────────────────────────────────────────┘
         ↓
┌─ Playground (测试沙盒) ────────────────────────────────────┐
│  - 保持原样（Form/JSON tab + Run Jev + AnswerSummary）   │
└────────────────────────────────────────────────────────────┘
         ↓
┌─ Logs (新增日志页) ────────────────────────────────────────┐
│  - 请求历史表格（时间/模型/上游/延迟/成本/状态）          │
│  - 事件时间线（failover/error/config change）             │
│  - 过滤器（按模型/上游/状态筛选）                         │
└────────────────────────────────────────────────────────────┘
```

**关键变化**:
- **Dashboard = 新首页**：集成所有关键信息 + 快速操作入口
- **Home → Dashboard 改名**：明确"仪表盘"定位
- **新增 Logs 页**：补齐可追溯性缺口
- **导航顺序**: Dashboard → Providers → Routes → Playground → Logs（按使用频率排序）

---

### 2.3 Dashboard 详细设计

#### **布局 wireframe**

```
┌────────────────────────────────────────────────────────────┐
│  Dashboard                               [☀] [🌐] v0.5.0  │ ← Shell 顶栏
└────────────────────────────────────────────────────────────┘

┌─ 状态卡组 ──────────────────────────────────────────────────┐
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────┐
│  │ Daemon      │  │ Mode        │  │ Providers   │  │ Rout│
│  │ ● Running   │  │ [====] Local│  │ ●● 2/2 OK  │  │ 5 ed│
│  │ 2h 34m      │  │             │  │ laya ●     │  │ 2 pr│
│  │             │  │             │  │ vercel ●   │  │     │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────┘
│                                                              │
└──────────────────────────────────────────────────────────────┘

┌─ 实时活动 ──────────────────────────────────────────────────┐
│  Recent Activity                                [View All →]│
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 14:32:05  jev → laya/jev-english   68ms  $0.0002  ✓  │ │
│  │ 14:32:03  jev → vercel/typesafe    124ms $0.0005  ✓  │ │
│  │ 14:31:58  jev → laya/jev-english   failed → vercel ✓ │ │ ← failover 高亮
│  │ ...                                                    │ │
│  └────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘

┌─ 快捷操作 ──────────────────────────────────────────────────┐
│  [+ Add Provider]  [Edit Routes]  [Run Playground]  [Logs →]│
└──────────────────────────────────────────────────────────────┘
```

#### **状态卡组（四张卡）**

**卡片 1: Daemon 状态**
- 指示灯：● Running（绿）/ ● Stopped（红）/ ● Starting（黄脉动）
- 运行时长：`2h 34m`（格式化为友好时间）
- 数据源：`GET /v1/admin/status` 的 `daemon_uptime`
- 交互：点击卡片无操作（纯展示）

**卡片 2: Mode 切换**
- toggle 开关：`[====] Local` / `[====] Cloud`（视觉为 switch 滑块）
- 一键切换，无二次确认框
- 数据源：`status.mode`
- 交互：点击 toggle → `PUT /v1/admin/mode {"mode": "cloud"}`
- tooltip: "Local: 回环免 token · Cloud: Bearer 调用"

**卡片 3: Providers 健康**
- 健康灯：`●● 2/2 OK`（两个绿点 = 2 个 provider 都健康）
- 单独灯：`laya ●` / `vercel ●`（绿/黄/红）
- 数据源：`providers` 数组的 `enabled` + 最后一次 probe 结果（存储在前端 context）
- 交互：点击跳转 Providers 页

**卡片 4: Routes 摘要**
- 文本：`5 edges` / `2 prefix`（5 条边 / 2 个 prefix 规则）
- 数据源：`routes` 数组长度 + 统计 `match: "prefix"`
- 交互：点击跳转 Routes 页

#### **实时活动（请求流水）**

- 表格形式，固定高度（最多显示 10 行，超出滚动）
- 列：`时间 | 模型 | 上游 | 延迟 | 成本 | 状态`
- failover 行高亮（黄色背景 + "failed → 备选" 文案）
- 数据源：**新增 WebSocket `/v1/admin/events`**（后端需实现）或轮询 `/v1/admin/logs`
- 交互：点击行跳转 Logs 页，高亮对应条目

#### **快捷操作（四个按钮）**

- `+ Add Provider`：打开 AddProviderPanel modal
- `Edit Routes`：跳转 Routes 页
- `Run Playground`：跳转 Playground 页
- `Logs →`：跳转 Logs 页

---

### 2.4 Logs 页设计（新增）

#### **功能需求**

1. **请求历史表格**
   - 列：`时间 | 模型 | 上游 | 延迟 | 成本 | 状态 | 详情`
   - 支持排序（按时间/延迟/成本）
   - 支持筛选（按模型/上游/状态）
   - 分页（每页 50 条）

2. **事件时间线**（可选，Phase 2 实现）
   - failover 事件：`14:31:58 laya failed → vercel`
   - 配置变更：`14:20:00 Added provider: deepseek`
   - 错误事件：`14:15:00 vercel 503 Service Unavailable`

3. **统计卡片**（可选，Phase 2 实现）
   - 今日请求总数
   - failover 次数
   - 平均延迟
   - 总成本

#### **数据源**

- **后端新增 API**：`GET /v1/admin/logs?limit=50&offset=0&model=jev&status=success`
- **WebSocket 实时推送**（可选）：`ws://127.0.0.1:11435/v1/admin/events`

---

## §3 视觉设计系统

### 3.1 设计方向定性

**选定风格**: **实用主义 + 精致工业风**（Pragmatic Industrial）

**Why this direction**:
1. **避开"终端腔"俗套**：当前 UI 过度使用 `font-mono uppercase tracking-widest text-[10px]`，对 CJK 不友好
2. **避开"SaaS 营销页"俗套**：紫色渐变 / 大 hero 标题 / 玻璃态卡片泛滥
3. **匹配程序本质**：控制台 = 工具，不是艺术品，不是营销材料

**具体执行**:
- **深色主题为主**：控制台标配，减少眼疲劳
- **清晰排版**：Inter Variable（可读性强，但不俗套）
- **精准动效**：服务于反馈，不是装饰
- **功能优先**：美观让位于可用性

**对标视觉参考**:
- Vercel Dashboard（简洁但不过度装饰）
- Linear（精致但实用）
- Grafana（数据密集但清晰）

---

### 3.2 色彩系统

#### **浅色主题（兼容）**

```css
:root {
  --bg: #f4f4f5;           /* zinc-100 背景 */
  --surface: #ffffff;      /* 白卡片 */
  --surface-hover: #fafafa;
  --border: #e4e4e7;       /* zinc-200 */

  --text: #09090b;         /* zinc-950 正文 */
  --text-muted: #71717a;   /* zinc-500 次要 */
  --text-subtle: #a1a1aa;  /* zinc-400 装饰 */

  --accent: #0891b2;       /* cyan-600 */
  --accent-hover: #0e7490; /* cyan-700 */

  --success: #059669;      /* emerald-600 */
  --success-bg: #d1fae5;   /* emerald-100 */
  --warning: #d97706;      /* amber-600 */
  --warning-bg: #fef3c7;   /* amber-100 */
  --danger: #dc2626;       /* red-600 */
  --danger-bg: #fee2e2;    /* red-100 */
}
```

#### **深色主题（默认）**

```css
[data-theme='dark'] {
  --bg: #0a0a0f;           /* 深邃背景（自定义） */
  --surface: #15151a;      /* 卡片面（zinc-900 偏蓝） */
  --surface-hover: #1a1a20;
  --border: rgba(255, 255, 255, 0.08);

  --text: #e4e4e7;         /* zinc-200 */
  --text-muted: #a1a1aa;   /* zinc-400 */
  --text-subtle: #71717a;  /* zinc-500 */

  --accent: #06b6d4;       /* cyan-500 */
  --accent-hover: #0891b2; /* cyan-600 */

  --success: #10b981;      /* emerald-500 */
  --success-bg: rgba(16, 185, 129, 0.1);
  --warning: #f59e0b;      /* amber-500 */
  --warning-bg: rgba(245, 158, 11, 0.1);
  --danger: #ef4444;       /* red-500 */
  --danger-bg: rgba(239, 68, 68, 0.1);
}
```

**色彩使用规范**:
- `--text`: 正文、标题、按钮文字
- `--text-muted`: 次要信息（时间戳、单位）
- `--text-subtle`: 占位符、禁用态
- `--accent`: 链接、焦点、主按钮、操作色
- `--success/warning/danger`: 状态指示、健康灯、错误提示

**对比度检查**:
- 正文 on surface: ≥ 7:1（AAA 级）
- 次要文字 on surface: ≥ 4.5:1（AA 级）
- 按钮文字 on accent: ≥ 4.5:1

---

### 3.3 排版系统

#### **字体栈**

```css
--font-sans: 'Inter Variable', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
--font-mono: 'JetBrains Mono', 'Geist Mono', 'IBM Plex Mono', monospace;
```

**使用场景**:
- `--font-sans`: 全部界面文字（标题、正文、按钮）
- `--font-mono`: 仅代码/ID/数字（provider ID、模型名、TOML 代码块）

**Why Inter Variable**:
- 清晰易读，支持 Variable Font（可微调 weight）
- 比 Roboto 精致，比 SF Pro 开放（跨平台一致）
- 避开 AI 俗套（Inter 虽流行，但比系统字体有个性）

**Why NOT 等宽字体作为主字体**:
- `font-mono` 对 CJK 不友好（当前 UI 的主要问题）
- 10px 等宽 + uppercase + tracking-widest = 不可读

#### **字号层级**

| 层级 | CSS 变量 | 像素值 | 使用场景 |
|------|----------|--------|----------|
| xs | `--text-xs` | 12px | 次要标签、时间戳 |
| sm | `--text-sm` | 14px | 按钮、表单标签 |
| base | `--text-base` | 16px | 正文（默认） |
| lg | `--text-lg` | 18px | 小标题 |
| xl | `--text-xl` | 20px | 卡片标题 |
| 2xl | `--text-2xl` | 24px | 页面标题 |

**规范**:
- 正文 ≥ 16px（当前很多地方 13-14px，需提升）
- 标签/次要信息 ≥ 12px（当前 10px 不可读）
- 页面标题 ≥ 24px（当前 10px 层级倒挂）

---

### 3.4 间距与圆角

#### **间距系统（基于 4px 网格）**

```css
--space-1: 4px;   /* 紧凑元素内边距 */
--space-2: 8px;   /* 按钮/输入框内边距 */
--space-3: 12px;  /* 卡片内边距 */
--space-4: 16px;  /* 卡片间距 */
--space-6: 24px;  /* 区块间距 */
--space-8: 32px;  /* 页面 section 间距 */
```

#### **圆角**

```css
--radius: 8px;  /* 统一圆角（按钮、输入框、卡片） */
```

**Why 8px**:
- 适度现代，不过度柔和（6px 偏方，12px 偏圆润）
- 与 Tailwind 默认 `rounded-lg` 对齐
- 比当前 `7px/6px` 更统一

---

### 3.5 阴影与边框

#### **阴影层级**

```css
--shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);  /* 微阴影（输入框 focus） */
--shadow-md: 0 4px 8px rgba(0, 0, 0, 0.1);   /* 卡片静态 */
--shadow-lg: 0 8px 16px rgba(0, 0, 0, 0.15); /* 卡片 hover */
--shadow-xl: 0 16px 32px rgba(0, 0, 0, 0.2); /* modal 弹窗 */
```

**使用规范**:
- 卡片默认：`border: 1px solid var(--border)` + `shadow-md`
- 卡片 hover：`shadow-lg` + `transform: translateY(-2px)`
- 弹窗/浮层：`shadow-xl`

#### **边框**

```css
--border: rgba(255, 255, 255, 0.08);  /* 深色主题 */
--border: #e4e4e7;                    /* 浅色主题 */
```

---

## §4 动效系统

### 4.1 动效原则

1. **服务于反馈，不是装饰**  
   动画必须传达信息（加载中 / 操作成功 / 状态变化）

2. **快速且克制**  
   持续时间 ≤ 300ms（超过会让人不耐烦）

3. **尊重用户偏好**  
   检测 `prefers-reduced-motion`，提供静态降级

---

### 4.2 动效清单

#### **页面加载（逐个淡入）**

```css
@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.fade-in-up {
  animation: fadeInUp 0.3s ease-out forwards;
}

.fade-in-up:nth-child(1) { animation-delay: 0s; }
.fade-in-up:nth-child(2) { animation-delay: 0.05s; }
.fade-in-up:nth-child(3) { animation-delay: 0.1s; }
```

**应用场景**: Dashboard 四张状态卡、Providers 列表

---

#### **Hover 交互（卡片上浮）**

```css
.card-hover {
  transition: transform 0.2s ease, box-shadow 0.2s ease, border-color 0.2s ease;
}

.card-hover:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-lg);
  border-color: var(--accent);
}
```

---

#### **Toggle 开关（滑块平滑过渡）**

```css
.toggle-switch {
  position: relative;
  width: 48px;
  height: 24px;
  background: var(--border);
  border-radius: 12px;
  transition: background 0.2s ease;
}

.toggle-switch.active {
  background: var(--accent);
}

.toggle-switch::after {
  content: '';
  position: absolute;
  width: 20px;
  height: 20px;
  background: white;
  border-radius: 50%;
  top: 2px;
  left: 2px;
  transition: transform 0.2s ease;
}

.toggle-switch.active::after {
  transform: translateX(24px);
}
```

---

#### **健康灯脉动（仅 warning/danger 态）**

```css
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.status-dot-warning,
.status-dot-danger {
  animation: pulse 2s infinite;
}
```

---

#### **路由拖线（虚线跟随）**

- 拖动时：虚线（`stroke-dasharray="4 3"`），跟随鼠标
- 松手后：虚线淡出，实线淡入（`transition: opacity 0.3s`）

---

#### **日志新行（从右滑入）**

```css
@keyframes slideInRight {
  from {
    opacity: 0;
    transform: translateX(100%);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

.log-entry-new {
  animation: slideInRight 0.3s ease-out;
}
```

---

### 4.3 响应式运动降级

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---



**这是什么**: 
- Jev-Switch 下一代 UI 的**完整设计规范 + 实施路线图**
- 供当前和后续 Agent 遵循的**单一事实来源**（Single Source of Truth）
- 可交接、可验收、可迭代的**活文档**

**不是什么**: 
- ❌ 不是概念草图（已有明确设计决策）
- ❌ 不是纯视觉 mockup（包含技术实现路径）
- ❌ 不是一次性文档（随迭代持续更新）

**使用方式**:
1. **开始实现前**: 完整阅读 §1-§5（设计理念 → 视觉系统）
2. **实现过程中**: 查阅 §6-§8（组件规范 → 实施路径 → 验收标准）
3. **遇到设计决策分歧**: 回到 §1.2 核心原则裁决
4. **发现计划书错误/过时**: 提 issue 或直接修改本文档，记录变更原因

---

