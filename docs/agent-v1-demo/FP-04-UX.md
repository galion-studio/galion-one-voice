# FP-04 · Real-life UX: how a person hears Galion and sees the dot
Galion Voice — Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1) · LOCAL ONLY

## What a person experiences (`scripts/run-v1-demo.sh`, measured on the box)
1. **Start:** they open `http://127.0.0.1:7878/`. The screen shows one off-white dot on near-black with a slow amber→violet wash.
   It has no text and no controls. A click, `F` or Enter makes it fullscreen. The cursor is hidden.
2. **Idle:** the dot breathes slowly (radius 0.094–0.106, brightness 0.81–0.87).
3. **Rika lean-in (≈160 ms):** when the room goes quiet and an in-breath arrives, the dot holds still at radius 0.112 (`lean` = 1).
   This is the "I'm about to listen" cue.
4. **Listening:** while the person (the you band) talks, the dot grows and stays almost still (r 0.119–0.126, brightness 0.96).
5. **Speaking:** Galion's reply words play **out loud** through paplay to the default sink. The dot is in the speaking state for exactly
   the length of the words: 1.36 s for "you said: hello galion". It flickers with the me-band RMS (r 0.119–0.136, brightness 0.97–0.99).
6. **Barge-in:** if the person talks over Galion, playback is killed and the dot drops to listening (`yields=2` per loop).
   Then it goes back to idle.

One loop is about 16 s. Measured `/dot` sequence (FEED_PORT=17906, 80 ms polling, A21 voice):
`idle › rika › listening › idle › speaking › idle › rika › listening › idle › speaking › listening › idle › speaking › idle`.
Samples: idle 103 · rika 14 · listening 34 · speaking 30.
At the same time, the PulseAudio `speakers.monitor` capture had about 3.1 s of audible audio in 21 s.
That is the reply words, including the cut-off replay after barge-in.

## What the voice sounds like
- **A21 (default):** Piper VITS `en_US-ljspeech-medium`, a neural female US-English voice running on CPU.
  - It is clearly intelligible. FP-03's ASR heard "you said hello gallon".
  - It sounds deterministic and slightly flat, because `noise_scale=0`. `GALION_A21_NOISE=natural` gives livelier prosody.
- **Flite fallback:** robotic, and slower at 1.92 s for the same text.
- A long sentence (72 chars) gives 5.76 s of audio. That played cleanly with paplay, rc 0.

## Gaps (honest)
| # | Gap | Impact | Simplest next step |
|---|---|---|---|
| G1 | **No microphone.** The you band is a scripted synthetic room/breath/voice. | The person cannot actually talk to it; "listening" is staged. | Add a mic → you-band path (needs owner consent rules). |
| G2 | **Rika needs breath input.** The lean-in fires only on room → in-breath in the you band. A bare speech WAV in the you band gives `lean_in_frames=0`. | With a real mic and no audible breath, the rika state may never appear. | Keep the synthetic you band for demos. Tune the breath detector on real mic audio later. |
| G3 | **The reply is an echo.** StubAsr + MockLlm say "you said: hello galion" every loop. | It feels like a loop, not a conversation. | Real ASR and LLM are out of V1 scope. |
| G4 | **Listening and speaking look alike.** The radius ranges overlap (0.119–0.126 vs 0.119–0.136). Speaking differs only by an 80 ms flicker and slightly higher brightness. | Without sound, an observer can confuse them. With sound it is clear. | Owner call. Locks forbid extra UI, so at most a slightly larger speaking radius range. |
| G5 | **Audio vs dot sync.** Playback is a separate process started on the speaking onset, so it lags by the player start-up time. `late=0` frames were measured, but there is no sub-frame sync measurement. | The words may start a few tens of ms after the dot. This is not noticeable in testing. | Measure with a loopback capture if it matters. |
| G6 | **The headless EN turn used `--in` (you band) until FP-04.** The words showed as *listening*. | The gate did not exercise the real speaking path. | **Fixed (FP-04):** the headless EN turn now uses `--reply --loops 1 --mimi off`, and the new soft check C12 asserts `voice=reply-wav`. |
| G7 | **The me band is the reply WAV, not a voice codec.** Mimi `IS_STUB` runs with `--mimi off`. | It sounds fine, but the duplex codec path is not exercised. | It waits on Mimi weights (counsel). |
| G8 | **Loopback only.** Viewing remotely needs an SSH tunnel. The VPS is not reachable (SSH denied). | Single-machine demo only. | Deploy later. |
| G9 | **The feed drops on a crash.** The page falls back to the local idle pulse after 1 s, so it is never dark, but it also gives no error sign. | A crashed demo looks like "idle forever". | Fine for V1, because the one-dot rule forbids an error UI. Check the terminal. |

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
