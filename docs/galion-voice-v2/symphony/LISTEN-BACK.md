# Listening back

Perpetual Open MIT License. (c) 2026 Galion Labs.

Layer S7 of MnE V2. The crate is `tools/symphony/listen-back`. It is the closed eval loop, not a renderer and not a gate.

```text
fixture → intent (S1 stub, or GALION_MNE_CONTRACT)
       → feeling (S2 stub, or GALION_FEELING)
       → body (lungs via GALION_LUNGS when set; throat S3, mouth S4, nose S5 stubbed)
       → sound out (synthetic vowel, or GALION_RENDER_CMD)
       → decode (galion-articulation analyse; GALION_ARTICULATION adds galion-voice-decode/2)
       → score → one correction → decode → score
```

One correction. The baseline voice is kept as its own field and is not steered. Reports are `galion-listen-back/1` JSON plus a short text summary.

The rollout gate stays WER ≤ 1.8%, ECAPA > 0.7057, emotion ≥ 0.83. The report records those numbers and sets `measured` to false. No dataset is downloaded and no real-person audio is committed.

CI is `cargo test --manifest-path tools/symphony/listen-back/Cargo.toml` on synthetic tones. A local cloner is `--render-cmd` or `GALION_RENDER_CMD`.
