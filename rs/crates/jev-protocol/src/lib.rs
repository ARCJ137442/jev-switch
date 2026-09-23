//! Jev 协议内核类型（P0-3 按 `docs/contracts/01-协议契约.md` 对齐）
//!
//! 真值基准：TypeSafe `/v1/systemone` + `jev-life/src/shared/types.ts`。
//! 分层铁律（contracts/01 红线）：厂商方言**不得**进入本 crate —— 方言只活在
//! adapter DTO；本 crate 零 IO，仅 serde / serde_json。
//!
//! 契约要点：
//! - 对外 `Question.type ∈ {choice, score, noul}`；`boolean` 输入按 noul 语义归一（§1）
//! - `criteria` 必填 + 按题型校验；缺失/错形态 → 本地 400（文案对齐上游，§6）
//! - `Answer` 判别联合；布尔族**无 confidence**；choice/score 三字段必填（§4）
//! - `NoulAnswer` 双键 `noul` 与 `probability` 并存保留（§4/§6）
//! - 概率读取顺序冻结：`probability > noul >（适配层兜底）` → [`noul_probability`]
//! - `Usage` 强类型，双拼写宽容读入、写出 snake_case（§5）
//! - `upstream_calls` 读缺省按 1；`cost_usd` null ≠ 0（未知就是 null）（§5）
//! - 未知 `type` → 拒绝（禁 best-effort 平移）（§6）
//!
//! 过渡命名：[`JevRequest`]/[`JevResponse`] 为契约主名；
//! [`SystemOneRequest`]/[`SystemOneResponse`] 保留为别名（deprecated 注释级）。
//!
//! 分层铁律（A5 已执行）：厂商方言 DTO 不在本 crate —— 只活在 jev-adapters。

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/* ══════════════════════════════════════════════════════════════════
   判别值
   ══════════════════════════════════════════════════════════════════ */

/// 问题类型枚举（capability / `/v1/models` 广告用）。
///
/// 布尔族在语义层是 `Noul`；`Boolean` 保留作 Vercel 方言的 capability 广告值
/// （contracts/05 `/v1/models` 示例含 `"boolean"`），不出现在合法输入里。
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

/// 布尔族答案的 wire 判别值：`"noul" | "boolean"`（wire 兼容，contracts/01 §4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(rename_all = "lowercase")]
pub enum NoulKind {
    Noul,
    Boolean,
}

/* ══════════════════════════════════════════════════════════════════
   请求
   ══════════════════════════════════════════════════════════════════ */

/// Jev 决策请求（contracts/01 §2）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct JevRequest {
    /// 必填。
    pub model: String,
    /// 必填（允许 Null；无 serde default —— 禁止吞掉字段）。
    pub state: serde_json::Value,
    /// Record，禁止数组；无 serde default（必填）。
    pub questions: BTreeMap<String, Question>,
}

/// 过渡别名：主名 [`JevRequest`]（contracts/01）；deprecated 名保留以降低迁移面。
pub type SystemOneRequest = JevRequest;

/// 单个问题（请求侧）—— 判别联合（contracts/01 §3）。
///
/// wire 形如：
/// ```json
/// { "type": "noul", "instructions": "...", "criteria": {"true": "yes", "false": "no"} }
/// ```
///
/// **反序列化即校验**（daemon 直接映射本地 400，不发上游）：
/// - `type` ∈ {choice, score, noul, boolean}；未知 → 拒绝；`boolean` → 归一为 [`Question::Noul`]
/// - `instructions` 必填非空
/// - `criteria` 必填；按题型校验形态（choice→Map / score→List / noul→Bool），
///   缺失文案对齐上游 `expected record, received undefined`
///
/// 序列化恒写契约三判别值（`Noul` 写 `"noul"`；adapter 出站需要 `boolean` 方言时
/// 在 outgoing 转换 —— 见 jev-core translate / 后续 ProtocolAdapter）。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Choice { instructions: String, criteria: Criteria },
    Score { instructions: String, criteria: Criteria },
    Noul { instructions: String, criteria: Criteria },
}

