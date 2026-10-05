# 配置能力注册表与 Agent API 计划

**状态：**架构设计完成；`GET /v1/admin/capabilities` 只读发现、provider lifecycle 领域读写/执行端点、主机命令开关和独立审计已实现；通用类型化配置 PATCH、细粒度 scope enforcement 和 Agent 写入面仍未实现。  
**目标：**让每个面向人的配置入口同时拥有可发现、可鉴权、可审计、可版本化的 Agent API，避免 UI、HTTP 和 Agent 三套配置逻辑长期漂移。

## 设计原则

- 人类 UI 与 Agent API 共享同一 capability descriptor、类型校验、默认值、迁移和权限声明。
- 注册表只生成“已声明能力”的发现和类型化访问，不生成任意 JSON 路径写入器或任意命令执行器。
- 领域动作仍保留领域端点，例如 provider lifecycle `start/stop/status`、路由提交和回滚；注册表负责发现、权限、审计和文档元数据。
- API key、调用 token、管理员密码和命令敏感变量永远只有 masked/presence 视图；Agent 只能在声明的 scope 下触发使用，不能读取明文。
- 每个 capability 有稳定 key、schema version、风险等级、读写 scope、确认策略、审计策略和错误码集合。

## Capability 描述

```text
ConfigCapability {
  key: "providers.lifecycle.controllable",
  resource: "provider",
  value_schema: boolean,
  default: false,
  read_scope: "config:read",
  write_scope: "provider:lifecycle:configure",
  execute_scope: null,
  risk: "host_command",
  confirmation: "cooldown_5s",
  audit: true,
  schema_version: 1
}
```

descriptor 必须能同时驱动：

1. UI 控件类型、标题、说明、默认值、禁用原因、确认模态框和国际化 key。
2. Agent 发现目录、输入校验、权限检查、风险提示和结构化 remediation。
3. 配置导入/导出、版本迁移、审计事件和文档生成。

## HTTP 表面

首版建议增加只读发现和类型化配置入口：

```text
GET   /v1/admin/capabilities
GET   /v1/admin/configuration
PATCH /v1/admin/configuration
```

`PATCH` 只允许注册过的 key，并按 descriptor 的 schema、scope、confirmation 和 migration 执行。高风险/有副作用动作不通过通用 PATCH：

```text
POST /v1/admin/providers/{id}/lifecycle/status
POST /v1/admin/providers/{id}/lifecycle/start
POST /v1/admin/providers/{id}/lifecycle/stop
```

Agent 发现目录应返回 capability key、当前值的非敏感视图、可用 scope、风险和需要的确认；不返回命令完整参数、API key 或管理员凭据。

## 权限模型

| Scope | 用途 |
|---|---|
| `config:read` | 读取非敏感配置和脱敏运行状态 |
| `config:write` | 修改普通 provider、入口、路由和 UI 偏好 |
| `provider:lifecycle:configure` | 修改 controllable、process policy、命令结构和启动策略 |
| `provider:lifecycle:execute` | 执行已登记 provider 的 status/start/stop |
| `host_commands:enable` | 开启全局主机命令总开关，要求管理员或本机用户确认 |
| `secrets:use` | 允许命令在运行时接收 API key 环境变量，不允许明文回读 |

云端和 LAN 的配置、生命周期执行都需要管理员会话或等价管理员 Agent scope；普通 caller token 永远不能改 capability 或执行主机命令。local 模式可由本机控制面免会话，但仍受全局开关和 provider `controllable` 限制。Android local 使用同一 scope 与确认流程。

## 错误与审计

所有 Agent/API 失败都返回稳定 code、message、request_id、必要时 execution_id 和 remediation：

```json
{
  "code": "host_commands_disabled",
  "message": "Host command execution is disabled for this instance.",
  "request_id": "req-…",
  "remediation": "An administrator must enable allow_host_commands after the safety confirmation."
}
```

至少区分：`forbidden`、`capability_not_found`、`schema_invalid`、`confirmation_required`、`host_commands_disabled`、`provider_not_controllable`、`command_not_configured`、`readiness_timeout`、`command_failed` 和 `state_unknown`。

配置修改、风险开关变化和 Agent 生命周期执行写入 append-only 审计事件，记录 actor、scope、capability、resource、结果、request/execution ID 和脱敏摘要。审计事件不得进入模型调用历史或 route_activity。

## 版本与维护

- capability key 永不复用；删除能力时保留 deprecated descriptor 和迁移提示。
- schema version 变化必须提供纯函数迁移和回滚测试。
- 注册表生成的 UI/API 文档进入 CI 检查，确保新增配置入口同时声明 Agent surface、权限和审计策略。
- 领域 API 仍由 Rust 类型和 HTTP contracts 作为协议真值；注册表不能绕过 ts-rs、鉴权中间件或领域校验。
- 每个新设置的最小门槛：人类 UI、Agent read/write/execute（按风险选择）、中英文文案、权限、错误码、审计、迁移和测试矩阵。

## 实施顺序

1. 在 daemon 内定义 typed descriptor、scope 和风险模型，先注册只读现有设置。
2. 增加 `/capabilities` 与 `/configuration` 只读发现，接入 Agent/CLI 查询。
3. 为普通低风险设置增加类型化 PATCH，验证迁移、权限和审计。
4. 接入 provider lifecycle 的高风险配置和领域动作，验证 5 秒确认与 host-command gate。
5. 将入口、路由、Dashboard 个性化和 Android 运行控制逐项迁移；每项保留兼容 API，不做一次性大爆炸迁移。
