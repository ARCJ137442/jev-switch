//! Vercel `noul` ↔ `boolean` 翻译层（M0.7）
//!
//! 参考 03-上游类别与协议兼容矩阵 §2.4：
//! - 入站：Jev `type: noul` → Vercel `type: boolean`（仅重命名 type，criteria 保持）。
//! - 出站：`build_jev_response_from_vercel` 是**唯一**出口翻译路径：
//!   - 概率取值优先级：`probability`（Vercel 单值）> `probabilities["true"]`
//!     > boolean 档位估计（0.95 / 0.05）。
//!   - 形态跟随原始请求：原本 `noul` → 翻译回 `noul`；原本 `boolean` → 保持 `boolean`
//!     （修复：早期实现无条件把 boolean 改写成 noul，boolean 调用方会拿到错形态）。
//! - Vercel 不报 `confidence`：从概率推断 `max(p, 1-p)`（上游有报则透传）。
//!
//! 为什么单独成文件：`VercelUpstream::evaluate` 入站调 `normalize_request_for_vercel`、
//! 出站调 `build_jev_response_from_vercel`，调用方视角的 request/response 始终保持
//! Jev 标准形态，协议差异对调用方不可见。

use crate::protocol::{DecisionAnswer, DecisionQuestion, SystemOneRequest, SystemOneResponse};
use std::collections::BTreeMap;

/// 把 Jev 请求翻译成 Vercel 形态。
///
/// - `Noul { instructions, criteria }` → `Boolean { instructions, criteria }`
/// - 其他 variant 保持不变
pub fn normalize_request_for_vercel(req: SystemOneRequest) -> SystemOneRequest {
    let mut out = req;
    for q in out.questions.values_mut() {
        if matches!(q, DecisionQuestion::Noul { .. }) {
            let (instructions, criteria) = match q {
                DecisionQuestion::Noul { instructions, criteria } => {
                    (instructions.clone(), criteria.clone())
                }
                _ => unreachable!("guarded by matches! above"),
            };
            *q = DecisionQuestion::Boolean { instructions, criteria };
        }
    }
    out
}

