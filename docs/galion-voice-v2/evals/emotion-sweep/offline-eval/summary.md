# Voice V2 emotion-sweep gate

Offline score of the frozen synthetic clips. No audio was generated and no voice was cloned.

Clips: `docs/galion-voice-v2/evals/listening-set/emotion-sweep`. Manifest: `docs/galion-voice-v2/evals/emotion-sweep/sweep.csv`. Reply line: "I have moved the meeting to Thursday at four, and the notes are in your inbox."

Rollout gate (not a release): WER ≤ 0.018, ECAPA > 0.7057, emotion_match ≥ 0.83 (`0.70*category_blend+0.30*emb_sim` against that persona's resting reference, not a per-sector capture-kit score). `rollout_claimed` is false. A measured pass is not a release. Nothing merges or deploys without Slavik's written go-ahead.

Gate: **unmeasured**. 0/4 candidates measured, 0 passed, 0 failed, 4 unmeasured.

| Candidate | Kind | Clips | WER | ECAPA | emotion_match | Gate |
| --- | --- | ---: | --- | --- | --- | --- |
| samantha | voice-v2 | 18 | absent | absent | absent | unmeasured |
| jarvis | voice-v2 | 18 | absent | absent | absent | unmeasured |
| samantha-v1 | v1-baseline | 1 | absent | absent | absent | unmeasured |
| jarvis-v1 | v1-baseline | 1 | absent | absent | absent | unmeasured |

## Scorers

- wer: absent. galion-wer is not on this machine
- ecapa: absent. galion-ecapa is not on this machine
- emotion_match: absent. superb/wav2vec2-base-superb-er weights are not on this machine. Refusing to download them. The CPU stub was not substituted. emotion_match is unmeasured.

## Blockers

- wer: galion-wer is not on this machine
- ecapa: galion-ecapa is not on this machine
- emotion_match: superb/wav2vec2-base-superb-er weights are not on this machine. Refusing to download them. The CPU stub was not substituted. emotion_match is unmeasured.

Unmeasured is not a pass and not a fail. The CPU stub was not used to fill emotion_match.

## Inventory

46/46 expected files present (36 persona cases, 2 resting references, 2 V1 baselines, 6 user cues). User cues are not candidates. V1 baselines use the same reply line and the same resting reference.

Prosody columns in sweep.csv (f0, rms, rate) belong to that sweep. This report does not copy them and does not treat them as WER, ECAPA, or emotion_match.
