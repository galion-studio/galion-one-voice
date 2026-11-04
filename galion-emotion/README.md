# galion-emotion

Emotion pipeline for **Galion Voice**, both directions, Rust-first, zero dependencies.

- `frame` — `EmotionFrame` (valence/arousal/dominance/confidence) + coarse `Affect` label, EMA smoothing
- `detect` — `AffectDetector` trait + `EnergyStubDetector` (RMS/ZCR heuristic; consent-gated)
- `express` — `EmpathyPolicy` (de-escalate, never mirror anger) + `ProsodyExpressor` → `ProsodyControl`. `StubExpressor::voice_color` is the one outbound decision: empathy weight `e`, prosody and OKLCH. Not a ninth label, not a reply line. See `docs/galion-voice-v2/EMPATHY.md`.
- `consent` — `ConsentGate` hook, default deny; GDPR / EU AI Act Art. 5(1)(f) & 50(3) / Croatia (NN 42/2018, AZOP)
- `band` — 8-byte `EmotionWire`, `DuplexPacket`, per-direction `EmotionBand` (100 B/s at 12.5 Hz)

Stubs only. No model downloads. Weights: safetensors + .gq only. MIT (Perpetual Open MIT).

```
cargo test
```

## Estimator seam (A11 wave 2) — `loudness`

`GatedDetector<E: AffectEstimator>` owns consent (gate checked before PCM is read), input checks, `seq`, clamp and EMA. The estimator only maps PCM → raw `Estimate { valence: Option<f32>, arousal, dominance, confidence }`; `valence: None` = no opinion (frame carries 0.0, `valence_known() == false`).

- `LoudnessEstimator` — RMS/ZCR heuristic; `EnergyStubDetector` is now a newtype over `GatedDetector<LoudnessEstimator>` (bit-identical frames, tested).
- `SyntheticEstimator` + `SyntheticMode` — deterministic fixtures with valence (`neutral`, `sweep`, `joy`, `angry`, `sad`, `tired`, `calm`, `valence:<v>`, `Script`).
- `ReferenceEstimator` — replays an offline label track (`t_ms LABEL posterior`, SenseVoice-style tags accepted) via a fixed label → VAD table. This is the SenseVoice-Small **offline reference** seam: the track is produced off-box after licence review; nothing is downloaded or run here. `fixtures/affect-ref-synthetic.txt` is hand-written synthetic data, not model output.
- Later: a learned affect head on student latents (`EstimatorKind::Learned`, `.gq` weights) drops in as another `AffectEstimator`.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
