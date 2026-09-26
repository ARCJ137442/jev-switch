# Jev-Switch UI 重构设计稿 v2.0
**日期**: 2026-09-24  
**作者**: Claude Opus 4.8  
**性质**: 推翻重来的完整设计方案（Architecture + Visual + Interaction）

> **历史方案接续说明（2026-09-24）：**以 [入口网关认知对齐与实施计划](ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) 作为当前施工入口。下文 Control/Logs 三页与 Playground 浮层属于旧草图，不应据此重新合并导航；演练场保留独立页并支持两类入口横比。落实 Routing“入口”配置、对外入口卡片到提供商复合卡片的交互 DAG，以及可扩展语言选择；旧中英 toggle 不再是目标交互。

---

## §0 设计理念（一句话）

**Jev-Switch 是本地 daemon 的实时控制台，不是配置文件编辑器，不是文档站。**

用户打开界面要的是：
1. **看清状态**：daemon 跑没跑、哪些 provider 健康、流量走哪了
2. **快速操作**：切 mode、改 listen、加 provider、测路由
3. **追溯历史**：看得到过去 1 小时 failover 了几次、哪个上游慢

**对标**: Docker Desktop（状态清晰）+ Proxyman（流量可视化）+ Grafana（数据密集但不乱）

---

## §1 信息架构（IA）全面重构

### 1.1 推翻旧架构

**旧**: `Home | Providers | Routing | Playground`（四页平铺，定位模糊）

**问题**:
- Home 像"设置页"而非"首页"（全是配置项，无状态概览）
- 缺日志页（无处查历史）
- Playground 埋太深（高频操作在末尾）
- Routing 页只有拖线图（表格降级形式割裂）

---

### 1.2 新架构：三页 + 浮层

```
┌─ Dashboard (仪表盘) ────────────────────────────────────┐
│  - 全局状态速览（daemon/mode/providers 健康/路由摘要） │
│  - 实时请求流水（最近 20 条 + failover 高亮）          │
│  - 快速操作区（切 mode / 测试 / 查日志）               │
└─────────────────────────────────────────────────────────┘
         ↓
┌─ Control (控制面板) ────────────────────────────────────┐
│  [Providers 标签]  [Routes 标签]                        │
│  ─────────────────────────────────────────────────────  │
│  - Providers: 卡片列表 + 健康监控 + 就地编辑           │
│  - Routes: 二部图 + 表格双视图（toggle 切换）          │
└─────────────────────────────────────────────────────────┘
         ↓
┌─ Logs (日志) ───────────────────────────────────────────┐
│  - 请求历史表格（时间/模型/上游/延迟/成本/状态）       │
│  - 过滤器（按模型/上游/状态/时间段）                   │
│  - 事件时间线（failover/error/config change）          │
└─────────────────────────────────────────────────────────┘

+ Playground 浮层（全局快捷键 Cmd/Ctrl+P 唤起）
```

**关键决策**:
1. **Dashboard = 真正的首页**：打开即知全局状态，不是配置入口
2. **Control = 两个 tab 合并**：Providers 和 Routes 本质都是"配置"，合并减少导航跳转
3. **Playground 改为浮层**：高频操作，用快捷键唤起更快（类似 Spotlight / Command Palette）
4. **Logs 独立页**：补齐可追溯性
5. **删掉 Home 命名**：Dashboard 更准确

---

### 1.3 导航结构

**顶栏**（简化）:
```
Jev-Switch  [Dashboard] [Control] [Logs]     [☀/☾] [EN/中] [⚙ Settings]
```

**右上角**:
- 主题切换：`☀/☾` 图标（toggle 直接切，无文字）
- 语言切换：`EN/中` 小文字（点击切换）
- 设置浮层：`⚙` 齿轮图标（点击打开 Settings modal）

**Settings modal 内容**（新增）:
- Admin 密码修改
- 版本信息
- 关于 / 文档链接
- 导出配置（下载 TOML）
- 导入配置（上传 TOML）

---

## §2 Dashboard 详细设计

### 2.1 布局 wireframe

