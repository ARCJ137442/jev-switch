# Changelog

## [0.9.0] - 2026-10-05

### Added

- 支持在统一 `POST /v1/systemone` 请求中携带显式 `extensions` 或兼容服务使用的未知顶层字段。
- local 模式默认保留扩展字段；`strip_unknown_fields` 可全局启用边界剥离。
- TypeSafe-compatible provider 增加 `forward_extensions` 选项，允许 OneJev 等本地/网关服务接收 `media`、`images` 等多模态字段。
- 响应与失败 route trace 记录扩展字段名及 gateway/provider disposition，不记录媒体内容或 Base64 数据。

### Compatibility

- 官方 TypeSafe provider 默认不会收到未知扩展字段，保持当前文本/JSON API 边界。
- 真实 OneJev/LLM2Jev/OpenJev 多模态推理、质量与校准仍需按独立实验报告验收。

### Verification

- Rust workspace 默认测试与 `ts-rs` 生成门禁通过。
- UI tests、TypeScript lint 通过；生产构建和远端 GitHub Actions 以 tag workflow 结果为准。
- 详细范围见 [`v0.9.0` 发布核验](docs/verification/v0.9.0-release-candidate-2026-10-05.md) 与[多模态兼容核验](docs/verification/multimodal-systemone-compatibility-2026-10-05.md)。
