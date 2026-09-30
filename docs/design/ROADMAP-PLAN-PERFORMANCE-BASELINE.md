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

## 2026-09-30 首轮快修状态

当前工作树已先处理一个可从代码与受控 fixture 直接验证的管理面热点：Dashboard 原来逐 provider 串行探测、每个结果单独 setState；改为最多 4 路并发、一次合并状态提交，并跳过尚未结束的轮次。Shell 与 Dashboard 原有重复 `/health` 轮询改为共享状态；health 与 telemetry 使用请求完成后再调度，Telemetry 页面隐藏时暂停。

同机受控 provider fixture 的 5 轮浏览器请求对照：每个 endpoint 约 300ms 延迟，串行 probe 中位数 1244.3ms，并发 probe 中位数 312.2ms。这个数字只说明四个 endpoint 同时可处理时 Dashboard 状态回填等待减少，不代表完整 Jev 请求变快。当前生产浏览器点时记录 LCP 288ms、CLS 0.01、无 long task；v0.3.0 旧点时为 LCP 265ms、CLS 0.01，但 viewport、DOM 数据量和资源状态不同，不能作为严格 A/B 改善结论。详情见 [图标与性能验证](../verification/ui-icon-performance-2026-09-30.md)。

## 分阶段工作

1. 固定测试环境：OS、CPU/RAM、Rust/UI revision、数据规模、上游 fake 延迟和请求 payload。
2. 建立四组场景：空闲 health、单直连、DAG failover、历史 50/500/5000 条查询；分别跑冷启动与暖启动。
3. 记录 p50/p95/p99：daemon 路由选择、SQLite 查询、完整网关耗时、用户看到的首屏和 Dashboard telemetry 请求。
4. 复用 `avg_upstream_latency_ms` 与 `upstream_attempts` 做 adapter processing 对照；明确逐次尝试计数和重试口径。若要得到逐跳 route latency，另加 request/attempt identity 与时间戳，不能从总耗时倒推。
5. 首轮 probe/轮询快修已在当前工作树；接着按固定硬件与相同数据完成 daemon/SQLite/UI 冷暖基线，再对新的热点做最小优化和同条件复测。

## 完成证据

- 一份覆盖 daemon 路由、SQLite、首屏和活动页、带硬件/数据量/方法/样本数的完整基线记录（尚未完成）。
- 至少一项优化有同条件前后 p50/p95 对照。
- 文档只宣称“本机条件下的测量结果”，不声称普遍优于其他网关。

## 不做

不在没有基线前引入缓存、异步队列或复杂 profiling 依赖；不把上游供应商延迟算成网关性能；不做跨平台性能结论。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
