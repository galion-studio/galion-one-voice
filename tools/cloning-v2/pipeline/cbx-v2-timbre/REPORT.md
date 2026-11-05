# Shared tone-colour / depth stage — cbx-v2-timbre

**When:** 2026-11-05 evening Europe/Zagreb (CET)  
**Work root:** `/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/cbx-v2-timbre/`  
**Why a copy:** isolate from concurrent `pipeline/cbx-v2` / other workers; originals + `own-samantha` / `own-jarvis` / `*.bak` untouched.

## What was built

| | |
|---|---|
| Tree | `cloning-v2/pipeline/cbx-v2-timbre/` (copy of cbx-v2 + timbre module) |
| Binary | `…/cbx-v2-timbre/target/release/cbx` |
| Module | `src/timbre.rs` — shared reference-driven LTAS / feature EQ |
| Speak flags | `--timbre-match 0..1` (**default 0 = off**), `--timbre-ref PATH` |
| Offline CLI | `cbx timbre apply --input … --reference … --strength … --out …` |
|             | `cbx timbre analyze --input … --reference …` |
| Pitch | **not** shifted (no `--pitch-semitones` in this stage) |

### Algorithm (same code for every voice)

1. Build 1/3-octave-smoothed LTAS of the voice’s own `reference.wav` and of the clone utterance.
2. Band-limit correction to ~80 Hz … clone **f99** (no invented air above the clone’s natural bandwidth).
3. Blend: smoothed LTAS residual (weighted) + body shelf (150–400 Hz) + alpha-ratio + Hammarberg soft targets derived from **ref − clone** deltas.
4. Clamp ±6 dB, STFT-OLA EQ, RMS preserve, then existing declick + −1 dBTP limiter.
5. Strength scales all corrections; `0` short-circuits to identity **before** any FFT.

No per-Samantha / per-Jarvis branches — pass each voice’s reference and the same flag.

### Flag-off bit-identical

- Unit test `timbre::tests::strength_zero_is_identity` **PASS**.
- Two `timbre apply --strength 0` runs on Sam t01 → **identical SHA-256**.
- Strength 0 before/after LTAS metrics identical on written files.

## Evaluation method

- **Inputs:** `own-clone-analysis/wav/{samantha,jarvis}/t01–t12.wav` (temp=0 analysis set).  
- **Post-only** via `cbx timbre apply` (box overloaded; other workers holding ~2 GB TTS sessions — avoid re-TTS OOM). Stage is post-decoder; numbers are valid for the matcher itself. Speak path is wired for production use with `--timbre-match` + `--timbre-ref`.
- Strengths **0 / 0.5 / 1.0**; hygiene on; threads=2; `nice -n 10`.
- Refs: `clip-store-own/{samantha,jarvis}/reference.wav`.
- Scorers: `galion-ecapa`, `galion-emotion` (fp32), `galion-{wer,mos,hygiene}`; LTAS via `cbx timbre analyze`.

## Acceptance (ECAPA drop ≤ 0.01 vs strength 0)

| Voice | s=0 | s=0.5 | Δ | s=1.0 | Δ | Gate |
|---|---:|---:|---:|---:|---:|---|
| Samantha | **0.7072** | **0.7039** | −0.0033 | **0.7003** | −0.0069 | **PASS** both |
| Jarvis | **0.7328** | **0.7331** | +0.0003 | **0.7338** | +0.0010 | **PASS** both |

## Per-voice before / after (means over t01–t12)

### Samantha

| Metric | s=0 | s=0.5 | s=1.0 | Ref |
|---|---:|---:|---:|---:|
| ECAPA | 0.7072 | 0.7039 | 0.7003 | ceil 0.828 |
| LTAS L2 (dB) | 4.316 | **4.047** | 4.581 | 0 |
| Alpha ratio (dB) | 15.59 | 16.24 | **16.96** | 17.19 |
| Hammarberg (dB) | 18.50 | 18.21 | **18.09** | 17.23 |
| f99 (Hz) | 2604 | 2655 | 2669 | 2660 |
| Body/mid (dB) | 15.26 | 16.38 | **17.49** | (ref-driven) |
| WER | 0.050 | 0.050 | 0.050 | |
| MOS (SQUIM) | 3.75 | 3.75 | **3.86** | |
| Emotion labels | ang7 neu3 hap2 | ang7 neu3 hap2 | ang7 neu3 hap2 | |
| Clicks / clipped | 5 / 0 | 9 / 0 | 12 / 0 | |