impl Question {
    pub fn question_type_str(&self) -> &'static str {
        match self {
            Question::Choice { .. } => "choice",
            Question::Score { .. } => "score",
            Question::Noul { .. } => "noul",
        }
    }

    /// 问题类型 → capability 校验枚举（handler 转发前调 `Router::check_capability`）。
    pub fn question_type(&self) -> QuestionType {
        match self {
            Question::Choice { .. } => QuestionType::Choice,
            Question::Score { .. } => QuestionType::Score,
            Question::Noul { .. } => QuestionType::Noul,
        }
    }

    pub fn instructions(&self) -> &str {
        match self {
            Question::Choice { instructions, .. }
            | Question::Score { instructions, .. }
            | Question::Noul { instructions, .. } => instructions,
        }
    }

    pub fn criteria(&self) -> &Criteria {
        match self {
            Question::Choice { criteria, .. }
            | Question::Score { criteria, .. }
            | Question::Noul { criteria, .. } => criteria,
        }
    }
}

/// `criteria` 三形态（contracts/01 §3）：Map（choice/noul）/ List（score 有序档位）/ Bool（noul 两键）。
///
/// 注意：standalone 反序列化按 untagged 顺序 Map 优先；**题型上下文内**的形态
/// 校验由 [`Question`] 的手工 Deserialize 完成（同一 JSON 对 noul 归 `Bool`、
/// 对 choice 归 `Map` —— untagged 单独做不到这一点）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(untagged)]
pub enum Criteria {
    Map(BTreeMap<String, String>),
    List(Vec<String>),
    Bool { r#true: String, r#false: String },
}

/// 上游对齐的错误文案（contracts/01 §6：`expected record, received undefined`）。
const CRITERIA_UNDEFINED: &str = "expected record, received undefined";

fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "record",
    }
}

fn custom_err<E: serde::de::Error, T>(msg: impl Into<String>) -> Result<T, E> {
    Err(E::custom(msg.into()))
}

impl<'de> Deserialize<'de> for Question {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = match &value {
            serde_json::Value::Object(map) => map,
            other => {
                return custom_err(format!("expected record, received {}", json_kind(other)));
            }
        };

        // ── type：未知拒绝；boolean 归一为 noul（§1/§6）──
        let ty: &str = match obj.get("type") {
            None => return custom_err("missing field `type`"),
            Some(serde_json::Value::String(s)) => s.as_str(),
            Some(other) => {
                return custom_err(format!(
                    "expected string for field `type`, received {}",
                    json_kind(other)
                ));
            }
        };
        match ty {
            "choice" | "score" | "noul" | "boolean" => {}
            other => {
                return custom_err(format!(
                    "unknown variant `{other}`, expected one of `choice`, `score`, `noul`"
                ));
            }
        }

        // ── instructions：必填非空（§3 不变量）──
        let instructions: String = match obj.get("instructions") {
            None => return custom_err("missing field `instructions`"),
            Some(serde_json::Value::String(s)) if s.is_empty() => {
                return custom_err("instructions must be non-empty");
            }
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(other) => {
                return custom_err(format!(
                    "expected string for field `instructions`, received {}",
                    json_kind(other)
                ));
            }
        };

        // ── criteria：必填 + 按题型校验形态（§3 不变量 / §6 文案）──
        let criteria_val = match obj.get("criteria") {
            None => return custom_err(CRITERIA_UNDEFINED),
            Some(v) => v,
        };
        let criteria = match ty {
            "choice" => {
                // 形态：record（任意键 → 值必须是 string）
                let map = match criteria_val {
                    serde_json::Value::Object(m) => m,
                    other => {
                        return custom_err(format!(
                            "expected record, received {}",
                            json_kind(other)
                        ));
                    }
                };
                let mut out = BTreeMap::new();
                for (k, v) in map {
                    match v {
                        serde_json::Value::String(s) => {
                            out.insert(k.clone(), s.clone());
                        }
                        other => {
                            return custom_err(format!(
                                "expected string, received {}",
                                json_kind(other)
                            ));
                        }
                    }
                }
                Criteria::Map(out)
            }
            "score" => {
                // 形态：有序数组
                let arr = match criteria_val {
                    serde_json::Value::Array(a) => a,
                    other => {
                        return custom_err(format!("expected array, received {}", json_kind(other)));
                    }
                };
                let mut out = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        serde_json::Value::String(s) => out.push(s.clone()),
                        other => {
                            return custom_err(format!(
                                "expected string, received {}",
                                json_kind(other)
                            ));
                        }
                    }
                }
                Criteria::List(out)
            }
            // "noul" | "boolean"：形态恰为 {true, false} 两键 record（§3）
            _ => {
                let map = match criteria_val {
                    serde_json::Value::Object(m) => m,
                    other => {
                        return custom_err(format!(
                            "expected record, received {}",
                            json_kind(other)
                        ));
                    }
                };
                let pick = |key: &'static str| -> Result<String, D::Error> {
                    match map.get(key) {
                        None => Err(serde::de::Error::custom(CRITERIA_UNDEFINED)),
                        Some(serde_json::Value::String(s)) => Ok(s.clone()),
                        Some(other) => Err(serde::de::Error::custom(format!(
                            "expected string, received {}",
                            json_kind(other)
                        ))),
                    }
                };
                Criteria::Bool {
                    r#true: pick("true")?,
                    r#false: pick("false")?,
                }
            }
        };

        // boolean 输入按 noul 语义归一（§1：对外判别值只有 choice|score|noul）
        Ok(match ty {
            "choice" => Question::Choice {
                instructions,
                criteria,
            },
            "score" => Question::Score {
                instructions,
                criteria,
            },
            // "noul" | "boolean"
            _ => Question::Noul {
                instructions,
                criteria,
            },
        })
    }
}

