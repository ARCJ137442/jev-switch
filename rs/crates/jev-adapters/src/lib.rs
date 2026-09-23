//! Jev-Switch 上游适配（P0-1 迁入 jev-adapters · A5：DTO + 方言归位）。
//!
//! 分层约束（contracts/02 §3）：厂商 DTO、HTTP、方言翻译只活在本 crate；
//! 禁止修改 core 签名、禁止往 `JevResponse` 正式字段塞专有字段（只进 `extra`）。
//!
//! - [`vercel_dto`]：Vercel 厂商 DTO（A5 从 jev-protocol 迁入）
//! - [`vercel_protocol`]：Vercel 方言 `ProtocolAdapter`（原 core translate 重组）
//! - [`upstream_vercel`] / [`upstream_laya`]：HTTP 上游（`UpstreamAdapter` 实现，
//!   capability 注册制 —— 能力表在各家 adapter 内，不再查 `capabilities_of`）

pub mod upstream_laya;
pub mod upstream_vercel;
pub mod vercel_dto;
pub mod vercel_protocol;
