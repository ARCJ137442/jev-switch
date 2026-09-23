//! Jev-Switch 上游适配（P0-1 迁入 jev-adapters）。
//!
//! 分层约束（contracts/02 §3）：厂商 DTO、HTTP、方言翻译只活在本 crate；
//! 禁止修改 core 签名、禁止往 `SystemOneResponse` 正式字段塞专有字段。

pub mod upstream_laya;
pub mod upstream_vercel;
