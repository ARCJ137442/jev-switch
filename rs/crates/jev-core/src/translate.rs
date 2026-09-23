//! Vercel `noul` ↔ `boolean` 翻译层（M0.7 · P0-3 同步 contracts/01）
//!
//! 参考 03-上游类别与协议兼容矩阵 §2.4 + contracts/01 §4/§6：
//! - 入站（`normalize_request_for_vercel`）：Jev `type: noul` → Vercel `type: boolean`
//!   （仅改写 type 键，criteria/instructions 保持）。
//! - 出站（`build_jev_response_from_vercel`）是**唯一**出口翻译路径：
//!   - 概率兜底链（§4 冻结）：`probability > noul > probabilities["true"] >
//!     boolean→{0.95,0.05}` —— 仅作最后回退；双缺 → 键缺省（`noul_probability`
//!     返回 0.0 = 未验到）。
//!   - 布尔族归一为 `Answer::Noul`（kind=Noul，§1 语义层只有 noul；写出键 `"noul"`），
//!     `probability` 原样保留（双键并存，§6 禁止吞掉）。
//!   - choice / score：三字段必填（缺 → Err，由上游映射 502，禁 best-effort）。
//!   - 未知 type → Err（§6 禁止平移）。
//!
//! ⚠ 历史差异（本 commit 修复/变更，见 commit body）：
//! - 旧实现把「归一后的请求」误当 original 传入 → 生产路径恒走 boolean 形态分支；
//!   契约下语义层统一 noul，不再形态跟随。
//! - 旧 DecisionAnswer 扁平全 Option + 硬推 confidence → 契约判别联合，布尔族无 confidence。

