# Fidelity (Symphony branch A)

Reference conditioning and an offline score report for Cloning V2. Rust is the
implementation. `py/crosscheck_fidelity.py` recomputes the closed-form numbers
from `galion-fidelity self-check`. Piper stays the TTS. This tool does not
synthesize and it does not evaluate consent.

Clone-time consent stays one `galion_consent::evaluate` call in
`crates/galion-consent`. Nothing here reads `GALION_CLIP_STORE` or writes audio
into the git tree. CC0 / public-domain clips are downloaded at eval time into
a cache outside the repo and are never committed. No weights are committed.

## What it does

1. **Segments.** 2 s windows, scored
   `0.50 * clamp(snr_db/30, 0, 1) + 0.30 * voiced_ratio + 0.20 * pitch_coverage`
   (8 log-spaced pitch bins, 70–400 Hz). The best window is placed first and
   the rest follow in time order, joined with a 15 ms crossfade.
2. **Length.** Under 8 s (a ~6 s best piece, including the current short
   Samantha and Jarvis references) the clip is not repeated to fill 50 s.
   At 8 s and above, assembly stops at 50 s. The LM slice is the first 6 s of
   that wav. Speaker tokens use the first 10 s. The speaker embedding pools
   the whole assembly.
3. **Embeddings.** A 48-D gain-robust log-mel mean/std vector. Multi-reference
   inputs from the **same speaker** are L2-normalised, averaged (duration
   weights per segment), and normalised again. This vector is not ECAPA.
   ECAPA vectors, when `galion-ecapa` has written them, use the same average.
4. **Envelope (opt-in).** `galion-fidelity timbre` matches a clone's cepstral
   spectral envelope to the reference, ±6 dB, 1/3-octave smoothed, RMS
   restored. Strength 0 is a copy. Short refs recommend strength 0.35; long
   refs recommend 0.55. Prepare does not bake this in.

## Laptop command

On galion-laptop (RTX 4060 8 GB), from the repo root:

```bash
tools/cloning-v2/fidelity/scripts/eval-laptop.sh
```

DSP runs on CPU. Optional local scorers, one file at a time, at most 2 threads:

```bash
GALION_ECAPA_BIN=.../galion-ecapa GALION_ECAPA_MODEL=.../ecapa.onnx \
GALION_WER_BIN=.../galion-wer GALION_FIDELITY_REF_TEXT='...' \
GALION_PERSONA_JSON=.../persona.json \
  tools/cloning-v2/fidelity/scripts/eval-laptop.sh
```

`GALION_FIDELITY_LONG_WAV` may point at a local public-domain wav outside the
repo. Do not average Amy with Alan; they are different speakers. The script
scores them separately.

Reports land in `${XDG_CACHE_HOME:-$HOME/.cache}/galion-fidelity/out/`.
Rollout numbers are reporting only: WER ≤ 1.8%, ECAPA > 0.7057,
emotion_match ≥ 0.83. Unmeasured fields stay null. The DSP cosine is not ECAPA.

## Tests

```bash
cargo test --manifest-path tools/cloning-v2/fidelity/Cargo.toml
python3 tools/cloning-v2/fidelity/py/crosscheck_fidelity.py "$CACHE/out/self-check.json"
```
