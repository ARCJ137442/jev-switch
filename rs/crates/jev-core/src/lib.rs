//! Jev-Switch 内核（P0-1 迁入 jev-core）。
//!
//! 分层约束（contracts/02 §3）：
//! - 允许：trait、Router、RetryPolicy、Capabilities、translate
//! - 禁止：axum、reqwest、`match "vercel"` 式硬编码 URL（translate 现状为
//!   P0 历史遗留，A5 将拆为 ProtocolAdapter）

pub mod router;
pub mod translate;
pub mod upstream;
