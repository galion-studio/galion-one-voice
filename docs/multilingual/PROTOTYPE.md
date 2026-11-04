# Galion Voice: English-only local prototype (W20-20)
*2026-11-02, 12:38 CET (Europe/Zagreb, UTC+1). Local only: nothing is pushed, there are no weights, and no `gh` is used. ENGLISH-ONLY: no he-IL, and pl/others are rejected.*

## What it is
A canned English round trip built on the W20-19 crate `crates/galion-ml-voice`:

```
canned EN "mic" audio (stub::fixture_audio("hello galion"), 24 kHz f32)
  └─> EnglishOnlyRouter.route_audio ─> En
  └─> StubAsr.transcribe(hint=En)   ─> Transcript{en, "hello galion"}
       └─> echo_reply               ─> "<|en|>you said: hello galion"
            └─> EnglishOnlyRouter.route_text ─> En   (reads the <|en|> token)
            └─> StubTts.synthesize  ─> Audio{24000 Hz, 5280 samples = 220 ms}
                 └─> wav_bytes       ─> RIFF/WAVE, PCM16, mono, 24 000 Hz
```

Files added by W20-20 (W20-19's `src/` and `tests/scaffold.rs` are untouched):
- `crates/galion-ml-voice/examples/common/demo.rs`: `round_trip`, `echo_reply`, `to_pcm16`, `wav_bytes` (zero deps)
- `crates/galion-ml-voice/examples/roundtrip.rs`: CLI
- `crates/galion-ml-voice/tests/roundtrip_wav.rs`: 7 tests (round trip, TTS→ASR loop closure, WAV header, determinism, non-EN rejection, empty input, clamp)

## Run
```bash
cd /workspace/galion-voice-code
cargo test --offline -p galion-ml-voice            # 7 scaffold (W20-19) + 7 round-trip (W20-20) tests
cargo run  --offline -p galion-ml-voice --example roundtrip
#   lang   : en
#   asr    : hello galion
#   reply  : <|en|>you said: hello galion
#   tts    : 5280 samples @ 24000 Hz = 220 ms
#   wav    : target/galion-ml-voice/roundtrip-en.wav (10604 bytes)
cargo run --offline -p galion-ml-voice --example roundtrip -- \
    --out /tmp/rt.wav --lang en --text "good morning"
cargo run --offline -p galion-ml-voice --example roundtrip -- --lang he-IL   # exit 2: unsupported language
python3 -c "import wave;w=wave.open('target/galion-ml-voice/roundtrip-en.wav');print(w.getframerate(),w.getnchannels(),w.getsampwidth())"  # 24000 1 2
```
Flags: `--out PATH` (default `target/galion-ml-voice/roundtrip-en.wav`), `--lang` (only `en`), `--text` (canned input text, encoded through the stub fixture). Exit codes: 0 ok, 2 unsupported language or empty input, 64 bad flags.

The stub audio is a synthetic fixture: each character becomes a 10 ms DC block whose amplitude is byte/255. It is not speech. It lets the stub ASR decode the stub TTS output exactly, so the loop can be checked byte-for-byte.

## V4 cascade wiring notes
The fallback lane `crates/galion-cascade` (W20-10, docs/cascade/EN-CASCADE-LANE.md) and `galion-ml-voice` overlap. These are the mismatches to reconcile before wiring:

| Aspect | galion-cascade | galion-ml-voice | Wiring decision (proposed) |
|---|---|---|---|
| Language type | `Lang::En` + `Lang::parse` (en, en-US, en-GB) | `LanguageId::En` (`#[non_exhaustive]`, `<\|en\|>` token) | Make `LanguageId` canonical. Have cascade either re-export it or `impl From<Lang>`. Region tags (en-US/en-GB) should be normalised to `en` by `from_code` later (it currently accepts only `en`). |
| Sample format | `&[i16]` @ **16 kHz** | `Audio{f32}` @ **24 kHz** (Mimi-class) | Use 24 kHz f32 as the internal bus (Mimi/heartbeat). Add a 16 kHz resampling adapter in front of 16 kHz ASR slots (Whisper/Parakeet). TTS output resamples to 24 kHz before the "me" band. |
| ASR trait | `Asr::transcribe(&[i16])` | `AsrEngine::transcribe(&Audio, hint)` | Write an adapter `struct MlAsr<A: AsrEngine>(A)` implementing `cascade::Asr` (i16→f32, 16→24 kHz). Pass the `hint` from `LangRouter::route_audio`. |
| LLM | `Llm::reply(&Transcript, Emotion)` | (none; the demo uses `echo_reply`) | Keep the LLM in cascade. Prefix the reply with `with_lang_token(lang, ..)` so `route_text` picks the TTS voice. |
| TTS trait | `Tts::synth(&str, Lang) -> Vec<i16>` @ 16k | `TtsEngine::synthesize(&str, LanguageId) -> Audio` @ 24k | Adapter `MlTts<T: TtsEngine>` with f32→i16 clamp (same as `demo::to_pcm16`). |
| Emotion | `EmotionIn` (SenseVoice slot) | none | Stays in cascade on the feeling band. ml-voice does not read it (Rika does not read the transcript, per LOCKS `[bands]`). |
| Framing | — | — | 80 ms frame = 1920 samples @ 24 kHz (LOCKS `[clock] frame_ms = 80`). Streaming TTS chunks should be frame-aligned, and barge-in ≤ 200 ms means TTS can be cancelled at frame boundaries (≤ 2.5 frames). |
| Budgets | onset p50 ≤ 800 ms, barge-in ≤ 200 ms | — | Add timing to the round-trip test once real engines land. Stubs run in ≪1 ms. |
| Feature gating | `cascade` feature, never default (A18) | always on (stubs) | The adapter lives behind `cascade`, so default builds stay native-lane only. |

Fast language switching (GROK-VOICE-SWITCHING.md): the per-turn route is `route_audio` (LID on the first ~1–2 frames), then the ASR `hint`, then the reply token `<|xx|>`, then `route_text`, then the TTS voice. Adding a language means adding a `LanguageId` variant and registering engines. The demo code does not change. Polish (`pl`) is the next language only after the multilingual plan and the A21 licence rows are approved. It stays rejected for now.

## Next steps (not done)
1. Adapter module (`MlAsr`/`MlTts`) in galion-cascade behind `cascade`, plus a 16↔24 kHz resampler (in galion-tensor or a tiny linear one for stubs).
2. Promote `wav_bytes` into a crate module if other agents need it. This touches W20-19's `lib.rs`, so coordinate first.
3. `from_code` should accept `en-US`/`en-GB` to match cascade's `Lang::parse`.
4. Real engine slots only after A21 counsel/licence rows. Still no weights and no legacy container formats — .gq only (G2).

## Live-demo hook (W21-03, 2026-11-02 14:25 CET)
Next step 1 has landed as an EN lane in `galion-cascade` (feature `cascade`): `MlAsr`/`MlTts` adapters wrap the ml-voice stubs as cascade `Asr`/`Tts` traits, and `route::CascadeRoute` applies **pin > hint > LID** (EN only; an unsupported pin is an error, and an unsupported hint falls through to LID). The sideband carries `language_hint`/`lang_pin` as `galion_sideband::lang` types (plan P0.1).

```bash
scripts/run-en-cascade-demo.sh [--test] [--play] [--text "..."] [--pin en] [--hint en] [--emotion neutral]
cargo run --offline -p galion-cascade --features cascade --example en_cascade -- --out-dir target/galion-demo
```
Stable outputs for `scripts/run-live-demo.sh`: `target/galion-demo/en-cascade.wav` (audible earcon, 24 kHz mono PCM16, one 80 ms frame per char), `en-cascade-fixture.wav` (exact stub signal), `en-cascade.json` (turn summary), `en-roundtrip.wav` (this crate's `roundtrip` example). The script's last line is `GALION_EN_CASCADE_WAV=<path>`. Details: `docs/agent-wave21/W21-03-STATUS.md`.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