use jev_protocol::{
    Answer, JevRequest, JevResponse, NoulAnswer, NoulKind, VercelResponse,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// 把 Jev 请求翻译成 Vercel 形态 JSON（出站方言化）。
///
/// - `type: "noul"` → `type: "boolean"`（仅布尔族；choice/score 不碰）
/// - 其余键原样保留；返回完整请求 Value（daemon body 取 `state`/`questions`，
///   `model` 走 `ai-model-id` header）
pub fn normalize_request_for_vercel(req: &JevRequest) -> Result<Value, serde_json::Error> {
    let mut v = serde_json::to_value(req)?;
    if let Some(questions) = v.get_mut("questions").and_then(Value::as_object_mut) {
        for q in questions.values_mut() {
            if q.get("type").and_then(Value::as_str) == Some("noul") {
                q["type"] = Value::String("boolean".into());
            }
        }
    }
    Ok(v)
}

/// 从 Vercel raw 形态构造 Jev `JevResponse`（唯一出口翻译路径）。
///
/// 返回 `Err(String)` 由调用方映射 `JevError::BadResponse`（→ HTTP 502）。
pub fn build_jev_response_from_vercel(raw: &VercelResponse) -> Result<JevResponse, String> {
    let mut answers: BTreeMap<String, Answer> = BTreeMap::new();
    if let Some(raw_answers) = &raw.answers {
        for (qid, va) in raw_answers {
            let ans = match va.r#type.as_deref() {
                Some("boolean") | Some("noul") => {
                    // §4 兜底链：probability > noul > probabilities["true"] > boolean 档位
                    let final_prob: Option<f64> = va
                        .probability
                        .or(va.noul)
                        .or_else(|| va.probabilities.as_ref().and_then(|p| p.get("true").copied()))
                        .or_else(|| va.boolean.map(|b| if b { 0.95 } else { 0.05 }))
                        .map(|p| p.clamp(0.0, 1.0));
                    Answer::Noul(NoulAnswer {
                        // 语义层归一为 noul（§1；adapter 需要 boolean 方言时在出站转换）
                        kind: NoulKind::Noul,
                        noul: final_prob,
                        probability: va.probability,
                    })
                }
                Some("choice") => Answer::Choice {
                    choice: va.choice.clone().ok_or_else(|| {
                        format!("answer '{qid}': missing field `choice`")
                    })?,
                    probabilities: va.probabilities.clone().ok_or_else(|| {
                        format!("answer '{qid}': missing field `probabilities`")
                    })?,
                    confidence: va.confidence.ok_or_else(|| {
                        format!("answer '{qid}': missing field `confidence`")
                    })?,
                },
                Some("score") => Answer::Score {
                    score: va
                        .score
                        .ok_or_else(|| format!("answer '{qid}': missing field `score`"))?,
                    probabilities: va.probabilities.clone().ok_or_else(|| {
                        format!("answer '{qid}': missing field `probabilities`")
                    })?,
                    confidence: va.confidence.ok_or_else(|| {
                        format!("answer '{qid}': missing field `confidence`")
                    })?,
                },
                Some(other) => {
                    return Err(format!(
                        "answer '{qid}': unknown variant `{other}`, expected one of `choice`, `score`, `noul`"
                    ));
                }
                None => {
                    return Err(format!("answer '{qid}': missing field `type`"));
                }
            };
            answers.insert(qid.clone(), ans);
        }
    }

    // usage：强类型（§5）；形态非法 → Err（502），禁回落 Value
    let usage = raw
        .usage
        .as_ref()
        .map(|u| {
            serde_json::from_value::<jev_protocol::Usage>(u.clone())
                .map_err(|e| format!("usage: {e}"))
        })
        .transpose()?;

    // providerMetadata + 顶层余量 → flatten extra
    let mut extra = raw.extra.clone();
    if let Some(pm) = &raw.provider_metadata {
        extra.entry("providerMetadata".to_string())
            .or_insert_with(|| pm.clone());
    }

    Ok(JevResponse {
        model: raw.model.clone(),
        answers,
        usage,
        // 单次上游实发（工具循环计量 A4+ 接线；此处如实 = 1）
        upstream_calls: Some(1),
        latency_ms: None,
        cost_usd: None, // 未知 → null（§5）
        extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::{noul_probability, Question, VercelAnswer};
    use std::collections::BTreeMap;

    fn make_noul_request() -> JevRequest {
        let mut q = BTreeMap::new();
        q.insert(
            "q".to_string(),
            Question::Noul {
                instructions: "is greeting?".into(),
                criteria: jev_protocol::Criteria::Bool {
                    r#true: "yes".into(),
                    r#false: "no".into(),
                },
            },
        );
        JevRequest {
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

    fn noul_of<'a>(resp: &'a JevResponse, qid: &str) -> &'a NoulAnswer {
        match resp.answers.get(qid).expect("answer present") {
            Answer::Noul(n) => n,
            other => panic!("expected Noul, got {other:?}"),
        }
    }

    #[test]
    fn noul_to_boolean_wire() {
        let req = make_noul_request();
        let v = normalize_request_for_vercel(&req).unwrap();
        assert_eq!(v["questions"]["q"]["type"], "boolean");
        // 其余键保持
        assert_eq!(v["questions"]["q"]["instructions"], "is greeting?");
        assert_eq!(v["model"], "typesafe-ai/jev");
        // 原请求未被改动（纯函数）
        assert_eq!(req.questions["q"].question_type_str(), "noul");
    }

    #[test]
    fn choice_stays_choice_wire() {
        let mut q = BTreeMap::new();
        q.insert(
            "c".to_string(),
            Question::Choice {
                instructions: "pick".into(),
                criteria: jev_protocol::Criteria::Map(BTreeMap::from([
                    ("A".into(), "alpha".into()),
                    ("B".into(), "beta".into()),
                ])),
            },
        );
        let req = JevRequest {
            model: "m".into(),
            state: serde_json::Value::Null,
            questions: q,
        };
        let v = normalize_request_for_vercel(&req).unwrap();
        assert_eq!(v["questions"]["c"]["type"], "choice");
    }

    #[test]
    fn vercel_boolean_true_to_noul() {
        // 生产路径：build_jev_response_from_vercel
        let resp = build_jev_response_from_vercel(&make_vercel_response(
            "q",
            boolean_answer(true),
        ))
        .unwrap();
        let n = noul_of(&resp, "q");
        assert_eq!(n.kind, NoulKind::Noul);
        assert_eq!(n.noul, Some(0.95));
        assert_eq!(n.probability, None);
        // 布尔族无 confidence 字段（§4 不变量）——写出不带
        let wire = serde_json::to_value(&resp.answers["q"]).unwrap();
        assert!(wire.get("confidence").is_none());
        assert_eq!(wire["type"], "noul");
        assert_eq!(noul_probability(n), 0.95);
    }

    #[test]
    fn vercel_boolean_false_to_noul() {
        let resp = build_jev_response_from_vercel(&make_vercel_response(
            "q",
            boolean_answer(false),
        ))
        .unwrap();
        let n = noul_of(&resp, "q");
        assert_eq!(n.noul, Some(0.05));
        assert_eq!(noul_probability(n), 0.05);
    }

    #[test]
    fn vercel_probability_field_honored() {
        // 回归（0.73 用例保持绿）：Vercel 单值 probability 优先于 0.95/0.05 档位
        let va = VercelAnswer {
            probability: Some(0.73),
            ..boolean_answer(true)
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("q", va)).unwrap();
        let n = noul_of(&resp, "q");
        assert_eq!(n.probability, Some(0.73));
        assert_eq!(n.noul, Some(0.73));
        assert_eq!(noul_probability(n), 0.73);
    }

    #[test]
    fn vercel_probability_069_full_path() {
        // P0-2 / §8 #1 全链路：{"type":"boolean","probability":0.69} → noul ≈ 0.69（不是 0.95）
        let va = VercelAnswer {
            r#type: Some("boolean".into()),
            probability: Some(0.69),
            boolean: Some(true), // 档位 0.95 不得覆盖 probability
            ..Default::default()
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("q", va)).unwrap();
        let n = noul_of(&resp, "q");
        assert_eq!(n.probability, Some(0.69));
        assert!((noul_probability(n) - 0.69).abs() < 1e-12);
        assert!((n.noul.unwrap() - 0.69).abs() < 1e-12);
    }

    #[test]
    fn boolean_input_answers_normalize_to_noul() {
        // §1：boolean 输入在反序列化时归一为 Noul；出口统一回 noul 形态
        // （旧「形态跟随 original」依赖已随契约移除 —— 原测试
        //  `boolean_origin_stays_boolean` 按 contracts/01 改写于此）
        let json = r#"{
            "model": "typesafe-ai/jev",
            "state": null,
            "questions": {"b": {"type": "boolean", "instructions": "x", "criteria": {"true":"y","false":"n"}}}
        }"#;
        let req: JevRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.questions["b"].question_type_str(), "noul");

        let resp =
            build_jev_response_from_vercel(&make_vercel_response("b", boolean_answer(true)))
                .unwrap();
        let n = noul_of(&resp, "b");
        assert_eq!(n.kind, NoulKind::Noul);
        assert_eq!(n.noul, Some(0.95));
        let wire = serde_json::to_value(&resp.answers["b"]).unwrap();
        assert_eq!(wire["type"], "noul");
    }

    #[test]
    fn non_noul_question_passthrough() {
        // 原请求是 choice，vercel 响应是 choice —— 三字段必填直通
        let mut q = BTreeMap::new();
        q.insert(
            "c".to_string(),
            Question::Choice {
                instructions: "pick".into(),
                criteria: jev_protocol::Criteria::Map(BTreeMap::from([(
                    "A".into(),
                    "alpha".into(),
                )])),
            },
        );
        let req = JevRequest {
            model: "m".into(),
            state: serde_json::Value::Null,
            questions: q,
        };
        assert_eq!(req.questions["c"].question_type_str(), "choice");

        let va = VercelAnswer {
            r#type: Some("choice".into()),
            choice: Some("A".into()),
            probabilities: Some(BTreeMap::from([("A".to_string(), 0.7)])),
            confidence: Some(0.7),
            ..Default::default()
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("c", va)).unwrap();
        match &resp.answers["c"] {
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => {
                assert_eq!(choice, "A");
                assert_eq!(probabilities.get("A"), Some(&0.7));
                assert_eq!(*confidence, 0.7);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_answer_type_rejected() {
        // §6：未知 type → Err（禁 best-effort 平移）→ 上游映射 502
        let va = VercelAnswer {
            r#type: Some("mystery".into()),
            ..Default::default()
        };
        let err =
            build_jev_response_from_vercel(&make_vercel_response("q", va)).unwrap_err();
        assert!(err.contains("unknown variant `mystery`"), "unexpected: {err}");
    }

    #[test]
    fn choice_missing_required_fields_rejected() {
        // 三字段必填：缺 confidence → Err（502），不静默产出残缺 Answer
        let va = VercelAnswer {
            r#type: Some("choice".into()),
            choice: Some("A".into()),
            probabilities: Some(BTreeMap::from([("A".to_string(), 1.0)])),
            confidence: None,
            ..Default::default()
        };
        let err =
            build_jev_response_from_vercel(&make_vercel_response("c", va)).unwrap_err();
        assert!(err.contains("missing field `confidence`"), "unexpected: {err}");
    }

    #[test]
    fn all_signals_absent_means_unverified() {
        // §4：双缺 = 未验到 —— 不再从 boolean.unwrap_or(false) 伪造 0.05
        let va = VercelAnswer {
            r#type: Some("boolean".into()),
            boolean: None,
            ..Default::default()
        };
        let resp = build_jev_response_from_vercel(&make_vercel_response("q", va)).unwrap();
        let n = noul_of(&resp, "q");
        assert!(n.noul.is_none() && n.probability.is_none());
        assert_eq!(noul_probability(n), 0.0); // 0.0 = 未验到（调用方以双键 None 区分）
    }

    #[test]
    fn provider_metadata_and_extra_flow_through() {
        // §5 flatten：providerMetadata + 顶层余量进 extra
        let mut raw = make_vercel_response("q", boolean_answer(true));
        raw.provider_metadata = Some(serde_json::json!({"gw": "vercel"}));
        raw.extra
            .insert("requestId".into(), serde_json::json!("r-1"));
        let resp = build_jev_response_from_vercel(&raw).unwrap();
        assert_eq!(resp.extra.get("providerMetadata"), Some(&serde_json::json!({"gw": "vercel"})));
        assert_eq!(resp.extra.get("requestId"), Some(&serde_json::json!("r-1")));
        // 计量字段按契约
        assert_eq!(resp.upstream_calls, Some(1));
        assert_eq!(resp.cost_usd, None); // null ≠ 0
    }

    #[test]
    fn upstream_calls_count_built_responses() {
        // build 路径单发上游 = 1（A4 failover 接线前的如实基线）
        let resp =
            build_jev_response_from_vercel(&make_vercel_response("q", boolean_answer(true)))
                .unwrap();
        assert_eq!(resp.upstream_calls, Some(1));
    }
}
