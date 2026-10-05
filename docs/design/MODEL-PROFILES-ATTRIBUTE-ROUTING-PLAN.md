# 模型画像与属性路由计划

**状态：**需求设计已归档，尚未实现。  
**输入：**`jev-decision-lab/docs/27-JevSwitch模型画像与属性路由需求-20261005.md`。  
**目标版本：**`0.11.0`；先完成 `0.10.x` 受控模型 provider 生命周期，再在其上接入画像证据与属性路由；不属于 `v0.9.3`。

## 目标与非目标

在既有入口 → DAG → provider 路由中增加可解释的模型属性过滤和候选排序，让隐私敏感入口声明自身的模态、隐私和延迟要求。它是候选生成前的约束层与既有路由 DAG 的组合，不替换 DAG/failover，也不让 Jev 递归选择承载自己的 provider。

本计划不让 Jev Switch 采集任意宿主机资源、终止进程、删除文件或发送消息。System0 执行资源采集、权限检查和可撤销副作用；Jev 只在允许的候选中提供有限选择，最终由策略或人工审批决定是否执行。

## 模型画像的正交维度

模型画像采用两个互不混淆的维度：

| 维度 | 取值示例 | 说明 |
|---|---|---|
| 时间性质 | static / dynamic | 描述属性变化频率，不代表可信度 |
| 证据来源 | automatic / manual | 描述谁提供了值，不代表新旧或优先级 |

静态画像候选字段：协议、支持模态、部署 locality、隐私边界、许可证、runtime 类型。动态观测候选字段：health、loaded、P50/P95 latency、Private Bytes、Working Set、VRAM、错误率、最近崩溃。

每个观测和人工覆盖带元数据：`value`、`source`、`observed_at`、`reason`、`confidence`、`expires_at`。人工值优先作为有效 override；自动探测只更新自动观测，不得覆盖或删除人工值。UI 同时展示生效值和冲突的自动证据，允许用户明确清除 override 后再采用自动值。

模型画像不是现有 `Capabilities` 的扩展字段。Capabilities 继续描述请求/响应协议可做什么；profile 描述部署、隐私、资源和运行态证据。路由计划需同时满足协议 capability 与 profile 约束。

## 属性过滤与排序

在 DAG 候选展开后、实际执行前，先做确定性硬过滤，再做软排序：

1. 硬过滤：required modalities、privacy/locality、健康/可用性、用户禁止远程、显式资源预算和 provider disabled/missing。
2. 软排序：质量、延迟、成本、已加载状态和用户偏好；排序只作用于硬过滤后的候选。
3. 将有序候选交给现有 DAG/failover 机制；实际失败、重试、fallback 和最终响应继续由既有路由策略处理。

若过滤结果为空，返回稳定、可读的 `no_eligible_provider` 诊断，列出被过滤候选及原因。只有入口显式声明了允许的 fallback 策略时，才可以执行降级；严禁自动放宽 privacy、required modality 或用户禁止远程约束。

初期软排序保持确定性，不调用 Jev。以后若评估 Jev 软选择，只允许发起一次独立决策，并记录有限候选、使用的非敏感特征、输出、置信状态和确定性 fallback；候选模型不得包含当前 Jev 决策请求最终要调用的 provider，避免递归。

## 入口需求声明

隐私敏感入口可配置可选的路由需求：

```json
{
  "required_modalities": ["text"],
  "privacy": "local_only",
  "max_latency_ms": 1000,
  "memory_budget_mb": 12000,
  "allow_on_demand_start": true
}
```

适用示例：文件概要、Edge GitHub README 适配器、通知摘要。入口先匹配模型画像，再调用符合约束的 provider；入口的默认行为与无此配置的现有入口兼容。

`allow_on_demand_start` 是对 lifecycle 的授权意图，不是命令执行授权。它只表示该入口允许请求路径提出“可启动此 provider”的动作；全局 `allow_host_commands`、provider `controllable`、管理员/用户确认、资源检查和进程策略仍需全部通过。

## 现有协议、DAG 与 trace

