//! Vercel `noul` ↔ `boolean` 翻译层（M0.7）
//!
//! 参考 03-上游类别与协议兼容矩阵 §2.4：
//! - 入站：Jev `type: noul` → Vercel `type: boolean`（仅重命名 type，criteria 保持）。
//! - 出站：Vercel `boolean: true/false`（**Vercel 实际还返回 `probability`，** 用它）→ Jev `noul`。
//! - Vercel 不报 `confidence`：从 `probabilities` 推断（`max(probs)`）。
//!
//! 为什么单独成文件：router 调用 `vercel_normalize_request` 后再交给
//! `VercelUpstream::evaluate`，调用方视角的 request 完全保持 Jev 标准
//! `type: noul`，协议差异对调用方不可见。

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

/// 把 Vercel 响应翻译回 Jev 形态。
///
/// 入口条件：上游是 Vercel，发出的 request 已经是 `normalize_request_for_vercel` 的结果。
/// 这里需要原始请求以知道哪些 qid 原本是 `noul`（要翻译回 `noul`）。
///
/// Vercel boolean 的概率信息：
/// - 实测 jev-decision-lab TS 看到 `probability` 字段（[0, 1]，true 的概率）
/// - 一些早期 Vercel 版本只给 `boolean: true/false`（没有概率）→ fallback 到 0.95 / 0.05
#[allow(dead_code)]
pub fn denormalize_response_from_vercel(
    resp: SystemOneResponse,
    original_request: &SystemOneRequest,
) -> SystemOneResponse {
    let mut out = resp;
    for (qid, ans) in out.answers.iter_mut() {
        let was_noul = matches!(
            original_request.questions.get(qid),
            Some(DecisionQuestion::Noul { .. })
        );
        if !was_noul {
            continue;
        }
        // 从 Vercel 的 boolean/probability 派生 Jev 的 noul。
        // 优先级：probability > boolean（probability 含更多信息）。
        let probability: f64 = if let Some(p) = ans.boolean {
            if p { 0.95 } else { 0.05 }
        } else {
            // 既无 boolean 也无 probability：保持原样（best-effort）
            continue;
        };
        // 同时兼容 Vercel 在 boolean 答案里附的 probability 字段
        // （用 probabilities["true"] 优先）
        let noul_value = ans
            .probabilities
            .as_ref()
            .and_then(|p| p.get("true").copied())
            .unwrap_or(probability)
            .clamp(0.0, 1.0);

        ans.r#type = Some("noul".to_string());
        ans.noul = Some(noul_value);
        // 从 noul 推断 boolean（仅在 raw boolean 缺失时补）
        if ans.boolean.is_none() {
            ans.boolean = Some(noul_value >= 0.5);
        }
        // 补 probabilities
        let probs = BTreeMap::from([
            ("true".to_string(), noul_value),
            ("false".to_string(), 1.0 - noul_value),
        ]);
        ans.probabilities = Some(probs);
        // 补 confidence（Vercel 不报，从 noul 推断）
        if ans.confidence.is_none() {
            ans.confidence = Some(noul_value.max(1.0 - noul_value));
        }
    }
    out
}

/// 从 Vercel raw 形态构造 Jev `SystemOneResponse`。
///
/// 上游 `VercelUpstream` 直接拿到 Vercel HTTP 响应 body（VercelAnswer 形态），
/// 这里统一转换为调用方能消费的 `SystemOneResponse`。
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
                    // 真 boolean 答案：上游确实只返回 boolean（不返回 probability）
                    let prob = if va.boolean.unwrap_or(false) { 0.95 } else { 0.05 };
                    ans.r#type = Some("noul".to_string());
                    ans.noul = Some(prob);
                    ans.boolean = va.boolean;
                    ans.probabilities = Some(BTreeMap::from([
                        ("true".to_string(), prob),
                        ("false".to_string(), 1.0 - prob),
                    ]));
                    ans.confidence = Some(prob.max(1.0 - prob));
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
    let mut resp = SystemOneResponse {
        model: raw.model.clone(),
        answers,
        usage: raw.usage.clone(),
    };
    // 应用最终 denormalize（处理从 boolean 派生的 noul 等）
    denormalize_response_from_vercel_inplace(&mut resp, original_request);
    resp
}

