# Swarm 10 — persona-v2v / v2v (C3 and C5)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Written 2026-11-15 22:12 +01:00 (Europe/Zagreb).

Lane plan: pull request #150 (`Swarm 10 persona-v2v plan`). This note stays on C3 and C5.

## What was checked

Showcase script in `docs/galion-voice-v2/V2V-SHOWCASE.md` (Rest, Strain, Joy, Command, hub on, hub off, Anger). Verification boxes in `docs/galion-voice-v3/VERIFICATION.md` Part A1 and A2 were read and left unticked. `NEXT-ITERATION.md` gap 4 was read and is not in git.

On this host (`cursor`, Linux 6.12.113+ x86_64), before the session:

| Check | Result |
|---|---|
| `weights/asr/vosk-model-small-en-us-0.15` | absent |
| `weights/tts/en_US-ljspeech-medium.onnx` | absent |
| `docs/galion-voice-v2/personas/refs/*.wav` | 0 files. The directory holds `samantha.placeholder` and `jarvis.placeholder` (ASCII text). |
| `GALION_SPEECH`, `GALION_LLM`, `GALION_LLM_URL`, `GALION_CLONE_STUB_PCM`, `GALION_CLONE_LOCAL_REFS` | unset |
| Clone worker generate | **not run.** The original wavs are not on this machine. |

`SpeechStack::detect` with `GALION_SPEECH` unset therefore opened Flite. The printed label is:

```text
fallback system TTS (libflite cmu_us_slt; real words; NOT A21, NOT neural)
```

Backend id: `system-flite`. That is the mouth of both sessions. It is Flite voice `slt`. C5 stays **BLOCKED**.

## What changed

- `scripts/eval/v2v-session.sh` and `crates/galion-console/examples/v2v_session.rs` walk the showcase beats through `DuplexOutline`, then `PhoneLane::speak_fixed` on the persona-floor reply.
- `SpeakPlan` now carries the floor's `llm` label, whether Hermes is attached, whether the in-house floor is on, the mood, `film_voice`, and `clone_live`, so the log can print those fields from the same plan.
- Session JSON (no audio) is in `sessions/samantha.json` and `sessions/jarvis.json`. WAV files stayed in `/tmp/galion-v2v-sessions/` and are not in this commit.
- `SOUL.md`, `config.yaml`, `LOCKS.toml`, `scripts/clone/REFS.sha256`, weights, and reference wavs were not edited.

## Sessions

Command, 2026-11-15 22:12:26 +01:00, runner commit `a8a784f931521c7a7fa648eead851886a157a90a`:

```text
bash scripts/eval/v2v-session.sh
```

An earlier run at 22:11:25 +01:00, still on parent `8980e1a` plus this uncommitted runner, wrote the same four WAV sha256 values. `tts_first_pcm_ms` is wall time and differed between the two runs (Samantha rest 29 ms, then 19 ms). It is not a product TTFA. `product_ttfa_claim` is false. No person listened. Pitch was not measured.

ASR on every beat is `text-inject`. `full_v2v` is false. Vosk did not run.

`GALION_LLM` unset and `GALION_LLM_URL` unset. The floor label is `hermes-agent`, `hermes_attached` is false, and `inhouse` is false. Matter turns therefore spoke the unreachable line `model can't be reached`. The phone lane's own LLM mode was `Canned`. `speak_fixed` does not ask that LLM for words. The phone-lane mode is recorded so it is not mistaken for the reply author.

`film_voice` is false and `clone_live` is false on every beat. `canned_empathy` is false on every beat. The nano request line was built and not sent.

### Samantha

| Beat | Sense | e | OKLCH | Wash | Warmth | Pause plan | Reply | Mouth |
|---|---|---|---|---|---|---|---|---|
| Rest | off (Tired frame supplied, not applied) | 0 | 0.971 0.003 286 | 0.62 | 0.62 | 0 ms | `model can't be reached` | `system-flite` |
| Strain | on, Tired −0.6 / 0.2 / 0.4 / 1 | 1 | 0.72 0.09 310 | 0.50 | 0.75 | 250 ms | same unreachable line | `system-flite` |
| Joy | on, Joy 0.8 / 0.8 / 0.5 / 0.9 | 0 | 0.971 0.003 286 | 0.62 | 0.62 | 0 ms | same unreachable line, mood `joy` | `system-flite` |

Rest and Joy are the same PCM (`sha256 03070dac42f594505708a7d50090f937b61694bd515b658f9ff49d6ad25d5ebe`, 80 684 bytes, 40 320 samples at 24 kHz, body RMS 0.157899, leading silence 1.5 ms / 37 samples). Strain is a different file (`sha256 ba54e4b39561fa1f193d8d852e7e605a39f6e7a011c92b0f77639d8e5458752a`, 92 684 bytes, 46 320 samples, body RMS 0.126318, leading silence 251.5 ms / 6 037 samples).

Same reply text, so the PCM difference is the delivery. Extra leading samples are 6 000, which is 250.0 ms at 24 kHz. Body RMS 0.126318 / 0.157899 = 0.800 to three decimals, against planned loudness 0.80. Planned pitch −0.5 st and rate 0.92 are on the plan. `apply_frame_prosody` rewrites pause and energy only, and pitch was not measured, so those two knobs are unmeasured on this mouth.

