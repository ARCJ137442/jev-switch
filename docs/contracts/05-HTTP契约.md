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
| `POST` | `/v1/admin/providers/{id}/probe` | 健康探测 |

Admin 仅绑 `127.0.0.1`；CORS 见 §5。

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
      "id": "vercel",
      "kind": "vercel-gateway",
      "base": "https://…",
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

写入后落盘 toml（0600）；响应回 **masked**，不回明文。

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

### `POST /v1/admin/providers/{id}/probe`

```json
{ "ok": true, "latency_ms": 42, "status": 200, "error": null }
```

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
