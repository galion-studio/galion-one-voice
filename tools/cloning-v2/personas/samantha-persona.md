# Samantha · Galion Voice V2

> Samantha of Galion Voice V2. Warm and close: a few words, then one short line. Plain English, no fluff, leaving you with more than you came with.

## Who she is
- Persona of Galion Voice V2 (pure voice-to-voice, one centre dot, no text UI).
- Voice cloned from Slavik's own recording (`Samantha.wav`, own professional mic). Consent: local zero-shot conditioning only; no Piper training without a fresh yes.
- Engine stance: Rust first; cloning with cbx (Chatterbox Turbo ONNX, Rust), fp32 pinned.

## How she sounds (measured, 2026-11-05)
- Reference pitch about 178 Hz; the clone runs higher (about 201 Hz) and about 20% faster (3.49 vs 2.82 words/s). Target is her natural pace and pitch.
- Emotion: reference is gently neutral; clones were tagged "angry" on 8/12 lines by a speech-emotion model, likely detector noise on TTS, but the clone is brighter, faster and flatter in loudness than her. Target: calm, warm, unhurried.
- Situation: scripted close-mic character read (Her-style monologue), not casual chat.
- Room (inferred): dry, noise floor about -52 dBFS, dual-mono, weak proximity bass.

## Scores (baseline, 12 lines, fp32)
| Metric | Clone | Reference / ceiling |
|---|---|---|
| Similarity (ECAPA) | 0.707 ± 0.030 | ceiling 0.828 |
| Word error rate | 5.0% | |
| Quality (SQUIM MOS) | 3.73 | 4.33 |
| Artifacts | 4 lines at 0 dBFS, 9 clipped samples, 32 clicks | |

## Key finding: high frequencies
- The source recording has almost nothing above about 2.7 kHz (99% of energy below; 0.4% above 4 kHz). This is her biggest limit: breath, sibilance and "air" are missing.
- V2 fix: Rust reference prep (mono, denoise, declick, loudness to about -20 LUFS, true peak at most -1 dBTP) plus high-frequency restoration. Restored highs are estimated, not recovered; a re-record with real presence is the true fix.

## Mouth expression (top priority, equal to HF)
- Audio-only, so it is measured from sound: vowel formants F1-F3 and vowel-space area, consonant clarity (fricative centroid, plosive bursts), articulation rate and rhythm, and match to Slavik's own words (MCD with DTW).
- Target: same vowel shapes, crisp but soft consonants, her own rhythm and pauses.

## V2 work in progress (Rust)
1. Emotion scorer and emotion fixes (top priority).
2. HF restoration for Samantha's reference.
3. Similarity gap: clean segments, multi-clip pack from her own recording, rep-penalty 1.0 vs 1.2, best-of-N.
4. Pipeline: sinc resampling, float path, true-peak limiter, declick, seed/temperature/top-p controls.
5. Rust scorers: similarity, articulation, quality/WER, plus a combined scoreboard.

## Limited by the original recording
- Missing highs above about 3 kHz; one 17 s scripted clip; can't capture lip movement without video.
- Best next step from Slavik: a re-record with full bandwidth and a phonetically balanced multi-clip set.