```
┌───────────────────────────────────────────────────────────────┐
│ Jev-Switch  [Dashboard] Control  Logs        ☀  EN  ⚙  v0.5.0│
└───────────────────────────────────────────────────────────────┘

┌─ System Status ─────────────────────────────────────────────┐
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ ● Running    │  │ ◉ Local Mode │  │ ●● 2/2 Healthy│     │
│  │ Uptime 2h34m │  │ :11435       │  │ laya ●       │     │
│  │              │  │ [Switch →]   │  │ vercel ●     │     │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Routes: 5 edges · 2 prefix · 3 exact                │   │
│  │ [Edit Routes →]                                      │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

┌─ Live Activity ─────────────────────────────────────────────┐
│  Recent Requests                              [View All →]  │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 14:32:05  jev → laya/jev-english    68ms   $0.0002 ✓│   │
│  │ 14:32:03  jev → vercel/typesafe    124ms   $0.0005 ✓│   │
│  │ 14:31:58  jev → laya/jev-english   ⚠ retry→vercel ✓│   │ ← failover 行
│  │ 14:31:45  local/* → (local-loop)     2ms   $0      ✓│   │
│  │ ...                                                  │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

┌─ Quick Actions ─────────────────────────────────────────────┐
│  [⚡ Test Jev]  [+ Provider]  [📊 Full Logs]  [⚙ Settings] │
└──────────────────────────────────────────────────────────────┘
```

---

### 2.2 状态卡组设计

#### **卡片 1: Daemon 状态**
- **指示灯**: `● Running`（绿色实心圆）/ `○ Stopped`（红色空心圆）/ `◐ Starting`（黄色半圆脉动）
- **运行时长**: `Uptime 2h 34m`（友好时间格式）
- **数据源**: `GET /v1/admin/status` → `daemon_uptime`
- **交互**: 无（纯展示）

#### **卡片 2: Mode 与 Listen**
- **第一行**: `◉ Local Mode`（实心圆 + 大文字）或 `◉ Cloud Mode`
- **第二行**: `:11435`（当前监听地址，缩写形式）
- **操作**: `[Switch →]` 按钮（点击直接切换 mode，无确认框）
- **tooltip**: hover 按钮显示"Switch to Cloud mode · Restart listen on :18765"

**Mode 切换逻辑**:
- 点击 `[Switch →]` → `PUT /v1/admin/mode {"mode": "cloud"}` → 刷新页面
- 无二次确认框（符合"就地操作"原则）
- 如果需要警告（比如 cloud mode 无密码），在按钮旁显示 `⚠` 图标 + tooltip

#### **卡片 3: Providers 健康**
- **第一行**: `●● 2/2 Healthy`（两个绿点 = 2 个 provider 都健康）
- **第二行**: 单独健康灯 `laya ●` / `vercel ●`（绿/黄/红）
- **数据源**: `GET /v1/admin/providers` + 前端存储的最后一次 probe 结果
- **交互**: 点击卡片跳转 Control 页 Providers tab

**健康灯颜色逻辑**:
- 绿 `●`: 最后一次 probe 成功 + `enabled: true`
- 黄 `●`: probe 失败但 `enabled: true`（降级态）
- 红 `●`: `enabled: false`（人为关闭）
- 灰 `○`: 从未 probe 过

#### **卡片 4: Routes 摘要**
- **文本**: `Routes: 5 edges · 2 prefix · 3 exact`
- **按钮**: `[Edit Routes →]`（跳转 Control 页 Routes tab）
- **数据源**: `GET /v1/admin/routes` → 统计数量

---

### 2.3 实时请求流水

**表格形式**（固定高度，最多显示 20 行，超出滚动）:

| 列名 | 宽度 | 内容示例 | 说明 |
|------|------|----------|------|
| Time | 80px | `14:32:05` | HH:mm:ss 格式 |
| Model | 140px | `jev` | 左列模型名 |
| Upstream | 180px | `laya/jev-english` | provider/model 或 `(local-loop)` |
| Latency | 80px | `68ms` | 响应时间 |
| Cost | 80px | `$0.0002` | 成本（2 位小数） |
| Status | 60px | `✓` / `⚠` / `✕` | 成功/重试/失败 |

