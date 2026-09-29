# Agent instructions

- Read and follow [`CLAUDE.md`](CLAUDE.md) for project boundaries, commands, and current documentation entry points.
- Use [`docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md`](docs/design/ENDPOINT-GATEWAY-ALIGNMENT-PLAN.md) for current decisions and implementation state.
- Use [`ROADMAP.md`](ROADMAP.md) for future directions only; roadmap items are not current release promises.
- Use [`docs/design/ROADMAP-EXECUTION-PLANS.md`](docs/design/ROADMAP-EXECUTION-PLANS.md) for current-code execution breakdowns; these plans are not release promises.
- Treat old progress and verification records as evidence for their stated revision and scope, never as current build status.
- Keep current capability claims aligned with the source: built-in upstream adapters are Vercel, Laya, TypeSafe SystemOne, and OpenRouter. OpenRouter is an upstream translation adapter, not a general OpenAI/Anthropic gateway entrance. TypeSafe local use requires a SystemOne-compatible HTTP endpoint.
- Keep the GitHub About description bilingual in the format `English sentence | 中文一句话介绍`; verify it against the current product positioning during release/documentation maintenance.
- Preserve unrelated user changes. Do not commit, push, publish, or change GitHub settings without explicit authorization.
