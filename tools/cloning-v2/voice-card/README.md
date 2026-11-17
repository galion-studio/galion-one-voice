# galion-voice-card — Stage 0 pre-clone analysis

Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.

Mate t1366u / t1368u: before cloning, work out the **voice, the emotion and the environment** of a
reference clip, and write a persona card that says much more than pitch.

```
galion-voice-card analyze --wav REF.wav --voice-id ID --out-dir DIR \
    [--name "Display name"] [--consent-ref "where consent lives"] [--config stage0.toml] [--repo-root DIR]
```

Output in `DIR`: `ID.voice.toml` (machine-readable, schema `galion-voice-card/1`), `ID.voice.md`
(short, human) and `raw/ID.<brick>.json` (each brick's untouched output). Emotion windows are written
to `DIR/raw/ID-windows/` (16 kHz mono, scratch — keep out of git).

**Universal (t1362u):** the voice is only an input. There is no per-voice code path; one config
(`stage0.toml`) for every voice. Tested on the Samantha reference and on the CC0 donor (Amy Koenig,
LibriVox reader 6620) with the same binary and config.

## What the card holds
| Section | Fields | Brick |
|---|---|---|
| identity | voice id, file name, sha256, duration, consent ref (stage 0 never checks consent; galion-consent gates cloning) | glue |
| pitch | median/mean, p5-p95, range st, SD st, voiced fraction, YIN cross-check, nearest note at A4 = 432 Hz + cents | galion-artic (pYIN) |
| timbre | vowel F1-F3 medians, F1/F2 ranges, vowel-space area, dispersion, refcheck tilt, LTAS tilt, alpha ratio, Hammarberg, HF tilt, f90/f99, energy above 4/8 kHz | galion-artic, refcheck, proxies.rs, galion-hygiene |
| tempo | speech and articulation rate (syl/s), syllables, pauses/min, median pause, pause ratio | galion-artic |
| voicing | jitter, shimmer (% and dB), glottal cycles, HNR proxy (YIN aperiodicity, labelled as such) | proxies.rs |
| loudness | LUFS (mono downmix and all channels), true peak, RMS, LRA, frame dynamic range | refcheck, galion-hygiene, proxies.rs |
| articulation | fricative/sibilant centroid, spread, skew, burst centroid/level/rate, VOT, long-lag fraction | galion-artic |
| emotion | emotion2vec+ whole-clip 14-emotion posterior + raw 9-class probs, 3 s window timeline, window top counts and mean posterior | galion-emotion12 (ort) |
| environment | noise floor, SNR estimate, RT60 (or why not), DC, clipping, clicks, rate/bits/channels, L/R correlation and dual-mono, refcheck verdict | refcheck, galion-hygiene, glue |
| delivery | plain-language summary from config bands (numbers stay raw) | glue |
| quality_limits, prep | what limits the clone; what refprep's shared config will do with these numbers (HF shelf, gate, loudness) | glue + refprep `configs/default.toml` |
| clone_targets | F0, rate, pause ratio, F1-F3, fricative centroid, LUFS with tolerances from config; gate WER <= 1.8 %, ECAPA > 0.7057, emotion >= 0.83 | glue |
| unmeasured | every field that could not be measured, with the reason | glue |

Nothing is invented: a missing brick or an unmeasurable value goes to `[[unmeasured]]`.
The per-emotion **gate** score (0.70 category + 0.30 embedding vs that emotion's reference of the
same voice) needs one reference per emotion, so stage 0 marks it pending; it reports the baseline
posterior only. The 14-emotion posterior is emotion2vec+'s 9 classes projected by the gate-emotions
file; shared classes are split evenly (config), so neutral = calm by construction.

## Bricks reused (assemble, don't code)
| Brick | Where | Licence |
|---|---|---|
| refcheck | tools/emotion-v2/refcheck | MIT (Galion Labs) |
| galion-artic | tools/cloning-v2/scorers/articulation | Galion (repo MIT) |
| galion-hygiene | tools/cloning-v2/scorers/quality/galion-quality | Galion (repo MIT) |
| proxies.rs + audio.rs (compiled in via `#[path]`, same source file, no copy) | tools/cloning-v2/scorers/emotion/src | Apache-2.0 OR MIT |
| galion-emotion12 (emotion2vec+ base via ort, separate process) | A2's tools/emotion-v2/emotion12 (not on main yet) | MIT; model export ziyu12345/emotion2vec_plus_base_onnx, model card MIT — recheck upstream emotion2vec+ licence before any public release (t1360u) |
| refprep shared config (read only) | tools/cloning-v2/refprep/configs/default.toml | MIT |
| crates: hound, rustfft, serde, serde_json, toml, sha2, anyhow, indexmap (=2.2.6) | crates.io | MIT and/or Apache-2.0 |

No `ort` or `candle` in this crate; it has its own `[workspace]`, so the locked voice workspace
(LOCKS.toml) does not build it. The ONNX model runs only inside galion-emotion12.

## Bricks and environment
Paths come from `stage0.toml` (relative to the repo root) or env overrides:
`GALION_REFCHECK_BIN`, `GALION_REFCHECK_SPEC`, `GALION_ARTIC_BIN`, `GALION_HYGIENE_BIN`,
`GALION_EMOTION12_BIN`, `EMOTION_V2_GATE_EMOTIONS`, `GALION_REFPREP_CONFIG`, plus
`ORT_DYLIB_PATH` and `EMOTION_V2_MODELS_DIR` for galion-emotion12 (never downloads).
`VOICE_CARD_TIMESTAMP` sets the `generated` field; with it fixed, two runs give byte-identical TOML.

## Build and test
```
cd tools/cloning-v2/voice-card
cargo build --release && cargo test --release
```
Box note (rustc 1.85): galion-artic and galion-quality do not build as-is there (artic is not
excluded from the root workspace; galion-quality deps need a newer rustc), so the run used their
existing release binaries. The laptop toolchain builds them normally.