/// 从 Vercel raw 形态构造 Jev `SystemOneResponse`（唯一出口翻译路径）。
///
/// 上游 `VercelUpstream` 直接拿到 Vercel HTTP 响应 body（`VercelAnswer` 形态），
/// 这里统一转换为调用方能消费的 `SystemOneResponse`：
/// - boolean 答案：按 `original_request` 判断原本是 `noul` 还是 `boolean`，
///   分别翻译成对应形态；概率按 `probability > probabilities["true"] > 0.95/0.05`
///   优先级取值（修复：早期实现丢弃 Vercel 的 `probability` 字段，恒用假值 0.95/0.05）。
/// - choice / score / 未知 type：best-effort 字段平移。
pub fn build_jev_response_from_vercel(
    raw: &crate::protocol::VercelResponse,
    original_request: &SystemOneRequest,
) -> SystemOneResponse {
    let mut answers: BTreeMap<String, DecisionAnswer> = BTreeMap::new();
    if let Some(raw_answers) = &raw.answers {
        for (qid, va) in raw_answers {
            let mut ans = DecisionAnswer::default();
            match va.r#type.as_deref() {
                Some("boolean") => {
                    // 概率取值优先级：单值 probability > probabilities["true"] > 档位估计
                    let prob = va
                        .probability
                        .or_else(|| {
                            va.probabilities.as_ref().and_then(|p| p.get("true").copied())
                        })
                        .unwrap_or(if va.boolean.unwrap_or(false) {
                            0.95
                        } else {
                            0.05
                        })
                        .clamp(0.0, 1.0);
                    let probs = BTreeMap::from([
                        ("true".to_string(), prob),
                        ("false".to_string(), 1.0 - prob),
                    ]);
                    // 形态跟随原始请求：noul 出口翻译成 noul，boolean 出口保持 boolean
                    let was_noul = matches!(
                        original_request.questions.get(qid),
                        Some(DecisionQuestion::Noul { .. })
                    );
                    if was_noul {
                        ans.r#type = Some("noul".to_string());
                        ans.noul = Some(prob);
                        ans.boolean = va.boolean;
                    } else {
                        ans.r#type = Some("boolean".to_string());
                        ans.boolean = va.boolean;
                    }
                    ans.probabilities = Some(probs);
                    ans.confidence = va
                        .confidence
                        .or(Some(prob.max(1.0 - prob)));
                }
                Some("choice") => {
                    ans.r#type = Some("choice".to_string());
                    ans.choice = va.choice.clone();
                    ans.probabilities = va.probabilities.clone();
                    ans.confidence = va.confidence;
                }
                Some("score") => {
                    ans.r#type = Some("score".to_string());
                    // Vercel score 字段结构与 Jev 同，但保险起见原样传
                    ans.probabilities = va.probabilities.clone();
                    ans.confidence = va.confidence;
                }
                _ => {
                    // 未知 type：best-effort，把所有字段平移过去
                    ans.r#type = va.r#type.clone();
                    ans.choice = va.choice.clone();
                    ans.probabilities = va.probabilities.clone();
                    ans.confidence = va.confidence;
                }
            }
            answers.insert(qid.clone(), ans);
        }
    }
    SystemOneResponse {
        model: raw.model.clone(),
        answers,
        usage: raw.usage.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{VercelAnswer, VercelResponse};
    use std::collections::BTreeMap;

    fn make_original_noul_request() -> SystemOneRequest {
        let mut q = BTreeMap::new();
        q.insert(
            "q".to_string(),
            DecisionQuestion::Noul {
                instructions: "is greeting?".into(),
                criteria: Some(serde_json::json!({"true": "yes", "false": "no"})),
            },
        );
        SystemOneRequest {
            model: "typesafe-ai/jev".into(),
            state: serde_json::json!("hi"),
            questions: q,
        }
    }

    fn make_vercel_response(qid: &str, va: VercelAnswer) -> VercelResponse {
        let mut answers = BTreeMap::new();
        answers.insert(qid.to_string(), va);
        VercelResponse {
            answers: Some(answers),
            model: Some("typesafe-ai/jev".into()),
            ..Default::default()
        }
    }

    fn boolean_answer(boolean: bool) -> VercelAnswer {
        VercelAnswer {
            r#type: Some("boolean".into()),
            boolean: Some(boolean),
            ..Default::default()
        }
    }

    #[test]
    fn noul_to_boolean() {
        let req = make_original_noul_request();
        let normalized = normalize_request_for_vercel(req.clone());
        let q = normalized.questions.get("q").unwrap();
        assert_eq!(q.question_type_str(), "boolean");
        // 其他字段保持
        assert_eq!(q.instructions(), "is greeting?");
    }

    #[test]
    fn choice_stays_choice() {
        let mut q = BTreeMap::new();
        q.insert(
            "c".to_string(),
            DecisionQuestion::Choice {
                instructions: "pick".into(),
                criteria: Some(serde_json::json!({"A": "alpha", "B": "beta"})),
            },
        );
        let req = SystemOneRequest {
            model: "m".into(),
            state: serde_json::Value::Null,
            questions: q,
        };
        let normalized = normalize_request_for_vercel(req);
        assert_eq!(
            normalized.questions.get("c").unwrap().question_type_str(),
            "choice"
        );
    }

    #[test]
    fn vercel_boolean_true_to_noul() {
        // 生产路径：build_jev_response_from_vercel（不再测死代码孪生函数）
        let original = make_original_noul_request();
        let resp =
            build_jev_response_from_vercel(&make_vercel_response("q", boolean_answer(true)), &original);
        let q = resp.answers.get("q").unwrap();
        assert_eq!(q.r#type.as_deref(), Some("noul"));
        assert_eq!(q.noul, Some(0.95));
        assert_eq!(q.boolean, Some(true));
        assert_eq!(q.confidence, Some(0.95));
        assert_eq!(
            q.probabilities.as_ref().unwrap().get("true"),
            Some(&0.95)
        );
    }

    #[test]
    fn vercel_boolean_false_to_noul() {
        let original = make_original_noul_request();
        let resp = build_jev_response_from_vercel(
            &make_vercel_response("q", boolean_answer(false)),
            &original,
        );
        let q = resp.answers.get("q").unwrap();
        assert_eq!(q.r#type.as_deref(), Some("noul"));
        assert_eq!(q.noul, Some(0.05));
        assert_eq!(q.confidence, Some(0.95)); // max(0.05, 0.95)
    }

    #[test]
    fn vercel_probability_field_honored() {
        // 修复回归测试：Vercel 单值 probability 字段必须优先于 0.95/0.05 档位估计
        let original = make_original_noul_request();
        let va = VercelAnswer {
            probability: Some(0.73),
            ..boolean_answer(true)
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("q", va), &original);
        let q = resp.answers.get("q").unwrap();
        assert_eq!(q.noul, Some(0.73));
        assert_eq!(q.confidence, Some(0.73));
        assert_eq!(
            q.probabilities.as_ref().unwrap().get("true"),
            Some(&0.73)
        );
    }

    #[test]
    fn boolean_origin_stays_boolean() {
        // 修复回归测试：调用方原本就发 boolean，出口不得改写成 noul
        let mut q = BTreeMap::new();
        q.insert(
            "b".to_string(),
            DecisionQuestion::Boolean {
                instructions: "x".into(),
                criteria: Some(serde_json::json!({"true": "y", "false": "n"})),
            },
        );
        let original = SystemOneRequest {
            model: "typesafe-ai/jev".into(),
            state: serde_json::Value::Null,
            questions: q,
        };
        let resp =
            build_jev_response_from_vercel(&make_vercel_response("b", boolean_answer(true)), &original);
        let a = resp.answers.get("b").unwrap();
        assert_eq!(a.r#type.as_deref(), Some("boolean"));
        assert_eq!(a.boolean, Some(true));
        assert!(a.noul.is_none());
        assert_eq!(
            a.probabilities.as_ref().unwrap().get("true"),
            Some(&0.95)
        );
    }

    #[test]
    fn non_noul_question_passthrough() {
        // 原请求是 choice，vercel 响应是 choice，不应该被翻译
        let mut q = BTreeMap::new();
        q.insert(
            "c".to_string(),
            DecisionQuestion::Choice {
                instructions: "pick".into(),
                criteria: Some(serde_json::json!({"A": "alpha", "B": "beta"})),
            },
        );
        let original = SystemOneRequest {
            model: "m".into(),
            state: serde_json::Value::Null,
            questions: q,
        };
        let va = VercelAnswer {
            r#type: Some("choice".into()),
            choice: Some("A".into()),
            probabilities: Some(BTreeMap::from([("A".to_string(), 0.7)])),
            confidence: Some(0.7),
            ..Default::default()
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("c", va), &original);
        let c = resp.answers.get("c").unwrap();
        assert_eq!(c.choice.as_deref(), Some("A"));
        assert_eq!(c.r#type.as_deref(), Some("choice"));
    }
}
