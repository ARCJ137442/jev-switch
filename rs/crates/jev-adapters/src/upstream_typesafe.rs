//! Native TypeSafe SystemOne HTTP upstream and TypeSafe-compatible local services.
//!
//! `base` is the complete SystemOne endpoint URL, for example
//! `https://api.typesafe.ai/v1/systemone` or `http://127.0.0.1:8000/v1/systemone`.
//! Requests and successful responses use the Jev protocol directly.

use jev_core::adapter::UpstreamAdapter;
use jev_core::upstream::{Capabilities, JevError, QuestionType};
use jev_protocol::{JevRequest, JevResponse};
use reqwest::Client;
use std::time::Duration;

pub const TYPESAFE_OFFICIAL_BASE: &str = "https://api.typesafe.ai/v1/systemone";
pub const TYPESAFE_CAPABILITIES: Capabilities = Capabilities {
    question_types: &[
        QuestionType::Choice,
        QuestionType::Score,
        QuestionType::Noul,
    ],
    has_confidence: true,
    has_usage: true,
    noul_via_boolean: false,
    retryable_status: &[408, 429, 500, 502, 503, 504, 529],
};

pub struct TypeSafeUpstream {
    id: String,
    base: String,
    api_key: Option<String>,
    forward_extensions: bool,
    http: Client,
}

impl TypeSafeUpstream {
    pub fn new(base: String, api_key: Option<String>) -> Result<Self, JevError> {
        Self::new_with_id_and_options("typesafe".into(), base, api_key, false)
    }

    pub fn new_with_id(
        id: String,
        base: String,
        api_key: Option<String>,
    ) -> Result<Self, JevError> {
        Self::new_with_id_and_options(id, base, api_key, false)
    }

    /// Construct a TypeSafe-compatible upstream with explicit extension forwarding.
    ///
    /// `forward_extensions` is opt-in because the official TypeSafe endpoint is
    /// text-only. Enabled providers must be a gateway or local implementation
    /// that documents the extension keys it accepts.
    pub fn new_with_id_and_options(
        id: String,
        base: String,
        api_key: Option<String>,
        forward_extensions: bool,
    ) -> Result<Self, JevError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| JevError::Config {
                upstream_id: id.clone(),
                message: format!("reqwest build failed: {e}"),
            })?;
        Ok(Self {
            id,
            base,
            api_key: api_key.filter(|key| !key.is_empty()),
            forward_extensions,
            http,
        })
    }
}

#[async_trait::async_trait]
impl UpstreamAdapter for TypeSafeUpstream {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> Capabilities {
        TYPESAFE_CAPABILITIES
    }

    async fn evaluate(&self, req: JevRequest) -> Result<JevResponse, JevError> {
        let extension_keys = extension_keys(&req);
        let payload = request_payload(&self.id, req, self.forward_extensions)?;
        let mut builder = self.http.post(&self.base).json(&payload);
        if let Some(key) = self.api_key.as_deref() {
            builder = builder.bearer_auth(key);
        }

        let resp = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                JevError::Timeout {
                    upstream_id: self.id.clone(),
                }
            } else {
                JevError::Network {
                    upstream_id: self.id.clone(),
                    message: e.to_string(),
                }
            }
        })?;

        let status = resp.status();
        let raw_bytes = resp.bytes().await.map_err(|e| JevError::Network {
            upstream_id: self.id.clone(),
            message: format!("read body: {e}"),
        })?;

        if !status.is_success() {
            return Err(JevError::Upstream {
                upstream_id: self.id.clone(),
                status: status.as_u16(),
                body: String::from_utf8_lossy(&raw_bytes).to_string(),
                retryable: self.capabilities().is_retryable_status(status.as_u16()),
            });
        }

        let raw = serde_json::from_slice::<serde_json::Value>(&raw_bytes).map_err(|e| {
            JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse TypeSafe SystemOne response: {e}"),
            }
        })?;
        if let Some(answers) = raw.get("answers").and_then(serde_json::Value::as_object) {
            for (question_id, answer) in answers {
                if answer.get("type").and_then(serde_json::Value::as_str) == Some("score")
                    && !has_typesafe_score_legend(answer)
                {
                    return Err(JevError::BadResponse {
                        upstream_id: self.id.clone(),
                        message: format!(
                            "answer '{question_id}' has missing or invalid TypeSafe score legend"
                        ),
                    });
                }
            }
        }

        let mut response =
            serde_json::from_value::<JevResponse>(raw).map_err(|e| JevError::BadResponse {
                upstream_id: self.id.clone(),
                message: format!("parse TypeSafe SystemOne response: {e}"),
            })?;
        if !extension_keys.is_empty() {
            response.extra.insert(
                "jev_switch".into(),
                serde_json::json!({
                    "request_extensions": {
                        "received": extension_keys,
                        "provider_disposition": if self.forward_extensions { "forwarded" } else { "stripped" },
                    }
                }),
            );
        }
        Ok(response)
    }
}

