# 多模态 SystemOne 兼容性与 Jev Switch 边界

日期：2026-10-05

## 结论

Jev Switch 可以承载多模态 SystemOne 请求，并且不需要另造一套 API。调用方继续使用
`POST /v1/systemone`，请求仍然以 `model`、`state`、`questions` 为标准字段；多模态或供应商自定义内容可以放入顶层未知字段（例如 `media`、`images`），也可以放入显式的 `extensions` 对象。

当前实现采用三层边界：

1. `jev-protocol` 在入口保存未知字段，不把它们伪装成 TypeSafe 官方稳定协议。
2. Jev Switch 的 `strip_unknown_fields` 配置默认为 `false`，因此 local 模式保留扩展；打开后在网关边界清除扩展。
3. TypeSafe adapter 按 provider 决定出站行为。loopback + local 模式默认转发；官方远程 TypeSafe 默认剥离。成功响应的 `jev_switch.request_extensions` 元数据记录收到的键名及 provider 处理结果，值和 Base64 内容不进入元数据。

这使本地 OneJev、LLM2Jev、OpenJev Multimodal 等服务可以共享 TypeSafe 的请求/响应外形，而官方纯文本 Jev 不会收到它不支持的媒体字段。

## 公开事实

TypeSafe 官方文档目前把 `state` 限定为字符串、JSON 对象或数组，并明确写出图片、音频和视频暂不支持：

- [官方 State 文档](https://docs.typesafe.ai/concepts/state)
- [官方文档索引](https://docs.typesafe.ai/llms.txt)

因此不能把官方 `api.typesafe.ai/v1/systemone` 宣称为已经支持多模态。`OneJev` 仓库的 README 自称是多模态 System One 模型并兼容 TypeSafe API，但当前公开材料没有证明它属于 TypeSafe 官方；本项目按第三方兼容服务处理：

- [OneJev GitHub](https://github.com/OmniJev/OneJev)
- [LLM2Jev GitHub](https://github.com/Yinsongxu/LLM2Jev)
- [LLM2Jev 多模态请求说明](https://github.com/Yinsongxu/LLM2Jev/blob/main/docs/multimodal.md)
- [OpenJev Multimodal GitHub](https://github.com/jev-skills/openjev-multimodal)

LLM2Jev 已公开 `type: "multimodal"` 的 state/instructions 形状，并支持本地文件 URI、HTTP(S) URL 和 data URL。OpenJev Multimodal 则公开了本地图片到 typed decision 的兼容 HTTP 服务。它们是可用于本机实验的候选实现，不能替代官方 Jev 校准承诺。

## 请求例子

标准文本请求保持原样：

```json
{
  "model": "jev-local",
  "state": "Inspect the attached screenshot",
  "questions": {
    "has_person": {
      "type": "noul",
      "instructions": "Is a person visible?"
    }
  }
}
```

本地多模态服务可以使用显式扩展：

```json
{
  "model": "onejev-local",
  "state": "Inspect the attached screenshot",
  "questions": {
    "next": {
      "type": "choice",
      "instructions": "What should happen next?",
      "criteria": {
        "click": "click an element",
        "scroll": "scroll the page",
        "stop": "stop"
      }
    }
  },
  "extensions": {
    "media": [
      {"type": "image", "data": "data:image/png;base64,..."}
    ]
  }
}
```

已经存在、使用顶层 `media` 或 `images` 的服务也可以直接接入；入口会把它们保存到未知字段区，再由 provider 策略决定是否发送。

## Jev Switch 配置

`rs/providers.example.toml`：

```toml
strip_unknown_fields = false

[providers.onejev-local]
kind = "typesafe"
base = "http://127.0.0.1:8000/v1/systemone"
models = ["onejev-local"]
forward_extensions = true
enabled = true
```

官方远程 TypeSafe 保持：

```toml
[providers.typesafe]
kind = "typesafe"
base = "https://api.typesafe.ai/v1/systemone"
forward_extensions = false
enabled = false
```

如果临时需要全局禁止未知字段出站，把根配置改为 `strip_unknown_fields = true`。处理结果在成功响应的 `jev_switch.request_extensions` 中可见；这项元数据只包含字段名和 `preserved`、`forwarded`、`stripped` 等状态。

## 真实边界与风险

- `media`/`images` 的具体 schema 不是 TypeSafe 官方协议，必须按目标本地服务的文档配置。
- 图片 data URL 会显著增加请求体和视觉编码耗时；Jev Switch 当前只负责转发和追踪，不替供应商做缩放、去重或抽帧。
- 公开项目的概率通常是候选条件分布，不自动等于 TypeSafe 的校准正确率；多模态准确率、ECE、Brier、图像模糊/遮挡风险需要单独测量。
- `strip_unknown_fields = false` 只表示网关保留输入。远程 provider 仍可按自身 adapter 策略剥离，且会在响应元数据中留下证据。
- 音频和视频应先转换为文本或抽样图片；本轮没有把二进制上传、远程 URL 拉取和文件读取权限扩展进 Jev Switch。

## 首个实验

1. 在本机以 loopback 启动一个不依赖 CUDA Python 的多模态 SystemOne-compatible 服务，优先使用 OpenJev Multimodal 或 LLM2Jev 的 llama.cpp 路径。
2. 注册为 `kind = "typesafe"` 的 loopback provider，配置 `forward_extensions = true`，下游只访问 Jev Switch `11435/v1/systemone`。
3. 用一张本地合成截图跑 Choice/Noul/Score 三个问题，核对请求扩展、响应 typed answers、`jev_switch.request_extensions`、route trace、延迟和资源占用。
4. 使用同一请求把 provider 切换到官方 TypeSafe，确认请求仍然成功且 `media` 被记录为 provider-side `stripped`，不把图片内容发送给官方接口。
5. 只有协议 smoke 通过后，才把图像生活记录或 GUI 场景加入独立多模态 benchmark；不把它们混入现有纯文本四集的分数。

## 当前状态

- 已实现：请求未知字段保存、显式扩展区、local loopback 默认转发、全局剥离开关、出站处理元数据。
- 已验证：Rust protocol、adapter、daemon 单元/集成测试；官方 TypeSafe 默认不会收到扩展；兼容 provider 可以收到媒体字段。
- 待验证：真实 OneJev/LLM2Jev/OpenJev 服务在本机运行、图片 smoke、Jev Switch 完整 route trace、资源与校准结果。
