# Clone-quality Rust continuation report

## Status

Resumed existing artifacts rather than re-exporting models or rewriting completed ports. `nice -n 10 cargo build --release -j 2` succeeded (Cargo: 0.09 s, cached release build). The three `bin/` executables are byte-identical (SHA-256) to their `galion-quality/target/release/` counterparts.

Binary directory: `/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/quality/bin/`

- `galion-wer`: Vosk recognition, text normalization, edit-distance WER, per-file and aggregate statistics; optional two jobs.
- `galion-mos`: SQUIM subjective/NORESQA MOS and objective STOI, PESQ, SI-SDR, using existing fp32 ONNX models. These are model estimates, not listening-test MOS or intrusive reference-based PESQ/STOI measurements. Other persona's reference is the subjective non-matching reference.
- `galion-hygiene`: sample/RMS/true peak, clipping samples/runs, clicks, maximum sample jump, Welch f90/f99, high-frequency energy fractions, integrated LUFS, LRA and maximum momentary/short-term loudness.

**Partial completion:** noise-floor estimation is not present in the existing Rust JSON or supplied Python cross-check. No numerical noise-floor baseline or parity claim is made. LRA also fails agreement with ffmpeg and must not be treated as validated. No Rust source changes were made: the required cloud coding-agent tool is unavailable in this executor. A coding-capable continuation is needed for missing noise-floor work and LRA investigation.

All writes were under this quality directory. Inputs remained read-only. No commits, pushes, deletions, or laptop operations. Prior benchmark files were preserved as `out/bench*.previous.off`.

## Validation

Fresh `bench.sh` run, plus NumPy hygiene checks on both reference WAVs: 12 clone lines per persona, 24 total. MOS and hygiene include two references (26 WAVs); WER covers the 24 transcribed clone lines. Numeric details: `out/verification.json`; fresh outputs: `out/bench_*.json` and `out/check_*hyg_refs.json`.

| Metric | Maximum absolute difference | Cross-check |
|---|---:|---|
| WER (single job) | 0 | Python/Vosk, per-line rounded WER |
| WER (two jobs) | 0 | Python/Vosk, per-line rounded WER |
| MOS | 5.164985657e-05 | Python torchaudio |
| STOI | 4.938783646e-05 | Python torchaudio |
| PESQ | 0.000284173584 | Python torchaudio |
| SI-SDR | 0.001907170105 | Python torchaudio |
| Clipped samples | 0 | NumPy, abs(sample) >= 0.99 |
| Clicks | 0 | NumPy, abs(sample jump) > 0.35 |
| Sample peak (dBFS) | 0.004984174118 | NumPy |
| Maximum sample jump | 4.995117188e-05 | NumPy |
| f90 (Hz) | 0.05 | NumPy Welch |
| f99 (Hz) | 0.05 | NumPy Welch |
| RMS (dBFS) | 0.004894620702 | NumPy |
| Clipped runs | 0 | NumPy |
| Energy fraction >4 kHz | 4.9601871e-06 | NumPy Welch |
| Energy fraction >8 kHz | 4.852361047e-07 | NumPy Welch |
| Integrated LUFS | 0.05 | ffmpeg ebur128 |
| True peak (dBTP) | 0.09 | ffmpeg ebur128 |
| LRA (LU), non-null outputs only | 20 | ffmpeg ebur128 — FAILED parity |

WER hypotheses match Python on all 24 clips. Most small hygiene differences reflect Rust JSON rounding (two decimals for dB, four for sample jump, one for frequency). Welch uses periodic Hann, nfft 2048, hop 1024, segment-mean detrending and channel averaging.

LRA is null on Samantha t04 and Jarvis t04. Largest non-null discrepancy: Samantha t05, Rust 0.0 LU versus ffmpeg 20.0 LU; evidence is retained in `out/ffmpeg_samantha_t05.log`. This short-clip behavior requires investigation; no equivalence is claimed. Maximum momentary and short-term loudness are emitted but were not independently cross-validated here.

The older `cmp_wer_hyg.py` comparison finds legacy clipping-count mismatches on Samantha t11/t12 and Jarvis t08. The current explicit-threshold NumPy cross-check agrees exactly; the legacy artifact counts should not silently be substituted for this definition. Legacy WER SDs also differ from the existing Rust aggregate SDs; per-line WER parity does not establish aggregate-SD parity.

