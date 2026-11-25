# S37 — listener feeling

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S37, listener feeling. Stacked on S27 (PR #284, `cursor/horbm-s27-ear-prosody-ba7e`). Not merged. |
| Written | 2026-11-24. No weight download, no voice file, no paid API, no merge. |
| Ear | S27. Pitch, energy, rate, pause, jitter, and shimmer are taken from `ProsodyFrame` as that crate already measures them. This layer does not run a second pitch detector. |
| Words | `galion_emotion::text_affect` and the `LEXICON` table in `galion-emotion/src/persona_mood.rs`. Perpetual Open MIT, (c) 2026 Galion Labs. Used as published. Not copied out. Not a model. |
| Emotion core | `EmotionCore::step` on her own intent. `appraise(&SAMANTHA)` and `project_universal` are used as published. The listener value is not written into the moment or the baseline. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |
| Research dir | `GALION_HORBM_RESEARCH_DIR` was unset in this run and is not read. |

## Reading

One call is still one 80 ms frame. `hear_frame` is Samantha's path. The listener reading is a separate value, `ListenerFeel`. It is not `MomentState` and not `VoiceBaseline`.

Five continuous dimensions, each with a confidence in 0..=1:

| Dimension | Range | What it is |
|---|---|---|
| valence | −1..=1 | pleasure, from tone, then words if they agree |
| arousal | 0..=1 | activation. 0.5 is the circumplex origin |
| tension | 0..=1 | roughness (jitter, shimmer), not a category |
| fatigue | 0..=1 | low, quiet, drawn out. Not her body's fatigue factor |
| confidence | 0..=1 | how assured the voice sounds |

The confidence on each dimension is how much this frame supports that number. A flat quiet tone can have a low vocal-confidence value and a high confidence in that reading.

`scores` is `project_universal` of the listener's valence and arousal. It is a view. It is not a distribution, it does not pick a winner, and it does not drive the step. There is no emotion-category count.

The map is a fixed coupling. It was not fit on a corpus. Louder, higher, and faster raise arousal, the direction Juslin and Laukka (Psychological Bulletin, 2003) treat as the reliable vocal cue. The listener's neutral pitch band is 90–280 Hz. Her stand-in centre, 216 Hz, is not that zero. A 216 Hz tone at the same energy sits above a 176 Hz tone.

Words are the transcript already in hand. This layer does not decode speech. No `ort`. When the word and the tone disagree in sign, and both clear a small threshold, the tone is kept and `prosody_led` is set. "I'm fine" is +0.2 in the lexicon. Said low and flat, the reading stays with the tone (about −0.45) and the word is dropped. The same words on a high, fast tone stay positive, and the word is allowed to nudge.

An unvoiced frame, including silence, is `ListenerFeel::neutral` and does not consult the words.

## Samantha

`response_intent` builds an `IntentMsg` with source `Own`, verb `with`, factor mask 0, consent equal to the sense bit only. The pull is `appraise(&SAMANTHA)` minus her rest, plus a small tension and fatigue nudge. Strength is the reading's confidence. A confidence of 0 adds nothing.

Her body factors are not written. The listener's fatigue does not become `Inside.fatigue`. Adapt is not set, so the S27 inbound offer still does not mix. Remember and share are not set. `persisted_baseline` stays empty. Nothing is written. Samantha is the only persona this layer calls. Aria is parked: there is no Aria type, branch, or call.

`push_frame` is unchanged.

## Consent

The switch is the ear's. `Ear::new` senses. `Ear::deny`, `EarConfig { sense: false }`, and `GALION_EAR_SENSE` set to `0`, `off`, or `deny` close it. Workplace and education stay shut because the ear does not sense for those purposes. While the switch is closed, `hear_frame` returns the neutral reading, does not step the core, and ignores the words. The samples are not read past the ear's own refusal.

`remember` on the ear grant does not open a store and is not copied onto her intent. `PERSISTS` on the ear stays false.

## Cost on this host

Not a lock p95. One process, no cgroup claim. Intel Xeon as reported by the guest (the model string is the generic hypervisor name), 4 cores, Linux 6.12.113+, rustc 1.85.0. Signal: 200 Hz sine, amplitude 0.2, 1280 samples, words `I'm fine`, which is the full path (pitch, lexicon, her step). Eight warmup frames, then 40. `--test-threads=1`.

Five release runs, mean then max, in µs: 55/67, 55/75, 53/53, 55/72, 55/64. An earlier run on the same host reached a max of 92. The number to quote from the five is mean 55 µs, max 75 µs. Debug, one run, mean 1410 µs, max 1539 µs. The test ceiling is the 60 ms compute budget inside the frame.

Reaction, release and debug, same result: a sustained neutral tone, then a low tone or a high tone. Her valence or arousal, compared with a hold that only runs the breath, moves on the cue frame. That is 1 frame, 80 ms. The target is 160 ms. After eight sustained frames her valence is −0.012 against a hold of +0.150 on the sad tone, and her arousal is 0.546 against a hold of 0.374 on the high tone. The listener on those tones is about −0.45 and 0.81. The baseline pitch centre stays 216 Hz.

## Cases

Generated in the test. No audio file.

| Case | F0 Hz | Energy | Rate Hz | Valence | Arousal | Tension | Fatigue | Vocal confidence | Prosody led |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| sad, low, slow | 100.0 | 0.032 | 1.50 | −0.446 | 0.257 | 0.000 | 0.721 | 0.098 | no |
| excited, high, fast | 260.0 | 0.156 | 3.00 | 0.523 | 0.808 | 0.087 | 0.000 | 0.907 | no |
| stressed, tense | 220.7 | 0.127 | 0.00 | 0.125 | 0.634 | 0.454 | 0.000 | 0.413 | no |
| flat "I'm fine" | 105.0 | 0.028 | 0.00 | −0.445 | 0.259 | 0.030 | 0.827 | 0.068 | yes |
| neutral | 176.0 | 0.095 | 0.00 | −0.001 | 0.501 | 0.010 | 0.000 | 0.620 | no |
| switch off | — | — | — | 0 | 0.5 | 0 | 0 | 0 | no, all confidences 0 |

The stressed row is a jittered sine. Jitter is 0.103 against about 0 on the pure tones. That is the voice-quality cue. Its rate is the steady-tone rate, not a fast pattern. Tension is the axis that separates it. The sad and excited rows use on/off bursts so the rate differs (1.50 Hz against 3.00 Hz). The off row does not read the buffer. Her core frame count stays 0.

Reading confidence on the five sensed rows is about 1 (0.97 on the jittered sine). The off row is the one with no claim.

## Tests

`cargo test -p horbm-ear` on rustc 1.85.0. `cargo clippy -p horbm-ear --all-targets --no-deps -- -D warnings` is clean. `scripts/check-locks.sh` reports all Eureka locks hold. The script was not edited. CI stays the existing Ubuntu `gates` workflow. No Windows workflow was added. No `cfg(unix)`, `cfg(windows)`, or `cfg(target_os)` in this layer.
