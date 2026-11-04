# V1-03 · Real-speech path · STATUS · 2026-11-02 (Europe/Zagreb)

Lane: V1-03 (speech). LOCAL ONLY: no git push, no gh, no HF downloads, English only (no he-IL).
Decisions applied: Q1 still a plumbing demo · **Q2 the EN cascade must say actual words** · Q3 A21 is funded and weights are coming.

## Result
The EN cascade reply WAV is now **spoken English words**, not tones.

| | |
|---|---|
| Sample (words) | `target/galion-demo/en-cascade.wav`: "you said: hello galion", 1.92 s, 24 kHz mono PCM16, padded to 80 ms frames |
| Longer sample | `/workspace/v103-sample/en-cascade.wav`: "you said: hello galion, this is the version one demo", 3.52 s |
| Spectrogram | `/workspace/v103-sample/spec.png`: speech (harmonics, formants, fricatives) on top, the old earcon tone ladder underneath |
| Backend in use | **interim system TTS**: distro `libflite1` (CMU Flite 2.2, voice `cmu_us_slt`), opened at runtime with `dlopen`. **This is not A21 and not neural.** |
| A21 slot | `A21WeightTts` / `WeightStatus` are wired in. Current state: **ABSENT** (no weights on disk). The slot never claims to speak. |

Priority ladder from the brief, and what happened with each option:
- (a) Piper / Pocket-TTS / `.gq` weights on disk: **none found.** `GALION_TTS_WEIGHTS` and `GALION_SPEECH_WEIGHTS` are unset, and there is no `weights/tts/` directory. Piper `.onnx` would also be **refused by LOCKS** (it needs a foreign runtime, and `.onnx` isn't an allowed format).
- (b) System TTS: **done.** No flite or espeak binary was available, but the installed `libflite1` shared libraries work through FFI with no build-time link, no vendoring and no network.
- (c) In-tree formant/Klatt synthesizer: **not built**, because (b) was available. It remains the fallback if A21 counsel rules flite out (see A21-BLOCKERS.md).

## How to run
```bash
cd /workspace/galion-voice-code
scripts/run-en-cascade-demo.sh --test            # tests + writes target/galion-demo/en-cascade.wav (speech)
scripts/run-en-cascade-demo.sh --text "hello galion, this is the version one demo" --play
# direct:
cargo run --offline -p galion-cascade --features cascade --example en_cascade -- --out-dir target/galion-demo --text "hello galion"
# live demo with the spoken reply on the you band:
LIVE_EN_CASCADE=1 scripts/run-live-demo.sh
```
Environment variables:
- `GALION_SPEECH=auto|a21|flite|off` (default `auto`)
- `GALION_FLITE_VOICE=slt|kal16|awb|rms|kal`
- `GALION_TTS_WEIGHTS` / `GALION_SPEECH_WEIGHTS` (file or directory), plus `GALION_TTS_WEIGHTS_SHA256` or a sibling `<file>.sha256`

Exit codes: 0 ok · 2 unsupported language or empty input · **3 no speech backend** (a tone reply is never written silently; `--allow-tones 1` forces the earcon, and it is labelled `earcon-fallback-NOT-SPEECH` in the JSON) · 64 bad flags.

Outputs in `$GALION_DEMO_OUT` (default `target/galion-demo`):

| File | Contents |
|---|---|
| `en-cascade.wav` | reply as **speech** (stable path; still the file `run-live-demo.sh` consumes) |
| `en-cascade-speech.wav` | the same speech under an explicit name |
| `en-cascade-earcon.wav` | the old tones, kept for diagnostics only |
| `en-cascade-fixture.wav` | StubTts loop-check signal (unchanged) |
| `en-cascade.json` | adds `spoken_text`, `reply_kind`, `speech_backend`, `speech_backend_label`, `a21_weights`, `a21_weights_detail` |

## What changed (code)
- `crates/galion-ml-voice` gets a new feature `speech` (off by default, so the default build is still no-I/O and no-FFI):
  - `src/speech/mod.rs`: `SpeechStack::detect()` tries A21 first, then flite, then none. Also `SpeechBackend` with an id and label (every label says what it is *not*), and a Catmull-Rom 16 kHz → 24 kHz resampler.
  - `src/speech/flite.rs`: `FliteTts: TtsEngine`, using runtime `dlopen` of `libflite.so.1`, `libflite_usenglish.so.1`, `libflite_cmulex.so.1` and `libflite_cmu_us_<voice>.so.1`. Calls go through a mutex, and `srand` is reseeded per call so output is deterministic (same text gives the same samples).
  - `src/speech/weights.rs`: the A21 slot. `WeightStatus` is `Absent`, `Refused` or `Present` (with sha256). `A21WeightTts::try_new` succeeds only when the weights are Present, the sha256 is verified **and** `DECODER_WIRED` is true (it is `false` today).
  - `src/speech/sha256.rs`: a dependency-free SHA-256 (checked against FIPS test vectors).
  - `lib.rs`: `#![forbid(unsafe_code)]` became `#![deny(unsafe_code)]`, and only `speech::flite` has `#[allow(unsafe_code)]`. Also adds `VoiceError::Backend(String)`.
- `crates/galion-cascade`:
  - The `cascade` feature now enables `galion-ml-voice/speech`.
  - `en_lane::DemoTurn` gets `speech`, `spoken_text`, `speech_backend` and `a21_weights`.
  - `spoken_text()` strips the `[Emotion]` tag before speaking.
  - The fixture and earcon paths are unchanged (dual path), and the loop-check test still passes.
- `examples/en_cascade.rs` and `scripts/run-en-cascade-demo.sh`: the reply WAV is now speech, exit code 3 is added, and the JSON and checks are extended.

## Tests (offline, all green)
- `cargo test --workspace --offline`: all pass.
- `galion-cascade --features cascade`: 26 tests pass, including these new ones:
  - `reply_is_speech_not_tones`: duration, ZCR (zero-crossing rate) variation typical of speech, not equal to the earcon, and deterministic
  - `a21_slot_never_pretends`
  - `spoken_text_drops_tag`
- `galion-ml-voice --features speech`: 7 unit tests covering SHA vectors, refusal of `.onnx`, "present + verified but no decoder means the slot stays inactive", sha mismatch, and the resampler.
- `scripts/check-locks.sh`: OK.

## Deploy note (server lanes)
The target host needs the distro package **`libflite1`** (`apt install libflite1`), which is a system package, not an HF download. If it is missing, the lane exits 3 with a clear message instead of playing tones.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
