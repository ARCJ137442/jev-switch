# Windows Release Candidate Build

**Date:** 2026-09-29  
**Author:** GPT-6 Luna xhigh (OpenAI Codex)  
**AI disclosure:** AI-assisted build and verification record.

## Candidate identity

The current working tree was rebuilt with `scripts/build-windows-release.ps1`. The build completed and produced MSI, NSIS, folder Portable, and single-file Standalone artifacts for version `0.1.0`.

| Artifact | SHA-256 |
|---|---|
| `jev-switch-standalone-0.1.0-windows-x64.exe` | `D0862E1AEFB5B1642E5E5264DE632EFDBC040C99A4DB40E3378A229E98E314BC` |
| `jev-switch_0.1.0_x64_en-US.msi` | `0BF2B9EA7188980FDA71DA6A032B473616EA6E92E6C1509619F5B50370D56D71` |
| `jev-switch_0.1.0_x64-setup.exe` | `65C73726ADD97D045A040BF714808E7A7602E25EE1E83EF0719A8D0A270779A8` |
| Portable `jev-switch.exe` | `A2F755EED88A5EDF9C48D6BC819ECF6035BABFD2F175F350B15ABFD7794D04FE` |
| Portable `jev-switch-daemon.exe` | `F5D5449594B7F0C424A516781C2694AF94895793F27ADC41ECF26FD55908D7CB` |

The Portable `build-manifest.json` reports version `0.1.0`, matching the shell and daemon. It also records the same daemon/UI asset hashes and the matching MSI/NSIS inputs. The packaged UI contains the model discovery implementation; its current production asset is `assets/index-C1UT1VDX.js`.

## Offline checks

- UI tests: 24 passed.
- UI TypeScript lint: passed.
- UI production build: passed.
- Rust workspace tests: passed.
- Tauri shell tests: 10 passed, 6 interactive/native tests ignored by their explicit Windows-session requirements.
- Provider model discovery tests: TypeSafe `models` and OpenAI `data` response shapes, de-duplication, URL derivation and Bearer forwarding passed.

## Runtime boundary

The previous Standalone/LM Studio record remains the evidence for the older candidate identified there. This new candidate was not launched in this turn because the desktop process-launch command was rejected by the active execution policy. Therefore this record does **not** claim a new WebView, real UI button click, tray interaction, or live model discovery from this exact EXE. No old runtime is being presented as evidence for the new hash.

The exact-candidate launch and Providers “从上游获取模型” click remain a manual/runtime acceptance step. The expected official TypeSafe directory result is documented separately as `jev-latest` and `jev-preview` in [the model discovery record](typesafe-model-discovery-2026-09-29.md).

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
