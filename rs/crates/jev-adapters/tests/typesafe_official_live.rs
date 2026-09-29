use jev_adapters::upstream_typesafe::{TypeSafeUpstream, TYPESAFE_OFFICIAL_BASE};
use jev_core::adapter::UpstreamAdapter;
use jev_protocol::{Answer, JevRequest};

#[tokio::test]
#[ignore = "Makes one live TypeSafe API request; requires TYPESAFE_API_KEY and explicit authorization"]
async fn official_typesafe_systemone_accepts_native_request_and_all_answer_types() {
    let api_key = std::env::var("TYPESAFE_API_KEY")
        .expect("set TYPESAFE_API_KEY from a protected local secret file before running this test");
    let request: JevRequest = serde_json::from_value(serde_json::json!({
        "model": "jev-latest",
        "state": "The deployment is ready for launch.",
        "questions": {
            "readiness": {
                "type": "noul",
                "instructions": "Does the state say the deployment is ready?",
                "criteria": {
                    "true": "It explicitly says the deployment is ready.",
                    "false": "It does not say the deployment is ready."
                }
            },
            "status": {
                "type": "choice",
                "instructions": "Choose the status that best matches the state.",
                "criteria": {
                    "ready": "The deployment is ready.",
                    "blocked": "The deployment is blocked."
                }
            },
            "score": {
                "type": "score",
                "instructions": "Rate the deployment readiness.",
                "criteria": ["Not ready", "Ready"]
            }
        }
    }))
    .expect("the public Jev request should match the current protocol schema");
    let upstream = TypeSafeUpstream::new(TYPESAFE_OFFICIAL_BASE.to_string(), Some(api_key))
        .expect("construct official TypeSafe adapter");

    let response = match upstream.evaluate(request).await {
        Ok(response) => response,
        Err(_) => panic!("official TypeSafe SystemOne request failed; inspect status without logging credentials"),
    };

    assert!(response
        .model
        .as_deref()
        .is_some_and(|model| model.starts_with("jev-")));
    assert!(matches!(
        response.answers.get("readiness"),
        Some(Answer::Noul(_))
    ));
    assert!(matches!(
        response.answers.get("status"),
        Some(Answer::Choice { .. })
    ));
    let Some(Answer::Score { legend, .. }) = response.answers.get("score") else {
        panic!("official response did not preserve the Score answer");
    };
    assert!(legend.as_ref().is_some_and(serde_json::Value::is_object));
    assert!(response.usage.is_some());

    let answer_types = response
        .answers
        .iter()
        .map(|(id, answer)| {
            let kind = match answer {
                Answer::Noul(_) => "noul",
                Answer::Choice { .. } => "choice",
                Answer::Score { .. } => "score",
            };
            format!("{id}:{kind}")
        })
        .collect::<Vec<_>>()
        .join(",");
    let usage = response.usage.expect("usage was checked above");
    println!(
        "typesafe-official-live: status=200 model={} answer_types={} input_tokens={:?} output_tokens={:?}",
        response.model.as_deref().unwrap_or("unknown"),
        answer_types,
        usage.input_tokens,
        usage.output_tokens
    );
}