- 保留现有 `/v1/systemone`、`extensions/media` 透传/剥离语义，不把媒体内容复制进画像或审计事件。
- 属性过滤位于 DAG 候选生成和 provider dispatch 之间；同一 DAG 可由不同入口策略约束，不复制 provider 或建平行路由图。
- `route_trace` 追加非敏感的 `eligibility` 解释：入口约束摘要、候选过滤原因、采用的人工 override 标识/时间、软排序输入摘要和最终候选顺序。
- Trace 不记录原始 prompt/state、媒体内容、API key 或用户机器完整环境；人工理由应支持敏感字段检查和长度限制。
- Provider 视角历史继续按实际 upstream attempts 展示；入口视角保持一个 parent call 聚合多个 attempts。

## 数据与 Agent API

模型 profile、动态观测、人工 override 与入口 requirements 都注册为类型化 configuration capabilities，遵循[配置能力注册表与 Agent API 计划](CONFIG-CAPABILITY-REGISTRY-PLAN.md)：

- Agent 可以查询 profile、观测 freshness、来源、人工 override 和入口约束。
- 修改人工 override 需要 `model-profile:configure` scope，必须提供 `reason`；执行 host probe/资源采集另需独立 scope。
- 观测写入与人工覆盖是不同操作；Agent 不得把自动值写入 override 通道来绕过用户控制。
- 敏感动作返回结构化权限错误、request ID 和 remediation；所有修改进入配置审计。

建议先建立纯领域结构和只读投影，不立即确定 SQLite schema 或公开 wire 字段。动态指标按来源和设备能力分别标注，缺失值为 unknown，不推断为零。

## 实施阶段

### Phase A：画像领域与来源优先级

- 定义静态/动态字段 schema 和 automatic/manual evidence envelope。
- 定义 override 优先、过期、冲突展示、reason 和清除语义。
- 建立类型化 capability descriptor 与兼容旧 provider/endpoint 配置的只读投影。

### Phase B：硬过滤与诊断

- 纯函数实现候选过滤、原因分类和无候选错误。
- 用入口约束、provider enabled、能力缺失、privacy/locality、模态、unknown health、资源预算构建组合矩阵。
- 不改变现有 DAG 执行，过滤结果只作为确定性的候选输入。

### Phase C：软排序与 route trace

- 依次加入 latency/cost/loaded/user preference 的确定性排序，并定义同分稳定排序。
- 扩展 route trace 与入口/提供商双视角展示，确保过滤候选和实发 attempt 可区分。

### Phase D：System0 与生命周期协作

- 资源数据由 System0/platform adapter 采集并声明权限、来源、时间和可用性。
- `allow_on_demand_start` 只向生命周期 orchestrator 提出请求；由 System0 执行资源检查、权限检查、用户确认和可撤销副作用。
- Jev 只提供候选选择，不自动杀进程、删除数据、触发未授权启动或发消息。

### Phase E：敏感入口模板

- 为文件概要、README 适配器、通知摘要提供入口模板。
- 默认 required modalities 与 privacy 收紧，不因过滤为空自动降级到远程 provider。
- 用模拟 provider 验证完整链路，不要求消耗真实上游额度。

## 验收矩阵

- 静态/动态 × automatic/manual 四种组合和 override 冲突/过期均有测试。
- 图像入口不能路由到 text-only provider；local-only 入口不能路由远程 provider；资源预算不足不得按软排序绕过。
- health unknown 与 unhealthy 分别按声明策略处理；unknown 不得伪装 healthy。
- 过滤为空时返回完整可解释原因；只在显式允许时使用 fallback，隐私和模态硬约束绝不放宽。
- DAG priority/failover 仍可预测；provider attempts、入口聚合、route trace 与统计一致。
- `extensions/media` 原样按现有 provider policy 转发；trace 不含媒体正文或 API key。
- Agent scope、人工 reason、审计、System0 权限拒绝和可撤销副作用均可自动测试并有人类验收步骤。
- TUI/API/UI 对同一 profile 和入口 requirements 展示相同生效值、来源、更新时间及拒绝原因。
