# Runbooks

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Every environment the symphony uses is started from the repo root. Rust 1.85 or newer. The short command list is the index section "How to run the environments". This page is the map from an environment to its script, its output, and its longer note.

The screen and the demos bind loopback. Staging is the one place a public name appears, and it is [STAGING.md](STAGING.md).

| Environment | Start | Output | Longer note |
|---|---|---|---|
| One-dot screen | `scripts/run-v1-screen.sh` | `http://127.0.0.1:7878/` | [ONE-DOT.md](ONE-DOT.md), [V1-DEMO.md](../../V1-DEMO.md) |
| Screen self-check | `scripts/run-v1-screen.sh --check` | exit 0 or 1 | same |
| One-UI note | `scripts/one-ui.sh` | prints the note, exit 0 | [ONE-UI.md](../ONE-UI.md) |
| V1 demo, headless | `scripts/run-v1-demo.sh --headless` | `galion-demo/out/v1/V1-SUMMARY.txt` | [V1-DEMO.md](../../V1-DEMO.md) |
| V1 demo, one live loop | `scripts/run-v1-demo.sh --check` | last line `GALION_V1_SCREEN=` | same |
| Heartbeat live | `scripts/run-live-demo.sh` | last line `GALION_LIVE_DEMO_OUT=` | [HEARTBEAT.md](HEARTBEAT.md), [LIVE-DEMO.md](../../LIVE-DEMO.md) |
| Heartbeat showcase | `scripts/run-live-demo.sh --showcase` | three variants under the out dir | same |
| Presence bin | `cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence` | spec report on stdout | [HEARTBEAT.md](HEARTBEAT.md) |
| Emotion tests | `cargo test --offline -p galion-emotion` | cargo summary | [SAMANTHA-EMOTIONS.md](SAMANTHA-EMOTIONS.md) |
| EN speech turn | `scripts/run-en-cascade-demo.sh --text "hello galion"` | `GALION_EN_CASCADE_WAV=` | [V1-DEMO.md](../../V1-DEMO.md) |
| Virtual user | `scripts/eval/virtual-user.sh` | `galion-gates/results/presence-eval/latest.json` | [SELF-EVAL.md](SELF-EVAL.md) |
| Persona self-test | `BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh` | curls `/v2` and `/v2/say` | [PERSONA.md](PERSONA.md) |
| Phone smoke | `scripts/check-phone-pipeline.sh` | exit 0 or 1 | [STAGING.md](STAGING.md) |
| Vosk model | `scripts/asr/fetch-vosk.sh` | `weights/asr/vosk-model-small-en-us-0.15` | [VOSK.md](../../asr/VOSK.md) |
| Piper pack | `scripts/a21/fetch-a21.sh` | `weights/` (sha256-pinned) | [V1-DEMO.md](../../V1-DEMO.md) |
| Clone weights | `GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh` | `weights/clone/` | [CLONE.md](CLONE.md) |
| TTFA harness | `scripts/eval/flash-ttfa.sh` | one JSON line, `measured: false` | [TTFA.md](TTFA.md) |
| Staging tunnel | see [STAGING.md](STAGING.md) | an `https://` name | [PLAN-V5.md](../../ops/PLAN-V5.md) |
| Deploy dry-run | `scripts/deploy/harden-dryrun.sh` | printed steps, no connection | [STAGING.md](STAGING.md) |
| Workspace tests | `cargo test --offline --workspace` | cargo summary | Rust 1.85 |

Exit codes for the V1 demo: `0` ok, `1` check failed, `64` bad argument, `69` port busy, `124` timeout. The virtual user uses `0` green, `1` a scored case failed, `2` usage or IO.

`V1_SCREEN_ADDR` defaults to `127.0.0.1:7878`. `V1_SCREEN_SECONDS` defaults to 3600. `GALION_LLM` defaults to `canned` on the screen. `GALION_ASR` is `vosk` when the small English model is on disk, and `stub` when it is not. `GALION_SPEECH=auto` uses Piper when the pack is present, otherwise flite.

## Gate

**PASS** when each command in the table has been run on a clean checkout and the output column matches, or the row in [SELF-EVAL.md](SELF-EVAL.md) says **BLOCKED** with the missing piece named.

**FAIL** when a command exits outside its documented codes, or when a script binds a public address.

**BLOCKED** when Rust, a model file, or a speaker tool the command needs is absent. Say which one.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