/* ══════════════════════════════════════════════════════════════════
   响应
   ══════════════════════════════════════════════════════════════════ */

/// 单个问题的答案 —— 判别联合（contracts/01 §4）。
///
/// - choice/score：`probabilities`/`confidence` 必填；`score: f64`
/// - 布尔族：[`Answer::Noul`]，概率即置信度 —— **没有 confidence 字段**
/// - 未知 `type` → 拒绝（禁 best-effort 平移）
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(
        export,
        export_to = "../../../../ui/src/generated/",
        // 手工 Serialize（非 serde derive）→ ts-rs 看不到 tag、默认生成错误的
        // 外部标签形状（{Choice:{…}}）。按 contracts/01 §4 wire 字面做**容器级覆盖**。
        type = "{ \"type\": \"choice\", choice: string, probabilities: { [key in string]: number }, confidence: number } | { \"type\": \"score\", score: number, probabilities: { [key in string]: number }, confidence: number } | { \"type\": \"noul\" | \"boolean\", noul?: number | null, probability?: number | null }"
    )
)]
pub enum Answer {
    Choice {
        choice: String,
        /// 必填，完整分布
        probabilities: BTreeMap<String, f64>,
        /// 分布集中度，≠ 最高项概率
        confidence: f64,
    },
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul(NoulAnswer),
}

/// 布尔族答案：概率本身就是置信度 —— 没有 confidence 字段（contracts/01 §4）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct NoulAnswer {
    /// `"noul" | "boolean"`（wire 兼容）。
    #[serde(rename = "type")]
    pub kind: NoulKind,
    /// 官方 / OpenRouter 键。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noul: Option<f64>,
    /// Vercel 键 —— 必须保留，禁止吞掉。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
}

/// 布尔族取概率 —— 冻结顺序 `probability > noul`（contracts/01 §4；双缺返回 0.0 =
/// 「未验到」，调用方须能区分「0.0」与「未知」——未知时 [`NoulAnswer`] 双键皆 None）。
///
/// 与 jev-life `noulProbability()` 的差异（`noul ?? probability`）以本契约为准。
pub fn noul_probability(a: &NoulAnswer) -> f64 {
    a.probability.or(a.noul).unwrap_or(0.0)
}

// Answer 手工编解码：Noul 为 newtype（内嵌自带 `type` 判别），
// serde 内部 tag 与内层 `type` 字段会撞键 —— 按契约 wire 形状手写。

#[derive(Deserialize)]
struct AnswerChoiceDe {
    choice: String,
    probabilities: BTreeMap<String, f64>,
    confidence: f64,
}

#[derive(Deserialize)]
struct AnswerScoreDe {
    score: f64,
    probabilities: BTreeMap<String, f64>,
    confidence: f64,
}