## Fresh speed / CPU benchmark

`bench.sh` ran under `nice -n 10`, with `OMP_NUM_THREADS=2`, `MKL_NUM_THREADS=2`, `OPENBLAS_NUM_THREADS=2`. WER has one-job and two-job rows; SQUIM uses two inference threads. CPU seconds = user + system; average CPU = CPU / wall * 100, where 100% is one core. Single sequential run on the loaded shared box, not a stable throughput guarantee. Original timings were preserved, not mixed with this run.

| Task | Implementation | Wall s | User s | Sys s | CPU s | Avg CPU % | Wall speedup vs Python | CPU speedup vs Python |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| hygiene | python | 0.403 | 0.212 | 0.030 | 0.242 | 60.0 | 1.000x | 1.000x |
| hygiene | ffmpeg-r128 | 2.580 | 1.243 | 0.446 | 1.689 | 65.5 | 0.156x | 0.143x |
| hygiene | rust | 0.189 | 0.168 | 0.012 | 0.180 | 95.2 | 2.132x | 1.344x |
| wer | python | 22.043 | 20.966 | 0.552 | 21.518 | 97.6 | 1.000x | 1.000x |
| wer | rust | 21.037 | 19.405 | 0.171 | 19.576 | 93.1 | 1.048x | 1.099x |
| wer | rust-jobs2 | 21.293 | 20.519 | 0.439 | 20.958 | 98.4 | 1.035x | 1.027x |
| mos | python | 151.504 | 105.341 | 14.855 | 120.196 | 79.3 | 1.000x | 1.000x |
| mos | rust | 59.399 | 69.670 | 19.643 | 89.313 | 150.4 | 2.551x | 1.346x |

Hygiene is not an identical-work benchmark: the Python helper covers core sample/FFT metrics, while Rust also computes loudness/true peak. ffmpeg-r128 is a separate loudness-only baseline, not a Python implementation of the same metric suite. MOS timing includes subjective plus objective scoring of the same 26 WAVs in both implementations, with model loading included in wall time. WER two jobs provided no advantage over one in this run. `bench.sh` suppresses command diagnostics and does not propagate each timed exit status; output coverage and fresh JSON parsing were therefore independently checked. No peak-RSS measurement was taken in this fresh run.

## Baselines

Earlier supplied Python baselines: WER Samantha **0.050**, Jarvis **0.018**; MOS Samantha **3.73**, reference **4.33**; Jarvis **4.14**, reference **4.55**.

Fresh arithmetic means below are calculated from saved per-line outputs, not copied from rounded aggregate summaries.

| Voice | WER Python = Rust (rounded per-line values) | Rust mean raw WER | MOS Python | MOS Rust | Reference MOS Python | Reference MOS Rust |
|---|---:|---:|---:|---:|---:|---:|
| samantha | 0.05025000 | 0.05026458 | 3.72846603 | 3.72846667 | 4.32554102 | 4.32550000 |
| jarvis | 0.01850000 | 0.01851858 | 4.14175995 | 4.14175000 | 4.54807901 | 4.54810000 |

Jarvis is at a rounding boundary: fresh mean of the three-decimal per-line WERs is 0.0185 in both languages; do not mistake Rust's summary 0.019 versus the older Python 0.018 for a recognition disagreement.

| Voice | Clipped samples, sum | Clicks, sum | Mean f99, Hz |
|---|---:|---:|---:|
| samantha | 9 | 32 | 2813.475 |
| jarvis | 4 | 39 | 6023.450 |

## Left in Python / unresolved

- Core WER and SQUIM scoring need no Python at inference time; Rust still depends on Vosk and ONNX Runtime shared libraries and the existing model files. Python remains for validation and ONNX export.
- **Noise floor: unported/unvalidated.** No implementation or matching Python definition is exposed by the supplied helpers. A definition and independent cross-check are required before reporting a value; no invented value is supplied.
- **LRA:** Rust emits it, but keep ffmpeg as the trusted comparison path until the 20 LU discrepancy and null short-clip outputs are resolved. This is not a validated replacement.
- Maximum momentary/short-term loudness: emitted in Rust but parity remains unverified.
- Legacy silence/gap/prosody metrics are outside the existing core cross-check suite and were not ported or validated by this continuation.