**特殊行处理**:
- **failover 行**: 黄色背景 + upstream 列显示 `⚠ retry→vercel`
- **local-loop 行**: upstream 列显示 `(local-loop)`，灰色文字
- **错误行**: 红色背景 + status 列显示 `✕`，hover 显示错误信息

**数据源**（后端需新增）:
- **方案 A**: WebSocket `/v1/admin/events` 实时推送
- **方案 B**: 轮询 `GET /v1/admin/logs?limit=20` 每 5 秒刷新

**交互**:
- 点击行 → 跳转 Logs 页，高亮该条目并展开详情
- `[View All →]` 按钮 → 跳转 Logs 页

---

### 2.4 快速操作区

四个按钮：

1. **`[⚡ Test Jev]`**: 打开 Playground 浮层（或跳转 Playground 页，如果不做浮层）
2. **`[+ Provider]`**: 打开 Add Provider modal
3. **`[📊 Full Logs]`**: 跳转 Logs 页
4. **`[⚙ Settings]`**: 打开 Settings modal

---

## §3 Control 页设计（Providers + Routes 合并）

### 3.1 布局结构

```
┌───────────────────────────────────────────────────────────────┐
│ Jev-Switch  Dashboard [Control] Logs          ☀  EN  ⚙       │
└───────────────────────────────────────────────────────────────┘

┌─ Control ───────────────────────────────────────────────────┐
│  [Providers]  [Routes]  ← tab 切换                           │
│  ─────────────────────────────────────────────────────────   │
│                                                               │
│  (Providers tab 内容 或 Routes tab 内容)                     │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

---

### 3.2 Providers Tab

**布局**: 卡片列表（垂直堆叠）+ 底部 `[+ Add Provider]` 按钮

**Provider 卡片结构**（简化版，去掉冗余信息）:

```
┌─ laya ──────────────────────────────────────────────┐
│  ● Healthy    [Probe]  [Edit]  [Delete]  [ON/OFF]  │
│  ────────────────────────────────────────────────   │
│  Kind: jev                                          │
│  Base: http://localhost:18765                       │
│  Key: ••••••••  [Replace]                           │
│  ────────────────────────────────────────────────   │
│  Last probe: 2m ago · 68ms · ▂▃▅▇█▇▅▃▂ (24h)      │
└─────────────────────────────────────────────────────┘
```

**关键改进**:
1. **健康灯前置**：`● Healthy` 比 badge 更直观
2. **操作按钮右对齐**：Probe / Edit / Delete / Toggle 一目了然
3. **Key 掩码简化**：`••••••••` 8 个点，点击 `[Replace]` 才展开输入框
4. **火花线简化**：`▂▃▅▇█▇▅▃▂` 纯 Unicode 字符，无 SVG（性能更好）
5. **去掉 API 路径说明**：`GET /v1/admin/providers/{id}/probe` 这类技术债全删

**Edit 模式**（点击 `[Edit]` 按钮）:
- 卡片展开，显示表单（kind / base / key 可编辑）
- `[Save]` / `[Cancel]` 按钮
- 保存后卡片收起

**Delete 确认**（点击 `[Delete]` 按钮）:
- 弹出确认 modal：`确认删除 provider "laya"？`
- `[确认删除]` / `[取消]` 按钮
- 无需打字 ID 二次确认（过度防御）

---

### 3.3 Routes Tab

**双视图切换**（右上角 toggle）:
- **Graph 视图**（默认）：BipartiteCanvas 拖线图
- **Table 视图**：RouteTableForm 表格

```
┌─ Routes ─────────────────────────────────────────────────┐
│                                    [Graph ◉] [Table ○]   │
│  ────────────────────────────────────────────────────     │
│                                                           │
│  (BipartiteCanvas 或 RouteTableForm)                     │
│                                                           │
│  ────────────────────────────────────────────────────     │
│  [+ Add Route]  [Import TOML]  [Export TOML]            │
└───────────────────────────────────────────────────────────┘
```

**Graph 视图改进**:
1. **移动端优化**：拖线锚点从 3×3px 增大到 12×12px（touch target ≥44px 靠 padding 补齐）
2. **边标注简化**：只显示 priority，其他信息点击边后在 EdgeInspector 浮层查看
3. **空态优化**：无路由时显示 `[开始拖线建立第一条路由]` 引导按钮（点击高亮左列第一个模型的锚点）

**Table 视图改进**:
1. **去掉底部说明文字**：`变更 400ms debounce 后 PUT...` 全删
2. **错误提示浮层化**：重复路由检测后，在对应行右侧显示 `⚠` 图标 + tooltip，而非整行标红
3. **移动端响应式**：横向滚动 + 固定首列（left 列）

---

## §4 Logs 页设计（新增）

### 4.1 布局结构

```
┌───────────────────────────────────────────────────────────────┐
│ Jev-Switch  Dashboard  Control  [Logs]        ☀  EN  ⚙       │
└───────────────────────────────────────────────────────────────┘

