# Rust translation

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The symphony runs on the Galion Rust stack that already speaks. Translation means the compiler's Galion voice file is what those crates open. Chatterbox remains a CPU child, PyTorch, one process, for the clone step. The hot path after that is Rust.

Minimum Rust is 1.85.

| Crate | Role on the symphony |
|---|---|
| `galion-heartbeat` | One `step` per 80 ms frame. You, me, feeling. |
| `galion-emotion` | OKLCH weight `e`, prosody, consent. Default deny. |
| `galion-cascade` | English lane and PhoneLane. Off the tick. |
| `galion-asr-vosk` | Vosk child for the microphone. |
| `galion-ml-voice` | Piper child, flite fallback, clone-worker slot. |
| `galion-console` | The one circle, the turn worker, `/turn` and `/reply`. |
| `galion-render` | The mark and the loopback HTTP surface. |
| `galion-live-demo` | Headless 80 ms demo. |
| `galion-presence`, `galion-presence-eval` | Replay and the virtual user. |
| `galion-openmind` | Nerve notes. The Open Mind tree is not vendored. |
| `galion-student` | Distillation plan. A `.gq` voice file is not in the tree yet. |

The student path, when the weights exist, is `.gq`. The nerve notes the voice side already emits are `voice.frame_miss`, `voice.fallback`, and `voice.backend_up`. Hermes is a side band for problem-solving. It is off the 80 ms tick. See [OPENMIND-NERVE-CORTEX.md](../OPENMIND-NERVE-CORTEX.md).

```bash
cargo test --offline -p galion-heartbeat
cargo test --offline -p galion-emotion
cargo test --offline --workspace
```

## Gate

**PASS** when a Galion voice file from [COMPILER.md](COMPILER.md) loads through the Rust path and the PyTorch process is still a child, with no torch linked into these crates.

**FAIL** when the runtime path opens a Chatterbox container as its primary voice file, or when a Python interpreter sits inside `Heartbeat::step`.

**BLOCKED** while the compiler has not emitted a file. The crates build and test today. The translated artifact is the missing piece.
# Galion Voice V2 — Rust translation

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. t287.

Master slogan: "The seamless connection with the human being, on the voice only."

The crate is `galion-symphony`. It turns the clone interchange into a pack the Rust stack can open. The runtime objects are Galion `.gq` tensors and a `.gclip` reference. Safetensors is read once and is not the format the loader keeps. This page does not edit `LOCKS.toml`. It does not add a second UI. The one dot stays where it is.

Self-eval: [`SELF-EVAL.md`](SELF-EVAL.md). Infra PASS. Owner wavs, clone weights, and generate are BLOCKED on this tree.

## Pack

`galion-symphony translate` writes a directory:

```text
OUT/
  SYMPHONY.txt
  SHA256SUMS
  tensors/<sector>.gq
  clip.gclip          # only when --wav is set

`SYMPHONY.txt` starts with `GALION-SYMPHONY 1`. It is line-oriented text, not a weight file. `galion-symphony load` checks `SHA256SUMS`, opens each `.gq` with `galion_tensor::gq::GqFile` (`Prefault::WillNeed` unless you pass another), runs `verify_all`, and opens `clip.gclip` when the manifest has one.

Tensor names inside `.gq` are `sym.<sector>.<sanitized>`. The interchange name is only the `source` token on the manifest line. Sectors:

| Interchange file | Sector |
| `shared/ve.safetensors` | `voice_encoder` |
| `shared/s3gen.safetensors` | `speech_gen` |
| `shared/s3gen_meanflow.safetensors` | `speech_flow` |
| `nano/t3_nano_v1.safetensors` | `token_nano` |
| `turbo/t3_turbo_v1.safetensors` | `token_turbo` |

Accepted interchange dtypes are F32 and BF16. Anything else is refused. There is no silent cast. A `.pt`, `.pth`, `.bin`, `.ckpt`, `.onnx`, `.ggml`, or a file whose first bytes are a foreign packed-weight magic is refused, including when the name ends in `.safetensors`. Audio suffixes under the interchange directory are refused. The clip is a separate path.

Without `--fixture`, each file must match `scripts/clone/PINS.sha256` (size and sha256). The five pins are copied into the crate and checked against that script. `--fixture` is for a synthetic tree with the same relative names. It is not a substitute for the pinned weights.

## Clip ABI

Magic `GALIONCL`, version 1, header 112 bytes, then the original WAV as the payload. Little-endian.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `GALIONCL` |
| 8 | 2 | version = 1 |
| 10 | 2 | flags = 0 |
| 12 | 2 | persona: 1 samantha, 2 jarvis |
| 14 | 2 | texture policy: 1 keep, 2 voice |
| 16 | 2 | `texture_strip`. Only 0 is valid. `open` refuses anything else |
| 18 | 2 | texture bits: bit 0 clicks, bit 1 alarms, bit 2 ambience |
| 20 | 2 | channels |
| 22 | 2 | bits per sample |
| 24 | 4 | sample rate |
| 28 | 2 | audio format (1 = PCM) |
| 30 | 2 | reserved 0 |
| 32 | 8 | payload length |
| 40 | 8 | duration, milliseconds |
| 48 | 32 | SHA-256 of the payload |
| 80 | 32 | reserved 0 |
| 112 |  | the WAV bytes |

The payload is not resampled, downmixed, or denoised. `galion-presence` downmixes stereo for the harness. This path does not. `interleaved_i16` returns one slot per channel. A left sample and a right sample stay different. An extra RIFF chunk in the WAV (the fixture uses `hub ` holding `clicks,alarms,ambience`) is still in the payload after `open`.

Jarvis is texture policy `keep`, bits `0b111`. Samantha is policy `voice`, bits 0. Both have strip 0. There is no function that strips the bed.

The clip rate is not the live lane. A Jarvis reference is 48 kHz stereo. The pack's `sample_rate` line stays 24000, which is the heartbeat you/me lane. The clip is not fed to `Heartbeat::step`.

Published hashes, checked only with `--check-pin` or a non-fixture translate:

| Persona | sha256 | Shape |
|---|---|---|
| samantha | `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693` | 44100 Hz, stereo, 16-bit |
| jarvis | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` | 48000 Hz, stereo, 16-bit |

