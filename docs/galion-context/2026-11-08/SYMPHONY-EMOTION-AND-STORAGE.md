# Symphony emotion and storage

Verified on voice `main` `3e1f5a12` unless marked historical or proposed. No audio was scored in this pass.

Three layers are not the same thing: how an emotion is **inferred**, how a **policy** answers it, and whether anything is **stored**.

## Inference (what is recognised)

`galion-emotion/src/detect.rs` is explicit: “Phase 2 stub: RMS energy + zero-crossing rate”. Valence stays unknown unless the caller already supplies one. Confidence is the heuristic constant, not a measured accuracy. The comment says a future head on shared encoder latents is the intended replacement, and that no separate SER model is on this path.

`galion-emotion/src/loudness.rs` states the same: arousal from loudness, dominance from loudness minus zero-crossing rate, valence unknown.

`recognize` maps those features onto eight `Affect` labels (the closed set in that crate). It does not emit the twelve Emotion V2 names.

The Nebula live feed refuses a four-class SUPERB head and a nine-class emotion2vec head rather than expanding them (`NEBULA-LIVE-FEED.md`). The offline nebula score (`nebula.py`) also refuses to invent that map. Both require a distribution the caller already has. Neither reads a wav for that field.

The harness can name `superb/wav2vec2-base-superb-er` and emotion2vec in `emotion-v2/emotions.toml`. The CI workflow runs the CPU stub only. A GPU nightly is documented as an operator run and is not a job. Historical laptop notes say Samantha `emotion_match` was below 0.83. Not remeasured here.

The browser Nebula does not consume the live feed. It plays `demo.rs`.

## Policy (how a reply is shaped)

Two policies exist and are not composed.

**Counterbalance** in `galion-emotion/src/film.rs` `check_counterbalance`: target valence must not be negative. Joy and Excited may be mirrored. Angry or Anxious must not be met with equal or higher arousal, and Angry must not be met with equal or higher dominance. Sad or Tired must not be pushed far above the caller’s arousal. Tests `naive_mirror_policy_is_rejected_for_negative_callers` and `mirror_policy_is_rejected` pin that. `StubExpressor::respond` is what those tests drive.

**Persona mood** in `galion-emotion/src/persona_mood.rs` `appraise`: Samantha `mirror_sorrow` is 0.45 and `valence_floor` is −0.35, so a strong negative user valence can produce a **negative** mood valence. Distress then calms arousal. The test `angry_user_is_not_mirrored` checks arousal under 0.5 and a longer pre-pause. It does not check `check_counterbalance`. The module comment says Samantha mirrors, then softens. `PERSONA-MOOD.md` says an angry user is not mirrored, meaning the voice gets quieter and slower, not that valence stays non-negative.

Nothing in the tree passes a persona-mood target into `check_counterbalance` as a failing test. `persona_mood_alloc.rs` only counts the boolean. There is no runtime resolution.

`galion-demo/galion_demo.py` documents COUNTERBALANCE and uses `respond` for the reply (the spoken turn). `mirror()` returns a copy of the caller and exists so the self-test can reject it (`galion-demo/tests/test_galion_demo.py` asserts `check_counterbalance` fails on `mirror`). It is not the path the demo speaks. `docs/agent-wave20/W20-04-STATUS.md` (2026-11-02) still says the Python demo mirrors. That sentence is stale against the current file. The same conclusion is published in [pull request 103](https://github.com/galion-studio/galion-one-voice/pull/103) (`f6832719`, `01-emotion-storage-and-unfinished-work.md`). The self-test was not executed in this review.

`docs/galion-voice-v2/SAMANTHA-V2.md` says anger is still a pause, never a mirror of anger. That matches the film policy’s intent and only partly matches `appraise`.

## Transport

`EmotionWire` is eight bytes (`galion-emotion/src/band.rs`). The live feed says it does not widen that wire. Nebula is a side JSON stream, not the heartbeat emotion word.

Rika (`rika.rs`) advances once per 80 ms frame. The comment says RAM only, and that today the stub uses RMS and zero-crossing rate, with tension as a future primary dimension.

## Persistence

`AffectStore` (`store.rs`) is the only place inbound affect may live after detection. Default state is `Unconsented`. `record` does not exist until a gate allows and the SENSE bit is set. Bits default off: SENSE, ADAPT, REMEMBER, SHARE.

`baseline_for_persist` returns the baseline only when REMEMBER is set and at least one sample exists. The only callers in the repository are tests in `store.rs`. Drop wipes the ring and the baseline. The comment says persisting is the host’s job **before** drop, and only with REMEMBER. No host in this tree writes that baseline to disk.

SHARE also needs a spoken confirmation. `spoken::classify` is a keyword stub on an already transcribed line.

`PersonaMood` is called persistent in `persona_mood.rs` and in `PERSONA-MOOD.md`. The code is a session struct with a half-life (Samantha 20 s, Jarvis 12 s). That is memory across turns in one process, not a file.

Rika state is wiped on OFF. No Rika disk format exists.

Nebula weights are not written to a user store. The offline score is a report object.

## What would count as proof later

A test or a log that shows a baseline leaving the process, surviving restart, and disappearing when REMEMBER is cleared or consent is withdrawn. A single API that can return `Some` is not that proof. This review did not run the process.
