# Emotion blend: measured profile and global pipeline fix

All measurements are from the local recordings and Rust scorers. Originals/deployed voices/backups were not changed. No TTS renders by A-global.

## Bottom line

**Partly supports the theory, not a literal single-tone collapse.** Samantha loses pitch contrast and some temporal SER diversity; output probabilities shift toward anger-like classification. But pooled short-window entropy and arousal variance do not collapse. Jarvis does not show pitch narrowing. The concrete shared information-loss layer is the exported encoder’s **first-six-second audio conditioning crop**, not an invented Rust emotion-averaging function.

## 1. Original whole recordings

| Recording | seconds | neu | hap | ang | sad | H bits | A / V / D | F0 median Hz | F0 SD st | F0 p5–p95 st | energy DR dB | speech syl/s |
|---|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|
| samantha | 17.407 | 0.349889 | 0.213338 | 0.119154 | 0.317619 | 1.896815 | 0.4990 / 0.5528 / 0.5685 | 176.157 | 2.9560 | 9.1121 | 28.7615 | 2.8790 |
| jarvis | 15.952 | 0.853256 | 0.135574 | 0.008683 | 0.002488 | 0.667168 | 0.5687 / 0.6979 / 0.6806 | 127.546 | 3.1798 | 10.3833 | 22.8684 | 2.0997 |

**Interpretation caution:** SER probabilities are classifier confidence/uncertainty, not percentages of literal feelings. “Sweet” and “playful” are not labels this model can identify. Whole-file inference is nonlinear: Samantha’s whole-file sadness probability 0.3176 is not a temporal sadness fraction; every short window has <=0.0374 sadness probability. A four-class model cannot validate the exact owner-described blend by itself.

### samantha: original temporal profile

| start–end s | neu | hap | ang | sad | H bits | A | V | D | F0 SD st | F0 range st | energy DR dB | speech syl/s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.000–2.487 | 0.9461 | 0.0348 | 0.0119 | 0.0072 | 0.3713 | 0.5262 | 0.4725 | 0.5862 | 3.075 | 11.098 | 31.326 | 3.614 |
| 2.487–4.973 | 0.9632 | 0.0192 | 0.0040 | 0.0136 | 0.2775 | 0.4282 | 0.5050 | 0.5048 | 2.215 | 6.408 | 29.234 | 2.410 |
| 4.973–7.460 | 0.9308 | 0.0655 | 0.0036 | 0.0000 | 0.3838 | 0.4089 | 0.5519 | 0.4936 | 1.720 | 4.904 | 24.905 | 4.065 |
| 7.460–9.947 | 0.3437 | 0.0563 | 0.5843 | 0.0156 | 1.3100 | 0.4833 | 0.4522 | 0.5350 | 2.866 | 8.115 | 29.842 | 2.791 |
| 9.947–12.434 | 0.5652 | 0.3550 | 0.0781 | 0.0017 | 1.2984 | 0.3972 | 0.6407 | 0.4985 | 2.541 | 9.199 | 32.626 | 2.410 |
| 12.434–14.920 | 0.6546 | 0.0161 | 0.2919 | 0.0373 | 1.1919 | 0.4694 | 0.4578 | 0.5515 | 2.388 | 7.153 | 30.807 | 2.008 |
| 14.920–17.407 | 0.0243 | 0.0113 | 0.9620 | 0.0025 | 0.2785 | 0.5638 | 0.3631 | 0.6110 | 3.657 | 12.135 | 27.328 | 1.896 |

### jarvis: original temporal profile

