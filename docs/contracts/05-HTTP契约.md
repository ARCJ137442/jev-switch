# 契约 05 · HTTP API

> **作者**：Mimo-V2.6-Pro · 2026-09-23 · 状态：**定稿**  
> **目的**：消灭 `37b4242` 类 UI↔Rust 契约漂移。**TS 类型从 Rust 生成**，禁止手写第二份。

## 1. 端点总表

| Method | Path | 用途 |
|---|---|---|
| `GET` | `/health` | 存活 |
| `GET` | `/v1/models` | 模型与能力 |
| `POST` | `/v1/systemone` | Jev 决策 |
| `GET` | `/v1/admin/providers` | 提供商列表（**密文密钥**） |
| `PUT` | `/v1/admin/providers` | 写入提供商（含新密钥） |
| `GET` | `/v1/admin/routes` | 模型路由 DAG |
| `PUT` | `/v1/admin/routes` | 写回 DAG（整表替换） |
| `GET` | `/v1/admin/graph` | 统一 DAG 节点只读投影（Phase 1） |
| `GET` | `/v1/admin/capabilities` | UI/Agent 配置能力只读发现 |
| `POST` | `/v1/admin/providers/{id}/probe` | 健康探测 |
| `POST` | `/v1/admin/providers/{id}/invoke` | 在 daemon 内用指定 provider 直接调用 Jev 请求（供演练场直连上游） |
| `POST` | `/v1/admin/providers/{id}/models` | 从已保存 provider 的同源 `/v1/models` 获取模型 ID |
| `POST` | `/v1/admin/providers/discover-models` | 为尚未保存的表单执行一次不落盘模型发现 |
| `GET/PUT` | `/v1/admin/providers/{id}/lifecycle` | provider 级结构化生命周期配置读写（admin；命令不经过 shell） |
| `POST` | `/v1/admin/providers/{id}/lifecycle/status` | 受 gate 保护的本地 provider 服务状态检查 |
| `POST` | `/v1/admin/providers/{id}/lifecycle/start` | 受 gate 保护、等待 readiness 的结构化 argv 启动 |
| `POST` | `/v1/admin/providers/{id}/lifecycle/stop` | 受 gate 保护的显式/托管进程停止 |
| `GET/PUT` | `/v1/admin/host-commands` | 全局主机命令开关；开启由 UI 5 秒确认后提交 |
| `GET` | `/v1/admin/telemetry` | 当前 daemon 会话的流量、请求和资源遥测（不落盘） |
| `GET` | `/v1/admin/events` | Agent/控制台共享的可分页调用历史查询 |
| `GET` | `/v1/events/my` | 调用方 Token 作用域内的同口径历史查询 |

Admin 仅绑 `127.0.0.1`；CORS 见 §5。

### `GET /v1/admin/events` 与 `GET /v1/events/my`

这两个端点是 Agent 与人工控制台共享的历史查询基础。默认 `view=entry`，每条记录代表一次入口父请求；`route_trace.attempts` 包含实际 provider 尝试。使用 `view=provider` 并传 `provider_id` 时，服务端只返回该提供商曾参与的父请求，调用方再按 `route_trace.attempts` 回溯失败、重试、成功和耗时。返回仍是一条父记录，不复制入口日志。

支持的查询参数：

| 参数 | 说明 |
|---|---|
| `view=entry\|provider` | 查询视角；默认 `entry` |
| `endpoint_id` | 精确筛选对外入口 |
| `provider_id` | 匹配 route trace 中实际尝试过的 provider |
| `from_ms` / `to_ms` | Unix 毫秒时间范围 |
| `success=true\|false` | 入口最终结果 |
| `request_id` | 精确筛选一个请求，例如 `jev-147` |
| `since` / `before` / `limit` | 稳定 ID 游标分页；`limit` 最大 500 |

示例：

```text
GET /v1/admin/events?view=provider&provider_id=vercel&from_ms=...&limit=50
Authorization: Bearer <admin-token>
```

`route_activity` 是路由画布的内存实时生命周期流，不属于这两个历史端点；它不会写入 `call_logs`，也不会污染调用历史分页。

## 2. 形状（冻结）

### `GET /health`

```json
{ "status": "ok", "version": "0.1.0" }
```

（文本 `"jev-switch MVP"` 兼容期可保留，**以 JSON 为准**。）

### `GET /v1/models`

**定死 OpenAI 风**（UI 已按此归一化）：