fn extension_keys(req: &JevRequest) -> Vec<String> {
    let mut keys = std::collections::BTreeSet::new();
    keys.extend(req.extra.keys().cloned());
    if let Some(extensions) = &req.extensions {
        keys.extend(extensions.keys().cloned());
    }
    keys.into_iter().collect()
}

fn request_payload(
    upstream_id: &str,
    req: JevRequest,
    forward_extensions: bool,
) -> Result<serde_json::Value, JevError> {
    let mut extensions = req.extra.clone();
    for (key, value) in req.extensions.clone().unwrap_or_default() {
        if extensions.insert(key.clone(), value).is_some() {
            return Err(JevError::Config {
                upstream_id: upstream_id.to_string(),
                message: format!("duplicate request extension key '{key}'"),
            });
        }
    }
    let mut payload = serde_json::to_value(req).map_err(|error| JevError::Config {
        upstream_id: upstream_id.to_string(),
        message: format!("serialize TypeSafe SystemOne request: {error}"),
    })?;
    let Some(object) = payload.as_object_mut() else {
        return Err(JevError::Config {
            upstream_id: upstream_id.to_string(),
            message: "serialized TypeSafe request was not an object".into(),
        });
    };
    // Remove every unknown field first. This makes the default path safe even
    // when a caller sent a provider-specific field directly at the top level.
    for key in extensions.keys() {
        object.remove(key);
    }
    object.remove("extensions");
    if !forward_extensions {
        return Ok(payload);
    }
    for (key, value) in extensions {
        if matches!(key.as_str(), "model" | "state" | "questions" | "extensions") {
            return Err(JevError::Config {
                upstream_id: upstream_id.to_string(),
                message: format!("extension key '{key}' conflicts with the Jev request contract"),
            });
        }
        if object.insert(key.clone(), value).is_some() {
            return Err(JevError::Config {
                upstream_id: upstream_id.to_string(),
                message: format!("extension key '{key}' conflicts with an existing request field"),
            });
        }
    }
    Ok(payload)
}

