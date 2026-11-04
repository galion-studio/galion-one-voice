# Phone EN Pipeline — mic → ASR → LLM → TTS → playback

Status: the loop below is in the tree as of the Galion Voice V2 program (phone path on mainline).
Real English ASR is still unratified. See `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md` and `docs/galion-voice-v2/GALION-VOICE-V2.md`.
This file keeps the original gap list so the remaining holes stay visible.

Scope: English spoken reply on a phone browser hitting the V1 console.

## Target architecture

```
[Phone browser: v1-screen.html]
  getUserMedia (mic) -> AudioWorklet/MediaRecorder capture (16/24 kHz mono PCM)
  -> client VAD / push-to-talk end-of-utterance
  -> POST /reply  (audio body, lang=en)                     [galion-console]
        -> ASR   (galion-ml-voice, real EN model)            [galion-cascade en_lane]
        -> route (pin > hint > LID) -> EN lane
        -> Emotion (FixedEmotion ok for v1)
        -> LLM   (real model; MockLlm echo is not acceptable for acceptance)
        -> TTS   (galion-ml-voice, 24 kHz)
     <- response: { transcript, reply_text, audio (wav/pcm, 24 kHz) }
  -> WebAudio decode -> playback (unlocked by user gesture on iOS/Android)
  -> UI: show transcript + reply text, state idle/listening/thinking/speaking
```

## Current gaps (from diagnosis)

1. **`tickMic()` is level-only** (`crates/galion-console/assets/v1-screen.html` ~L150): it polls the analyser for a meter every 80 ms; no PCM is captured, buffered, or sent anywhere.
2. **No `/reply` endpoint**: galion-console exposes no route that accepts utterance audio and returns reply audio; the phone has nothing to call.
3. **`StubAsr` in the EN lane** (`crates/galion-cascade/src/en_lane.rs`): `EnCascade = Cascade<MlAsr<StubAsr>, FixedEmotion, MockLlm, MlTts<StubTts>>` — ASR returns fixture text, LLM echoes, TTS is stub. Real speech cannot produce a real reply.
4. No playback path in the page for server audio, and no mobile audio-unlock handling.

## Work items (proposed)

- Client: capture PCM (AudioWorklet preferred), end-of-utterance (VAD or push-to-talk), POST to `/reply`, play returned audio.
- Server: `/reply` handler in galion-console wiring into the EN cascade; size/time limits; JSON + audio response.
- Cascade: swap `StubAsr` → real EN ASR, `MockLlm` → real LLM, `StubTts` → real TTS behind feature flags; keep stubs for tests.
- HTTPS (required for getUserMedia on phones).

## Acceptance criteria — English spoken reply on phone

1. On iOS Safari and Android Chrome over HTTPS, tapping the mic prompts for permission and shows listening state.
2. Speaking an English sentence (e.g. "What time is it in Zagreb?") results in a `/reply` request carrying real captured audio (not level data).
3. Returned transcript matches the spoken words (≤ ~15% WER on 5 scripted test phrases) — proves StubAsr is gone.
4. Reply text is a contextual LLM answer, not an echo of the transcript.
5. Reply audio plays audibly through the phone speaker without an extra tap after the initial gesture.
6. End-of-speech → first audio ≤ 3 s on demo network (target; record actual).
7. Errors (mic denied, network, ASR empty) show a visible message; no silent hang.
8. Existing tests (`roundtrip_wav`, `scaffold`, en_lane tests) still pass using stubs.
9. Evidence: screen recording from a phone + server log of one full round-trip, linked in PROGRESS.md.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