| start–end s | neu | hap | ang | sad | H bits | A | V | D | F0 SD st | F0 range st | energy DR dB | speech syl/s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.000–2.659 | 0.9886 | 0.0113 | 0.0001 | 0.0000 | 0.0907 | 0.5181 | 0.7331 | 0.6348 | 2.025 | 6.796 | 21.291 | 2.459 |
| 2.659–5.317 | 0.6968 | 0.0325 | 0.2704 | 0.0002 | 1.0370 | 0.6226 | 0.7311 | 0.7219 | 4.132 | 10.737 | 19.433 | 3.008 |
| 5.317–7.976 | 0.9983 | 0.0009 | 0.0008 | 0.0000 | 0.0194 | 0.5209 | 0.6182 | 0.6121 | 2.815 | 9.487 | 24.938 | 2.256 |
| 7.976–10.635 | 0.9776 | 0.0119 | 0.0105 | 0.0000 | 0.1773 | 0.5306 | 0.4802 | 0.6124 | 1.933 | 5.845 | 22.496 | 1.128 |
| 10.635–13.293 | 0.9867 | 0.0065 | 0.0067 | 0.0001 | 0.1158 | 0.5016 | 0.4293 | 0.6104 | 2.762 | 9.955 | 22.986 | 2.439 |
| 13.293–15.952 | 0.8903 | 0.0708 | 0.0390 | 0.0000 | 0.6023 | 0.4822 | 0.4598 | 0.6039 | 4.027 | 10.437 | 26.351 | 2.041 |

Samantha: neutral-like first ~7.46s, strongest happy-like window 9.947–12.434s (hap 0.3550, V 0.6407), strong anger-like tail 14.920–17.407s (ang 0.9620). The positive-valence/happy-like example is **outside the six-second conditioning crop**, including after the existing r2 rotation. Tiny boundary shifts materially change SER confidence; do not read these labels as human judgments. Jarvis is predominantly neutral-like in all windows with smaller transient anger/happiness probabilities.

## 2. Production own-samantha / own-jarvis versus original

Production WAVs are the existing 12-line merged-v2 renders: Samantha r2 @ T0 + timbre 0.5, Jarvis baseline + timbre 0.5. Every line’s whole-file SER/AVD/proxies and all generated temporal windows are in `profile.json`; articulation profiles were freshly run with `galion-artic -j 2`.

### Comparable short-window summaries

| Persona/cohort | n windows | mean neu/hap/ang/sad | mean H bits | temporal JS bits | mean probability spread | F0 SD st | F0 range st | energy DR dB | A variance |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| samantha/original | 7 | 0.6326 / 0.0798 / 0.2765 / 0.0111 | 0.730189 | 0.563755 | 0.7942 | 2.6373 | 8.4304 | 29.4382 | 0.00326848 |
| samantha/pre_timbre | 23 | 0.4795 / 0.1105 / 0.4074 / 0.0027 | 0.960846 | 0.449330 | 0.7124 | 2.2790 | 7.1800 | 27.8904 | 0.00414947 |
| samantha/merged | 23 | 0.4684 / 0.1057 / 0.4225 / 0.0034 | 0.984013 | 0.424446 | 0.7196 | 2.3150 | 7.1912 | 27.7662 | 0.00396224 |
| jarvis/original | 6 | 0.9231 / 0.0223 / 0.0546 / 0.0001 | 0.340436 | 0.118423 | 0.9230 | 2.9489 | 8.8762 | 22.9158 | 0.00198113 |
| jarvis/pre_timbre | 25 | 0.7816 / 0.0548 / 0.1629 / 0.0007 | 0.746321 | 0.194987 | 0.7910 | 3.4911 | 10.6601 | 24.7705 | 0.00120766 |
| jarvis/merged | 25 | 0.7782 / 0.0563 / 0.1649 / 0.0006 | 0.735461 | 0.215047 | 0.7950 | 3.4681 | 10.4323 | 24.2981 | 0.00119175 |

H is within-window uncertainty; temporal JS = H(mean probabilities) − mean H measures time-varying categorical contrast. They are different quantities. Probability ranges/variances for every class are in JSON. Original vs generated text differs, so this is descriptive, not matched-text causal proof. Production pooled A variance includes line-to-line changes. Mean within-line A variance is Samantha 0.00164089 / Jarvis 0.00037802, but 1-window lines necessarily give zero and are shorter than the originals.

**Where the loss is visible:** Samantha window F0 p5–p95 range 8.4304→7.1912 st, F0 SD 2.6373→2.3150 st, temporal JS 0.563755→0.424446 bits (−24.7%), energy DR 29.4382→27.7662 dB; mean anger probability 0.2765→0.4225. **Not lost:** pooled mean H rises 0.730189→0.984013 bits; pooled A variance rises 0.00326848→0.00396224. No honest conclusion of universal emotion-variance collapse is supported. Jarvis F0 range/SD and temporal JS increase, while A variance decreases.

