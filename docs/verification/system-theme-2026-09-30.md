# System Theme Preference

**Date:** 2026-09-30  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted implementation and verification record.

> **Release status (2026-09-30):** `light/dark/system` shipped in v0.2.0 and remains in v0.4.0. This record's native OS screenshot boundary remains open; v0.4.0's automated tri-state checks are recorded separately, and the release has not received manual human acceptance.

## Delivered

- Theme preference now supports `light`, `dark` and `system`.
- The first frame resolves the OS preference before React renders.
- `matchMedia('(prefers-color-scheme: dark)')` updates the UI while `system` is selected.
- Manual light/dark selection remains stable across OS theme changes.
- The existing theme control cycles light -> dark -> system and exposes the current mode through its accessible label/title.
- Existing `jev_theme` light/dark values remain valid.

## Verification

- UI tests: 24 passed.
- UI TypeScript lint: passed.
- UI production build: passed.
- The change is included in the rebuilt Standalone candidate recorded in [the Windows candidate record](windows-release-candidate-2026-09-29.md).

Native OS theme switching and a new Tauri WebView screenshot remain runtime acceptance work; the current session could not launch the exact candidate because the process-launch policy rejected it.

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
