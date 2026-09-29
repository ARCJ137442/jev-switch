# 网关性能基线计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**目标**：建立可重复的“网关自身快不快”证据，把网关路由、SQLite、前端轮询和上游响应分开测量。

## 现有入口

- `rs/crates/jev-switch-daemon/src/telemetry.rs`：会话字节、请求数、资源和平均 gateway latency。
- `rs/crates/jev-switch-daemon/src/lib.rs`：`systemone_handler` 计算 route trace 与 gateway latency。
- `rs/crates/jev-switch-daemon/src/tokens.rs`：持久历史查询和统计。
- `ui/src/components/dashboard/RuntimeTelemetry.tsx`：当前首页轮询与曲线。

`abc793f` 已提交 `TimedUpstream` / `avg_upstream_latency_ms`；它提供每次 adapter attempt 的 transport + parsing 总耗时，已进入主线但仍不是纯网络耗时。不要把它与 gateway latency 相加，也不要把平均值描述成逐跳网络 RTT。

## 分阶段工作

1. 固定测试环境：OS、CPU/RAM、Rust/UI revision、数据规模、上游 fake 延迟和请求 payload。
2. 建立四组场景：空闲 health、单直连、DAG failover、历史 50/500/5000 条查询；分别跑冷启动与暖启动。
3. 记录 p50/p95/p99：daemon 路由选择、SQLite 查询、完整网关耗时、用户看到的首屏和 Dashboard telemetry 请求。
4. 复用 `avg_upstream_latency_ms` 与 `upstream_attempts` 做 adapter processing 对照；明确逐次尝试计数和重试口径。若要得到逐跳 route latency，另加 request/attempt identity 与时间戳，不能从总耗时倒推。
5. 对发现的热点做最小优化，再用同一输入重复对照；每次优化保留前后数据和回归测试。

## 完成证据

- 一份带硬件、数据量、方法和样本数的基线记录。
- 至少一项优化有同条件前后 p50/p95 对照。
- 文档只宣称“本机条件下的测量结果”，不声称普遍优于其他网关。

## 不做

不在没有基线前引入缓存、异步队列或复杂 profiling 依赖；不把上游供应商延迟算成网关性能；不做跨平台性能结论。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
