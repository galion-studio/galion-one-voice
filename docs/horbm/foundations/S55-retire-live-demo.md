# S55 — retire galion-live-demo

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S55. Archive the heartbeat live-demo crate. |
| Written | 2026-11-25. No weight download, no voice file, no paid API, no merge. |
| Base | `main` `5d14d7b6c589884691f1b9e001f00f7664a219ea` |
| Step | First-principles audit step 8 (`docs/horbm/foundations/FP-first-principles-audit.md`). |
| Persona | Samantha only. Aria is not loaded. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. Ear prosody stays on (`GALION_EAR_SENSE` is the off switch). |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## What step 8 required

Retire `galion-live-demo` after `scripts/run-live-demo.sh` and `galion-gates/lv-live-demo.sh` no longer build it. `galion-heartbeat` stays.

The crate was a root workspace member. `cargo test --workspace` in `.github/workflows/gates.yml` compiled it for that reason. No workflow file named the package. Nothing else depended on it by path.

## Where a live loop already is

The duplex contract on main is `horbm-duplex`: one `Duplex::tick`, 80 ms, 16 kHz, Samantha, tone mouth unless the optional `piper` feature is turned on. CI already runs it.

```bash
cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex
```

The operator smoke is the same script CI calls from `.github/workflows/voice-stack-horbm-router.yml`:

```bash
bash scripts/horbm-e2e-smoke.sh
```

`horbm-e2e` is its own manifest (`horbm/crates/horbm-e2e/Cargo.toml`). It is not a member of the `horbm` workspace. The device binary, off in CI, is already documented in `docs/horbm/foundations/F-duplex-s28.md`:

```bash
cargo run --manifest-path horbm/Cargo.toml -p horbm-duplex --features live --bin horbm-duplex-live
```

This change does not add a crate, a feature, or a second loop.

## Call sites

| Site | Before | This change |
|---|---|---|
| Root `Cargo.toml` members | listed `crates/galion-live-demo` | member removed. `Cargo.lock` drops that package only. |
| `scripts/run-live-demo.sh` | `cargo build -p galion-live-demo` and ran the binary, including `--showcase` WAVs | runs `cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex`. `--e2e` execs `scripts/horbm-e2e-smoke.sh`. Old flags parse and do not select a binary. Last line is `GALION_LIVE_DEMO_OUT=archived`. |
| `galion-gates/lv-live-demo.sh` | called the script and required showcase WAVs | calls the same script and requires `live-demo: PASS duplex horbm-duplex`. Default timeout 600s. |
| `galion-gates/run-gates.sh` | soft-calls LV | still soft-calls LV. Comment now names duplex. |
| `galion-gates/v1-demo.sh` | `scripts/run-v1-demo.sh --headless` | same call. Default timeout 900s so a cold duplex build fits. |
| `scripts/run-v1-demo.sh` | built `galion-live-demo --features dot-viz-web` and could exec the binary | headless and `--check` run the duplex script. The web feed is not started. Last line `GALION_V1_SCREEN=archived`. |
| `scripts/v1/soft-checks.sh` | C1–C3 required showcase WAVs and a Rika capture; C12 required `voice=reply-wav`; brand grep scanned `crates/galion-live-demo` | C1 passes on the duplex line in `live.log`. C2, C3, and C12 are INFO: those captures belonged to the archived binary. Lean-in and workplace deny stay in `galion-heartbeat` tests. Brand grep scans the archive path. `SB` is set before use so a missing cascade JSON does not abort the checker under `set -u`. |
| `scripts/eval/symphony-heartbeat.sh` | `cargo test -p galion-live-demo --features dot-viz --test live` | `cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex`. Verdict token is `duplex=`. |
| `scripts/run-en-cascade-demo.sh` | comment said the live-demo script should read its WAVs | comment says the cascade WAVs stay here and the live loop does not read them. The script still builds the cascade lane. |
| `scripts/symphony/check-f-runbooks.sh` | requires the string `scripts/run-live-demo.sh` in the symphony docs, then `bash -n` on the file | not edited. The file remains, so the needle still resolves. |
| `.github/workflows/gates.yml` | `cargo test --workspace` built the member | workflow file not edited. The member is gone, so that step no longer builds the crate. |
| `.github/workflows/voice-stack-horbm-router.yml` | already tests `horbm-duplex` (default and `piper`) and runs `scripts/horbm-e2e-smoke.sh` | not edited. |

