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

## Rebuilt after Phase 2/OpenRouter/theme changes

The current source was rebuilt on 2026-09-30. The new candidate identity is:

| Artifact | SHA-256 |
|---|---|
| `jev-switch-standalone-0.1.0-windows-x64.exe` | `6FDA1D523BE7CB7793BB86DA13DDBDE3310687F3254515F7261BE06DE93EB7FC` |
| `jev-switch_0.1.0_x64_en-US.msi` | `117EDC3F2A3C4812BD8D12FD43D7CC717CEC98A4C470F63DA815136B530C5EA8` |
| `jev-switch_0.1.0_x64-setup.exe` | `B62476F1B2F81646D749CEA73E0E6F18558D7DC3F03DB271F7D68391D0B4670C` |

The embedded daemon hash is `E438FE41A03C64DF1E79CE1F8A7BC1CB738BBED0A697A83F462F749699022CB0`. Resource validation/recovery and non-interactive shell tests passed. Launching this exact EXE was rejected by the active process policy in the current session, so this section does not claim health, WebView, model discovery, tray, or desktop UI runtime evidence for the new hash.

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理

## v0.2.0 exact candidate (2026-09-30)

The release script was rerun from the current `0.2.0` source after filtering stale bundle-directory artifacts by the version in `src-tauri/tauri.conf.json`. MSI, NSIS, Portable, and Standalone all came from this build input.

| Artifact | SHA-256 |
|---|---|
| `jev-switch-standalone-0.2.0-windows-x64.exe` | `6F47D42FE60CDBF3F2FA52D40D8013B367C73A1E11DB31F79BAFA0E94038C462` |
| `jev-switch_0.2.0_x64_en-US.msi` | `CE047E4D9BDC67D2F9A222AABA42CE336386A56E928B031C35404453C61FF306` |
| `jev-switch_0.2.0_x64-setup.exe` | `399C1C40EE6F551E3B7845165E1C82C2CE2BA3F177F037E1C7CC720A19F85349` |
| Portable `jev-switch.exe` | `5389AC1C2AFD3DFA6862D3E1E222D49A37041AB5F3393A92F95BA937AEE7741C` |
| Portable `jev-switch-daemon.exe` | `D4868FA3FF081EB7E21121E840ED69738C68BF9078AEA47A68A23A248144653E` |

The exact Standalone was launched from the build output. It extracted its embedded daemon to the versioned cache under `AppData\\Local\\Jev-Switch\\runtime\\0.2.0`, served the root UI with HTTP 200, served the matching production JavaScript with HTTP 200, and returned `/health` as `status=ok`, `product=jev-switch`, `version=0.2.0`, `api_revision=1`. The shell and daemon were then stopped; port `11435` was free afterward. This proves cold launch, resource extraction/reuse path, health identity, and static asset delivery for this exact hash. It does not replace the interactive matrix rows for WebView clicks, tray behavior, OpenRouter live credentials, Pages deployment, or multi-run shutdown timing.

The build script now selects installer artifacts by the configured package version, so stale artifacts left in the local bundle cache cannot be mistaken for the current release.

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
