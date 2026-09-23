//! Jev 协议类型（M0.2 · P0-1 迁入 jev-protocol，零 IO）
//!
//! 参考 04-架构设计 §1.1：与 sys1 共享的 serde 类型。
//! 这里把整个 `SystemOneRequest` / `SystemOneResponse` / `DecisionQuestion` / `DecisionAnswer`
//! 落地为 Rust serde 类型，且支持 `noul` ↔ `boolean` 双向解析（Vercel 适配）。
//!
//! 设计要点：
//! - `DecisionQuestion` 用 `#[serde(tag = "type", rename_all = "lowercase")]`，
//!   因此四种变体序列化时 `type` 字段为 `"choice" | "score" | "noul" | "boolean"`。
//! - `DecisionAnswer` **宽容**：所有字段都可选，Jev-Switch 在边界把上游响应尽量
//!   归一为标准 Jev shape 给调用方。
//! - `criteria` 用 `serde_json::Value` 兼容 dict 和 list 两种 Jev 提交习惯。
//!
//! 分层约束（contracts/02 §3）：本 crate 零 IO —— 只允许 serde / serde_json，
//! 禁止厂商字段名、URL、header、axum、reqwest。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Jev SystemOne 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneRequest {
    pub model: String,
    #[serde(default)]
    pub state: serde_json::Value,
    #[serde(default)]
    pub questions: BTreeMap<String, DecisionQuestion>,
}

/// Jev SystemOne 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub answers: BTreeMap<String, DecisionAnswer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

/// 问题类型枚举（capability 用）。
///
/// P0-1 注：原先定义在 `upstream.rs`（core 层），拆分时随
/// `DecisionQuestion::question_type()` 一并迁入协议层（零 IO 枚举，
/// core 经 `jev_core::upstream::QuestionType` 重导出，路径不变）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionType {
    Choice,
    Score,
    Noul,
    Boolean,
}

impl QuestionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionType::Choice => "choice",
            QuestionType::Score => "score",
            QuestionType::Noul => "noul",
            QuestionType::Boolean => "boolean",
        }
    }
}

/// 单个问题（请求侧）。
///
/// 用 `#[serde(tag = "type")]` 后，JSON 形如：
/// ```json
/// { "type": "noul", "instructions": "...", "criteria": {"true": "yes", "false": "no"} }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DecisionQuestion {
    Choice {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<serde_json::Value>,
    },
    Score {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<serde_json::Value>,
    },
    Noul {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<serde_json::Value>,
    },
    /// Vercel 适配：Jev 标准 spec 在 Vercel 网关上的等价值。
    /// Jev-Switch 在出口会把 `Noul` 翻译为 `Boolean` 发到 Vercel，入口把
    /// `Boolean` 翻译回 `Noul`。
    Boolean {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<serde_json::Value>,
    },
}

impl DecisionQuestion {
    #[allow(dead_code)]
    pub fn question_type_str(&self) -> &'static str {
        match self {
            DecisionQuestion::Choice { .. } => "choice",
            DecisionQuestion::Score { .. } => "score",
            DecisionQuestion::Noul { .. } => "noul",
            DecisionQuestion::Boolean { .. } => "boolean",
        }
    }

    /// 问题类型 → capability 校验枚举（handler 转发前调 `Router::check_capability`）。
    pub fn question_type(&self) -> QuestionType {
        match self {
            DecisionQuestion::Choice { .. } => QuestionType::Choice,
            DecisionQuestion::Score { .. } => QuestionType::Score,
            DecisionQuestion::Noul { .. } => QuestionType::Noul,
            DecisionQuestion::Boolean { .. } => QuestionType::Boolean,
        }
    }

    #[allow(dead_code)]
    pub fn instructions(&self) -> &str {
        match self {
            DecisionQuestion::Choice { instructions, .. }
            | DecisionQuestion::Score { instructions, .. }
            | DecisionQuestion::Noul { instructions, .. }
            | DecisionQuestion::Boolean { instructions, .. } => instructions,
        }
    }
}

/// 单个问题的答案（响应侧）。
///
/// 字段全可选：调用方按 `type` 决定用哪个字段。Vercel 不报 `confidence`，
/// Jev-Switch 会从 `probabilities` 推断补齐。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionAnswer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noul: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Vercel 网关响应里的单条 answer 原始形态（参考 03- §2.4 + jev-decision-lab TS）。
///
/// Vercel 用 `probability`（不是 `noul`）字段表达 true 的概率。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VercelAnswer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probabilities: Option<BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Vercel 网关响应外壳（仅 MVP 关心字段）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VercelResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answers: Option<BTreeMap<String, VercelAnswer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_noul() {
        let json = r#"{
            "model": "laya-english",
            "state": "hi",
            "questions": {
                "q": {
                    "type": "noul",
                    "instructions": "is this a greeting?",
                    "criteria": {"true": "yes", "false": "no"}
                }
            }
        }"#;
        let req: SystemOneRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "laya-english");
        assert_eq!(req.questions.len(), 1);
        let q = req.questions.get("q").unwrap();
        assert_eq!(q.question_type_str(), "noul");
        assert_eq!(q.instructions(), "is this a greeting?");
        // round-trip
        let back = serde_json::to_string(&req).unwrap();
        let req2: SystemOneRequest = serde_json::from_str(&back).unwrap();
        assert_eq!(req2.questions.get("q").unwrap(), q);
    }

    #[test]
    fn deserialize_boolean_variant() {
        let json = r#"{
            "model": "typesafe-ai/jev",
            "state": null,
            "questions": {
                "q": {"type": "boolean", "instructions": "x", "criteria": {"true": "y", "false": "n"}}
            }
        }"#;
        let req: SystemOneRequest = serde_json::from_str(json).unwrap();
        let q = req.questions.get("q").unwrap();
        assert_eq!(q.question_type_str(), "boolean");
    }

    #[test]
    fn deserialize_choice_with_list_criteria() {
        // score + list criteria 是 Jev 一种合法形态
        let json = r#"{
            "model": "m",
            "state": null,
            "questions": {
                "s": {"type": "score", "instructions": "rank", "criteria": ["low","med","high"]}
            }
        }"#;
        let req: SystemOneRequest = serde_json::from_str(json).unwrap();
        let q = req.questions.get("s").unwrap();
        assert_eq!(q.question_type_str(), "score");
    }

    #[test]
    fn response_default_is_open() {
        let ans = DecisionAnswer::default();
        let json = serde_json::to_string(&ans).unwrap();
        // 空 answer 序列化应该什么都不带（skip_serializing_if = Option::is_none）
        assert_eq!(json, "{}");
    }
}
