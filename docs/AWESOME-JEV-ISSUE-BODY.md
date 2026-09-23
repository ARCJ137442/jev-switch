**Project:** https://github.com/ARCJ137442/jev-switch (Rust + TypeScript, MIT)

**Suggested section:** Tools & Infrastructure

**Relationship:** I'm the author (ARCJ137442)

**What Jev-Switch does:** A local multi-upstream router for Jev protocol — native Jev entry point × upstream switching × bridging. Rust axum backend + React control panel, targeting both local and cloud deployment modes.

**Core positioning:** Lightweight, local/cloud dual-compatible, fast configuration & highly integratable Jev router. Focuses on performance and user-friendliness (UI & config fluency). Fills the niche for "lightweight Jev routing" as large relay stations become bloated.

**Key Features:**

1. **Visual Routing Configuration** — Circuit board metaphor: drag connections between API entry points (left) and upstream provider models (right), just like soldering wires on a circuit board
2. **Hot Mode Switching** — Switch between `local` (loopback-only, password-free) and `cloud` (0.0.0.0, token-based) modes without restarting
3. **Heterogeneous Upstream Support** — Route across Vercel AI Gateway, local Laya daemon, or any future Jev-compatible provider
4. **Real-time Dashboard** — Status overview, provider health monitoring, route summary
5. **Token-based Access Control** — Single admin + API token hierarchy for team sharing

**Primitives:** All Jev SystemOne primitives (Choice, Score, Noul) via upstream protocol translation

**Tech Stack:**
- Backend: Rust (axum 0.7, jev-protocol, jev-core)
- Frontend: React + TypeScript (Vite, shadcn-inspired design tokens)
- Deployment: Standalone binary + Docker (auto-build on tag)

**Screenshots:**

**Dashboard Page** — Status overview with mode/bind display, provider health cards, route summary
![Dashboard](https://raw.githubusercontent.com/ARCJ137442/jev-switch/main/docs/design/previews/dashboard-screenshot.png)

**Routing Page** — Visual circuit board: drag to connect models to upstream endpoints
![Routing](https://raw.githubusercontent.com/ARCJ137442/jev-switch/main/docs/design/previews/routing-screenshot.png)

**Providers Page** — Manage upstream providers, probe health, configure API keys
![Providers](https://raw.githubusercontent.com/ARCJ137442/jev-switch/main/docs/design/previews/providers-screenshot.png)

**Playground Page** — Test Jev calls interactively with form/JSON input
![Playground](https://raw.githubusercontent.com/ARCJ137442/jev-switch/main/docs/design/previews/playground-screenshot.png)

**Architecture:**
- `jev-protocol` crate: Jev protocol types & validation
- `jev-core` crate: Router + upstream trait + protocol translation
- `jev-adapters` crate: Laya/Vercel adapters
- `jev-switch-daemon` crate: axum HTTP server + admin API

**Runbook:**
```bash
# Start backend
export JEV_SWITCH_CONFIG=rs/providers.example.toml
cargo run --manifest-path rs/Cargo.toml
# → Listening on 127.0.0.1:11435

# Frontend (optional, daemon serves built UI from ui/dist)
cd ui && npm run dev
```

**Jev call site:**
- Entry: `POST /v1/systemone` (contracts/01-protocol.md)
- Routing: `jev-core/src/router.rs` → weighted upstream selection → protocol translation
- Failover: `on_error=next` cascades through priority-sorted upstreams

**Verification:** All 15 backend tests pass. Full E2E verification report: [docs/E2E-VERIFICATION-REPORT.md](https://github.com/ARCJ137442/jev-switch/blob/main/docs/E2E-VERIFICATION-REPORT.md)

**One-sentence summary:** A lightweight local/cloud Jev router with visual circuit-board routing config, hot mode switching, and heterogeneous upstream bridging — Rust backend + React UI, fills the niche between bloated relay stations and DIY scripts.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