Joy kept `e` at 0 and returned the neutral mark. The spoken words did not become a question about the room. Both matter turns hit the unreachable line, so Joy and Rest share one utterance.

Samantha's hub field stayed the three-sample asset `[0.1, -0.2, 0.3]`. That is the outline rule that Samantha playback is the speech asset alone. It is a unit buffer, not a hub recording.

### Jarvis

| Beat | What happened | Mouth |
|---|---|---|
| Command `jarvis`, sense on, Tired frame held | Persona `jarvis`. Reply `Here. What do you need?`. `e` 1, OKLCH 0.72 0.09 310, wash 0.50, warmth 0.55, planned pause 120 ms, planned pitch −0.4 st. WAV `sha256 b523d49d8ca2a65d16734dfcab929b95be38f393f07885c2db40143ac6c0f5a6`, 83 084 bytes, leading silence 121.5 ms, body RMS 0.156686. | `system-flite` |
| Hub on | No new words. Unit mix `[0.11, 0.3, 0.2]` (asset plus layer). Not spoken. Not a recording of clicks or alarms. | not spoken |
| Hub off | Toggle `{"persona":"jarvis","hub_texture":false}` parsed. Unit mix copied the asset `[0.1, -0.2, 0.3]`. Not spoken. | not spoken |
| Anger, task sentence `Confirm the gate for the six o'clock flight.`, Angry −0.8 / 0.9 / 0.8 / 1 | Persona stayed `jarvis`. `e` stayed 1. Reply `model can't be reached`. Mood word `calm` (the sentence does not match the grief/joy/vulnerability word list). WAV `sha256 9e3f653141da5e8df5b686ab98919be08426f42357b16b53ed0b4ddee6308e5a`, 86 444 bytes, leading silence 121.5 ms. | `system-flite` |

No zero-pause take of the same Jarvis line was recorded, so the 121.5 ms leading silence is the measurement against a 120 ms plan, and the split between Flite's own onset and the added pause was not separated on that sentence.

## C3

One session record exists for Samantha and one for Jarvis. Colour and `e` moved with sense: off held `e` at 0 and the neutral mark; the Tired frame with sense on reached `e` 1 and heather `oklch(0.72 0.09 310)`; Joy held `e` at 0. On the Flite mouth, the same Samantha sentence gained the planned 250 ms pause and the 0.80 loudness scale. Canned empathy lines were absent.

This row does not claim full voice-to-voice. The words were typed. Vosk weights are absent, so a mic turn was not run and no transcript was invented.

The matter replies are the unreachable line, because Hermes was not attached and `GALION_LLM` did not select `galion-reflect`. The showcase table's sentence `Your mother died this morning.` was not what this process printed.

## C5

**BLOCKED.** Clone generate was not run. Original reference wavs are not on this machine. A0 remains BLOCKED-OWNER (`LOCKS.toml` `ref_clip_decision = "t623u"` and `scripts/clone/REFS.sha256` were not edited). No clone PCM was produced. `mouth_is_clone_of_originals` is false. Verification A1 and A2 stay open. This note does not sign V3-start.

The mouth that spoke is Flite `cmu_us_slt`. Calling that mouth the original Samantha or Jarvis clone would be false, and this note does not do that.

## Tests

All four showcase commands, offline, 2026-11-15, rustc 1.85.0, this host. Exit 0.

| Command | Result |
|---|---|
| `cargo +1.85.0 test -p galion-emotion --offline --lib duplex` | 4 passed, 0 failed |
| `cargo +1.85.0 test -p galion-render --offline --lib canned_empathy` | 1 passed, 0 failed |
| `cargo +1.85.0 test -p galion-console --offline --features dot-viz --lib v2v` | 7 passed, 0 failed |
| `cargo +1.85.0 test -p galion-heartbeat --offline --lib` | 9 passed, 0 failed |
| `bash scripts/eval/v2v-session.sh` | exit 0. Logs above. WAV under `/tmp/galion-v2v-sessions/`. |

`--offline` succeeded. An online retry was not required.

## CI

GitHub `gates`, `emotion-v2`, and `one-desktop` are the checks for this pull request. Their status is the check runs on the current head. This file does not invent a green result. `nebula-voice-v2` stays skipped unless a nebula path changes. This branch does not change one.

## Open questions

- Matter turns on an unset `GALION_LLM` and an unset Hermes URL speak `model can't be reached`, while the floor label still reads `hermes-agent`. The in-house `galion-reflect` reply is opt-in. This run left the default in place.
- Pitch and rate are planned and were not measured in the Flite PCM.
- A Piper pack is not on this host, so this session cannot extend the Piper receipt from the other baseline host.

## Decisions for Maciej

1. A0 stays his. Re-open the t273 originals on the live clone path, or state that main stays on the t623u pins. This lane did not choose, and C5 stays blocked until lane A has real clone PCM from the originals.
2. The live mouth on this host was Flite, because the Piper pack is absent here. The owner deploy default remains Piper plus the Samantha persona until a clone mouth is actually live. This session does not move that default.
3. Whether unset matter turns should keep the unreachable line, or whether the live page should opt into `galion-reflect`. This run did not flip that.
4. Verification A1–A2 and `ADMIN-UI-CHECK.md` stay unsigned.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
