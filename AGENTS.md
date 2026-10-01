# Agent instructions

- Read and follow [`CLAUDE.md`](CLAUDE.md) for project boundaries, commands, and current documentation entry points.
- Use [`docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md`](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) for current decisions and implementation state.
- Use [`ROADMAP.md`](ROADMAP.md) for future directions only; roadmap items are not current release promises.
- Use [`docs/design/ROADMAP-EXECUTION-PLANS.md`](docs/design/ROADMAP-EXECUTION-PLANS.md) for current-code execution breakdowns; these plans are not release promises.
- Use [`docs/design/USER-FEEDBACK-ITERATION-PLAN-NEXT.md`](docs/design/USER-FEEDBACK-ITERATION-PLAN-NEXT.md) as the next user-feedback implementation checklist; update its status when work begins, and do not treat it as current capability or a release promise.
- Treat old progress and verification records as evidence for their stated revision and scope, never as current build status.
- Keep current capability claims aligned with the source: built-in upstream adapters are Vercel, Laya, TypeSafe SystemOne, and OpenRouter. OpenRouter is an upstream translation adapter, not a general OpenAI/Anthropic gateway entrance. TypeSafe local use requires a SystemOne-compatible HTTP endpoint.
- Keep the GitHub About description bilingual in the format `English sentence | 中文一句话介绍`; verify it against the current product positioning during release/documentation maintenance.
- Treat Android as a supported APK distribution channel for ordinary mobile users without Termux, with lower maintenance priority than Windows desktop. The APK must contain an Android-buildable daemon core, start with the gateway off, and expose explicit in-app start/stop. Use one app-specific stable release keystore across builds; never reuse another app's key. Keep service, notification, tile, network, touch, and lifecycle claims tied to their actual device evidence. See `docs/design/ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md`.
- Preserve unrelated user changes. Do not commit, push, publish, or change GitHub settings without explicit authorization.
- v0.4.0 desktop core paths (portable/Standalone, tray, theme, layout, data restoration and official TypeSafe call) were manually accepted by the maintainer; installer, stable Android release signing and full-chain performance remain unverified.
- v0.6.0 adds cross-platform LAN opt-in, gateway service control, JSON settings backup, structured statistics pagination, Playground selection, stable DAG layout, correlated route activity visuals and Android keepalive source integration. Keep native Tauri/Android lifecycle evidence separate from automated tests.