fn has_typesafe_score_legend(answer: &serde_json::Value) -> bool {
    let Some(legend) = answer.get("legend").and_then(serde_json::Value::as_object) else {
        return false;
    };
    let Some(probabilities) = answer
        .get("probabilities")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    !probabilities.is_empty()
        && legend.len() == probabilities.len()
        && legend.iter().all(|(level, description)| {
            description.is_string() && probabilities.contains_key(level)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev_protocol::{Answer, Criteria, CriterionValue, Instructions, Question, Usage};
    use serde_json::json;
    use std::collections::BTreeMap;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn complete_request() -> JevRequest {
        let questions = BTreeMap::from([
            (
                "choice".into(),
                Question::Choice {
                    instructions: Instructions::Array(vec![
                        json!("choose one"),
                        json!({"detail": "structured"}),
                    ]),
                    criteria: Criteria::Map(BTreeMap::from([
                        ("A".into(), "first".into()),
                        (
                            "B".into(),
                            CriterionValue::Object(BTreeMap::from([(
                                "label".into(),
                                json!("second"),
                            )])),
                        ),
                        (
                            "C".into(),
                            CriterionValue::Array(vec![json!("third"), json!({"nested": true})]),
                        ),
                        ("D".into(), CriterionValue::Null),
                    ])),
                },
            ),
            (
                "noul".into(),
                Question::Noul {
                    instructions: "is it true?".into(),
                    criteria: None,
                },
            ),
            (
                "score".into(),
                Question::Score {
                    instructions: Instructions::Object(BTreeMap::from([
                        ("prompt".into(), json!("rate this")),
                        ("context".into(), json!(["structured"])),
                    ])),
                    criteria: Criteria::List(vec![
                        "low".into(),
                        CriterionValue::Object(BTreeMap::from([("level".into(), json!(2))])),
                        CriterionValue::Array(vec![json!("high"), json!("certain")]),
                    ]),
                },
            ),
        ]);
        JevRequest {
            model: "jev-latest".into(),
            state: json!({"text": "hello"}),
            questions,
            extensions: Default::default(),
            extra: Default::default(),
        }
    }

    #[tokio::test]
    async fn sends_official_systemone_request_and_parses_all_answer_variants() {
        let server = MockServer::start().await;
        let request = complete_request();
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .and(header("authorization", "Bearer test-typesafe-key"))
            .and(body_json(json!({
                "model": "jev-latest",
                "state": {"text": "hello"},
                "questions": {
                    "choice": {"type": "choice", "instructions": ["choose one", {"detail": "structured"}], "criteria": {"A": "first", "B": {"label": "second"}, "C": ["third", {"nested": true}], "D": null}},
                    "noul": {"type": "noul", "instructions": "is it true?"},
                    "score": {"type": "score", "instructions": {"prompt": "rate this", "context": ["structured"]}, "criteria": ["low", {"level": 2}, ["high", "certain"]]}
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model": "jev-latest",
                "answers": {
                    "choice": {"type": "choice", "choice": "A", "probabilities": {"A": 0.8, "B": 0.2}, "confidence": 0.8},
                    "noul": {"type": "noul", "noul": 0.91},
                    "score": {"type": "score", "score": 1.5, "legend": {"1": "low", "2": "high"}, "probabilities": {"1": 0.5, "2": 0.5}, "confidence": 0.7}
                },
                "usage": {"input_tokens": 42, "output_tokens": 3}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let upstream = TypeSafeUpstream::new(
            format!("{}/v1/systemone", server.uri()),
            Some("test-typesafe-key".into()),
        )
        .unwrap();
        let response = upstream.evaluate(request).await.unwrap();

        assert_eq!(response.model.as_deref(), Some("jev-latest"));
        assert!(matches!(response.answers["choice"], Answer::Choice { .. }));
        assert!(matches!(response.answers["noul"], Answer::Noul(_)));
        let Answer::Score { score, legend, .. } = &response.answers["score"] else {
            panic!("expected score answer");
        };
        assert_eq!(*score, 1.5);
        assert_eq!(legend.as_ref(), Some(&json!({"1": "low", "2": "high"})));
        assert_eq!(
            response.usage,
            Some(Usage {
                input_tokens: Some(42),
                output_tokens: Some(3),
                reasoning_tokens: None,
            })
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn local_typesafe_compatible_endpoint_needs_no_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers": {}})))
            .expect(1)
            .mount(&server)
            .await;

        let upstream =
            TypeSafeUpstream::new(format!("{}/v1/systemone", server.uri()), None).unwrap();
        upstream.evaluate(complete_request()).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].headers.contains_key("authorization"));
    }

    #[tokio::test]
    async fn extension_forwarding_is_opt_in_and_flattens_media_for_compatible_gateways() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers": {}})))
            .expect(1)
            .mount(&server)
            .await;

        let mut request = complete_request();
        request.extensions.get_or_insert_default().insert(
            "media".into(),
            json!([{"type":"image","data":"data:image/png;base64,AA=="}]),
        );
        let upstream = TypeSafeUpstream::new_with_id_and_options(
            "multimodal-gateway".into(),
            format!("{}/v1/systemone", server.uri()),
            None,
            true,
        )
        .unwrap();
        let response = upstream.evaluate(request).await.unwrap();
        assert_eq!(
            response.extra["jev_switch"]["request_extensions"]["provider_disposition"],
            "forwarded"
        );

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["media"][0]["type"], "image");
        assert!(body.get("extensions").is_none());
    }

    #[tokio::test]
    async fn extension_payload_is_stripped_for_official_compatible_defaults() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers": {}})))
            .expect(1)
            .mount(&server)
            .await;

        let mut request = complete_request();
        request
            .extensions
            .get_or_insert_default()
            .insert("media".into(), json!([]));
        let upstream =
            TypeSafeUpstream::new(format!("{}/v1/systemone", server.uri()), None).unwrap();
        let response = upstream.evaluate(request).await.unwrap();
        assert_eq!(
            response.extra["jev_switch"]["request_extensions"]["provider_disposition"],
            "stripped"
        );

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert!(body.get("media").is_none());
        assert!(body.get("extensions").is_none());
    }

    #[tokio::test]
    async fn score_answer_requires_a_matching_string_legend_map() {
        let invalid_legends = [
            None,
            Some(json!(null)),
            Some(json!(["low"])),
            Some(json!({"1": 1})),
            Some(json!({"other": "low"})),
        ];
        for legend in invalid_legends {
            let server = MockServer::start().await;
            let mut score = json!({
                "type": "score",
                "score": 1.0,
                "probabilities": {"1": 1.0},
                "confidence": 1.0
            });
            if let Some(legend) = legend {
                score["legend"] = legend;
            }
            Mock::given(method("POST"))
                .and(path("/v1/systemone"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "answers": {"score": score}
                })))
                .expect(1)
                .mount(&server)
                .await;

            let upstream =
                TypeSafeUpstream::new(format!("{}/v1/systemone", server.uri()), None).unwrap();
            let error = upstream.evaluate(complete_request()).await.unwrap_err();
            assert!(matches!(
                error,
                JevError::BadResponse { message, .. }
                    if message.contains("invalid TypeSafe score legend")
            ));
        }
    }

    #[test]
    fn capabilities_match_native_typesafe_and_retry_documented_rate_limits() {
        assert!(TYPESAFE_CAPABILITIES.supports(QuestionType::Choice));
        assert!(TYPESAFE_CAPABILITIES.supports(QuestionType::Score));
        assert!(TYPESAFE_CAPABILITIES.supports(QuestionType::Noul));
        assert!(TYPESAFE_CAPABILITIES.has_confidence);
        assert!(TYPESAFE_CAPABILITIES.has_usage);
        assert!(TYPESAFE_CAPABILITIES.is_retryable_status(429));
        assert!(TYPESAFE_CAPABILITIES.is_retryable_status(529));
        assert!(!TYPESAFE_CAPABILITIES.is_retryable_status(401));
        assert_eq!(
            TYPESAFE_OFFICIAL_BASE,
            "https://api.typesafe.ai/v1/systemone"
        );
    }
}
