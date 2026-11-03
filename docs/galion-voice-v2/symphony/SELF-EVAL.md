# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is complete only when its gate passes and the result is written here.

Master slogan: "The seamless connection with the human being, on the voice only."

Demo = Spec = Test. The gate is `scripts/symphony/self-eval-a.sh`.

## Section A — compiler

Verdict: **PASS**

Host for the numbers below: the build CPU for this change (Rust 1.85.0, `cargo test` debug profile). Not an AMD EPYC 9655P and not a CX23/CX43. The budget those hosts must meet is what the test asserts.

Command:

```text
cargo test -p galion-ml-voice --lib artifact::
cargo test -p galion-ml-voice --test voice_artifact -- --test-threads=1 --nocapture
```

Tick line from the integration test (debug, unoptimized):

```text
section-a tick p99_ns=7316 max_ns=7970 budget_ns=800000
```

p99 7.3 µs, max 8.0 µs, budget 800 µs (1% of 80 ms). Late frames are inside that loop. Debug profile, so this is the slow build.

| # | Gate | Result |
|---|---|---|
| A1 | Crate tests compile and the 16 `artifact::` tests pass | **PASS** |
| A2 | Round trip: synthetic s16le → `.gq` → `GqFile::verify_all` → same samples. Output is not raw PCM and not RIFF | **PASS** |
| A3 | `.safetensors` / `.pt` / `.wav` / `.pcm` / `.gci` / `.ggml` are not `open_runtime`. CLI exit 3, no output file | **PASS** |
| A4 | Safetensors bytes or a GGML-family magic renamed to `.gq` are refused | **PASS** |
| A5 | `chatterbox_runtime` flag and a GPU flag are refused. `CHATTERBOX_IS_RUNTIME` is false | **PASS** |
| A6 | Rate 24000, frame 80 ms, 1920 samples, equal to `galion-heartbeat` | **PASS** |
| A7 | `fill_frame` allocates 0 times (counting allocator), including late and past-end | **PASS** |
| A8 | Late frame is silence and the same index still plays. No catch-up | **PASS** |
| A9 | p99 and max of `fill_frame` under 800 µs | **PASS** (7.3 µs / 8.0 µs) |
| A10 | Sidecar sha256 required. Stem must match the persona. Atomic write leaves no `.tmp` | **PASS** |
| A11 | Stereo is mixed, not stripped. A 48 kHz click survives resample | **PASS** |
| A12 | `LOCKS.toml` not edited. PyTorch not linked. No GGML-family container written | **PASS** |
| A13 | Tick measured on EPYC 9655P-class or CX23/CX43 | **BLOCKED** — those hosts are not in this run. The budget gate ran here instead |
| A14 | Live Chatterbox `generate` for Samantha/Jarvis | **BLOCKED** — section B. Weights are not in git. This gate uses synthetic PCM in the worker's s16le 24 kHz shape |

`LOCKS.toml` diff: none.

## Sections B–G

Not this change. No verdict.

| Section | State |
|---|---|
| B Clone landing | Not run |
| C Jarvis texture | Not run. The compiler does not denoise; that is not the texture gate |
| D Persona / emotion | Not run |
| E One-dot | Not run |
| F Runbooks beyond the compiler | Compiler run steps are in `COMPILER.md` |
| G Latency on a live stream | Frame constant and the clip tick are gated here. A streaming session is not measured |

— Maciej Grajczyk