`crates/galion-console/src/workflow.rs` still returns `Next::Program("live-demo")` for the words "run the demo". That is a label in the console word table. It does not build this crate. The console crate was not edited.

## What the archived demo did

The crate cut a you-band into 24 kHz frames of 1920 samples and called `Heartbeat::step` with `RikaReplayBackend`. `--showcase` wrote `default`, `opener`, and `workplace` stereo WAVs plus `console_capture.txt`, and checked workplace hard-off and a 160 ms lean-in. Optional features served a one-dot page. `LIVE_EN_CASCADE` could feed the cascade WAV.

`horbm-duplex` and `horbm-e2e` do not write that showcase tree, do not replay Rika, and do not serve that page. Their clock is 16 kHz, 1280 samples, `Duplex::tick`. Workplace deny and the 160 ms lean-in remain tests in `galion-heartbeat`, which stays a workspace member. The one-dot page without a live feed remains `scripts/run-v1-screen.sh`.

## Archive

Moved to `archive/2026-11-25/crates/galion-live-demo`, same layout as `archive/2026-11-24/`. `ARCHIVED.md` is the note. The manifest still has `version.workspace = true` and the old sibling path dependencies. Cargo does not build it. It is not a root member. Do not point those paths back at the live crates to put `Heartbeat::step` on the HORBM loop.

## Files this change edits

- `archive/2026-11-25/crates/galion-live-demo/` (moved crate, `ARCHIVED.md`, archive comment on `Cargo.toml`)
- `Cargo.toml` members line (the package had to leave the workspace)
- `Cargo.lock` (that package block only)
- `scripts/run-live-demo.sh`
- `scripts/run-v1-demo.sh`
- `scripts/run-en-cascade-demo.sh`
- `scripts/eval/symphony-heartbeat.sh`
- `scripts/v1/soft-checks.sh`
- `galion-gates/lv-live-demo.sh`
- `galion-gates/run-gates.sh`
- `galion-gates/v1-demo.sh`
- this note

Not edited: `LEDGER.md`, `LOCKS.toml`, `SOUL.md`, `.github/workflows/*`, `horbm/**`, `voice-stack/**`, `crates/galion-ml-voice`, `crates/galion-cascade`, `crates/galion-heartbeat`, `crates/galion-student-backend`.

Historical notes that still name the old binary were left as history. The commands they name now run the duplex tests. The notes are `docs/LIVE-DEMO.md`, `docs/V1-DEMO.md`, `docs/agent-wave21/*`, `docs/agent-v1-demo/*`, `docs/galion-voice-v2/symphony/{80MS,HEARTBEAT,HEARTBEAT-WIRE,LOCAL-DEMO,README,RUNBOOKS,ONE-DOT-WORKFLOW}.md`, `docs/multilingual/PROTOTYPE.md`, `docs/galion-context/2026-11-08/10-galion-os-boot-sounds.md`, `docs/horbm/foundations/FP-first-principles-audit.md` (step 8 is the instruction this change carries out), and `docs/horbm/foundations/F5-conductor-deep-dive.md`.

## Tests

Host: rustc 1.85.0 (`4d91de4e4` 2025-02-17), Linux x86_64. `horbm` is a virtual workspace, so those lines pass `--workspace`.

| Command | Result |
|---|---|
| `cargo test --workspace` | 1059 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 248 passed, 0 failed, 1 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `bash scripts/run-live-demo.sh --showcase` | exit 0. Last line `GALION_LIVE_DEMO_OUT=archived` |

The root total does not include the archived crate. The log of that root run does not compile `galion-live-demo`.

## Linux

The default of `scripts/run-live-demo.sh` is `cargo test -p horbm-duplex` with default features. That test does not open a device. The `live` feature (cpal, ALSA on Linux) stays off. `--e2e` is `scripts/horbm-e2e-smoke.sh`, which this repo already ships: on Linux it can use an ALSA null PCM, libflite, and Silero, and its header says the pinned libvosk zip is Linux x86_64 only. This change does not add that behavior. No `.exe`, no drive letter, no new address, no new dependency.