### Whole-file comparison (original versus mean of 12 complete output lines)

| Persona/cohort | H bits | F0 SD st | F0 range st | energy DR dB | speech syl/s |
|---|---:|---:|---:|---:|---:|
| samantha/original | 1.896815 | 2.9560 | 9.1121 | 28.7615 | 2.8790 |
| samantha/merged 12-line mean | 1.115658 | 2.5992 | 7.9730 | 28.1148 | 3.4692 |
| jarvis/original | 0.667168 | 3.1798 | 10.3833 | 22.8684 | 2.0997 |
| jarvis/merged 12-line mean | 0.858751 | 3.6700 | 11.8903 | 24.3055 | 2.6453 |

Independent Rust articulation corroborates Samantha pitch narrowing: original pYIN range/SD 9.502/3.045 st → output-line means 7.7497/2.4544 st. Its energy DR is 20.631→20.7386 dB (not compressed), and syllable speech pace 4.142→4.9407 syl/s. Jarvis range/SD 9.811/3.037→10.9953/3.5168 st. The emotion scorer and articulation scorer use different pitch/energy/syllable estimators; do not mix their absolute scales.

## 3. Flattening layer and cheap hypothesis tests

### Proven shared bottleneck: encoder conditioning context exclusion

Inspected the actual installed fp32 `speech_encoder.onnx`, without loading external tensor weights. `encoder-graph-evidence.json` records crop constants and output ancestry:
- `/Slice_5`: resampled 16k audio [0:96000] = **first 6 seconds**; ancestor of **audio_features**.
- `/Slice`: 24k audio [0:240000] = **first 10 seconds**; ancestor of **audio_tokens and speaker_features**.
- `/ReduceMean`, axis 1: ancestor of **speaker_embeddings**, whose output shape is [batch,192]. This is global identity pooling, not a separate Rust emotion mixture vector.
- `audio_features` is sequence-shaped [batch,sequence,1024]; the new montage preserves a sequence of style examples rather than averaging several embeddings.

Rust `chatterbox.rs` passes one prepared reference into the encoder and caches the four output tensors. There is **no explicit speaker-emotion averaging stage in Rust** to remove. Turbo conditional_decoder has no emotion/exaggeration/CFG input; the existing CLI warns that exaggeration is ignored.

Measured crop effects:
- Samantha original full-file H 1.896815 / neu 0.349889 / hap 0.213338. First6 H **0.5183**, neu **0.9193**, hap **0.0421**.
- Production r2 full reference H **1.8847**; its actual first6 H **0.5711**, neu **0.9022**, hap **0.0712**. r2’s full-file blend is therefore not what the audio-conditioning branch sees. The first6 F0 range is 6.5217 st vs full r2 8.9660 st.
- Ordinary 24k reference conversion preserves Samantha proxies almost exactly: F0 SD 2.955950→2.955957, range 9.112062→9.112066, energy DR 28.761463→28.766167. Not the main flattening stage.

### Loudness / limiter hypothesis
The shared encode path resamples and applies a constant-gain peak ceiling; refprep’s LUFS stage also computes one global gain, not per-segment compression. Constant gain cannot reduce relative energy dynamic range. Measured half-gain Samantha: energy DR **28.7617** vs original **28.7615**, F0 SD/range unchanged. A remains ~0.4990. Yet SER happy probability changes **0.2133→0.3653** from gain alone: label/probability changes are not necessarily actual emotional changes. Noise gating/phrase selection can change content in other references, but the deployed own voices’ raw-reference conversion does not remove the later happy-like segment; the encoder crop does.

### Timbre-match hypothesis (matched existing waveform ablation)
Samantha pre/post timbre 0.5: short-window F0 range **7.17999→7.19122**, SD **2.27896→2.31496**, energy DR **27.89036→27.76618** (−0.12418 dB), temporal JS **0.449330→0.424446**. Timbre can move SER probabilities slightly, but pitch narrowing already exists before it; it is not the main range-loss layer. Jarvis pre/post range 10.6601→10.4323 st, energy DR 24.7705→24.2981 dB.

