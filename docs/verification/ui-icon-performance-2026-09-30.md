# UI 图标与性能迭代验证（2026-09-30）

**作者：** GPT-6 Luna xhigh（OpenAI Codex）
**AI 披露：** AI 辅助实现与测量；性能数字只代表本机、当前浏览器和本地 fixture 条件。内容已随 v0.4.0 发布，但 v0.4.0 尚未经人工实测。

## 已实现

- 顶部五个主导航补充 Lucide 语义图标：仪表盘、提供商、入口、路由、演练场；文字标签保留，图标只增强定位，不替代可访问名称。
- Dashboard 状态速览加入 daemon、运行模式、提供商、路由图标；测试入口、添加提供商、编辑路由动作加入图标+文字。
- Routing 工具栏的撤销、重做、表格、刷新、丢弃改为固定 32px 图标按钮，保留 tooltip 和 `aria-label`，避免文字按钮在窄屏挤压布局。
- 删除无消费者的动态 `Icon.tsx` 包装，保留现有 Lucide 具名 import，避免 `any` 和不必要的命名空间运行时查找。
- Dashboard provider probe 改为最多 4 路并发、单批 state 提交、轮次防重叠；失败 provider 不阻塞其他结果。
- Shell 的 health 状态改为共享上下文，Dashboard 不再启动第二个独立健康轮询。
- RuntimeTelemetry 从固定 `setInterval` 改为请求完成后调度下一次，并在页面隐藏时暂停，避免慢请求重叠和后台唤醒。

## 测量

生产 `v0.3.0` 原始 UI baseline（Windows Edge，1600×709）：

```text
JS  406.12 kB / gzip 123.03 kB
CSS 43.92 kB / gzip 9.72 kB
LCP 265 ms，CLS 0.01，long task 0
DOM 263 elements，最大布局更新 45 ms
```

本轮最终 production build 为 JS 418.70 kB / gzip 125.58 kB、CSS 44.33 kB / gzip 9.80 kB。相对 v0.3.0 baseline，JS gzip 增加约 2.55 kB（主要是新增具名图标及活动指标符号）；导航现有文字标签未移除。没有用 bundle 体积换取未经证实的“整体变快”宣称。

使用四个仅监听 `127.0.0.1`、每个响应延迟约 300ms 的本地 fixture provider，Dashboard 首轮 probe：

| 方式 | 5 轮中位数 | 说明 |
|---|---:|---|
| 原串行调度（受控浏览器脚本复现） | 1244.3 ms | 4 个约 300ms 请求相加 |
| 新并发调度（并发上限 4） | 312.2 ms | 接近最慢单请求，最多 4 个同时进行 |

浏览器首屏真实同源 daemon 结果：最终构建 trace 在 1600×900 深色视口测得 LCP 288ms、CLS 0.01、无 long task，DOM 342 elements，最大 layout update 46ms。旧 baseline 的 LCP 265ms 来自 1600×709、空活动数据视口，因此两者不构成严格 A/B；DOM 差异还受活动行与 provider fixture 数据量影响。

生产 UI 在 1600×900、390×844、320×568 视口核对：文档宽度分别与 viewport 相等；320×568 下主导航换行但不溢出，Routing 图标工具保持 32×32 并有可访问名称/tooltip。访问活动成功/失败状态图标和统计分类图标保留了对应文字标签。

遥测后台调度在 Chrome 页面用 `visibilitychange` 受控模拟：隐藏 2.3 秒期间新增 telemetry 请求数为 0；改回可见后请求恢复。手动刷新按钮与自动采样共用 in-flight guard，不会并行发送同一 telemetry 请求。该测试验证页面策略事件，不冒充真实浏览器标签切后台或 Tauri 最小化生命周期实测。Shell/Dashboard 健康状态已改为单一 context producer，源码中只由 Shell 发出 8 秒 health poll。

自动化门禁：UI 测试 27/27 通过，TypeScript lint 与 production build 通过；`cargo test --manifest-path src-tauri/Cargo.toml --locked` 为 10 passed、0 failed、6 ignored。被忽略的测试要求活动 daemon、可见 Windows 桌面或真实 Tauri WebView/托盘交互；本轮没有把这些项目记作已验收。

## 边界

- fixture probe 只证明调度与渲染行为，不证明真实上游质量、网络 RTT 或跨硬件速度。
- `LCP/DOM` 是当前浏览器实验值，不等同于 Tauri WebView 或低端 Android 性能；原始与当前视口/fixture 不一致时不比较性能增减。
- 下一步性能计划仍需按固定 hardware、payload、冷/暖启动、p50/p95/p99 补完整基线；当前优化只承诺减少重复健康请求、探测串行等待和隐藏页 telemetry 轮询。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
