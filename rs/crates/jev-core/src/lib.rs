//! Jev-Switch 内核（P0-1 迁入 jev-core · A4 模型路由 DAG · A5 冻结扩展点）。
//!
//! 分层约束（contracts/02 §3）：
//! - 允许：trait（`ProtocolAdapter`/`UpstreamAdapter`）、Router、Registry、
//!   RetryPolicy、Capabilities、JevError
//! - 禁止：axum、reqwest、`match "vercel"` 式硬编码 URL / 厂商 DTO
//!   （A5 已删除 `translate` 模块与 `capabilities_of` 硬编码 —— 方言与厂商 DTO
//!   全部迁往 jev-adapters）

pub mod adapter;
pub mod redact;
pub mod router;
pub mod upstream;
