# Awesome Jev submission draft

**Target:** [AbdelStark/awesome-typesafe-jev](https://github.com/AbdelStark/awesome-typesafe-jev)

**Suggested category:** Client libraries and integrations (verify the current category label when preparing the PR)

**Status:** Ready for post-release review; not submitted.

## Proposed one-line entry

> [Jev-Switch](https://github.com/ARCJ137442/jev-switch) — A Rust-and-React Jev gateway that routes typed `/v1/systemone` calls across configurable providers through an editable multi-hop DAG, with a Tauri desktop console, Docker deployment, and a side-by-side Playground.

## Review notes for the submission

- **Public project:** the repository is public and has a README, runnable source, CI, and a Windows release workflow. Add the final `v0.1.0` Release link here after the tag workflow succeeds: https://github.com/ARCJ137442/jev-switch/releases/tag/v0.1.0
- **License:** MIT OR Apache-2.0; see [`LICENSE-MIT`](../LICENSE-MIT) and [`LICENSE-APACHE`](../LICENSE-APACHE).
- **Jev use:** the gateway accepts Jev-native `POST /v1/systemone` requests and routes them to configured Jev-capable upstreams. The shipped adapters currently cover Vercel AI Gateway and Laya.
- **Data boundary:** `local` means the daemon runs on the user's machine and normally binds loopback; it can still send requests to a remote configured upstream. In local mode, provider configuration/credentials and call metadata are stored in local app data; gateway history does not persist request bodies or model answers. In cloud mode, caller requests reach the gateway host, which stores its metadata and forwards the request to the selected upstream.
- **Limitations:** this is a Jev gateway, not an OpenAI- or Anthropic-compatible chat API; built-in upstream adapters are limited to Vercel and Laya. The desktop release currently targets Windows; macOS/Linux desktop packaging, code signing, and an incremental updater are not included.
- **Screenshots:** the public-safe UI gallery is [`docs/screenshots.md`](../screenshots.md). The Providers screenshot remains private because it includes a masked credential fragment and a configured upstream address.

## Proposed pull request

**Title:** Add Jev-Switch gateway integration

Submit the one-line entry under the verified category after the `v0.1.0` Release page contains the final assets. Keep the upstream contribution to the list's requested concise format; use the README and screenshot gallery as supporting links rather than expanding the list entry into a project review.