```json
{
  "object": "list",
  "data": [
    { "id": "jev", "object": "model", "upstream": "vercel" }
  ],
  "upstreams": [
    {
      "id": "vercel",
      "question_types": ["choice", "score", "boolean"],
      "has_confidence": false,
      "has_usage": false,
      "noul_via_boolean": true
    }
  ]
}
```

### `POST /v1/systemone`

请求/响应 = 契约 `01-协议契约` 的 `JevRequest` / `JevResponse`。

### `GET /v1/admin/providers`

```json
{
  "providers": [
    {
      "id": "typesafe",
      "kind": "typesafe",
      "base": "https://api.typesafe.ai/v1/systemone",
      "models": ["jev-latest"],
      "enabled": true,
      "api_key_masked": "sk-****a1b2",
      "api_key_set": true
    }
  ]
}
```

**禁止**字段 `api_key` 明文。

### `PUT /v1/admin/providers`

```json
{ "providers": [ { "id": "vercel", "kind": "…", "base": "…", "enabled": true, "api_key": "…" } ] }
```

`kind` 当前接受 `vercel`、`laya`、`typesafe`、`openrouter`。TypeSafe 官方地址是完整 endpoint `https://api.typesafe.ai/v1/systemone`，Bearer key 由 daemon 添加；官方模型示例为 `jev-latest`。OpenRouter 使用完整 chat endpoint `https://openrouter.ai/api/v1/chat/completions`，daemon 将 Jev 请求转换为结构化 JSON chat 请求，再严格解析回 Jev answers；这不表示对外入口兼容一般 OpenAI/Anthropic Chat API。本地 TypeSafe kind 也必须提供相同 Jev `POST /v1/systemone` wire format。

Provider 响应另含非敏感 `lifecycle` 摘要：`controllable`、`process_policy`、`mode`、`configured`、`service_state` 和有界超时。普通 provider 列表不返回命令正文或任何 API key；专用 lifecycle 资源在 admin 权限下返回结构化命令配置（仍不返回 key），供 UI/Agent 配置使用。`PUT` 中可选 `lifecycle` 字段省略时保留现有配置。

写入后落盘 toml（0600）；响应回 **masked**，不回明文。`api_key` 省略表示保留已有密钥，空串表示清除；`api_key_env` 省略表示保留已有环境变量名。整表中移除已有 provider 时，会在同一运行时快照写入中移除其直接引用以及因此无法再解析到任何现存 provider 的别名分支，并热替换内存路由；历史调用记录保留。

### `GET /v1/admin/capabilities`

返回 UI/Agent 可发现的配置能力描述，包括稳定 key、资源、读写/执行 scope、风险、确认方式、审计标记和 schema 版本。当前只读发现已实现；类型化 configuration PATCH、scope enforcement 和生命周期执行仍按对应设计计划推进。

### Provider lifecycle 0.10.x

```text
GET  /v1/admin/providers/{id}/lifecycle
PUT  /v1/admin/providers/{id}/lifecycle
POST /v1/admin/providers/{id}/lifecycle/status
POST /v1/admin/providers/{id}/lifecycle/start
POST /v1/admin/providers/{id}/lifecycle/stop
GET  /v1/admin/host-commands
PUT  /v1/admin/host-commands   {"enabled": boolean}
```

这些端点受现有 admin 会话门保护；桌面 LAN 开放时只有 loopback 走本机所有者直通，其他 LAN/cloud 请求需管理员会话或 admin token。执行同时要求全局 `allow_host_commands=true` 与 provider `lifecycle.controllable=true`，使用结构化 `program`/`args`，不经过 shell。`start` 只有 readiness 成功才返回 `running`；unknown、超时、命令失败均返回稳定的 `code/message/remediation` 错误。`startup_check` 与 `on_demand` 只在明确 `stopped` 或请求前置时触发，生命周期所有权和审计独立存储，不进入调用历史；细粒度 Agent scope、跨 daemon 进程组回收、UI 和平台验收仍在后续阶段。

### `GET/PUT /v1/admin/routes`

```json
{
  "routes": [
    {
      "left": "jev", "match": "exact", "right": "vercel",
      "upstream_model": "typesafe-ai/jev",
      "priority": 10, "sticky": "session", "on_error": "next"
    }
  ]
}
```

`PUT` = **整表替换**；写入 toml；DAG 含环则 `400`。

