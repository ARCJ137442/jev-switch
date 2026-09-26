//! 外部应用以 path-dep 依赖 `jev-protocol` 后即此用法（P0-1 验收同源）。
//!
//! 在你自己的 crate 里：
//! ```toml
//! [dependencies]
//! jev-protocol = { path = "../jev-switch/rs/crates/jev-protocol" }
//! ```
//! 然后 `use jev_protocol::{JevRequest, Question, Criteria, noul_probability, …}`。
//!
//! 运行（仓库根）：
//! ```bash
//! cargo run --manifest-path rs/Cargo.toml -p jev-protocol --example basic_use
//! ```
//!
//! 契约要点（docs/contracts/01）：
//! - `criteria` 必填；noul 题用 Bool 形态（`{true, false}` 两键）
//! - 布尔族答案无 `confidence`；取概率用冻结顺序的 [`noul_probability`]

use jev_protocol::{
    noul_probability, Answer, Criteria, JevRequest, JevResponse, NoulAnswer, NoulKind, Question,
};
use std::collections::BTreeMap;

fn main() {
    /* ── 1) 构造含 noul 题的 JevRequest（criteria 用 Bool 形态） ──── */
    let mut questions = BTreeMap::new();
    questions.insert(
        "q".to_string(),
        Question::Noul {
            instructions: "is this a test?".to_string(),
            criteria: Criteria::Bool {
                r#true: "yes".to_string(),
                r#false: "no".to_string(),
            },
        },
    );
    let req = JevRequest {
        model: "laya-english".to_string(),
        state: serde_json::json!({"source": "basic_use example"}),
        questions,
    };
    println!(
        "JevRequest:\n{}",
        serde_json::to_string_pretty(&req).expect("JevRequest serializes")
    );

    /* ── 2) 构造 JevResponse / NoulAnswer，演示 noul_probability() ───
    夹具 0.69 对齐 P0-2 验收（{"type":"boolean","probability":0.69} → 0.69，
    不是历史 bug 的硬编码 0.95）。此处用官方键 `noul`；Vercel 方言键
    `probability` 同样保留，读取顺序冻结为 probability > noul。 */
    let noul = NoulAnswer {
        kind: NoulKind::Noul,
        noul: Some(0.69),
        probability: None,
    };
    let mut answers = BTreeMap::new();
    answers.insert("q".to_string(), Answer::Noul(noul.clone()));

    let resp = JevResponse {
        model: Some("laya-english".to_string()),
        answers,
        usage: None,             // 无用量信息 = None（null ≠ 0）
        upstream_calls: Some(1), // 实发次数；读缺省按 1
        latency_ms: None,
        cost_usd: None,         // 未知就是 null（契约：null ≠ 0）
        extra: BTreeMap::new(), // providerMetadata 等上游余量（flatten）
    };
    println!(
        "JevResponse:\n{}",
        serde_json::to_string_pretty(&resp).expect("JevResponse serializes")
    );

    /* ── 3) noul_probability() 冻结读取顺序 ─────────────────────────── */
    let p = noul_probability(&noul);
    println!("noul_probability(&noul_answer) = {p}");
    assert_eq!(p, 0.69, "P0-2：概率必须原样读出，不是 0.95/0.05 硬编码");

    println!("ok — 外部应用 path-dep jev-protocol 即此用法（P0-1 验收同源）");
}
