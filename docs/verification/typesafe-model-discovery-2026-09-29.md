# Provider Model Discovery

**Date:** 2026-09-29  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted implementation and verification record.

## Contract

TypeSafe documents `GET /v1/models` as the account model directory. The endpoint uses the same host as `POST /v1/systemone` and requires the same Bearer API key. The current account returned two model IDs:

```text
jev-latest
jev-preview
```

The key was read from a protected local file for one live request; the command output contained only the count and model IDs, never the key.

## Jev-Switch implementation

The daemon now exposes:

- `POST /v1/admin/providers/{id}/models` for a saved provider. It derives the same-host `/v1/models` URL, adds the daemon-held key, parses TypeSafe `models` and OpenAI-style `data` arrays, and returns IDs, status and latency.
- `POST /v1/admin/providers/discover-models` for an unsaved form. The draft key is used only for that request and is never persisted or returned.

The Providers form offers “从上游获取模型”. A successful discovery fills the model list and reports HTTP status/latency. A failed discovery leaves manual values untouched. The browser never calls the upstream directly.

Offline daemon tests cover URL derivation, TypeSafe/OpenAI response shapes, de-duplication and Bearer forwarding. UI lint, build and tests pass after the feature.

## Boundary

The directory endpoint is a management-time connectivity/authentication check, not an inference health check. Providers that do not expose a compatible model catalog return a visible failure and can still use manually entered model IDs. The UI does not infer or fabricate IDs from the provider kind.
