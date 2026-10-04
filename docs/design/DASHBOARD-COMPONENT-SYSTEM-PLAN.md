# Jev Switch 仪表盘组件系统与页面布局编辑计划

**状态：** Phase 1 Dashboard 工作树原型已实现；专用页面布局和 Release 集成尚未开始
**日期：** 2026-10-04
**参考：** `obsidian-life-series-tools` 的 Dashboard 组件、布局编辑、预设和迁移设计

> 本文只规划 UI 组件系统、布局编辑器和个性化边界，不改变网关、路由策略、Android 生命周期或调用记录协议。

## 1. 目标与边界

Jev Switch 需要让用户拥有高度可定制的首页，同时保持路由、设置等专用页面的任务完整性。目标分为两层：

- **首页仪表盘：** 用户可以从组件库添加、移除、排序、隐藏、调整宽度，保存多个布局预设，并在编辑态实时预览。
- **专用页面：** 路由、设置、统计等页面可以复用同一套组件注册和布局约束，但只能在页面职责允许的槽位内变化；不能把路由画布或设置表单变成任意拖拽卡片集合。

本轮不把 Obsidian 的插件 API、DOM 工具、宿主生命周期或数据模型复制进 Jev Switch。只借鉴它已经验证过的布局领域模型和编辑事务。

## 2. 参考设计中值得保留的经验

参考项目当前的有效设计可归纳为六点：

1. **组件定义与布局实例分离。** 组件库描述 `id/label/category/description/icon/implemented/singleton`；布局只保存 `instanceId/id/visible/width` 及少量展示参数。
2. **布局有规范化入口。** 读取未知组件、重复单例、旧字段或非法宽度时，统一转换为可渲染的安全布局，而不是让页面各自容错。
3. **预设按身份隔离。** 内置预设、自定义预设和设备本地布局分开保存；切换预设不会覆盖其他预设。
4. **编辑使用草稿事务。** 进入编辑时复制快照；实时预览只改草稿；完成才提交，取消恢复进入前布局；编辑器提供撤销/重做。
5. **布局设置与配色/材质分层。** 间距、列数、标题大小和内容宽度不与业务组件硬编码在一起；外观偏好可以单独导入/导出。
6. **触摸和键盘都能编辑。** 原生拖放、Pointer Events、自动滚动、上下移动按钮、键盘撤销/重做同时存在，拖动失败不能留下半成品。

这些经验适合成为 Jev Switch 的布局基础，但 Jev Switch 的组件数据来源必须继续走现有 API、健康状态和本地偏好边界。

## 3. Jev Switch 目标模型

### 3.1 组件注册表

建立类型安全的组件注册表，组件定义只描述展示契约，不持有跨页面业务状态：

```text
DashboardComponentDefinition {
  id: string,
  label: MessageKey,
  description: MessageKey,
  category: overview | traffic | routing | history | diagnostics | utility,
  icon: LucideIconName,
  singleton: boolean,
  implemented: boolean,
  defaultWidth: full | wide | half | narrow | quarter,
  minWidth: number,
  allowedPages: dashboard | routing | statistics | settings,
  dataSource: local_snapshot | daemon_health | call_history | route_activity | none
}
```

组件渲染器接收只读的页面数据快照和动作回调；组件不能自行读取 API key、管理会话或修改路由配置。动作必须回到页面拥有者或既有 API client。

### 3.2 布局文档

```text
LayoutDocument {
  schemaVersion: number,
  page: dashboard | routing | statistics | settings,
  presetId: string,
  items: LayoutItem[],
  display: DisplaySettings,
  revision: number
}

LayoutItem {
  instanceId: string,
  componentId: string,
  visible: boolean,
  width: full | wide | half | narrow | quarter,
  order: number,
  settings?: Record<string, safe-json-value>
}
```

`settings` 只允许组件声明过的展示设置，例如时间范围、是否显示图例、指标密度；请求内容、密钥、调用 Token、路由草稿和探测结果不能进入布局文档。组件运行状态与布局状态分开，刷新或切换布局时不会伪造新的业务事件。

### 3.3 显示设置与个性化

将以下内容放入设置的“个性化/外观”分组：

- 默认首页布局预设。
- 默认密度、间距、列数、最大内容宽度和标题字号。
- 首页是否进入编辑模式的入口偏好。
- 专用页面允许的布局模式，例如路由 HUD 默认收起、统计区块密度。
- 组件库中是否显示实验组件。

这些是 UI 本地偏好，按 API origin、应用 profile 和设备保存；不写入 daemon 配置，不影响其他用户。现有 UI scale、主题、语言、状态栏和路由 HUD 偏好保持独立键，避免一个布局导入覆盖全局行为。

## 4. 页面布局策略

