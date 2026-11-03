# Self-eval

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

This file is the ledger. A section is complete when its gate is **PASS** and the row below has been filled in. The blank table in the index is the form. Write the result here.

Demo, spec, test, and teacher are the same artifact. The model does not grade itself without a human-calibrated anchor. A missing support is an honest gap, and that honesty is a success. Scores are numbers in this file. They are not a mood.

Three loops, from the Eureka note:

| Loop | When | What is scored |
|---|---|---|
| Inner | each turn | confidence and groundedness of that turn |
| Middle | each session | the session held together |
| Outer | each release | the gates in the table below |

## Harnesses already in the tree

```bash
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
scripts/eval/virtual-user.sh
scripts/eval/flash-ttfa.sh
BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
scripts/run-v1-screen.sh --check
```

Presence reports land in `galion-gates/results/presence-eval/` (gitignored). The flash harness prints `measured: false` unless `GALION_FLASH_EVAL=1`. A protocol-stub clock is evidence about the pipe. It is a product TTFA only when `engine` is a model. See [TTFA.md](TTFA.md) and [FLASH-EVAL.md](../FLASH-EVAL.md).

## Ledger

Replace `PASS / FAIL / BLOCKED` with one token. Evidence is a path, a command, or a hash. When is Europe/Zagreb.

| Section | Gate | Result | Evidence | When |
|---|---|---|---|---|
| Compiler | Round-trip fixture. The Rust path opens the Galion voice file. | PASS / FAIL / BLOCKED | | |
| Clone | Worker PCM. Ref sha256 matches. Smoke speak logged. | PASS / FAIL / BLOCKED | | |
| Persona | Emotion maps to prosody and the wash. Words stay on the matter. | PASS / FAIL / BLOCKED | | |
| One-dot | `GET /` is the only screen. Mic records. `GET /v2` redirects to `/`. | PASS / FAIL / BLOCKED | | |
| Runbooks | Commands in [RUNBOOKS.md](RUNBOOKS.md) match their outputs. | PASS / FAIL / BLOCKED | | |
| 80 ms | Frame is 80 ms. Streaming budget measured or marked stub-measured. | PASS / FAIL / BLOCKED | | |
| Self-eval | The rows in this file are the record of the gates. | PASS / FAIL / BLOCKED | | |
| Eureka | Presence replay and the heartbeat harness ran. Scores are in the log. | PASS / FAIL / BLOCKED | | |
| Plus / Ultra / Research | Local, Plus, Ultra, and Research are written with their figures. | PASS / FAIL / BLOCKED | | |
| Rust translation | A Galion voice file loads on the Rust path. PyTorch stays a child process. | PASS / FAIL / BLOCKED | | |
| Jarvis texture | Hub clicks, alarms, and ambience remain against a clean-speech baseline. | PASS / FAIL / BLOCKED | | |
| Samantha emotions | Her prosody column is the one the feeling band applies. | PASS / FAIL / BLOCKED | | |
| One-percent | Non-inference CPU ≤ 1% of the host. Tick p99 well under 800 µs. Late pauses. | PASS / FAIL / BLOCKED | | |
| Heartbeat | `Heartbeat::step` advances one frame and allocates nothing on that path. | PASS / FAIL / BLOCKED | | |
| Clip store | Both WAV refs exist. sha256 matches. Git holds no reference audio. | PASS / FAIL / BLOCKED | | |
| Auto-workflow | A spoken turn records itself and the workflow hook fires. | PASS / FAIL / BLOCKED | | |
| TTFA | Default harness prints `measured: false`. A product claim names a model. | PASS / FAIL / BLOCKED | | |
| Staging | Loopback bind holds. Public name has auth. CORS origin is exact. | PASS / FAIL / BLOCKED | | |

No row above has been scored by the landing of this index. Fill a row when its gate has been run.

— Maciej Grajczyk
