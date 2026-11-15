# Swarm 10 — brain-eureka self-eval (D3)

**Written:** 2026-11-15 22:09 +01:00 (Europe/Zagreb)
**Role:** self-eval. Lane D, backlog D3. Plan: [PR #152](https://github.com/galion-studio/galion-one-voice/pull/152) (`docs/swarm-10/2026-11-15/brain-eureka/PLAN.md`). This file does not edit that plan.
**Owner:** Maciej Grajczyk. Copyright line stays Maciej Grajczyk.
**Base:** `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`
**Host:** `cursor`, Linux 6.12.113+ x86_64, 4 CPUs, `free -h` total 15 GiB. This is not `grok-bot-vm-511777615`. NEXT-ITERATION.md was read and was not committed. Its clocks are not copied into the ledger.

## What was checked

`bash scripts/symphony/self-eval.sh` on this tip, 2026-11-15 22:08 +01:00. The script rewrote the matrix and the last-run block in `docs/galion-voice-v2/symphony/SELF-EVAL.md`. Exit **1**. `LOCKS.toml` sha256 before and after: `b97fa057f6136d724f1283bdeb77d9d175a08a0245177ad85ae8e94ca3040f59`. Exit was not 2.

Script lines:

```text
A PASS — compiler witness crates/galion-ml-voice/src/artifact.rs round-trips a Galion clip and refuses Chatterbox as primary
B OPEN — protocol stub emitted 402 B PCM (not a ref speak); worker refused a reference clip; samantha.wav absent; jarvis.wav absent; no smoke-speak log names both ref sha256 values
C OPEN — checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle
D PASS — persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines
E PASS — one / surface, mic records on that page, workflow hook in crates/galion-console/src/auto_workflow.rs
F OPEN — README has no run-the-demo section
G PASS — frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms
== symphony A–G: 4 PASS, 3 OPEN, exit 1
```

`scripts/check-locks.sh` printed `OK: all Eureka locks hold (LOCKS.toml)`, exit 0.
`scripts/check-no-forbidden.sh` printed `OK: forbidden-format + forbidden-runtime gates pass`, exit 0.

Ref directory on this host: `samantha.placeholder` and `jarvis.placeholder` only. `weights/clone` is absent. `gqvoice` is absent. `python3 -c 'import torch'` raised `ModuleNotFoundError`. No WAV was hashed and none was committed.

## B and C

Both stay **OPEN/BLOCKED on A0**. They are not closed.

B. The wavs are absent, so the original t273 digests were not measured here: Samantha `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693`, Jarvis `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. The 402 B PCM is the protocol stub (`GALION_CLONE_STUB_PCM=1`). That is not a smoke speak. Piper was not run as a stand-in.

Disagreement, not resolved: `scripts/symphony/eval.py` still compares the ref paths to the t623u pins `e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641` and `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6` (the active rows in `scripts/clone/REFS.sha256`). The SELF-EVAL "What closes a row" table, `CLONE-LANDING.md`, and the symphony README name the t273 digests. The hash constants, `REFS.sha256`, and `LOCKS.toml` were not edited. This run did not PASS B. A t623u match, a skip, or the stub would not be committed as PASS.

C. The gate's synthetic tone self-test passed and is not the original Jarvis clip. `jarvis.toml` has no `[texture] preserve` table. The texture key on this tip is `texture = "preserve-hub"` under `[reference]` (read, not edited). `jarvis.wav` and `jarvis-clean-baseline.wav` are absent, so no spectral compare ran. `GALION_JARVIS_TEXTURE` was not measured. Lane A3 owns the texture policy. This PR does not add a `[texture]` table.

## F

F moved from the 2026-11-06 17:18 PASS to OPEN. README's heading is `## Build and run` since `1f2e196` (2026-11-06 17:39 +01:00). `gate_f` requires the string `## Run the demo` and returns before `bash -n`. `bash -n` on the README scripts was not run. The README was not retitled to close the row.

## What changed

- Ledger matrix and last-run block, from the script, plus the D3 note under that block.
- This notes file.

No other lane's notes. No `PLAN.md`. No `SOUL.md`, `config.yaml`, `LOCKS.toml`, or `REFS.sha256`. No weights, no audio.

## Tests

| Command | Result |
|---|---|
| `bash scripts/symphony/self-eval.sh` | exit 1 (B, C, F OPEN). Expected while those rows are open. |
| `scripts/check-locks.sh` | exit 0 |
| `scripts/check-no-forbidden.sh` | exit 0 |
| `cargo test --workspace` | not run |
| `cargo test -p galion-ml-voice --features speech` | not run |
| `scripts/eval/virtual-user.sh` | not run |
| `scripts/eval/symphony-heartbeat.sh` | not run (lane B) |
| Presence replay | not run |

rustc on this host is 1.83.0 (`90b35a623`, 2024-11-26), cargo 1.83.0. Crate `rust-version` is 1.85. No toolchain file was added. The seven `galion-ml-voice` failures in NEXT-ITERATION were on the other host and were not re-run here. They stay lane A5.

A, D, E, and G PASS are the script's source and docs checks on this tip. Not a live clone, not a spoken turn, not a product TTFA. Product TTFA was not measured.

## CI

Not run at the time this note was written. GitHub `gates`, `emotion-v2`, and `one-desktop` start on the pull request. `scripts/symphony/self-eval.sh` is not one of those jobs. A later update of this note should quote the check names and conclusions from the head that is marked ready, or say they were still pending.

## Open questions

- A0 is still Maciej's. Until the original refs are the ones a smoke speak logged, B and C stay open.
- Whether `gate_f` should accept `## Build and run` is not decided here.

## Decisions for Maciej

1. A0 stays BLOCKED-OWNER. This PR does not re-open t273 and does not re-state B and C onto the t623u pins.
2. B and C are OPEN/BLOCKED on this host. Placeholders only. No original-ref smoke, no Jarvis spectral compare.
3. F is OPEN because the gate string and the README heading differ. Left as the script printed it.
4. `cargo test --workspace` was not run on this worker. Do not read the 2026-11-06 cargo-absent BLOCKED rows, or the other host's 842/7/5 total, as a result from this machine.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