### 4.1 Dashboard：首页自由组合

首页采用组件网格：

- 运行状态、活跃请求、可达提供商、路由数等现有摘要先注册为系统组件，默认预设保持当前信息层级。
- 用户可以调整顺序、显隐和宽度；组件宽度只在注册表允许范围内变化。
- 没有数据时组件呈现未知/空状态，不因布局编辑而生成数据。
- 多次添加只允许非单例组件；组件实例用 `instanceId` 区分，不能以数组位置作为持久身份。
- 预设切换先进入草稿并提示影响范围，不能直接覆盖用户自定义布局。

### 4.2 Routing：图引擎优先的受限布局

路由页不是普通卡片仪表盘：

- DAG 画布、节点布局、边走廊、辉光状态和 HUD 是固定工作区组件。
- 允许用户配置 HUD 是否自动隐藏、表格/Inspector 是否默认展开、辅助信息密度和画布占用比例。
- 节点坐标、缩放和平移继续由路由画布自己的模型管理，不写入 Dashboard 网格布局。
- 禁止把节点卡片拆成首页式小组件后绕过 DAG 交互；节点图仍是完整、可平移、无滚动条的画布。

### 4.3 Settings：职责固定的分区布局

设置页可以使用可排序分区和渐进披露，但不能自由删除安全、连接、备份等必要入口：

- 组件级能力只控制分区顺序、展开状态和密度。
- 搜索结果始终能找到隐藏分区；恢复默认布局必须可用。
- 包含敏感操作的分区不能被自定义组件覆盖，导入布局不得改写权限、API 地址或密钥。

### 4.4 Statistics：可配置摘要，不改变统计口径

统计页可以让用户选择最近调用、热力图、入口/提供商切换和筛选栏的排列，但：

- 热力图正方形、无横向滚动、时间范围联动仍是硬约束。
- 组件设置只保存筛选默认值、显示列和密度；不保存或伪造历史数据。
- 入口聚合与 provider attempt 两种视角继续由历史契约决定，不由组件组合改变。

## 5. 编辑器交互

编辑器建议采用“组件库 + 当前布局 + 实时预览”的三段式结构：

1. 进入编辑：保存当前 `LayoutDocument` 快照，显示取消、完成、撤销和重做。
2. 组件库：按类别搜索，显示已实现/未实现、单例和允许页面；添加操作只产生草稿实例。
3. 当前布局：拖动排序、上下移动、显示/隐藏、宽度选择、组件设置编辑；支持 Pointer Events 和键盘等价操作。
4. 预览区：直接使用当前草稿渲染真实组件，组件动作仍可用，但写配置动作应显示确认或进入独立编辑表单。
5. 完成：规范化并校验草稿，成功后一次性保存；保存失败保留草稿并显示原因。
6. 取消：丢弃草稿、恢复编辑前布局和焦点，不触发业务刷新。

布局导入先解析到草稿，检查 schema、组件 ID、页面类型、宽度范围和未知设置；只有用户点击完成才覆盖当前布局。导出只包含布局和展示设置，不含 API key、调用 Token、请求详情或实时事件。

## 6. 存储与迁移

第一阶段采用 UI 本地存储，与现有 `localStorage` 偏好机制一致：

```text
jev-ui-layouts-v1:<api-origin>:<profile>:<page>:<preset>
jev-ui-layout-display-v1:<api-origin>:<profile>:<page>
jev-ui-layout-preferences-v1:<api-origin>:<profile>
```

要求：

- key 中不能出现 API key、调用 Token 或完整请求地址中的凭据部分。
- 每个页面、预设、API origin 和设备 profile 隔离，避免不同 daemon/版本串布局。
- 读入失败、未知 schema 或未知组件回退到该页面默认预设，并保留原始坏数据供诊断，不让页面白屏。
- 新版本用显式迁移函数转换旧布局；不通过“数组位置碰巧一致”恢复实例。
- JSON 设置备份可以增加布局和个性化分区，但必须与 provider/API key 明文备份分层提示；导入布局不能隐式导入凭据。

后续若需要跨设备同步，再设计 daemon 管理 API；在此之前不把 UI 布局写入 `providers.toml`、运行时图快照或调用历史。

## 7. 与参考项目的边界

| 参考能力 | Jev Switch 处理 |
|---|---|
| 组件 registry | 采用，改为 React/TypeScript 类型与 i18n MessageKey |
| `instanceId` 与单例约束 | 采用，避免重排/删除后错绑状态 |
| 布局 normalize 与旧版本迁移 | 采用，作为唯一入口 |
| 草稿、取消、撤销/重做 | 采用，编辑器事务化 |
| Pointer Events、自动滚动、键盘操作 | 采用，适配桌面和 Android WebView |
| 配色/材质独立于布局 | 只保留主题和密度等已有 Jev 语义，不引入独立材质系统 |
| Obsidian workspace/Leaf 生命周期 | 不采用；Jev 页面由 React/Tauri 壳管理 |
| 任意组件执行宿主命令 | 不采用；组件动作必须走受控回调和既有权限边界 |

