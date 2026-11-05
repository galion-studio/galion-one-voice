# FP-03 · TTS / audio · A21 BUILT (CPU, MIT) · 2026-11-02 16:15 CET (Europe/Zagreb)

**Result:** A21 is built. The demo now speaks real English words through a neural TTS on CPU only, and the words play to the speakers while the dot is speaking.
- The voice is Piper VITS `en_US-ljspeech-medium`, decoded on CPU, built per owner decision t123u.
- Flite is the fallback only. The one-dot flow is unchanged: idle, rika, listening and speaking, with the 80 ms Heartbeat and the Rika path.
- Local only: nothing was deployed or pushed.

## What was built
| Piece | File | What it does |
|---|---|---|
| A21 pack (real, downloaded) | `weights/tts/en_US-ljspeech-medium.onnx` (+ `.onnx.json`, `.sha256` sidecars) | Piper VITS voice, 63,531,379 B, 22.05 kHz. From HF `rhasspy/piper-voices` @ `b4e358ff…`. Repo MIT; LJSpeech data public domain. Not vendored (`weights/.gitignore`) |
| CPU runtime | `weights/runtime/piper/` | Piper 2023.11.14-2 linux x86_64 (MIT) with onnxruntime 1.14.1 (MIT, CPU EP) and espeak-ng 1.52 (GPL-3, see blockers) |
| Fetch / verify | `scripts/a21/fetch-a21.sh` (`--check` = no network) | Pinned URLs and sha256. It rejects a mismatch, is idempotent, writes the sidecars and runs a CPU smoke test |
| A21 decoder in the slot | `crates/galion-ml-voice/src/speech/weights.rs` | `A21WeightTts` runs `piper --output_raw` in a separate process, then converts s16le to f32 and resamples 22.05 to 24 kHz. Prosody is deterministic (`noise_scale=0`, `noise_w=0`; `GALION_A21_NOISE=natural` opts out). `PINNED` sha table; `decoder_for()`; `GALION_NATIVE_DECODER_WIRED=false` for `.gq`/`.safetensors` |
| Backend select | `crates/galion-ml-voice/src/speech/mod.rs` | `auto` tries A21 first, then flite. `a21` is A21 only (exit 3 if inactive), `flite` is flite only, `off` is none. `speech_token()` returns `a21`, `flite` or `ABSENT` |
| Speaker playback | `crates/galion-live-demo/src/play.rs` (+ `--no-play` in main.rs) | `PlaySink` (FrameSink) spawns paplay, pw-play, `aplay -q` or ffplay with the `--reply` WAV on each me-voiced onset, and kills it on barge-in (yield). It runs outside `Heartbeat::step`. `GALION_PLAY=0` turns it off; `GALION_PLAYER` overrides the player. There is no audio element on the HTML page |
| CLI token / exit 3 | `crates/galion-cascade/examples/en_cascade.rs`, `scripts/run-en-cascade-demo.sh` | Prints `GALION_SPEECH=a21\|flite\|ABSENT`, also as `speech` in the JSON. Exit 3 means SPEECH=ABSENT, with the fix in the message: `fetch-a21.sh` or `apt install libflite1` |
| Dead branch removed | `scripts/run-v1-demo.sh` | `V1_SPEECH_CMD` / `scripts/v1/speech.sh` is gone, comments are updated, and the exit-3 message is clearer. screen-host / :7870 is not restored |
| Docs | `A21-BLOCKERS.md` (rewritten), `docs/V1-DEMO.md`, `PROGRESS.md`, `LEDGER.md` (t123u entry), `soft-checks.sh` labels | The stale "flite interim / A21 ABSENT" text is replaced |
| Tests | `crates/galion-ml-voice/tests/a21_piper.rs`, weights.rs unit tests, `en_lane::a21_slot_never_pretends` (updated) | The real Piper pack must give deterministic, non-tonal speech of 0.8–4 s (skips if the pack is not fetched). Weight tests: a bare `.onnx` without `.json` is refused, `.pt` is refused, an unverified sha stays inactive, and the JSON rate is parsed |

## How it runs on CPU
- There is no GPU on the box (no `nvidia-smi`), and nothing requests one: Piper uses onnxruntime's default CPU execution provider.
- Speed: 1.35 s of audio takes about 0.53 s per call including model load (RTF ≈ 0.05 synth only, ≈ 0.4 cold). A full EN cascade run takes about 1.9 s.
- It runs out of process, so there is no `ort` or foreign crate in the Rust tree. `check-no-forbidden.sh` and `check-locks.sh` both stay OK.

## Swap path vs flite
`GALION_SPEECH=auto` is the default:
1. A21 is used when `WeightStatus=present-verified` (file present and sha256 matching the env value, sidecar or PINNED) **and** the Piper runtime is found (`$GALION_A21_RUNTIME`, if set, is the only place looked).
2. Otherwise libflite (`cmu_us_slt`) is used as the fallback.
3. Otherwise SPEECH=ABSENT and the run exits 3. Tones are never presented as the reply.

The reply WAV keeps FP-02's `--reply` contract: 17 frames = **1360 ms**, so the dot is in the speaking state while the words play.

## Test results (16:05–16:15 CET)
- **Words (vosk small-en ASR, test-only, /tmp):**
  - A21 `en-cascade.wav` → **"you said hello gallon"**
  - flite → "you said hello dolly volume"
  - earcon → "" (tones)
- **Speakers (end to end):** `galion-live-demo --realtime --reply en-cascade.wav --loops 1` printed "local playback via paplay on each speaking onset" with `me_onsets=3 yields=2 late=0`. A recording of the PulseAudio `speakers.monitor` during the run was transcribed by ASR as "…said hello … hello gallon", so the words reached the speakers.
- **Determinism:** two runs gave the same sha256, `89fa3c1b…a728`, which matches `galion-demo/out/v1/cascade/en-cascade.wav` (regenerated with A21).
- **Fallbacks:**
  - `GALION_SPEECH=flite` → flite, `SPEECH=flite`, rc 0
  - runtime missing → flite fallback, rc 0
  - `GALION_SPEECH=a21` with the runtime missing → `SPEECH=ABSENT`, rc 3
  - `GALION_SPEECH=off` → rc 3
- **`scripts/run-v1-demo.sh --headless`:** `fails=0 soft=0 speech=words`
  - C7 PASS via a21-weights
  - C11 PASS (A21 runtime present)
  - C12 PASS (me band = reply words)
  - C1–C3 PASS (80 ms heartbeat, Rika, workplace hard-OFF)
- **`scripts/run-v1-demo.sh --check`:** PASS (`/health`, `/dot`, `/`), with `me_onsets=3`.
- **cargo tests:** all pass.
  - `galion-ml-voice` (default and `--features speech`, including `a21_piper`)
  - `galion-cascade --features cascade` (26)
  - `galion-live-demo`
- **Gates:** `check-locks.sh` OK; `check-no-forbidden.sh` OK.

## Remaining (see A21-BLOCKERS.md)
- The LOCKS `.onnx` / foreign-runtime exception needs ratifying. LOCKS.toml was not edited; the exception is logged in the LEDGER.
- espeak-ng GPL-3 needs counsel.
- Voice consent or choice needs confirming.
- The Galion-native `.gq` decoder is not built.
- Mimi is `IS_STUB`; ASR is StubAsr; the LLM is MockLlm.
- The runtime is pinned for x86_64 only.
- VPS deploy is out of scope.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