┌─ Logs ──────────────────────────────────────────────────────┐
│  [Requests]  [Events]  ← tab 切换                            │
│  ─────────────────────────────────────────────────────────   │
│  Filters: [Model ▾] [Upstream ▾] [Status ▾] [Time Range ▾]  │
│  ─────────────────────────────────────────────────────────   │
│                                                               │
│  (Requests 表格 或 Events 时间线)                            │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

---

### 4.2 Requests Tab（请求历史）

**表格列**:

| 列名 | 宽度 | 内容 | 排序 |
|------|------|------|------|
| Time | 100px | `2024-09-24 14:32:05` | ✓ |
| Model | 120px | `jev` | ✓ |
| Upstream | 180px | `laya/jev-english` | ✓ |
| Latency | 80px | `68ms` | ✓ |
| Cost | 80px | `$0.0002` | ✓ |
| Status | 80px | `✓` / `⚠` / `✕` | ✓ |
| Details | 60px | `[›]` 展开按钮 | - |

**展开详情**（点击 `[›]` 按钮）:
- 折叠行展开，显示请求详情：
  - Request body（JSON 格式化）
  - Response body（JSON 格式化）
  - Headers
  - Error message（如果失败）

**分页**: 每页 50 条，底部分页器

**数据源**（后端需新增）:
- `GET /v1/admin/logs?limit=50&offset=0&model=jev&status=success&from=2024-09-24T00:00:00Z&to=2024-09-24T23:59:59Z`

---

### 4.3 Events Tab（事件时间线）

**时间线形式**（垂直排列）:

```
┌─ Events ──────────────────────────────────────────────────┐
│                                                            │
│  ● 14:31:58  Failover                                     │
│     laya/jev-english failed (503) → switched to vercel    │
│                                                            │
│  ● 14:20:00  Config Changed                               │
│     Added provider: deepseek                              │
│                                                            │
│  ● 14:15:00  Error                                        │
│     vercel 503 Service Unavailable                        │
│                                                            │
│  ● 13:45:00  Mode Switched                                │
│     local → cloud                                         │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

**事件类型**:
- `Failover`: 上游切换
- `Config Changed`: 配置变更（provider 增删、routes 修改）
- `Error`: 错误事件（503 / 密钥失效 / 超时）
- `Mode Switched`: mode 切换
- `Listen Changed`: listen 地址修改

**数据源**（后端需新增）:
- `GET /v1/admin/events?limit=100&type=failover,error`

---

## §5 Playground 重构（浮层化）

### 5.1 当前问题

- Playground 页埋在末尾，但测试是高频操作
- 每次测试需要点两次（跳转 + 点 Run）

### 5.2 改进方案：全局浮层

**触发方式**:
- Dashboard 快速操作区 `[⚡ Test Jev]` 按钮
- 全局快捷键：`Cmd/Ctrl + P`（类似 Command Palette）
- 顶栏右上角新增 `⚡` 图标

**浮层设计**（居中 modal，宽 800px，高 600px）:

```
┌─ Test Jev ─────────────────────────────────────── [✕] ┐
│                                                        │
│  Model: [jev ▾]                → laya/jev-english     │
│  ─────────────────────────────────────────────────    │
│  State: (textarea, 4 rows)                            │
│  Questions: [Form] [JSON]  (tabs)                     │
│            (question editor, 6 rows)                  │
│  ─────────────────────────────────────────────────    │
│  [Run Jev]  (Cmd/Ctrl+Enter to run)                  │
│  ─────────────────────────────────────────────────    │
│  Output:                                              │
│  (AnswerSummary + pre output, 8 rows)                │
│                                                        │
└────────────────────────────────────────────────────────┘
```

**快捷键**:
- `Cmd/Ctrl + P`: 打开浮层
- `Esc`: 关闭浮层
- `Cmd/Ctrl + Enter`: 运行测试

---

## §6 视觉设计系统

### 6.1 色彩系统（深色优先）

#### **深色主题（默认）**

```css
[data-theme='dark'] {
  /* 背景层级 */
  --bg: #0a0a0f;              /* 深邃背景 */
  --surface: #15151a;         /* 卡片面 */
  --surface-hover: #1a1a20;   /* 卡片 hover */
  --border: rgba(255, 255, 255, 0.08);

  /* 文字 */
  --text: #e4e4e7;            /* zinc-200 正文 */
  --text-muted: #a1a1aa;      /* zinc-400 次要 */
  --text-subtle: #71717a;     /* zinc-500 装饰 */

  /* 交互色 */
  --accent: #06b6d4;          /* cyan-500 */
  --accent-hover: #0891b2;    /* cyan-600 */

  /* 状态色 */
  --success: #10b981;         /* emerald-500 */
  --success-bg: rgba(16, 185, 129, 0.1);
  --warning: #f59e0b;         /* amber-500 */
  --warning-bg: rgba(245, 158, 11, 0.1);
  --danger: #ef4444;          /* red-500 */
  --danger-bg: rgba(239, 68, 68, 0.1);
}
```

#### **浅色主题（兼容）**

```css
:root {
  --bg: #f4f4f5;
  --surface: #ffffff;
  --surface-hover: #fafafa;
  --border: #e4e4e7;

  --text: #09090b;
  --text-muted: #71717a;
  --text-subtle: #a1a1aa;

  --accent: #0891b2;
  --accent-hover: #0e7490;

  --success: #059669;
  --success-bg: #d1fae5;
  --warning: #d97706;
  --warning-bg: #fef3c7;
  --danger: #dc2626;
  --danger-bg: #fee2e2;
}
```

---

### 6.2 字体系统

```css
--font-sans: 'Inter Variable', 'Inter', system-ui, sans-serif;
--font-mono: 'JetBrains Mono', 'Geist Mono', monospace;
```

**使用规范**:
- `--font-sans`: 全部界面文字（标题、正文、按钮）
- `--font-mono`: **仅**代码/ID/数字（provider ID、TOML 代码块、数字对齐）
- **禁止**: `font-mono uppercase tracking-widest` 组合（对 CJK 破坏性强）

**字号层级**:

| 层级 | CSS 变量 | 像素 | 使用场景 |
|------|----------|------|----------|
| xs | `--text-xs` | 12px | 时间戳、次要标签 |
| sm | `--text-sm` | 14px | 按钮、表单标签 |
| base | `--text-base` | 16px | **正文默认** |
| lg | `--text-lg` | 18px | 小标题 |
| xl | `--text-xl` | 20px | 卡片标题 |
| 2xl | `--text-2xl` | 24px | 页面标题 |

**硬性规范**:
- 正文 ≥ 16px（当前很多地方 13-14px，不可读）
- 标签 ≥ 12px（当前 10px 对中文不可读）
- 页面标题 ≥ 24px（当前 10px 层级倒挂）

---

### 6.3 间距与圆角

```css
--space-2: 8px;   /* 按钮/输入框内边距 */
--space-3: 12px;  /* 紧凑卡片内边距 */
--space-4: 16px;  /* 标准卡片内边距 */
--space-6: 24px;  /* 区块间距 */
--space-8: 32px;  /* 页面 section 间距 */