## 8. 分阶段计划

### Phase 0：组件与页面能力清单

- 盘点 Dashboard、Routing、Statistics、Settings 的现有区块，定义组件 ID、数据源、权限、单例和最小宽度。
- 把当前 Dashboard 现有卡片映射成默认布局，但不改变视觉排列。
- 确定哪些设置属于全局偏好，哪些是页面布局，哪些属于业务配置。

**门槛：**每个组件都有明确 owner、数据来源和空/错误/加载状态；没有“万能组件”。

### Phase 1：纯布局模型与注册表

- 新增 `LayoutDocument`、normalize、版本迁移、预设隔离、网格列/宽度纯函数。
- 为 normalize、单例、重复实例、非法宽度、未知组件、布局导入失败补测试。
- 先以默认布局驱动 Dashboard，不开放编辑器，保证功能输出不变。

### Phase 2：首页编辑器

- 引入编辑草稿、实时预览、完成/取消、撤销/重做、组件库搜索、拖动排序和键盘操作。
- 保存到按 origin/profile 隔离的 UI 本地存储。
- 设置加入“个性化：首页布局、密度、默认预设、恢复默认、导入/导出布局”。

**门槛：**桌面、窄桌面、Android 竖屏均无横向滚动；编辑取消不改变业务状态；重启和版本升级不串布局。

### Phase 3：专用页面受限布局

- Routing 只开放 HUD/Inspector/表格等辅助槽位；画布几何继续独立。
- Statistics 允许摘要区和筛选区布局；不改变热力图和历史口径。
- Settings 允许分区顺序/展开状态/密度；保留安全和恢复入口。
- 每个页面使用独立组件 registry 过滤，禁止把 Dashboard 专用组件拖进不适合的页面。

### Phase 4：预设与备份收口

- 支持内置布局、用户自定义布局和设备局部布局的独立命名。
- JSON 导入/导出布局先草稿、后确认；与含 API key 的配置备份明确分离。
- CLI/Agent 初期只查询布局状态；编辑布局的 API 等页面模型稳定后再开放。

### Phase 5：跨平台验收

- Vite fixture、Tauri WebView、Android WebView 分别验证同一布局文档。
- 覆盖浅/深/系统主题、10%–200% UI scale、窄窗口、触摸拖动、键盘撤销和 reduced motion。
- 不把 UI 布局通过自动测试的结果冒充 Android 原生生命周期验收。

## 9. 验收矩阵

| 类别 | 场景 |
|---|---|
| 默认基线 | 新 profile 默认 Dashboard 与当前信息层级一致 |
| 编辑事务 | 添加、排序、宽度、显隐、取消、完成、撤销、重做 |
| 数据边界 | 组件加载/空/错误/权限不足状态正确；不伪造健康与历史 |
| 迁移 | 旧布局、未知组件、重复单例、非法宽度、坏 JSON、跨版本恢复 |
| 预设 | 内置、自定义、设备局部预设互不覆盖；切换可回退 |
| 个性化 | 设置搜索可找到布局选项；恢复默认不影响 API、主题、密钥和路由 |
| Routing | DAG 平移/缩放/接线/辉光不受编辑器布局系统破坏，无横向滚动 |
| Settings | 搜索、敏感操作、备份恢复入口始终可达 |
| Statistics | 热力图方格、筛选范围、入口/provider 视角保持既有契约 |
| 平台 | 浏览器、Tauri、Android WebView 的布局和键盘/触摸交互一致 |

## 10. 暂不决策的事项

1. 是否允许用户创建完全自由的首页文本/分隔线组件，还是只开放产品组件。
2. 布局预设是否随 JSON 设置备份跨设备迁移，还是仅设备本地保存。
3. Dashboard 组件是否允许多实例；指标组件默认单例，历史/入口摘要可能需要多实例。
4. 是否允许用户保存专用页面布局，还是只保存 Dashboard 个性化。
5. 是否把“个性化”纳入未来 Agent API；初期建议只读查询，避免 Agent 改坏用户界面。

## 11. 结论

参考项目最值得借鉴的不是某个 CSS 或拖拽库，而是 **组件定义、布局实例、显示设置、预设和编辑事务的分层**。Jev Switch 应先把 Dashboard 变成可组合首页，再把同一套模型以受限策略复用于路由、统计和设置；不要建立一个可以任意重排所有业务页面的万能组件系统。
