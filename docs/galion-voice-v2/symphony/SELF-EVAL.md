# Symphony self-eval · Eureka A–G

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. t287. The teacher is a run of `scripts/symphony/self-eval.sh`, not this prose.

Demo = Spec = Test. The harness is the spec. A section stays **open** until a real run of its gate exits 0 and that run is written between the markers below. **PASS** is the only closed state. **OPEN** means the gate ran and the section is not done. An open section is not shipped. The script exits 1 while any row is open, and exits 2 if `LOCKS.toml` changes under it. This file does not edit `LOCKS.toml`.

CPU bar carried by gate G: `frame_ms = 80` (EPYC-class hosts still use that frame). The one-percent tick and the vCPU session budget are not rows A–G.

## Run

```bash
scripts/symphony/self-eval.sh
scripts/symphony/self-eval.sh --only G
scripts/symphony/gates/a.sh
```

`self-eval.sh` with no `--only` rewrites the matrix and the run log. A single letter does not.

## What closes a row

| ID | Section | The run that closes it |
|---|---|---|
| A | Compiler | A clone-to-Galion clip compiler, outside this harness, round-trips a Galion clip (`.gq` / `GALIONGQ`) and refuses Chatterbox raw as the primary runtime. The weight reader in `crates/galion-tensor/src/gq.rs` is not that compiler while it says reader-only. |
| B | Clone landing | `personas/refs/samantha.wav` sha256 `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` and `jarvis.wav` sha256 `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. No mp3 refs. A smoke-speak log names both hashes. The protocol stub may emit PCM and must refuse a reference clip; that stub is not the smoke speak. |
| C | Jarvis texture toggle | `personas/voice/jarvis.toml` has `[texture] preserve = true` (hub clicks, alarms, and ambience stay). `jarvis.wav` keeps a high-band ratio at least the clean-speech baseline `jarvis-clean-baseline.wav`. The synthetic checker in the gate is not the Jarvis recording. |
| D | Persona and emotion | Samantha, Jarvis, and Galion One docs have an emotion section and forbid scripted empathy. Samantha names `[laugh]` and `[chuckle]`. Both voice tomls map `e0` / `e1` prosody and wash. `voice_color` blends prosody. The feeling band sits on the 80 ms heartbeat. Canned replies do not speak the banned empathy lines. |
| E | One-dot | LOCKS stays one dot, chrome off. The phone page is one canvas, the mic records there, `/v2` redirects to `/`. A test outside this harness fires a workflow hook after auto-record. |
| F | Runbooks | `README.md` tells how to run the demo, and every `scripts/*.sh` it names is executable and passes `bash -n`. |
| G | 80 ms frame | LOCKS `frame_ms = 80`, `frame_hz = 12.5`, compute budget ≤ 60 ms. Heartbeat constants match. A late frame pauses. The streaming note names 80 ms. `scripts/eval/flash-ttfa.sh` stub-measures: no `ttfa_ms` unless a real Flash eval is asked for. |

## Matrix

<!-- SYMPHONY-MATRIX-START -->
Recorded 2026-11-03 10:51 CET. A row closes only when this run says PASS.

| ID | Section | Status | Evidence |
|---|---|---|---|
| A | Compiler | OPEN | .gq container is reader-only in galion-tensor; no clone-to-Galion clip compiler round-trip that refuses Chatterbox as the primary path |
| B | Clone landing | OPEN | protocol stub emitted 402 B PCM (not a ref speak); worker refused a reference clip; samantha.wav absent; jarvis.wav absent; no smoke-speak log names both ref sha256 values |
| C | Jarvis texture toggle | OPEN | checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle |
| D | Persona and emotion | PASS | persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines |
| E | One-dot | OPEN | no test shows speak, auto-record, then a workflow hook |
| F | Runbooks | PASS | bash -n ok for 8 README scripts: scripts/asr/fetch-vosk.sh, scripts/a21/fetch-a21.sh, scripts/run-v1-screen.sh, scripts/run-v1-demo.sh, scripts/eval/virtual-user.sh, scripts/check-phone-pipeline.sh, scripts/check-locks.sh, scripts/check-no-forbidden.sh |
| G | 80 ms frame | PASS | frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms |

Closed: D, F, G. Open: A, B, C, E.
<!-- SYMPHONY-MATRIX-END -->

## Last run

<!-- SYMPHONY-RUN-START -->
stamp: 2026-11-03 10:51 CET
locks_sha256: 6b69ffe258bffab8608566d808aefad4dba310d691aa97d1216c7844c0eb6397
command: scripts/symphony/self-eval.sh
A OPEN — .gq container is reader-only in galion-tensor; no clone-to-Galion clip compiler round-trip that refuses Chatterbox as the primary path
B OPEN — protocol stub emitted 402 B PCM (not a ref speak); worker refused a reference clip; samantha.wav absent; jarvis.wav absent; no smoke-speak log names both ref sha256 values
C OPEN — checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle
D PASS — persona docs, e0/e1 prosody and wash, voice_color, feeling band at 80 ms; canned replies have no scripted empathy lines
E OPEN — no test shows speak, auto-record, then a workflow hook
F PASS — bash -n ok for 8 README scripts: scripts/asr/fetch-vosk.sh, scripts/a21/fetch-a21.sh, scripts/run-v1-screen.sh, scripts/run-v1-demo.sh, scripts/eval/virtual-user.sh, scripts/check-phone-pipeline.sh, scripts/check-locks.sh, scripts/check-no-forbidden.sh
G PASS — frame_ms=80, 24 kHz, p95 budget <= 60 ms, late frame pauses; streaming doc names 80 ms; flash-ttfa stub-measures and prints no ttfa_ms
exit: 1
<!-- SYMPHONY-RUN-END -->

— Maciej Grajczyk
