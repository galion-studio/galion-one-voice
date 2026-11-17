# Swarm 10 — persona-live (Lane C, C1)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Written 2026-11-15 22:16 +01:00 (Europe/Zagreb).

The spelling is **Galion**. The product is Galion Voice. Owner: Maciej Grajczyk.

Worker: persona-live. Item: C1. Plan: [Swarm 10 persona-v2v plan](https://github.com/galion-studio/galion-one-voice/pull/150), merged. This branch is `cursor/persona-live-emotion-3584`, rebased onto `origin/main` `2ecca8645adb1bf069af912cb7066334f97d1b3c`.

## Verdict (split, not a hidden pass)

| Half | Result | Why |
|---|---|---|
| Feeling-band wash | Moved, with `sense` on | `POST /v2/say` JSON and `GET /dot` carry `feeling_e`, `wash_amber`, and OKLCH. Samantha `e=1` wash `0.50`, L `0.72`, h `310`. Jarvis `e=0` wash `0.28`, L `0.971`. Jarvis `e=1` wash `0.50`. `sense` false does not publish wash on `/dot`. |
| Mouth that spoke on this host | Moved | Engine `system-flite` (libflite `cmu_us_slt`). Same matter line, four columns. Lead, RMS, and length moved for both personas. Samantha F0 moved. Rate was not applied to the PCM. |
| Piper PCM | **not measured** | No Piper binary and no `en_US-ljspeech-medium` pack on this host. The say JSON still prints `"mouth":"piper"` because that is the floor's declared mouth. That string is not a Piper process. |
| Piper `with_emotion` scales | Unit only | `grief` / `Sad` set `length_scale` `1.09` and `sentence_silence` `0.25`. `Neutral`, `neutral`, `""`, and `joy` stay on the caller's scales. Piper reads those scales at spawn. This run did not spawn Piper, and `/v2/say` does not call `with_emotion`. A unit change is not a spoken Piper turn. |

The brief's done line is wash and/or mouth, or an honest FAIL. Wash moved, and the mouth that could speak here moved. The backlog's "actual mouth in use (Piper today)" was not the process that spoke. Do not read this file as a Piper-prosody pass or a clone pass.

Canned empathy lines are absent from the say replies under test. The test path has no model, so the reply is the unreachable line, not "Your mother died this morning." The Flite measurement speaks that matter line on purpose. Words are the matter. No "I hear you."

## What was checked

- `docs/galion-voice-v2/symphony/PERSONA.md`. Gate is a spoken turn that carries prosody and wash, words on the matter.
- `galion-emotion` `StubExpressor`: `e` lerps OKLCH from neutral `(0.971, 0.003, 286)` to heather `(0.72, 0.09, 310)`. The circle fill stays `#F5F5F7`. Heather is the decision colour, not paint on the disc.
- `PiperProsody::with_emotion` on the base returned `self` for every label, including non-neutral. `weights.rs` calls it only with `"Neutral"`.
- `POST /v2/say` already lerped the persona column into the JSON when `e` was set, and then dropped those numbers on the way to the inbox. The mouth used the heartbeat slot. `apply_frame_prosody` applied energy and pause, and ignored pitch and rate.
- Persona columns, not edited: Samantha `e0` pitch `0`, rate `1.00`, loudness `1.00`, pause `0`, wash `0.62`. Samantha `e1` pitch `-0.5`, rate `0.92`, loudness `0.80`, pause `250`, wash `0.50`. Jarvis `e0` pitch `-0.3`, rate `0.96`, loudness `0.92`, pause `80`, wash `0.28`. Jarvis `e1` pitch `-0.4`, rate `0.94`, loudness `0.88`, pause `120`, wash `0.50`.
- `scripts/eval/v2-persona-selftest.sh` is STUB-PASS without a live server. Not this item's pass. Not run.
- `scripts/check-locks.sh` and `scripts/check-no-forbidden.sh`: both OK on this tree.
- Orchestrator plan PR #150 was merged before this branch was rebased. Open drafts #145, #149, #97, #86, #87, #78, #83 were not edited. #156 (`/one` Samantha candidate) was not edited. This diff does not assign `data-persona`.

## What changed

- `sense` on a say queues `MouthProsody` (pitch, rate, loudness, pause) and `PhoneHost` maps it onto `FrameProsody` for that turn. `sense` off leaves the mouth queue empty. Mic turns still use the heartbeat slot.
- `apply_frame_prosody` now applies energy, then a pitch resample (`2^(st/12)`, clamped to ±4 st by `FrameProsody`), then the leading pause. `rate` is stored and is not a time-stretch. No phase vocoder. No Piper respawn.
- `/v2/say` JSON adds `feeling_e` and `oklch` beside the existing `wash_amber` and `prosody`. `GET /dot` adds `feeling_e`, `wash_amber`, and `oklch` only when the last turn had `sense` on. Heartbeat `e` is left as it was.
- The one-dot page, when `/dot` has a numeric `wash_amber`, uses it as the existing amber mix. The mood table is unchanged. No new chrome. The circle stays `#F5F5F7`.
- `PiperProsody::with_emotion` for `sad`, `tired`, `anxious`, `angry`, `grief`, and `vulnerability` sets `length_scale` `1.09` (`1/0.92`, Samantha `e1` rate, two decimals) and `sentence_silence` `0.25` (250 ms). Noise scales stay. The pack is not selected. Joy stays on `self`, matching `empathy_weight`. This hook still does not respawn a warm worker.

## Measurements

Host: this cloud VM. CPU. Date 2026-11-15 22:14 +01:00. Command:

```text
GALION_SPEECH=flite cargo run -p galion-ml-voice --features speech --example persona_live_mouth --quiet
```

No WAV was written.

```text
engine=system-flite
label=fallback system TTS (libflite cmu_us_slt; real words; NOT A21, NOT neural)
text=Your mother died this morning.
rate_applied=false
piper_pack=absent_on_this_run
stub=false
persona=samantha e=0 ... samples=47760 lead_ms=13 rms=0.1921 f0_hz=196.7 pitch_st=0 energy=1 pause_ms=0 rate_knob=1
persona=samantha e=1 ... samples=55159 lead_ms=271 rms=0.1539 f0_hz=192.0 pitch_st=-0.5 energy=0.8 pause_ms=250 rate_knob=0.92
persona=jarvis e=0 ... samples=50514 lead_ms=98 rms=0.1769 f0_hz=193.5 pitch_st=-0.3 energy=0.92 pause_ms=80 rate_knob=0.96
persona=jarvis e=1 ... samples=51756 lead_ms=140 rms=0.1693 f0_hz=192.0 pitch_st=-0.4 energy=0.88 pause_ms=120 rate_knob=0.94
delta samantha e0 -> e1 lead_ms_delta=258 rms_ratio=1.249 samples 47760 -> 55159
delta jarvis e0 -> e1 lead_ms_delta=42 rms_ratio=1.045 samples 50514 -> 51756
verdict=MOUTH_MOVED engine=system-flite
```

Expected from the columns, not from a Piper scale: Samantha energy `1/0.8 = 1.25` (measured RMS ratio `1.249`); pause step about `250` ms (measured lead delta `258`; the `13` ms is Flite onset). Jarvis energy `0.92/0.88 ≈ 1.045` (measured `1.045`); pause step `40` ms (measured `42`). Samantha F0 `196.7 → 192.0` against `2^(-0.5/12) ≈ 0.972` (`192/196.7 ≈ 0.976`). Jarvis `0.1` st is inside the autocorrelation bin; lead and RMS are the Jarvis movers. Both personas used the same Flite voice. The persona difference is the DSP, not a second speaker.

`rate_knob` is printed and was not applied. Do not claim rate moved the waveform.

Piper first-PCM, Piper TTFA, clone PCM: **not measured**.

## Tests

| Command | Result |
|---|---|
| `cargo test -p galion-render --features web --lib sensed_say_moves_wash` | 1 passed (loopback `POST /v2/say` and `GET /dot`, both personas, `sense` on and off) |
| `cargo test -p galion-render --features web --lib` | 129 passed, 0 failed, earlier on this same source |
| `cargo test -p galion-emotion --lib` | 114 passed, 0 failed |
| `cargo test -p galion-ml-voice --lib speech::piper_worker` | 0 tests. The module is behind feature `speech`. |
| `cargo test -p galion-ml-voice --features speech --lib speech::piper_worker` | 5 passed, including `neutral_prosody_keeps_scales` (grief changes length, Neutral and joy do not) |
| `cargo test -p galion-ml-voice --features speech --lib frame_prosody` | 1 passed (`frame_prosody_identity_is_a_noop_and_pause_is_silence`) |
| `cargo test -p galion-ml-voice --features speech --lib pitch_shift_changes_length` | 1 passed |
| `cargo test -p galion-console --features phone-turn --lib` | 88 passed, 0 failed, on this source before the rebase onto `2ecca86`. The rebase did not touch those files. Not re-run after the rebase. |
| `cargo test -p galion-ml-voice --features speech --lib` (full) | **not run**. This host has no t273 originals and no clone weights. The seven failures in backlog §5b are Lane A item A5. |
| `scripts/eval/v2-persona-selftest.sh` | **not run**. STUB-PASS is not this item. |
| `scripts/run-v1-screen.sh --check` | **not run**. That is C2. |
| `dot_console_demo --web` curl | **not run**. The loopback evidence is the render test above. |

`CI` at commit time: **not run**. The pull request checks start on push. This file is not edited again just to paste a pending run.

Pre-existing warning, left alone: `galion-ml-voice` `speech/clip_store.rs` field `counsel` is never read.

## Open questions for Maciej

1. Piper PCM was not measured here. A spoken Piper pass needs the pack on a machine that has it. Until that run exists, the Piper half stays unmeasured.
2. `with_emotion` now writes the Samantha `e1` length and pause for six empathy labels, and still does not respawn the warm worker. `/v2/say` does not call it. Say so if the hook should stay a no-op until a spawn actually reads it.
3. `rate` is carried on the frame and is not in the waveform. Applying it means a respawn or a time-stretch. This lane did not add either.
4. `sense` gates the mouth queue and the `/dot` wash. The say JSON still reports the harness column when `sense` is false. That was already true for `wash_amber`.
5. A0, C5, Emotion V2, and the `/one` persona candidate (#156) are untouched. C5 stays BLOCKED on A0.

— Maciej Grajczyk
