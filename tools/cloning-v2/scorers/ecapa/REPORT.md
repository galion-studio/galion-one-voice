# galion-ecapa — Rust-native ECAPA speaker similarity

Replaces the Python scorer `bakeoff-t648/tools/score_t648.py` (speechbrain ECAPA + cosine) for ref-vs-clone scoring.

## Pipeline (all Rust at runtime)
1. WAV load (hound): int/float, any channel count → mono mean (torchaudio normalization: int/2^(bits-1)).
2. Resample to 16 kHz: exact port of `torchaudio.functional.resample` (sinc_interp_hann, width 6, rolloff 0.99, polyphase conv, ceil target length).
3. speechbrain `Fbank(n_mels=80)`: center-padded (200 zeros) STFT n_fft=400 hop=160, periodic Hamming, power spectrum, symmetric triangular mel filters (2595·log10 mel, f32 linspace), 10·log10(clamp 1e-10), top_db=80 over the utterance.
4. `InputNormalization(sentence, std_norm=False)` = per-bin mean subtraction.
5. ECAPA-TDNN (ecapa-local/embedding_model.ckpt) via ONNX Runtime (ort 2.0.0-rc.10, static), intra-op threads 2, inter 1.
6. L2 normalize, cosine. Ceiling = embedding(first half of 16 kHz ref) vs (second half).

Python was used only for the one-time export (`py/export_onnx.py`); ONNX vs torch on random features T=150/777/2400: cos 1.0000, max abs 1.5e-4.

## Accuracy (refs clip-store-own/{samantha,jarvis}/reference.wav, own-clone-analysis/wav t01–t12)
| | Python | Rust | max per-clip abs diff |
|---|---|---|---|
| Samantha mean | 0.7072 | 0.7072 | 5e-5 |
| Jarvis mean | 0.7327 | 0.7327 | 5e-5 |
| Samantha ceiling | 0.8283 | 0.8283 | — |
| Jarvis ceiling | 0.7251 | 0.7251 | — |
Format test (22.05k/24-bit, 44.1k/16-bit, 48k/float32, stereo): diffs ≤ 5e-5. Python numbers are rounded to 4 dp, so the diffs are rounding noise.

## Speed (12 clips + ref + 2 halves, nice 10, 2 threads; box load avg 40–57 on 8 cores, so wall time is noisy and CPU time is the reliable number)
See out/bench.txt. Python CPU (user+sys) 29.7 / 40.4 / 35.3 s; Rust 7.0 / 7.9 / 7.9 s → ~4.6× less CPU.
Wall: Python 44–187 s vs Rust 10.7–15.6 s (5–12×). Rust startup (model load) ≈ 0.3 s vs Python ≈ 3–4 s (torch/speechbrain import).