/// in-place 版本：在已有的 `SystemOneResponse` 上应用 Vercel boolean → noul 翻译。
///
/// 这是 `denormalize_response_from_vercel` 的可变借用版，因为上游可能想在构造
/// 响应后立即应用翻译（避免中间变量）。
pub fn denormalize_response_from_vercel_inplace(
    resp: &mut SystemOneResponse,
    original_request: &SystemOneRequest,
) {
    for (qid, ans) in resp.answers.iter_mut() {
        let was_noul = matches!(
            original_request.questions.get(qid),
            Some(DecisionQuestion::Noul { .. })
        );
        if !was_noul {
            continue;
        }
        let probability: f64 = if let Some(b) = ans.boolean {
            if b { 0.95 } else { 0.05 }
        } else {
            continue;
        };
        let noul_value = ans
            .probabilities
            .as_ref()
            .and_then(|p| p.get("true").copied())
            .unwrap_or(probability)
            .clamp(0.0, 1.0);

        ans.r#type = Some("noul".to_string());
        ans.noul = Some(noul_value);
        if ans.boolean.is_none() {
            ans.boolean = Some(noul_value >= 0.5);
        }
        let probs = BTreeMap::from([
            ("true".to_string(), noul_value),
            ("false".to_string(), 1.0 - noul_value),
        ]);
        ans.probabilities = Some(probs);
        if ans.confidence.is_none() {
            ans.confidence = Some(noul_value.max(1.0 - noul_value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DecisionAnswer;
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
        let original = make_original_noul_request();
        let mut answers = BTreeMap::new();
        let mut ans = DecisionAnswer::default();
        ans.r#type = Some("boolean".to_string());
        ans.boolean = Some(true);
        answers.insert("q".to_string(), ans);
        let resp = SystemOneResponse {
            model: Some("typesafe-ai/jev".into()),
            answers,
            usage: None,
        };
        let translated = denormalize_response_from_vercel(resp, &original);
        let q = translated.answers.get("q").unwrap();
        assert_eq!(q.r#type.as_deref(), Some("noul"));
        assert_eq!(q.noul, Some(0.95));
        assert_eq!(q.confidence, Some(0.95));
        assert_eq!(
            q.probabilities.as_ref().unwrap().get("true"),
            Some(&0.95)
        );
    }

    #[test]
    fn vercel_boolean_false_to_noul() {
        let original = make_original_noul_request();
        let mut answers = BTreeMap::new();
        let mut ans = DecisionAnswer::default();
        ans.r#type = Some("boolean".to_string());
        ans.boolean = Some(false);
        answers.insert("q".to_string(), ans);
        let resp = SystemOneResponse {
            model: Some("typesafe-ai/jev".into()),
            answers,
            usage: None,
        };
        let translated = denormalize_response_from_vercel(resp, &original);
        let q = translated.answers.get("q").unwrap();
        assert_eq!(q.noul, Some(0.05));
        assert_eq!(q.confidence, Some(0.95)); // max(0.05, 0.95)
    }

    #[test]
    fn vercel_probability_field_used_directly() {
        // 如果 Vercel 返回了 probability 字段，优先用它
        let original = make_original_noul_request();
        let mut answers = BTreeMap::new();
        let mut ans = DecisionAnswer::default();
        ans.r#type = Some("boolean".to_string());
        ans.boolean = Some(true);
        ans.probabilities = Some(BTreeMap::from([("true".to_string(), 0.73)]));
        answers.insert("q".to_string(), ans);
        let resp = SystemOneResponse {
            model: None,
            answers,
            usage: None,
        };
        let translated = denormalize_response_from_vercel(resp, &original);
        let q = translated.answers.get("q").unwrap();
        assert_eq!(q.noul, Some(0.73));
        assert_eq!(q.confidence, Some(0.73));
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
        let mut answers = BTreeMap::new();
        let mut ans = DecisionAnswer::default();
        ans.r#type = Some("choice".to_string());
        ans.choice = Some("A".to_string());
        ans.probabilities = Some(BTreeMap::from([("A".to_string(), 0.7)]));
        ans.confidence = Some(0.7);
        answers.insert("c".to_string(), ans);
        let resp = SystemOneResponse {
            model: None,
            answers,
            usage: None,
        };
        let translated = denormalize_response_from_vercel(resp, &original);
        let c = translated.answers.get("c").unwrap();
        assert_eq!(c.choice.as_deref(), Some("A"));
        assert_eq!(c.r#type.as_deref(), Some("choice"));
    }
}