impl<'de> Deserialize<'de> for Answer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let ty = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("missing field `type`"))?;
        match ty {
            "choice" => {
                let de: AnswerChoiceDe =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Answer::Choice {
                    choice: de.choice,
                    probabilities: de.probabilities,
                    confidence: de.confidence,
                })
            }
            "score" => {
                let de: AnswerScoreDe =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Answer::Score {
                    score: de.score,
                    probabilities: de.probabilities,
                    confidence: de.confidence,
                })
            }
            "noul" | "boolean" => Ok(Answer::Noul(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            )),
            other => Err(serde::de::Error::custom(format!(
                "unknown variant `{other}`, expected one of `choice`, `score`, `noul`"
            ))),
        }
    }
}

impl Serialize for Answer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => serde_json::json!({
                "type": "choice",
                "choice": choice,
                "probabilities": probabilities,
                "confidence": confidence,
            }),
            Answer::Score {
                score,
                probabilities,
                confidence,
            } => serde_json::json!({
                "type": "score",
                "score": score,
                "probabilities": probabilities,
                "confidence": confidence,
            }),
            // Noul 自带 `type` 判别键（kind），直通内层序列化
            Answer::Noul(n) => return n.serialize(serializer),
        };
        value.serialize(serializer)
    }
}

/// token 用量 —— 强类型（contracts/01 §5）。
///
/// 反序列化接受驼峰与下划线两套拼写（宽容读入，禁 400）；写出统一 snake_case。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct Usage {
    // ts-rs：u64 默认映射 bigint；wire 是 JSON number —— 覆盖对齐运行时
    #[serde(default, alias = "inputTokens", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub input_tokens: Option<u64>,
    #[serde(default, alias = "outputTokens", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub output_tokens: Option<u64>,
    #[serde(
        default,
        alias = "reasoningTokens",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub reasoning_tokens: Option<u64>,
}

fn default_upstream_calls() -> Option<u32> {
    Some(1)
}

/// Jev 决策响应（contracts/01 §5）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct JevResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub answers: BTreeMap<String, Answer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 工具循环/重试/多候选实发次数；**读缺省按 1**（显式 null → None）。
    /// 双拼写宽容：别名 `upstreamCalls`（jev-life 驼峰）。
    #[serde(
        default = "default_upstream_calls",
        alias = "upstreamCalls"
    )]
    pub upstream_calls: Option<u32>,
    /// 双拼写宽容读入；写出 snake_case。null ≠ 0。
    /// （ts-rs：u64 默认 → bigint，wire 是 JSON number —— 覆盖对齐运行时）
    #[serde(default, alias = "latencyMs")]
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub latency_ms: Option<u64>,
    /// null ≠ 0：未知就是 null —— 不 skip，显式写 null（契约字面）。
    #[serde(default, alias = "costUsd")]
    pub cost_usd: Option<f64>,
    /// providerMetadata 等上游顶层余量（契约 flatten）。
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// 过渡别名：主名 [`JevResponse`]（contracts/01）；deprecated 名保留以降低迁移面。
pub type SystemOneResponse = JevResponse;

// A5：厂商 DTO（VercelAnswer / VercelResponse）已迁出至 jev-adapters
// （contracts/02 §3 分层铁律：厂商 DTO 只活在 adapter crate）。

#[cfg(test)]
mod tests {
    use super::*;

