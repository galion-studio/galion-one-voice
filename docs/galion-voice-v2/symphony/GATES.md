# Self-eval gates

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

One command writes the ledger and exits non-zero if any section check fails:

```bash
bash scripts/symphony/self-eval.sh
```

The ledger is `docs/galion-voice-v2/symphony/SELF-EVAL.md`. A section is complete only when its check exits 0 and the row says PASS. `SELF-EVAL PASS` on the first status line means every section check exited 0.

The frame bar alone, the same check section G runs:

```bash
bash galion-gates/sy-frame.sh
bash scripts/check-locks.sh
```

`galion-gates/run-gates.sh` calls `sy-frame.sh` as soft gate **SY**. SY counts toward the exit code only with `GALION_GATES_HARD=1`. `GALION_GATES_NO_SY=1` skips it. G0–G3 are unchanged.

## Cited gates

| Section | What the check cites | What this harness runs |
|---|---|---|
| A Compiler | `galion-gates/g5-gq-synthetic.sh` and `crates/galion-tensor/tests/gq_synthetic_g5.rs` | The script's SYNTHETIC-PASS contract and the `.gq` fixture on disk. Cargo G5 is not launched. Real-weights G5 stays PENDING, which is what that gate already prints. |
| B Clone | `scripts/clone/chatterbox_worker.py` and `crates/galion-ml-voice/src/speech/clone_worker.rs` | Protocol-stub PCM and the reference refusal (exit 3). The Rust test name `two_turns_reuse_one_clone_process_and_the_clock_is_not_a_product_claim` is the warning that a worker clock is not a product latency. |
| C Texture | Clip-store rule in [CLIP-STORE.md](CLIP-STORE.md) | No denoise step in the worker. Spectral compare only when both wavs exist. |
| D Persona | `scripts/eval/v2-persona-selftest.sh`, `docs/galion-voice-v2/EMPATHY.md`, `galion-emotion/src/express.rs` | Docs, prosody tables, `voice_color`, and the absence of scripted empathy lines in code. The HTTP half of the persona self-test is not started. |
| E One-dot | `scripts/run-v1-screen.sh` and `crates/galion-console/assets/v1-screen.html` | VAD → `endUtterance` → `postTurn` → `fetch("turn")`, and `/v2` redirect. No browser. |
| F Runbooks | This directory | Files, bash fences, `bash -n`, `scripts/one-ui.sh`, `scripts/check-locks.sh`. |
| G Latency | `crates/galion-heartbeat` `FRAME_MS` and `galion-gates/sy-frame.sh` | Stub `Heartbeat::step` timing, zero-alloc test, late-frame test. See [LATENCY.md](LATENCY.md). |

```bash
bash scripts/symphony/check-a-compiler.sh
bash scripts/symphony/check-b-clone.sh
bash scripts/symphony/check-c-texture.sh
bash scripts/symphony/check-d-persona.sh
bash scripts/symphony/check-e-one-dot.sh
bash scripts/symphony/check-f-runbooks.sh
bash scripts/symphony/check-g-latency.sh
```

Each script prints one `X PASS — …` or `X FAIL — …` line and exits 0 only on PASS.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
