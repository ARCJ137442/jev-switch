# TypeSafe SystemOne Adapter Verification

**Date:** 2026-09-28  
**Scope:** Current source workspace; not included in the published v0.1.0 release.  
**Verification:** Local Rust tests with wiremock plus a separate local Jev-Switch → JevK5 GGUF bridge smoke. No official TypeSafe credential was configured; no cloud or billable API calls were made.

## Current Support

The daemon accepts provider `kind = "typesafe"`. The adapter posts the Jev request unchanged to the configured full endpoint URL. The official endpoint is `https://api.typesafe.ai/v1/systemone`; local endpoints can use the same adapter when they implement Jev `POST /v1/systemone`.

| Item | Implemented behavior |
|---|---|
| Method and path | `POST` to the exact configured `base` URL |
| Authentication | Adds `Authorization: Bearer <key>` when a non-empty key is configured; local compatible services can omit it |
| Request | Preserves `model`, `state`, and `questions`, including `choice`, `score`, and `noul` question types |
| Instructions | Accepts and preserves string, object, or array values |
| Criteria | Choice requires a record with string/object/array/null values; Score requires an ordered array with string/object/array levels; Noul criteria may be omitted or supplied as a record |
| Response | Reads Jev `answers` and optional `usage.input_tokens` / `usage.output_tokens` |
| Score answers | Requires numeric `score`, a `legend` object whose string descriptions cover exactly the probability-level keys, `probabilities`, and `confidence`; preserves the validated legend JSON |
| Retry | Retries the documented TypeSafe rate-limit statuses 429 and 529; also retains the transient statuses 408, 500, 502, 503, and 504 |
| Non-retryable example | 401 is returned without retry |

`legend` is kept as opaque JSON in the shared response type because other Jev-compatible providers may use different shapes; the native TypeSafe adapter validates the official map-of-string shape. It is available in the response type and serialized output; this adapter does not render the legend in a dedicated UI control. The numeric `score` remains `f64`. The official model account and its actual model IDs were not checked with a live request.

The shared Jev request type requires `instructions` but accepts its documented string/object/array forms. Choice and Score require their documented criteria containers; Noul criteria may be omitted and, when present, must be a record. `state` accepts any JSON value, and `questions` is a keyed object. These shapes are validated at Jev-Switch's public request boundary and passed unchanged to each selected upstream adapter.

## Configuration

Official TypeSafe API:

```toml
[providers.typesafe]
kind = "typesafe"
base = "https://api.typesafe.ai/v1/systemone"
api_key_env = "TYPESAFE_API_KEY"
enabled = true

[[routes]]
left = "jev-typesafe"
match = "exact"
right = "typesafe"
upstream_model = "jev-latest" # replace with a model ID available to the account
priority = 20
on_error = "next"
```

Local TypeSafe-compatible SystemOne:

```toml
[providers.local-typesafe]
kind = "typesafe"
base = "http://127.0.0.1:8000/v1/systemone"
enabled = true
```

Set `api_key` or `api_key_env` for a local service that requires authentication. The base must be the complete inference endpoint; Jev-Switch does not append or rewrite a path. Ordinary OpenAI or Anthropic chat-completions endpoints are not TypeSafe-compatible and are not adapted by this provider kind.

The existing provider `/probe` endpoint is a GET reachability check. It does not exercise this POST contract, validate Bearer authentication, or run inference. Use a separately authorized model request when live account validation is needed.

## Offline Evidence

The current source tests cover:

- Exact `POST /v1/systemone`, Bearer header, and complete native request body.
- Choice, Noul, and Score responses; TypeSafe Score `legend` object shape/key validation; snake-case token usage.
- A local compatible service with no authorization header.
- Registry retries for 429 and 529, with exactly two upstream requests before success.
- A 401 response that remains non-retryable and produces exactly one request.
- Daemon `build_upstreams` registration for a local TypeSafe-compatible endpoint with no API key.

The wiremock tests prove adapter transport and local retry behavior. They do not prove current official credentials, account model availability, provider UI guidance, or compatibility with arbitrary OpenAI-style local servers.

## Local JevK5 Integration Smoke

The adapter was exercised against the loopback JevK5 GGUF bridge using the isolated profile [`rs/providers.local-typesafe.example.toml`](../../rs/providers.local-typesafe.example.toml). The TypeSafe provider had no API key; Laya remained a separate `kind = "laya"` provider. Jev-Switch ran from the current source build on `127.0.0.1:11439` with an isolated SQLite directory under `E:\tmp`; the bridge served `http://127.0.0.1:8766/v1/systemone` and read logits from llama.cpp on `127.0.0.1:8080`.

On the same 80 human-labeled life-series articles:

| Public model | Selected provider/model on every trace | Success | Accuracy | Mean end-to-end |
|---|---|---:|---:|---:|
| `jevk5-local` | `jevk5` / `jevk5-9b-v0.3.3-q5` | 80/80 | 44/80 = 55.0% | 569.4 ms |
| `laya-local` | `laya` / `laya-router` (runtime backend `multilingual`) | 80/80 | 18/80 = 22.5% | 45.6 ms |

All 80 JevK5 traces and all 80 Laya traces matched their configured provider and upstream model. No key was sent. A multitype request through the JevK5 route returned Noul, Choice with a complete probability map, and Score with numeric value, confidence, probability map and legend; usage reported 406 input tokens and zero generated tokens for that request. The local gateway route trace recorded one upstream call. The exact JSON results live in the sibling experiment workspace: [JevK5 via Jev-Switch](../../../jev-decision-lab/life-series-benchmark/results/local-jevk5-gguf-switch-80.json), [Laya via Jev-Switch](../../../jev-decision-lab/life-series-benchmark/results/local-laya-router-switch-80.json).

Initial direct-runner and gateway outputs differed because Rust `BTreeMap` canonicalizes state/criteria keys while the first Python direct run preserved insertion order; model prompts are order-sensitive. After the benchmark canonicalized state and criteria in the same order, direct and gateway predictions and probabilities matched on all 80 rows for both upstreams. Mean gateway overhead over those canonical direct runs was 21.2 ms for JevK5 and 7.1 ms for Laya. Uncanonicalized result files remain historical and must not be used as a direct-versus-gateway comparison.

One initial live local request was correctly rejected because the experiment bridge sent fractional `latency_ms` while Jev-Switch expects an integer. The bridge now sends integer milliseconds; the multitype and 80-row route smokes above passed after that correction. The currently launched test daemon is an isolated binary, not the installed v0.1.0 application.

## References

- [TypeSafe API](https://docs.typesafe.ai/api)
- [Project protocol contract](../contracts/01-协议契约.md)
- [Provider compatibility history](../03-上游类别与协议兼容矩阵.md)
