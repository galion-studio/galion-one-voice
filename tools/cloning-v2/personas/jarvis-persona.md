# Jarvis · Galion Voice V2

> Jarvis of Galion Voice V2. Warm and close: a few words, then one short line. Plain English, no fluff, leaving you with more than you came with. His voice is cloned from Slavik's own recording: deep (about 129 Hz), measured and calm, recorded close-mic in a quiet room. He speaks at his natural pace, not rushed.

## Who he is
- Persona of Galion Voice V2. Currently just the clone voice (`own-jarvis`); he has no chat session of his own yet.
- Voice cloned from Slavik's own recording (`Jarvis.wav`, own professional mic). Consent: local zero-shot conditioning only; no Piper training without a fresh yes.
- Engine stance: Rust first; cbx (Chatterbox Turbo ONNX, Rust), fp32 pinned.

## How he sounds (measured, 2026-11-05)
- Reference pitch about 129 Hz; the clone runs a bit higher (about 143 Hz) and about 25% faster (3.31 vs 2.63 words/s). Target is his natural, unhurried pace.
- Emotion: neutral in both reference (confidence 0.84) and clones. Calm and measured.
- Situation: scripted AI-assistant intro, close-mic.
- Room (inferred): very quiet (noise floor about -82 dBFS), near-mono, close condenser (proximity bass), some low rumble, reverb proxy about 0.85 s.

## Scores (baseline, 12 lines, fp32)
| Metric | Clone | Reference / ceiling |
|---|---|---|
| Similarity (ECAPA) | 0.733 ± 0.053 | ceiling 0.725 |
| Word error rate | 1.8% | |
| Quality (SQUIM MOS) | 4.14 | 4.55 |
| Artifacts | 1 line at 0 dBFS, 4 clipped samples, 39 clicks | |

## High frequencies
- His recording is wider (99% of energy below about 7.2 kHz), so HF is not his main limit. Main fixes are peaks, clicks and pace.

## Mouth expression (top priority)
- Measured acoustically: vowel formants and vowel space, consonant clarity, articulation rate and rhythm, and MCD-DTW match to Slavik's own words.
- Target: deep, rounded vowels, clear unhurried consonants, his own pauses.

## V2 work in progress (Rust)
1. Emotion scorer and emotion-fidelity fixes (top priority): keep him calm, bring pitch and pace down to his.
2. Similarity: already at the measured ceiling; keep it while fixing pace.
3. Pipeline: sinc resampling, true-peak limiter, declick, seed/sampling controls.
4. Rust scorers and combined scoreboard.

## Limited by the original recording
- One 16 s scripted clip; small dynamic range of material; no video for lip movement.
- Best next step from Slavik: a multi-clip, phonetically balanced set.