`GET /v1/admin/graph` 返回当前运行时快照的 typed node projection：`public`（入口）、`internal`（中间节点）和 `provider`（出口）。它与 `/v1/admin/routes` 共享同一运行时快照；当前只读，provider 凭据不出现在节点文档中。

### `POST /v1/admin/providers/{id}/probe`

```json
{ "ok": true, "latency_ms": 42, "status": 200, "error": null }
```

此探测只对上游 `GET base` 检查网络连通性；收到任何 HTTP 状态都代表端点可达，不验证 TypeSafe Bearer key，也不运行推理。

### `POST /v1/admin/providers/{id}/models`

daemon 将 provider 的完整 `POST /v1/systemone` 地址解析为同 host/base path 下的 `GET /v1/models`，使用 daemon 持有的 key 添加 Bearer header。成功响应为：

```json
{ "ok": true, "latency_ms": 42, "status": 200, "models": ["jev-latest"], "error": null }
```

当前解析 TypeSafe 的 `models` 数组和 OpenAI 风格的 `data` 数组，支持字符串条目及 `{id}`/`{name}` 条目并去重。请求不运行推理；模型目录失败时返回 `ok=false` 和已掩码错误。

`POST /v1/admin/providers/discover-models` 使用相同响应形状，但从请求体读取未保存表单的 `kind/base/api_key`，只在本次请求内使用，不写入配置或响应。

### `GET /v1/admin/telemetry`

该端点返回当前 daemon 进程自启动以来的只读会话遥测。它不读取或保存请求体、答案、密钥或历史调用正文；重启 daemon 后累计值从零开始。`samples` 是最近约 10 分钟的内存环形采样，前端默认约每秒读取一次：

```json
{
  "schema_version": 1,
  "session_started_at_ms": 0,
  "sampled_at_ms": 0,
  "ingress_bps": 0,
  "egress_bps": 0,
  "ingress_bytes_total": 0,
  "egress_bytes_total": 0,
  "active_requests": 0,
  "total_requests": 0,
  "success_requests": 0,
  "failed_requests": 0,
  "failover_requests": 0,
  "avg_gateway_latency_ms": null,
  "avg_upstream_latency_ms": null,
  "daemon": { "available": true, "memory_bytes": 0, "cpu_percent": 0, "source": "daemon-process", "precision": "process-snapshot" },
  "shell": { "available": false, "memory_bytes": null, "cpu_percent": null, "source": "tauri-shell-not-exposed", "precision": "unavailable" },
  "samples": []
}
```

`ingress`/`egress` 是网关客户端边界上可观测的请求/响应 JSON 字节；当前失败响应也计入已知 JSON body 大小。上游延迟和 Tauri WebView 资源尚未由 daemon 精确提供时返回 `null`/`available=false`，不得由客户端估算后冒充真实值。

### `POST /v1/admin/providers/{id}/invoke`

请求体为契约 `01-协议契约` 中的 `JevRequest`；daemon 使用该 ID 已保存的适配器与密钥，不接受请求体传入 base/key。成功时返回 `JevResponse` 并带 `x-jev-request-id`；密钥与请求正文不写入调用历史。

## 3. 错误体（统一）

```json
{ "error": "…", "upstream": "vercel", "retryable": true }
```

| 情况 | HTTP |
|---|---|
| 未知 model | 404 |
| capability 不匹配 | 422 |
| 上游 429/5xx（retryable） | 503 |
| 上游其它错误 | 透传上游码 |
| 请求体协议不合法 | 400 |
| 反序列化上游失败 | 502 |

错误 `error` 字符串必须过 **redact**（契约 04）。

## 4. 类型单一来源

- Rust `#[derive(JsonSchema)]` 或 `ts-rs` 导出 → `ui/src/` 生成物
- **禁止** UI 手写与 Rust 重复的 `ModelsResponse` 等
- CI（有后）：生成物与手写 diff 为空

## 5. CORS

| 阶段 | 策略 |
|---|---|
| MVP | `very_permissive`（已知债务） |
| M1+ | **origin 白名单**（默认 `http://127.0.0.1:5173`） |
| Admin | 默认不跨源；仅同主机 UI |

## 6. 验收

- [x] 端点与形状冻结成文
- [ ] OpenAPI/JSON Schema 提交进 `docs/contracts/schemas/`
- [ ] UI 生成类型替换手写
- [ ] 错误体 redact 单测
- [ ] admin 无明文泄露单测

— Mimo-V2.6-Pro