### Jarvis

| Metric | s=0 | s=0.5 | s=1.0 | Ref |
|---|---:|---:|---:|---:|
| ECAPA | 0.7328 | **0.7331** | **0.7338** | ceil 0.725 |
| LTAS L2 (dB) | **3.806** | 4.508 | 5.870 | 0 |
| Alpha ratio (dB) | 12.27 | 12.24 | 12.21 | 12.12 |
| Hammarberg (dB) | 15.73 | 15.58 | **15.35** | 14.78 |
| f99 (Hz) | 5256 | 5670 | **6063** | 7219 |
| Body/mid (dB) | 12.38 | 12.46 | 12.53 | |
| WER | 0.019 | 0.019 | 0.019 | |
| MOS (SQUIM) | **4.12** | 4.12 | 4.09 | |
| Emotion labels | neu12 | neu12 | neu12 | |
| Clicks / clipped | 0 / 0 | 4 / 0 | 10 / 0 | |

**Note on LTAS L2:** absolute per-bin L2 vs a *different-text* reference is noisy (esp. Jarvis wider-band ref). Feature deltas (alpha / Hammarberg / body / f99) are the intended product signals; they move toward the reference for both voices. Samantha L2 improves at 0.5; Jarvis L2 can rise while features still improve — do not gate on cross-utterance L2 alone.

## Speed cost

| Op | Wall (CET box, nice 10, 2 threads) |
|---|---|
| Single apply (Sam t09, s=0.5) | **~0.19 s** process wall (~60–120 ms in-process) |
| 12-line batch apply | **~1.4 s** (~0.12 s/utt) |
| vs TTS synth | negligible (orders of magnitude below fp32 Turbo speak) |

CPU-light: pure Rust FFT STFT, no ONNX in the timbre path.

## Recommendation

| Flag | Verdict |
|---|---|
| `--timbre-match 0` (default) | Keep as default — bit-identical baseline. |
| **`--timbre-match 0.5`** | **Recommended promote setting** for both voices: ECAPA within −0.01, tone features closer to ref, WER flat, MOS flat/↑ (Sam), Jarvis stays `neu`, modest click uptick. |
| `--timbre-match 1.0` | Gate-pass on ECAPA but more clicks; Jarvis MOS −0.04; use only if depth/body is prioritized and hygiene re-check is acceptable. |

**Premium speak example (shared):**
```bash
cbx --intra-threads 2 speak --dtype fp32 --temperature 0 --voice own-<p>-v2-… \
  --timbre-match 0.5 \
  --timbre-ref /workspace/swarm/voice-v2-sprint/clip-store-own/<p>/reference.wav \
  --text "…" --out-wav out.wav
```

Or post-hoc without re-TTS:
```bash
cbx timbre apply --input clone.wav --reference ref.wav --strength 0.5 --out out.wav
```

## Limits (honest)

1. Evaluated **post-only** on analysis WAVs under box load; full speak+timbre A/B when quieter is still desirable as a smoke check.
2. Cross-utterance full-bin LTAS L2 is an imperfect distance; feature metrics preferred.
3. STFT EQ can add a few residual clicks at high strength — keep 0.5 for production.
4. Does not fix Samantha missing >3 kHz source air (hf-samantha) or Turbo emotion/CFG (emotion-fix).
5. No new `own-*-v2-timbre` voice profiles created — this is a **shared post flag**, not a re-encode.

## Artefacts

- Wavs: `cbx-v2-timbre/wav/{samantha,jarvis}/s{0p0,0p5,1p0}/t01–t12.wav`
- Scores: `cbx-v2-timbre/scores/*_{ecapa,ltas,emo,wer,mos,hyg}.json`
- Logs: `cbx-v2-timbre/logs/`