--radius: 8px;    /* 统一圆角 */
```

---

### 6.4 动效系统（克制但精准）

#### **1. 页面加载（逐个淡入）**

```css
@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(20px); }
  to { opacity: 1; transform: translateY(0); }
}

.fade-in {
  animation: fadeInUp 0.3s ease-out forwards;
}
.fade-in:nth-child(1) { animation-delay: 0s; }
.fade-in:nth-child(2) { animation-delay: 0.05s; }
.fade-in:nth-child(3) { animation-delay: 0.1s; }
```

**应用**: Dashboard 状态卡、Providers 列表

---

#### **2. Hover 交互（卡片微上浮）**

```css
.card-hover {
  transition: transform 0.2s ease, border-color 0.2s ease;
}
.card-hover:hover {
  transform: translateY(-2px);
  border-color: var(--accent);
}
```

---

#### **3. Toggle 开关（滑块平滑）**

```css
.toggle {
  width: 48px; height: 24px;
  background: var(--border);
  border-radius: 12px;
  position: relative;
  transition: background 0.2s ease;
}
.toggle.active { background: var(--accent); }
.toggle::after {
  content: '';
  width: 20px; height: 20px;
  background: white;
  border-radius: 50%;
  position: absolute;
  top: 2px; left: 2px;
  transition: transform 0.2s ease;
}
.toggle.active::after { transform: translateX(24px); }
```

---

#### **4. 健康灯脉动（仅 warning/danger）**

```css
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
.status-warning, .status-danger {
  animation: pulse 2s infinite;
}
```

---

## §7 后端 API 新增需求

以下接口当前不存在，需后端实现：

### 7.1 请求日志

```
GET /v1/admin/logs
Query params:
  - limit: int (default 50, max 500)
  - offset: int (default 0)
  - model: string (filter by model)
  - upstream: string (filter by provider/model)
  - status: enum[success, retry, error]
  - from: ISO8601 timestamp
  - to: ISO8601 timestamp

Response:
{
  "logs": [
    {
      "timestamp": "2024-09-24T14:32:05Z",
      "model": "jev",
      "upstream": "laya/jev-english",
      "latency_ms": 68,
      "cost": 0.0002,
      "status": "success",
      "request_body": { ... },
      "response_body": { ... },
      "error": null
    },
    {
      "timestamp": "2024-09-24T14:31:58Z",
      "model": "jev",
      "upstream": "vercel/typesafe",
      "latency_ms": 124,
      "cost": 0.0005,
      "status": "retry",
      "failover_to": "laya/jev-english",
      "error": "503 Service Unavailable"
    }
  ],
  "total": 1234
}
```

---

### 7.2 事件时间线

```
GET /v1/admin/events
Query params:
  - limit: int (default 100)
  - type: enum[failover, error, config_change, mode_switch]

Response:
{
  "events": [
    {
      "timestamp": "2024-09-24T14:31:58Z",
      "type": "failover",
      "message": "laya/jev-english failed (503) → switched to vercel"
    },
    {
      "timestamp": "2024-09-24T14:20:00Z",
      "type": "config_change",
      "message": "Added provider: deepseek"
    }
  ]
}
```

---

### 7.3 WebSocket 实时推送（可选）

```
ws://127.0.0.1:11435/v1/admin/events

