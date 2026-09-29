# Runtime Telemetry Dashboard

**Date:** 2026-09-30  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted implementation and verification record.

## Delivered in Phase 1

- Added `GET /v1/admin/telemetry`, protected by the existing admin boundary.
- Added process-local session counters: request/response JSON bytes, current rates, active requests, total requests, success/failure, failover and average gateway latency.
- Added a 600-sample in-memory ring buffer for the Dashboard traffic chart.
- Added daemon process CPU and memory snapshots through `sysinfo`.
- Phase 2 boundary: adapter-attempt timing is now aggregated in-session as
  `upstream_attempts` and `avg_upstream_latency_ms`. Each real adapter
  attempt, including retries and failed attempts, contributes once. This is
  adapter processing time (transport plus parsing), not a pure network metric;
  it is not persisted and is not added to gateway latency.
- Added a generated Rust-to-TypeScript contract and a responsive Dashboard panel with 1 s/5 s refresh, pause, manual refresh, reduced-motion-compatible charting and expandable diagnostics.
- Added a focused HTTP regression proving the endpoint reports non-zero ingress/egress after a real in-process Jev request.

## Deliberate boundaries

“Ingress” and “egress” mean JSON body bytes observed at the gateway client boundary. They are session counters and reset when the daemon restarts; durable history, token statistics and upstream billing retain their own meanings. Request bodies, answers, keys and sensitive address fragments never enter telemetry.

The daemon currently reports its own process snapshot. Tauri WebView memory/CPU remains unavailable/null because no reliable cross-process measurement path has been added yet. The UI surfaces that state instead of inventing an estimate.

## Verification

- `cargo test --manifest-path rs/Cargo.toml --workspace --locked --offline`: passed.
- `cargo test --manifest-path rs/Cargo.toml --workspace --features ts-rs --offline`: passed; generated `ResourceSnapshot`, `TelemetrySample` and `TelemetrySnapshot` types are current.
- `cargo test --manifest-path rs/Cargo.toml -p jev-switch-daemon --test integration_http --offline telemetry_reports_session_counters_after_a_request`: passed.
- `npm test --prefix ui`: 24 passed.
- `npm run lint --prefix ui`: passed.
- `npm run build --prefix ui`: passed.

Phase 2 verification:

- `telemetry::tests::upstream_attempt_latency_is_averaged_separately`: passed.
- daemon workspace tests and ts-rs generation: passed; `avg_upstream_latency_ms` and `upstream_attempts` are present in the generated TypeScript contract.

This record does not claim a new Tauri WebView or real traffic screenshot; the desktop process was not started during this implementation turn.

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
