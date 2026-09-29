# 路由图实时动效计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**目标**：让 Routing DAG 呈现真实调用的经过、成功、失败和重试，同时不把静态配置图伪装成历史事件。

## 现有入口

- `rs/crates/jev-switch-daemon/src/events.rs` 与 `/v1/admin/events/stream`：事件总线和 SSE。
- `call_logs.route_trace_json`：持久化 request ID、attempts、selected hops 和状态。
- `ui/src/api/access.ts`：已有带 Authorization 的 fetch SSE 实现。
- `ui/src/pages/RoutingPage.tsx`、`ui/src/components/routing/DagCanvas.tsx`：DAG 渲染与边身份。

## 分阶段工作

1. 定义前端事件投影：`request_started`、`hop_succeeded`、`hop_failed`、`retry`、`request_finished`；旧事件缺字段时显示未知，不补画。
2. 服务端在事件详情里提供可关联的 `request_id`、route edge identity、attempt、outcome 和 timestamp；不发送请求体/答案/key。
3. Routing 页面订阅管理员 SSE，将事件按 request ID 映射到当前 DAG 边；同一请求用颜色/短暂亮度表示状态，多个请求用队列/计数区分。
4. 增加暂停、关闭动效、`prefers-reduced-motion`、事件过期和当前图与历史 trace 不一致时的提示。
5. 用 fake upstream 生成成功、同候选重试、跨候选 failover、全失败和并发事件，做事件顺序与视觉回归。

## 完成证据

动效只由已收到的真实事件驱动；成功/失败/重试可区分；重连不重复播放旧事件；低动态模式不产生持续动画；没有 route trace 的历史行不会亮边。

## 不做

不在第一阶段修改路由策略、不把 CSS 动画当作性能指标、不引入完整图形重写或 3D 渲染。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
