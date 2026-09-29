# TypeSafe Official API Live Smoke

**Date:** 2026-09-29  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted verification record.  
**Scope:** One direct-provider request through the current source daemon. This does not verify the published `v0.1.0` binary, public service-entry routing, or the Standalone desktop UI.

## Result

The official endpoint accepted a Jev-native request using model `jev-latest` and Bearer authentication:

| Field | Result |
|---|---|
| Endpoint | `https://api.typesafe.ai/v1/systemone` |
| HTTP status | `200` |
| Request ID | `jev-1` |
| Response model | `jev-1.13.0` |
| Answers | `readiness:noul`, `status:choice`, `score:score` |
| Score legend | Parsed and validated as a string map matching the probability keys |
| Usage | 404 input tokens, 61 output tokens |
| End-to-end latency | 1,554 ms |

The synthetic state was “The deployment is ready for launch.” The response exercised Noul, Choice, and Score in one request. The account key was read from the user-provided local secret file into the test process environment; it was not printed, put in the temporary config, or written to repository files or daemon logs. The isolated test config/data directory was removed after the request.

## Boundary

This proves that the current TypeSafe adapter's endpoint, Bearer header, request body and response parsing work with the supplied account for this request. It does not validate every account model, public routing/failover, call-history UI, or the installed `v0.1.0` release. A separate desktop E2E must verify the Standalone app through its visible UI to a local TypeSafe-compatible service.