### Temperature 0 hypothesis (existing 12-line r2 renders only)
T0 vs T0.8: complete-line mean H **1.065674→1.268349**, F0 range **7.927707→8.378698** st, SD **2.560991→2.661456**, A variance **0.00259482→0.00354170**. Higher temperature restores some sampled variation but does not recover omitted conditioning context. Existing measured WER worsens ~1.8%→6.8%, identity ~0.7082→0.7032 (see `emotion-fix/VALIDATE12.md`). This is not a safe global fix or controlled multi-seed proof; decoder stochastic nodes also remain unseeded at T0.

## 4. Built global fix (experimental, off by default)

- Rust stage: `pipeline/emotion-blend-ref/src/main.rs`; binary `pipeline/emotion-blend-ref/target/release/galion-blend-ref`.
- Shared new-voice entry point: `pipeline/bin/cbx-add-voice --emotion-blend --blend-work-dir NEW_DIR --name own-PERSONA-v2-blend-TAG --voice-wav ORIGINAL.wav --dtype fp32`. The wrapper owns the encoding flock; do not acquire the same lock outside it.
- Per-voice opt-in: `pipeline/voice-defaults/NEW.json` → `{"emotion_blend":{"enabled":true}}`; explicit `--no-emotion-blend` overrides. No deployed configs were changed.
- New `cbx-blend` binary, optional `voice add --conditioning-wav` in shared Rust cbx source. Original speaker tensors are preserved; only audio_features/audio_tokens come from the montage. Append-only candidate saving refuses existing names.
- No voice-specific algorithm/target probabilities. Eligible ~2.5s speech windows scored fp32; farthest pair + farthest-first SER/AVD/prosody diversity; up to three chronological 2s excerpts fit within six seconds. Edge ramps only, no segment gain normalization. All decisions/scores/timestamps retained.
- Rejected a measured medoid-first selector: it chose three similar anger-like windows and crowded out the happy-like window. Added a regression test and changed to farthest-pair initialization. The initial `*-blend-reference.*` artifacts are retained as rejected exploratory evidence; final assets are `*-diverse-reference.*`.

Final selected original timestamps:
- samantha: 2.487–4.487s, 9.947–11.947s, 14.920–16.920s
- jarvis: 0.000–2.000s, 2.659–4.659s, 10.635–12.635s

### Measured input-stage effect (not an output-quality claim)

| Persona | first6 three 2s chunks: temporal JS bits | selected three 2s chunks: temporal JS bits |
|---|---:|---:|
| samantha | 0.254825 | 0.663450 |
| jarvis | 0.010905 | 0.319413 |

This proves the input reference exposes more measured style contrast to the cropped conditioning branch. It does **not** prove synthesis preserves that contrast, improves perceived sweetness/playfulness, or meets WER/identity gates. Diversity selection can amplify minority affect; Jarvis is analysis-only and remains deployed unchanged. C owns new-Samantha rendering/12-line validation. No A candidate was rendered, no WER/ECAPA score invented, and no voice promoted.

### Tests and compatibility
- **6 Rust builder unit tests passed** (`builder-test.log`).
- **1 Rust cbx split-conditioning identity-byte test passed** (`cbx-test.log`).
- **6 wrapper dispatch/config/protection tests passed** (`wrapper-test.log`).
- Original cbx executable + both deployed voice profiles + their .bak hashes verified unchanged: `protected-before.sha256`, `protected-verify.log`. Off path invokes that same original binary and untouched voice arguments; no new stage is called. Decoder nondeterminism means a second audio render is not a valid byte-for-byte behavior proof, so none is claimed.
- Build logs: `builder-build.log`, `cbx-build.log`. Full usage/build/limits: `pipeline/EMOTION-BLEND.md`.

## Artifacts / reproducibility
`profile.json` contains every source/line/window, both scorer families, class probability ranges/variances, selected metadata, all ablations. `build_profile.py` reconstructs it from the retained raw score JSONs. No commits/pushes, no deletes, no laptop/VPS/training, no deployed voice/config modifications.
