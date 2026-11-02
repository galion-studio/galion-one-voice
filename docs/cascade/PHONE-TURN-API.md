# Phone turn API (S4+S5+S6) — `galion_cascade::phone` (feature `cascade`)

> ⚠ **S4 English ASR is NOT ratified.** See `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md`
> (A: Vosk out of process · B: A21 ONNX exception extended to ASR · C: hold stub until
> native Whisper-tiny.en `.safetensors`). `GALION_ASR=interim` is a **placeholder**:
> it does not recognise words; it returns a canned English transcript by utterance length.
> No GGML, no LOCKS.toml change.

```rust
let mut lane = galion_cascade::PhoneLane::from_env();      // once per worker thread (Send)
let t = lane.turn(&pcm16k_mono)?;                            // or lane.turn_wav(&wav_bytes)?
// t.transcript, t.reply_text, t.wav_bytes (24 kHz PCM16 RIFF), t.ms, t.stage_ms,
// t.asr (StubFixture | InterimPlaceholder — label the UI!), t.llm, t.speech_backend
```
One-shot: `galion_cascade::phone_turn(&pcm)` / `phone_turn_wav(&bytes)`.

| Env | Values |
|---|---|
| `GALION_ASR` | unset/`stub` (fixtures only; real speech → `AsrUnratified`) · `interim` (placeholder) · `off` |
| `GALION_LLM` | unset/`canned` (CannedLlm, phone default) · `echo` |
| `GALION_SPEECH` | `auto` (A21 Piper, else flite) · `a21` · `flite` · `off` → `NoSpeech` |

Errors: `PhoneError::{Empty, BadWav, AsrUnratified, NoSpeech, Cascade}` (impl `std::error::Error`).
Smoke: `GALION_ASR=interim cargo run -p galion-cascade --features cascade --example phone_turn [in.wav [out.wav]]`.