Messages:
{
  "type": "request",
  "data": { ... } // same as /v1/admin/logs item
}
{
  "type": "event",
  "data": { ... } // same as /v1/admin/events item
}
```

---

## §8 实施路径

### 8.1 Phase 1: Dashboard + 视觉系统（当前 sprint）

**任务清单**:
- [ ] 更新 `tokens.css`：新色彩系统 + 字号层级
- [ ] 重构 `HomePage.tsx` → `DashboardPage.tsx`：
  - [ ] 四张状态卡（Daemon / Mode / Providers / Routes）
  - [ ] 实时请求流水（mock 数据先行）
  - [ ] 快速操作区
- [ ] 简化 `Shell.tsx` 顶栏：去掉 footer，右上角改为图标
- [ ] 新增 `SettingsModal.tsx`：密码修改 + 版本信息 + 导入导出
- [ ] `/show` 浏览器预览

**验收标准**:
- Dashboard 打开后 5 秒内能看清所有关键状态
- Mode 切换无二次确认框
- 视觉层级清晰（标题 ≥ 正文）

---

### 8.2 Phase 2: Control 页（Providers + Routes 合并）

- [ ] ProvidersPage → Control/Providers tab
  - [ ] Provider 卡片简化（健康灯前置、去 API 路径）
  - [ ] 就地编辑（Edit 模式展开表单）
- [ ] RoutingPage → Control/Routes tab
  - [ ] Graph/Table 双视图 toggle
  - [ ] BipartiteCanvas 移动端优化
  - [ ] 去掉冗余说明文字

---

### 8.3 Phase 3: Logs 页（新增）

- [ ] 后端实现 `/v1/admin/logs` + `/v1/admin/events`
- [ ] 前端实现 `LogsPage.tsx`：
  - [ ] Requests tab（表格 + 过滤器 + 分页）
  - [ ] Events tab（时间线）

---

### 8.4 Phase 4: Playground 浮层化

- [ ] 提取 `PlaygroundPanel.tsx`（可复用组件）
- [ ] 全局快捷键 `Cmd/Ctrl + P`
- [ ] Dashboard 快速操作区集成

---

## §9 需用户裁定的决策点

### 决策 1: Playground 是独立页还是浮层？

**✅ 已裁决（2026-09-24）**: 保留独立页

- **理由**: 空间充足，适合复杂测试场景
- **实施**: Playground 保持独立路由，不做浮层化

---

### 决策 2: Dashboard 实时流水的数据源？

**✅ 已裁决（2026-09-24）**: SSE (Server-Sent Events)

- **理由**: 服务端内核向客户端单向推送，一个 SSE HTTP 连接足够
- **实施**: 后端实现 `GET /v1/admin/events` SSE 端点，前端用 EventSource 消费
- **优先级**: 阻塞 Dashboard 开发，Phase 1 必须完成最小可用 SSE 接口
- **要求**: 留足扩展性，Tauri 端和云部署模式都要测试

---

### 决策 3: Mode 切换要不要二次确认？

**✅ 已裁决（2026-09-24）**: 需要确认模态框并提示功能变化

- **文案内容**: 提示 local/cloud 核心差异（loopback vs Bearer token）
- **示例**: 「切换到 cloud mode 后，监听地址将从 127.0.0.1:11435 变为 0.0.0.0:11435，外部设备可访问，需要 Bearer token 认证。是否继续？」
- **实施**: 点击 Mode 切换按钮后弹出确认框，用户确认后才执行切换

---

### 决策 4: 用户叙事要不要翻新？

**✅ 已裁决（2026-09-24）**: 绝对需要重写并扩写

- **范围**: 扩写为完整的用户旅程地图（多角色、多场景）
  - USER-PERSONAS.md（开发者/运维/爱好者三类画像）
  - USER-JOURNEYS.md（首次配置/日常监控/故障排查三类旅程）
  - DECISION-TREE.md（用户在每个页面的决策流）
- **原有叙事**: 保留故事性叙述文档（如「Kiro 的第一天」），但改写匹配新三页架构（Dashboard + Control + Logs）
- **定位**: 日后成为项目文档的一部分，与故事案例各有侧重

---

## §10 实施状态

**设计方向**: ✅ 已确认 Dashboard + Control + Logs 三页架构（Home 是旧称，Dashboard 是新定位）

**四个决策点**: ✅ 已全部裁决（2026-09-24）

**下一步**: 
1. 实施后端 SSE 接口（阻塞 Dashboard 开发）
2. 实施权限系统（单管理员密码 + API token 分级 + 双角色 Dashboard）
3. 重写用户叙事文档（完整旅程地图 + 故事性案例）
4. 根据新架构调整所有 docs/ 文档和 contracts/

详细实施计划见 `docs/design/IMPLEMENTATION-PLAN.md`。

---

**设计稿完成时间**: 2026-09-24  
**字数**: ~8000 字  
**覆盖范围**: IA / 交互 / 视觉 / 实施路径 / 决策点
