# OpenRouter Adapter

**Date:** 2026-09-30  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted implementation and verification record.

## Protocol evidence

OpenRouter's official OpenAPI specification reports the production server as `https://openrouter.ai/api/v1`. The current documented operations used here are:

- `POST /chat/completions` for OpenAI-shaped chat requests.
- `GET /models` for the model catalog, returning `{ data: [...] }` entries with `id`.
- Bearer API-key authentication.
- JSON mode through `response_format: { "type": "json_object" }`.
- Retry-relevant statuses include 408, 429, 500, 502, 503, 504 and 529.

Source: OpenRouter official API reference and `https://openrouter.ai/openapi.json`, checked 2026-09-30 through the web-access workflow.

## Implementation

- Added `OpenRouterUpstream` at `rs/crates/jev-adapters/src/upstream_openrouter.rs`.
- The adapter translates a Jev request into a structured JSON decision prompt sent to `/chat/completions`.
- The assistant content is strictly parsed back into `JevResponse`; malformed content becomes `BadResponse`.
- Usage maps `prompt_tokens`, `completion_tokens` and `reasoning_tokens` into the Jev usage shape.
- Provider kind, default base, default model, model discovery and daemon kind validation include `openrouter`.
- The public Jev endpoint remains Jev-native; this does not add an OpenAI/Anthropic-compatible gateway entrance.

## Offline verification

- OpenRouter adapter success/usage/JSON answer mapping: passed.
- 429 retryability mapping: passed.
- Fenced JSON response tolerance: passed.
- Provider kind preset and model discovery path conversion: passed with the existing UI/daemon test suites.
- Rust workspace with `ts-rs`, UI tests/lint/build: passed.

## Remaining boundary

No live OpenRouter key was used in this implementation turn. The adapter is source-complete and wiremock-tested, but it is not part of the v0.1.0 Release until a controlled live call, capability matrix review, and the next Windows/Docker release gates complete.

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