Those files are gitignored. They were not in this checkout. See the BLOCKED rows in the self-eval.

The clone child still refuses a reference on stdin (`CLONE-WORKER.md`). Do not put `clip.gclip` on that request line. Rust reads the clip with `clip show` or `Loaded::clip`.

## Persona

Prosody numbers are the draft tables in `personas/voice/samantha.toml` and `jarvis.toml`. Emotions are the register on the persona pages. They are not new affect-wire labels.

| | Samantha e0 / e1 | Jarvis e0 / e1 |
|---|---|---|
| pitch st | 0.00 / −0.50 | −0.30 / −0.40 |
| pitch range | 1.00 / 0.88 | 0.92 / 0.90 |
| rate | 1.00 / 0.92 | 0.96 / 0.94 |
| loudness | 1.00 / 0.80 | 0.92 / 0.88 |
| warmth | 0.62 / 0.75 | 0.42 / 0.55 |
| pre-pause ms | 0 / 250 | 80 / 120 |
| tags | laugh, chuckle | off |
| emotions | curiosity, delight, tenderness, play, wonder, hurt | calm, amusement, focus, satisfaction |

Empathy on the pack is `live-oklch`. `scripted_empathy` is `false`. A manifest that says otherwise is refused.

## CPU

The pack records one host class and the same per-session budget on every class. Voice stays on CPU. No GPU field is accepted (`device cpu` only).

| Id | Role | What is recorded |
|---|---|---|
| `epyc9655p` | flagship | 96 cores, 192 threads, 768 GB, about 614 GB/s. Air cooled. The LLM node on this CPU is 384 GB and is not this pack. |
| `cx23` | plus | 16 GB RAM, 100 GB SSD, $20/mo, COGS about $6 |
| `cx43` | ultra | 2 GB SSD, $220/mo, CX43-class / Titanaxe Max 3XL. RAM was not specified, so the field stays empty |

Session budget, from the CPU foundation, stored as design figures rather than a measurement of this machine:

- about 1.0 vCPU average (ASR 0.4, TTS 0.3, VAD/SER 0.15, the rest headroom)
- burst 4 vCPU
- about 0.65–1.15 GB RAM
- non-inference work at most 1% of the host
- tick ceiling 800 µs inside the 80 ms frame
- a late frame pauses (`galion-heartbeat`: `late` or a buffer that is not 1920 samples sets `paused`)

## Alignment

| Crate | Contract this pack uses |
| `galion-tensor` | `.gq` v1 reader. Magic `GALIONGQ`. `verify_all` on load. |
| `galion-heartbeat` | `FRAME_MS` 80, `SAMPLE_RATE` 24000, `FRAME_SAMPLES` 1920. Late frame pauses. |
| `galion-ml-voice` | `stub::SAMPLE_RATE` is the same 24000 Hz lane as the clone worker's s16le stdout. |
| `galion-presence` | Report schema `galion-presence-report/v1`. Bands `you`, `me`, `feeling`. The manifest has no engagement field. |

Compile-time asserts in `galion-symphony` tie the three rate constants together.

## How to run

Rust 1.85 or newer, from the repo root.

```text
cargo test -p galion-symphony
cargo run -p galion-symphony -- eval
cargo run -p galion-symphony -- hosts

cargo run -p galion-symphony -- translate \
  --in weights/clone \
  --out weights/symphony/jarvis \
  --persona jarvis \
  --model nano \
  --host cx43 \
  --wav docs/galion-voice-v2/personas/refs/jarvis.wav

cargo run -p galion-symphony -- load weights/symphony/jarvis --prefault willneed

cargo run -p galion-symphony -- clip wrap \
  --persona jarvis \
  --wav docs/galion-voice-v2/personas/refs/jarvis.wav \
  --out weights/symphony/jarvis.gclip \
  --check-pin

cargo run -p galion-symphony -- clip show weights/symphony/jarvis.gclip

`--model turbo` expects `turbo/t3_turbo_v1.safetensors` instead of the nano file. `--host` is `epyc9655p`, `cx23`, or `cx43`. `--fixture` skips the published weight pins and the published clip hash. `--force` replaces an existing pack. Output must sit outside the interchange directory. `weights/` is gitignored; do not commit a pack or a wav.

Exit codes: `eval` returns 0 when Fails is 0 (BLOCKED does not fail the process) and 1 when any gate FAILs. `load` returns 1 when the pack does not verify. Usage errors return 2.

## What is not in this change

- `LOCKS.toml` is untouched.
- The clone child is not started. It still does not call generate, and it still refuses a reference field.
- Samantha and Jarvis WAVs and the pinned safetensors are not in this checkout, so a real pack was not written here.
- No llama.cpp weight container is read or written.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
