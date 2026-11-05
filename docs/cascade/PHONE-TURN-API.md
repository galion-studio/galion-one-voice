# Phone turn API (S4+S5+S6) — `galion_cascade::phone` (feature `cascade`)

> **S4 letter A is Vosk, out of process** (`galion-asr-vosk`, small-en, CPU).
> `GALION_ASR=vosk` returns the worker's words. A missing model is
> `AsrUnavailable`. `GALION_ASR=interim` does not select the length-bucket
> placeholder. No GGML. The Vosk exception is not in `LOCKS.toml`.
> See `docs/asr/VOSK.md`.

```rust
let mut lane = galion_cascade::PhoneLane::from_env();      // once per worker thread (Send)
let t = lane.turn(&pcm16k_mono)?;                            // or lane.turn_wav(&wav_bytes)?
let (text, src, reply) = lane.inject_text("hello")?;         // src = text-inject; not recognition
let spoken = lane.turn_text("hello")?;                       // same, plus 24 kHz WAV
// t.transcript, t.reply_text, t.wav_bytes (24 kHz PCM16 RIFF), t.ms, t.stage_ms,
// t.asr (StubFixture | Vosk | InterimPlaceholder | TextInject — label the UI!), t.llm, t.speech_backend
```
One-shot: `galion_cascade::phone_turn(&pcm)` / `phone_turn_wav(&bytes)`.

| Env | Values |
|---|---|
| `GALION_ASR` | unset/`stub` (fixtures only; real speech → `AsrUnratified`) · `vosk` (worker; missing model → `AsrUnavailable`) · `off` · `interim` is ignored (treated as stub) |
| `GALION_LLM` | unset/`canned` (CannedLlm, phone default) · `echo` |
| `GALION_ASR` | unset/`stub` (fixtures only; real speech → `AsrUnratified`) · `interim` (placeholder) · `off` |
| `GALION_LLM` | unset/`canned` (CannedLlm, explicit test opt-in) · `echo` · `hermes` (loopback OpenAI chat). Any other value is an error, not a canned fallback. |
| `GALION_LLM_URL` | `http://127.0.0.1:8642/v1/chat/completions` (default when mode is `hermes`). Numeric loopback `http://` only. No TLS. |
| `GALION_LLM_MODEL` | model name sent to the gateway. Default `hermes-agent`. Shown on `/v2/say` and `/v2/state` as `llm`. |
| `GALION_LLM_KEY` | optional bearer token. Not logged. |
| `GALION_LLM_TIMEOUT_MS` | default `3000`. Timeout or a refused connect speaks `model can't be reached`. No canned fallback. |
| `GALION_SPEECH` | `auto` (A21 Piper, else flite) · `a21` · `flite` · `off` → `NoSpeech` |

A21 keeps one Piper process for the lane (`--json-input --output_raw`).
`PhoneLane::new` pre-warms it. Two turns reuse that process (`piper_worker()`
is `(Some(pid), 1)`). The reply's first clause is flushed at punctuation or
8 words; `stage_ms.tts_first_pcm` is the time to that PCM. Cold vs warm
numbers: `scripts/bench-piper-warmth.sh` (no PCM file). `off` does not speak;
the turn worker publishes the two-tone WAV with `backend=tone` and `error`.

Errors: `PhoneError::{Empty, BadWav, AsrUnratified, AsrUnavailable, NoSpeech, Superseded, Cascade}` (impl `std::error::Error`).
Smoke: `GALION_ASR=vosk cargo run -p galion-cascade --features cascade --example phone_turn [in.wav [out.wav]]` after `scripts/asr/fetch-vosk.sh`.

Virtual user (offline, `127.0.0.1` only): `scripts/eval/virtual-user.sh`. Report under `galion-gates/results/presence-eval/`. See `docs/eval/PRESENCE-EVAL.md`. `text-inject` is the harness supplying words. It is not recognition. Interim stays `interim-placeholder`.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
