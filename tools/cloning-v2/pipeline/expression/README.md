# Expression (branch B)

Opt-in profile, conditioning plan, and Samantha love/warmth diagnostic.
Sibling of `pipeline/missing-emotion-targets`. Default behaviour of that
stage is unchanged. This crate does not select reference clips (that work
belongs to the fidelity branch). It does not edit persona TOML files, and
it does not deploy a voice.

Crying and yelling are excluded for Samantha and Jarvis. Seductive stays
Samantha-only. Jarvis stays on neutral and logic; his passing gate is left
alone. Piper is the TTS and is not invoked here. Qwen is not used.

## What it measures

For each labelled clip:

- F0 contour (YIN) and its mean, median, semitone SD, and p5–p95 range
- energy (RMS dB) and frame-energy SD
- spectral tilt, dB/octave, on energy-active frames (voice colour)
- sung phrases: a voiced run of at least 0.40 s whose F0 vibrates between 4 and 8 Hz
- emotion2vec+ probabilities and CLAP love/seductive numbers when a score JSON is passed in

The plan copies those numbers forward as conditioning targets. `apply` stays
false until a later stage opts in. The love/warmth layer is a second switch
(`--love-warmth`) and arms only when the persona file lists love.

## Published love gate

Samantha's persona rescore:

| set | emotion_match | gate |
|---|---|---|
| original love decision | 0.9390544380026881 | |
| merge-v2 | 0.6869450087862239 | fail |
| coherent | 0.6713695158531341 | fail |
| clone love match (both) | 0.06094556199731194 | decision 0 |

Jarvis merge-v2 0.8730685436870271 and coherent 0.8791185805790467 pass.

The clone match is `1 - 0.9390544380026881`: every clone line's love decision
is 0, which is what the love plugin writes when the mean cosine against the
love prompt bank is at or below 0.20. The committed window measurement
(w03) has love cosine 0.27918557822704315 and neutral cosine
0.15484974160790443 (margin 0.12433583661913872). Clone cosines are not in
the repo, so the published verdict is `insufficient_evidence`. Supply a
margin or a prosody delta and the diagnostic returns `real_loss`,
`detector_miss`, or `mixed`.

Replacing the love row with a perfect match would lift merge-v2 and coherent
over 0.83. The 0.83 gate itself is not lowered. `own-samantha.toml` still
records the legacy universal-CLAP love target 0.8109382317899566.

## Commands

From `tools/cloning-v2` (this crate's `rust-toolchain.toml` selects Rust 1.85, which current crates.io indexmap needs):

```
cd pipeline/expression
cargo test
cargo build --release
cd ../..
python3 pipeline/expression/py/crosscheck_expression.py
```

```
pipeline/expression/target/release/galion-expression diagnose \
  --case pipeline/expression/fixtures/samantha_love_gate.json
```

```
pipeline/expression/target/release/galion-expression analyze \
  --voice own-samantha \
  --channel love=/path/to/clip.wav \
  --scores /path/to/score_persona.json \
  --out /tmp/profile.json

pipeline/expression/target/release/galion-expression plan \
  --profile /tmp/profile.json \
  --persona pipeline/personas/own-samantha.toml \
  --love-warmth --published-love-prior \
  --out /tmp/plan.json
```

## Laptop eval (RTX 4060 8 GB)

One file, one CLAP load. Emotion2vec+ stays on the CPU provider inside
`score_persona.py`. Do not start a second model in the same process.

```
cd tools/cloning-v2
( cd pipeline/expression && cargo build --release )

# Prosody only (no weights):
pipeline/expression/target/release/galion-expression eval \
  --wav /path/to/consent-or-cc0.wav \
  --voice own-samantha \
  --emotion love \
  --persona pipeline/personas/own-samantha.toml \
  --case pipeline/expression/fixtures/samantha_love_gate.json \
  --out-dir /tmp/galion-expression-eval \
  --device cpu --gpu-mem-gb 8

# CLAP love/neutral margins on the 4060. Weights must already be local.
GALION_CLAP_DEVICE=cuda \
pipeline/expression/target/release/galion-expression eval \
  --wav /path/to/consent-or-cc0.wav \
  --voice own-samantha \
  --emotion love \
  --persona pipeline/personas/own-samantha.toml \
  --case pipeline/expression/fixtures/samantha_love_gate.json \
  --clone-obs /tmp/clone_observation.json \
  --out-dir /tmp/galion-expression-eval \
  --score --love-warmth \
  --device cuda --gpu-mem-gb 8
```

`--score` runs `pipeline/expression/py/eval_obs.py`, which calls the existing
love plugin. `GALION_CLAP_DEVICE=cuda` moves that CLAP module; the default
remains CPU, so persona scores measured without the variable stay as they are.

Own-voice audio is consent-local and is not in git. Tests fetch
[File:Ului12.wav](https://commons.wikimedia.org/wiki/File:Ului12.wav) (CC0)
at runtime.
