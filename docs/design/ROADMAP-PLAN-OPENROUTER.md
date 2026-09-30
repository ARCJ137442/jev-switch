# OpenRouter Adapter 计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**当前状态（2026-09-30）**：OpenRouter 上游适配器已随 v0.2.0 发布，v0.4.0 继续包含；协议/WireMock 与离线 UI/daemon 验证已完成。无获授权的 live key 验证，不宣称真实账户调用、计费或所有模型能力通过。v0.4.0 尚未经人工实测；该文下方阶段清单保留实现前计划语境，不再代表 adapter 尚未写入源码。

**优先级**：最高。  
**目标**：让 OpenRouter 成为一个真实可测试的上游适配器，定义它在 Jev Noul/Choice/Score 能力上的完整边界，不把普通 OpenAI Chat Completions 兼容误称为 Jev 原生支持。

## 现有入口

- `rs/crates/jev-core/src/adapter.rs`：`UpstreamAdapter`、capability、retry/failover、route trace。
- `rs/crates/jev-adapters/src/upstream_typesafe.rs`、`upstream_vercel.rs`、`upstream_laya.rs`：三种 adapter 的错误、鉴权、响应转换参考。
- `rs/crates/jev-switch-daemon/src/lib.rs`：按 provider `kind` 装配 adapter。
- `ui/src/api/providerKinds.ts`、Provider 表单和模型发现：kind 选择、预设、`/v1/models`。
- `docs/03-上游类别与协议兼容矩阵.md`：仅作历史参考，不能直接当作现行协议结论。

## 阶段 A：协议核对（先做）

1. 固定 OpenRouter 当前公开 endpoint、鉴权 header、模型目录 endpoint、错误状态、限流语义和请求响应格式。
2. 用非破坏性/低成本方式确认 OpenRouter 是否有 Jev decisions endpoint；若只有普通 chat/completions，建立明确的协议转换设计，不直接复用 TypeSafe adapter。
3. 建立能力矩阵：Choice、Score、Noul、confidence、usage、streaming、model discovery；每个“不支持”都要能在 Provider/Playground 中解释。
4. 决定模型字段语义：OpenRouter model ID、公开入口 model ID、默认模型和别名如何区分。

## 阶段 B：实现

1. 新建 `rs/crates/jev-adapters/src/upstream_openrouter.rs`，只负责 OpenRouter DTO、鉴权、请求/响应映射和错误分类。
2. 在 `jev-adapters/src/lib.rs`、daemon provider kind 装配、配置示例和 UI provider kind 注册 `openrouter`。
3. 接入 `/v1/models` 模型发现；对不兼容的目录响应返回可读失败，不覆盖手填模型。
4. 保留 route trace 中 provider/model、实际尝试、失败转移和上游调用数；key 只由 daemon 持有。

## 阶段 C：验证

- WireMock 覆盖成功、Noul/Choice/Score、错误 body 脱敏、401/429/5xx、重试/不重试和模型目录。
- 使用明确授权的真实账号做一次受控 live request；记录模型、状态、usage、延迟和费用口径，不写 key。
- UI 通过 Providers 添加、模型发现、入口挂接和 Playground 直连/公开入口测试。
- 更新 README、HTTP 契约、能力矩阵、用户旅程和验证记录；未支持能力必须显示限制。

## 完成证据

adapter 单测、daemon 集成、UI 测试、真实受控调用和文档边界全部具名；OpenRouter 不进入 Release，直到 Windows/Docker 的完整门禁和产品范围说明同步完成。

## 不做

不在本计划里承诺 OpenAI/Anthropic 通用入口、不偷偷把 OpenAI chat API 当成 Jev 原生、不实现 streaming/计费估算，除非协议核对确认并另列范围。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
