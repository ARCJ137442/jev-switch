//! Vercel 网关厂商 DTO（A5 · 从 jev-protocol 迁入 —— contracts/02 §3：
//! **厂商 DTO 只活在 adapter crate**，内核 `extra` flatten 不动）。
//!
//! 参考 03- §2.4 + jev-decision-lab TS：Vercel 用 `probability`（不是 `noul`）
//! 字段表达 true 的概率。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Vercel 网关响应里的单条 answer 原始形态。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VercelAnswer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    /// 少数经 Vercel 形态的网关也给 `noul` 键（§4 兜底链含它）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noul: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    /// score 题分值（契约 `Score.score: f64`；缺失则无法构成 Answer，报 502）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Vercel 网关响应外壳（关心字段 + 顶层余量透传进 `JevResponse.extra`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VercelResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<BTreeMap<String, VercelAnswer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(
        default,
        alias = "providerMetadata",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_metadata: Option<serde_json::Value>,
    /// 顶层未知键余量 → 翻译时并入 `JevResponse.extra`（契约 flatten 语义）。
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vercel_dto_parses_probability_and_extra() {
        let raw = r#"{
            "answers": {"q": {"type": "boolean", "probability": 0.69, "boolean": true}},
            "providerMetadata": {"gw": "vercel"},
            "requestId": "r-1"
        }"#;
        let v: VercelResponse = serde_json::from_str(raw).unwrap();
        let a = v.answers.as_ref().unwrap().get("q").unwrap();
        assert_eq!(a.probability, Some(0.69));
        assert_eq!(a.boolean, Some(true));
        assert!(v.provider_metadata.is_some());
        assert_eq!(v.extra.get("requestId"), Some(&serde_json::json!("r-1")));
    }
}
