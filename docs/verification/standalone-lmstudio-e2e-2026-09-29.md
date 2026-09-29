# Standalone + LM Studio Desktop E2E

**Date:** 2026-09-29  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted verification record.  
**Candidate:** `jev-switch-standalone-0.1.0-windows-x64.exe`  
**SHA-256:** `97958B1252CCD37488B0F72C40ECBE736E233CA26A22985D3B3D5294F5BA7381`

## What passed

The single EXE was started directly from the build output. It released the embedded daemon/UI into the versioned cache below and started the daemon without a user-installed sidecar:

```text
%LOCALAPPDATA%\Jev-Switch\runtime\0.1.0\886ee32d6089623ee146833de642c722a65874f3752a56efdc3e41f119eaac28
```

`GET http://127.0.0.1:11435/health` returned `product=jev-switch`, `version=0.1.0`, `api_revision=1`. Existing AppData configuration was readable in the same run: the UI displayed the existing providers and seven public entries before the temporary local test entry was added. The cache tests also covered cold extraction, identical-cache reuse, byte corruption repair, missing required assets, embedded asset validation and path traversal rejection.

The current UI was inspected from the Standalone-served page. The primary navigation contained five hash routes in this order:

```text
dashboard → providers → endpoints → routing → playground
```

The Routing page directly rendered its DAG and did not render the removed secondary tabs. The native WebView regression test passed across the five routes, three logical window sizes (`760x480`, `900x560`, `1280x720`), light/dark/restored themes, full-width header alignment and document overflow checks:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked -- --ignored native_webview_routes_the_five_pages_and_measures_responsive_layout
# 1 passed
cargo test --manifest-path src-tauri/Cargo.toml --locked -- --ignored tray_dispatch_shows_hides_and_quits_an_isolated_window
# 1 passed
```

## Fresh local-model request

The temporary provider and public entry were configured through the visible Providers/Entries pages:

```text
provider: typesafe-lmstudio
kind:     typesafe
base:     http://127.0.0.1:8783/v1/systemone
model:    jevk5-4b-session
entry:    jevk5-lmstudio
```

LM Studio loaded `jevk5@q4_k_m` with GPU offload 100%, context 4096, parallel 1. The existing non-CUDA Python bridge then served `127.0.0.1:8783`; no Python-CUDA path was used. The request was launched from the Playground UI and completed successfully:

| Field | Result |
|---|---|
| Request ID | `jev-51` |
| Status | `200` |
| Selected provider | `typesafe-lmstudio` |
| Selected model | `jevk5-4b-session` |
| Route trace kind | `direct_upstream` |
| Selected hops | `typesafe-lmstudio -> jevk5-4b-session` |
| Upstream calls | `1` |
| Gateway latency | `517 ms` |
| Upstream latency | `515 ms` |
| Usage | `201 input / 0 output` |

The same `jev-51` record was read back from `/v1/admin/events`; its route trace and usage matched the Playground result. The earlier failed request `jev-50` was also preserved as a `503` trace when LM Studio's TTL had unloaded the model, which demonstrates an observable failure path rather than a silent success claim.

The maintainer then repeated the same flow manually from the local-service console. The console auto-detected the loaded `jevk5@q4_k_m` instance, started the bridge, and the Playground request `jev-57` completed with HTTP 200:

| Field | Result |
|---|---|
| Public entry | `jevk5-lmstudio` |
| Provider/model | `typesafe-lmstudio / jevk5-4b-session` |
| Request ID | `jev-57` |
| Strategy | `failover` |
| Upstream calls | `1` |
| Gateway/upstream latency | `373 ms / 372 ms` |
| Usage | `157 input / 0 output` |
| Answer | Noul `1` |

The event history contains the same `jev-57` route trace with `status=200`, `selected_provider=typesafe-lmstudio`, and `selected_model=jevk5-4b-session`. The bridge reports its model as `jevk5@q4_k_m`; its one-hot confidence is intentionally marked uncalibrated.

## Cleanup and residual boundary

After the successful request, the bridge was stopped and LM Studio reported no loaded models. Memory returned to about 60% used. The Standalone shell/daemon remains running for inspection, and the temporary provider/entry remain in AppData so the trace and configuration can be reviewed before cleanup.

The isolated tray lifecycle test covers show/hide/quit dispatch, but the actual OS tray icon click and a second real desktop double-click were not automated: the candidate has no MCP bridge, and the available approval policy rejected a second process-launch attempt. These two manual interactions remain the only desktop acceptance gaps; no claim of actual tray-click or duplicate-launch focus was made.

## Later rebuild

The source was rebuilt after the model discovery implementation. The resulting Standalone candidate is recorded separately in [Windows Release Candidate Build](windows-release-candidate-2026-09-29.md), with SHA-256 `D0862E1AEFB5B1642E5E5264DE632EFDBC040C99A4DB40E3378A229E98E314BC`. The runtime evidence above remains tied to the candidate hash written at the top of this document; it is not silently reattributed to the later build.