    /* ── 请求 ──────────────────────────────────────────────── */

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
        let req: JevRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "laya-english");
        let q = req.questions.get("q").unwrap();
        assert_eq!(q.question_type_str(), "noul");
        assert_eq!(
            q.instructions(),
            "is this a greeting?"
        );
        // 上下文校验：noul → Bool 形态（而非 untagged 的 Map）
        assert_eq!(
            q.criteria(),
            &Criteria::Bool {
                r#true: "yes".into(),
                r#false: "no".into()
            }
        );
        // round-trip
        let back = serde_json::to_string(&req).unwrap();
        let req2: JevRequest = serde_json::from_str(&back).unwrap();
        assert_eq!(req2, req);
    }

    #[test]
    fn boolean_input_normalized_to_noul() {
        // §1：boolean 不是合法对外判别值 —— 收下也按 noul 语义归一
        let json = r#"{
            "model": "typesafe-ai/jev",
            "state": null,
            "questions": {
                "q": {"type": "boolean", "instructions": "x", "criteria": {"true": "y", "false": "n"}}
            }
        }"#;
        let req: JevRequest = serde_json::from_str(json).unwrap();
        let q = req.questions.get("q").unwrap();
        assert_eq!(q.question_type_str(), "noul");
        // 写出恒为 noul 键（§6：Noul 默认写 noul）
        let back = serde_json::to_value(&req).unwrap();
        assert_eq!(back["questions"]["q"]["type"], "noul");
    }

    #[test]
    fn score_with_list_criteria() {
        let json = r#"{
            "model": "m",
            "state": null,
            "questions": {
                "s": {"type": "score", "instructions": "rank", "criteria": ["low","med","high"]}
            }
        }"#;
        let req: JevRequest = serde_json::from_str(json).unwrap();
        let q = req.questions.get("s").unwrap();
        assert_eq!(q.question_type_str(), "score");
        assert_eq!(
            q.criteria(),
            &Criteria::List(vec!["low".into(), "med".into(), "high".into()])
        );
    }

    #[test]
    fn choice_with_map_criteria() {
        let json = r#"{
            "model": "m",
            "state": null,
            "questions": {
                "c": {"type": "choice", "instructions": "pick", "criteria": {"A": "alpha", "B": "beta"}}
            }
        }"#;
        let req: JevRequest = serde_json::from_str(json).unwrap();
        let q = req.questions.get("c").unwrap();
        assert_eq!(q.question_type_str(), "choice");
        assert_eq!(
            q.criteria(),
            &Criteria::Map(BTreeMap::from([
                ("A".into(), "alpha".into()),
                ("B".into(), "beta".into()),
            ]))
        );
    }

    #[test]
    fn missing_criteria_rejected_with_upstream_message() {
        // §8 验收：缺 criteria → 反序列化错误；文案对齐上游（§6）
        let json = r#"{
            "model": "m",
            "state": null,
            "questions": {
                "q": {"type": "noul", "instructions": "x"}
            }
        }"#;
        let err = serde_json::from_str::<JevRequest>(json).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected record, received undefined"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn wrong_shape_criteria_rejected_locally() {
        // 错形态 → 本地拒绝，不发上游（§3/§6）
        // noul 收 array
        let noul_arr = r#"{
            "model": "m", "state": null,
            "questions": {"q": {"type": "noul", "instructions": "x", "criteria": ["a","b"]}}
        }"#;
        let err = serde_json::from_str::<JevRequest>(noul_arr).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected record, received array"),
            "unexpected: {err}"
        );

        // choice 收 array
        let choice_arr = r#"{
            "model": "m", "state": null,
            "questions": {"c": {"type": "choice", "instructions": "x", "criteria": ["a"]}}
        }"#;
        let err = serde_json::from_str::<JevRequest>(choice_arr).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected record, received array"),
            "unexpected: {err}"
        );

        // score 收 record
        let score_rec = r#"{
            "model": "m", "state": null,
            "questions": {"s": {"type": "score", "instructions": "x", "criteria": {"a": "1"}}}
        }"#;
        let err = serde_json::from_str::<JevRequest>(score_rec).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected array, received record"),
            "unexpected: {err}"
        );

        // noul 缺 true/false 键
        let noul_partial = r#"{
            "model": "m", "state": null,
            "questions": {"q": {"type": "noul", "instructions": "x", "criteria": {"other": "1"}}}
        }"#;
        let err = serde_json::from_str::<JevRequest>(noul_partial).unwrap_err();
        assert!(
            err.to_string()
                .contains("expected record, received undefined"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn unknown_question_type_rejected() {
        // §6：未知 type → 拒绝（禁 best-effort 平移）
        let json = r#"{
            "model": "m", "state": null,
            "questions": {"q": {"type": "weird", "instructions": "x", "criteria": {"a": "b"}}}
        }"#;
        let err = serde_json::from_str::<JevRequest>(json).unwrap_err();
        assert!(
            err.to_string().contains("unknown variant `weird`"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn state_and_questions_are_required() {
        // §2：state 必填（禁止 serde default 吞掉）；questions 必填 Record
        let no_state = r#"{"model": "m", "questions": {}}"#;
        let err = serde_json::from_str::<JevRequest>(no_state).unwrap_err();
        assert!(
            err.to_string().contains("missing field `state`"),
            "unexpected: {err}"
        );

        let no_questions = r#"{"model": "m", "state": null}"#;
        let err = serde_json::from_str::<JevRequest>(no_questions).unwrap_err();
        assert!(
            err.to_string().contains("missing field `questions`"),
            "unexpected: {err}"
        );

        // questions 是 record，不是数组
        let arr = r#"{"model": "m", "state": null, "questions": []}"#;
        assert!(serde_json::from_str::<JevRequest>(arr).is_err());
    }

    /* ── Answer / noul_probability ─────────────────────────── */

    #[test]
    fn noul_fixture_boolean_probability_069() {
        // §8 #1：{"type":"boolean","probability":0.69} → noul_probability == 0.69（不是 0.95）
        let a: NoulAnswer =
            serde_json::from_str(r#"{"type":"boolean","probability":0.69}"#).unwrap();
        assert_eq!(a.kind, NoulKind::Boolean);
        assert_eq!(noul_probability(&a), 0.69);
    }

    #[test]
    fn noul_fixture_noul_096() {
        // §8 #2：{"type":"noul","noul":0.96} → 0.96
        let a: NoulAnswer = serde_json::from_str(r#"{"type":"noul","noul":0.96}"#).unwrap();
        assert_eq!(a.kind, NoulKind::Noul);
        assert_eq!(noul_probability(&a), 0.96);
    }

    #[test]
    fn dual_keys_preserved_and_probability_first() {
        // §4 冻结顺序：probability > noul；双键并存时都保留（§6）
        let a: NoulAnswer =
            serde_json::from_str(r#"{"type":"boolean","noul":0.9,"probability":0.7}"#).unwrap();
        assert_eq!(a.noul, Some(0.9));
        assert_eq!(a.probability, Some(0.7));
        assert_eq!(noul_probability(&a), 0.7);
    }

    #[test]
    fn both_keys_absent_is_zero_but_distinguishable() {
        // §4：双缺 = 未验到 → 0.0；与「真的 0.0」的区分靠双键皆 None
        let a: NoulAnswer = serde_json::from_str(r#"{"type":"noul"}"#).unwrap();
        assert_eq!(noul_probability(&a), 0.0);
        assert!(a.noul.is_none() && a.probability.is_none());
    }

    #[test]
    fn choice_answer_three_required_fields() {
        // §8 #3：choice 三字段必填
        let ok = r#"{"type":"choice","choice":"A","probabilities":{"A":0.7,"B":0.3},"confidence":0.7}"#;
        let a: Answer = serde_json::from_str(ok).unwrap();
        match &a {
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

        let missing_confidence =
            r#"{"type":"choice","choice":"A","probabilities":{"A":1.0}}"#;
        assert!(serde_json::from_str::<Answer>(missing_confidence).is_err());

        let missing_probs = r#"{"type":"choice","choice":"A","confidence":0.5}"#;
        assert!(serde_json::from_str::<Answer>(missing_probs).is_err());
    }

    #[test]
    fn score_answer_score_is_f64() {
        // §8 #3：score.score 为 f64（整数字面量也接受）
        let a: Answer =
            serde_json::from_str(r#"{"type":"score","score":1,"probabilities":{"1":1.0},"confidence":1.0}"#)
                .unwrap();
        match a {
            Answer::Score { score, .. } => assert_eq!(score, 1.0),
            other => panic!("unexpected: {other:?}"),
        }

        // 必填校验
        assert!(serde_json::from_str::<Answer>(
            r#"{"type":"score","probabilities":{"1":1.0},"confidence":1.0}"#
        )
        .is_err());
    }

    #[test]
    fn noul_answer_has_no_confidence() {
        // §4 不变量：布尔族没有 confidence —— 多余键宽容忽略，绝不产出 confidence
        let a: Answer =
            serde_json::from_str(r#"{"type":"noul","noul":0.5,"confidence":0.99}"#).unwrap();
        let Answer::Noul(n) = &a else {
            panic!("expected noul");
        };
        assert_eq!(n.noul, Some(0.5));
        assert_eq!(n.kind, NoulKind::Noul);
        // 写出不带 confidence
        let back = serde_json::to_value(&a).unwrap();
        assert!(back.get("confidence").is_none());
        assert_eq!(back["type"], "noul");
    }

    #[test]
    fn answer_unknown_type_rejected() {
        // §6：未知 type → 拒绝（禁 best-effort 平移到扁平 Option）
        let err = serde_json::from_str::<Answer>(r#"{"type":"mystery","x":1}"#).unwrap_err();
        assert!(
            err.to_string().contains("unknown variant `mystery`"),
            "unexpected: {err}"
        );
    }

    /* ── JevResponse / Usage ───────────────────────────────── */

    #[test]
    fn response_upstream_calls_defaults_to_one() {
        // §5：upstream_calls 读缺省按 1
        let v: JevResponse = serde_json::from_str(r#"{"answers":{}}"#).unwrap();
        assert_eq!(v.upstream_calls, Some(1));

        // 显式 null → None（区别于缺省）
        let v: JevResponse = serde_json::from_str(r#"{"answers":{},"upstream_calls":null}"#)
            .unwrap();
        assert_eq!(v.upstream_calls, None);

        // 双拼写（jev-life 驼峰）
        let v: JevResponse = serde_json::from_str(r#"{"answers":{},"upstreamCalls":3}"#).unwrap();
        assert_eq!(v.upstream_calls, Some(3));
    }

    #[test]
    fn response_cost_usd_null_is_not_zero() {
        // §5：cost_usd null ≠ 0 —— 未知就是 null
        let v: JevResponse =
            serde_json::from_str(r#"{"answers":{},"cost_usd":null}"#).unwrap();
        assert_eq!(v.cost_usd, None);

        let v: JevResponse = serde_json::from_str(r#"{"answers":{},"cost_usd":0.0}"#).unwrap();
        assert_eq!(v.cost_usd, Some(0.0));

        // 写出：未知 → 显式 null（不 skip）
        let v = JevResponse {
            answers: BTreeMap::new(),
            model: None,
            usage: None,
            upstream_calls: Some(1),
            latency_ms: None,
            cost_usd: None,
            extra: BTreeMap::new(),
        };
        let out = serde_json::to_value(&v).unwrap();
        assert!(out.get("cost_usd").is_some_and(|x| x.is_null()));
        // model/usage 为可选呈现 → absent
        assert!(out.get("model").is_none());
        assert!(out.get("usage").is_none());
    }

    #[test]
    fn response_extra_flattened_roundtrip() {
        // §5：providerMetadata 等进 flatten extra，读写保留
        let json = r#"{
            "answers": {},
            "providerMetadata": {"src": "vercel"},
            "upstream": "vercel"
        }"#;
        let v: JevResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            v.extra.get("providerMetadata"),
            Some(&serde_json::json!({"src": "vercel"}))
        );
        assert_eq!(
            v.extra.get("upstream"),
            Some(&serde_json::json!("vercel"))
        );
        let back = serde_json::to_value(&v).unwrap();
        assert_eq!(back["providerMetadata"]["src"], "vercel");
        assert_eq!(back["upstream"], "vercel");
    }

    #[test]
    fn usage_dual_spelling_read_snake_write() {
        // §5：双拼写宽容读入、写出 snake_case
        let camel: Usage = serde_json::from_str(
            r#"{"inputTokens":10,"outputTokens":20,"reasoningTokens":5}"#,
        )
        .unwrap();
        assert_eq!(camel.input_tokens, Some(10));
        assert_eq!(camel.output_tokens, Some(20));
        assert_eq!(camel.reasoning_tokens, Some(5));

        let snake: Usage = serde_json::from_str(
            r#"{"input_tokens":10,"output_tokens":20,"reasoning_tokens":5}"#,
        )
        .unwrap();
        assert_eq!(snake, camel);

        let out = serde_json::to_value(&snake).unwrap();
        assert_eq!(out["input_tokens"], 10);
        assert_eq!(out["output_tokens"], 20);
        assert_eq!(out["reasoning_tokens"], 5);
        assert!(out.get("inputTokens").is_none());
    }

    /* ── 过渡别名仍可用 ────────────────────────────────────── */

    #[test]
    fn systemone_aliases_still_resolve() {
        let req: SystemOneRequest =
            serde_json::from_str(r#"{"model":"m","state":null,"questions":{}}"#).unwrap();
        let resp: SystemOneResponse = serde_json::from_str(r#"{"answers":{}}"#).unwrap();
        assert_eq!(req.model, "m");
        assert_eq!(resp.upstream_calls, Some(1));
    }
}
