# Bake-off t648 — CPU voice-cloning options vs Samantha / Jarvis CC0 refs (BOX ONLY)

Status: **COMPLETE** (2026-11-04 23:16 CET). Owner approval t648u. Times Europe/Zagreb (CET).
Nothing pushed/merged; galion-one-voice tracked files untouched (Piper binary + pinned weights only *read*).

## Setup (common)
- Box: 8-core Xeon, no GPU, shared; every timed run under `flock latency-240/BENCH.lock` (one lock per invocation, fair sharing). Builds/downloads nice 10 without lock.
- Refs: `refs/samantha-orig.wav` (= clip-store sha e4bc60c3…, 9.60 s), `refs/jarvis-orig.wav` (= 60ad5457…, 10.71 s),
  **ALT Samantha** `refs/samantha-ALT-src-17.84-29.25.wav` (11.41 s, copy of samantha-investigation candidate).
- Held-out windows (no overlap with ref or ALT): `refs/samantha-heldout-29.60-38.20.wav` (amy-src.mp3 29.60–38.20 s, 8.6 s), `refs/jarvis-heldout-43.30-53.60.wav` (alan-src.mp3 43.30–53.60 s, 10.3 s). Mono 44.1 kHz, no processing.
- Scorer: `tools/score_t648.py` = loader copied verbatim from `tools/spk_sim_cc0_t628u.py` (same local ECAPA ckpt). Sanity: reproduces Nano baselines exactly — Samantha **0.6886**, Jarvis **0.7974** (n=1 each). Nano vs held-out: Samantha 0.5614, Jarvis 0.7583.
- **Real-speaker ceiling** (same donor, different words): Samantha held-out vs orig ref **0.784**, vs ALT 0.819; ALT vs orig 0.894. Jarvis held-out vs orig **0.884**. So ~0.78 / ~0.88 is what the *actual person* scores; 1.0 is only reachable by the reference clip itself.
- Text (`text/s1..s3.txt`, Piper 8.35 / 9.59 / 8.84 s):
  1. The morning light came slowly over the hills, and the whole valley seemed to wake up at once, quiet and golden, as the birds began to sing.
  2. If you need anything at all tonight, just tell me, and I will take care of it while you get some rest, so nothing important slips through the cracks.
  3. We checked the numbers twice this afternoon, and the results are clear: the new system is faster, simpler, cheaper to run, and far more reliable.
- Intelligibility: `tools/wer_t648.py` (local vosk-model-small-en-us-0.15). Piper source WER 0.000 on all 3.
- Stats: per option×voice over 3 sentences × 5 reps (n=15) unless noted; median, mean ± sd.

## Option A — OpenVoice v2 tone-colour converter, Rust/candle (jingangdidi/voice_clone @ e353a127)
- Build: `cargo build --release` OK in 46 s (simple VAD, CPU). Weights: the README S3 link is dead (`NoSuchBucket`, saved as `src/ckpt_v2.zip.bad-nosuchbucket.off`); used official HF mirror `myshell-ai/OpenVoiceV2@f36e7edf` `converter/` (checkpoint.pth sha256 9652c27e…). License: code MIT, weights MIT.
- Source audio = pinned Piper A21 `en_US-ljspeech-medium` (sha 6f52a751…) via `repo/weights/runtime/piper/piper`, `noise_scale 0, noise_w 0, length_scale 1.0` (galion default deterministic). Both personas use the same Piper voice.
- 4 threads (`-T 4`), 5 reps (converter is stochastic: 15/15 outputs unique). Outputs `out/A/<voice>/s{1,2,3}-r{1..5}.wav`.
- Timing: per-utterance conversion RTF (tool's elapsed / audio s) median **0.955–0.981**; whole invocation (load + 3 conversions) wall 25–29 s for 26.8 s audio, **≈2.5 CPU-s per audio-s**. Peak RSS **1.24–1.36 GB**. No streaming: first audio = Piper + full-utterance conversion ≈ 8–9 s for a 9 s sentence.

| A: OpenVoice v2 (Rust) | vs orig ref median | mean ± sd | vs held-out median | mean ± sd | WER mean (≤10% frac) |
|---|---|---|---|---|---|
| Samantha (orig ref) | 0.279 | 0.293 ± 0.046 | 0.212 | 0.225 ± 0.035 | 0.037 (1.00) |
| Samantha **ALT ref** (scored vs orig ref) | 0.282 | 0.295 ± 0.038 | 0.241 | 0.240 ± 0.017 | 0.054 (0.93) |
| Jarvis | 0.369 | 0.374 ± 0.021 | 0.328 | 0.330 ± 0.021 | 0.058 (0.80) |

ALT-ref outputs vs the ALT clip itself: median 0.298. Verdict A: keeps Piper's intelligibility but the identity moves only from ~0.05 (Piper) to ~0.3 — far below Nano (0.69/0.80). Not viable as the clone path or as a fine-tune corpus.

## Option B — Pocket TTS official Rust port (kyutai-labs/pocket-tts @ 2e117a1e, rust/)
- **Cloning BLOCKED**: every voice-cloning checkpoint (default `english`, and small `english_18m/7m/2m_2026-10`) lives in gated `kyutai/pocket-tts` → HTTP 401 `GatedRepo` without an HF login; no token on box, no account created. Exact error: `ERROR: We could not download the weights for the model with voice cloning, but you're trying to use voice cloning ... go to https://huggingface.co/kyutai/pocket-tts and accept the terms, then make sure you're logged in locally`. The ungated `kyutai/pocket-tts-without-voice-cloning` only allows the built-in catalogue voices, so **no Samantha/Jarvis similarity numbers exist for B**.
- Source build not possible on box: needs C++ compiler + CMake (SentencePiece); `cc1plus` absent and apt is unreachable (500 / unsigned repo). Used the official release binary v4.0.1 `pocket-tts-3.3.0-x86_64-unknown-linux-gnu` (SHA256SUMS verified OK).
- License: code MIT; weights CC-BY-4.0 (gated, auto-approval).
- Engine speed measured with built-in voice `alba` (speed is voice-independent; cloning only adds a one-off prompt encode), same 3 sentences, `RAYON_NUM_THREADS`:

| B: Pocket TTS (alba, no clone) | RTF T1 | RTF T2 | RTF T4 | peak RSS | first audio (serve, streamed) |
|---|---|---|---|---|---|
| english (100M, default) | 0.40–0.48 | 0.23–0.30 | 0.13–0.17 | 652 MB | T1 212–234 ms · T2 147–179 ms · T4 89–111 ms |
| english_18m_2026-10 | 0.067–0.072 | 0.064–0.087 | 0.044–0.067 | 229–281 MB | T1 47–68 ms · T4 30–54 ms |
RTF = model-reported generation time / audio s (excludes ~1.5 s model load per CLI call). First-audio = time from POST /tts to >50 ms PCM received from the streaming server.

## Option C — Chatterbox Turbo ONNX via Rust (srv1n/cbx @ 902f5f00)
- Build: needed two box-local workarounds (no pkg-config, no `libstdc++.so` dev symlink, apt unreachable): `OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu OPENSSL_INCLUDE_DIR=/usr/include` and `RUSTFLAGS="-L bakeoff-t648/linkfix"` (`linkfix/libstdc++.so -> /usr/lib/x86_64-linux-gnu/libstdc++.so.6`). Then `cargo build --release` OK (~52 s). ort 2.0.0-rc.10 downloaded its prebuilt ONNX Runtime into `~/.cache/ort.pyke.io`.
- Weights: `ResembleAI/chatterbox-turbo-ONNX@d21799bd` (ungated), fp32 (3.09 GiB) + quantized (1.05 GiB) into the shared `~/.cache/huggingface/hub` (cbx ignores HF_HOME for models). Voice profiles `t648-*` in `~/.cache/huggingface/cbx/voices`.
- License: cbx code Apache-2.0 OR MIT; weights MIT. Output carries Resemble's PerTh watermark per upstream (not verified here).
- Determinism: the LM is greedy argmax (deterministic tokens) but `conditional_decoder.onnx` contains 19 `RandomNormalLike` + 5 `RandomUniformLike` ops with no seed exposed → waveforms differ per run (15/15 unique). So 5 real reps, no fixed seed possible.
- fp32, `--intra-threads 4`, cached voice profile, 3 sentences × 5 reps. Outputs `out/C-fp32/<voice>/s{1,2,3}-r{1..5}.wav`.
- Timing (cbx's own `Timings(ms)`): synth RTF median **1.08 / 1.11 / 1.10** (Sam / Sam-ALT / Jarvis), i.e. slightly *slower* than real time on 4 threads (LM ≈ 6.5 s + decoder ≈ 2.4 s per ~8 s sentence). Per CLI call model load ≈ 4.7–5.4 s (incl. load RTF ≈ 1.70) — a warm worker would avoid it. Voice-profile encode 6.1–8.0 s once per ref. Peak RSS **2.57 GB** (speak), 2.24 GB (voice add). No streaming in cbx → first audio = whole utterance (~9 s for a ~8 s sentence).

| C: Chatterbox Turbo ONNX fp32 (Rust) | vs orig ref median | mean ± sd | vs held-out median | mean ± sd | WER mean (≤10% frac) |
|---|---|---|---|---|---|
| Samantha (orig ref) | **0.721** | 0.719 ± 0.022 | 0.625 | 0.624 ± 0.028 | 0.012 (1.00) |
| Samantha **ALT ref** (scored vs orig ref) | 0.667 | 0.664 ± 0.019 | **0.637** | 0.639 ± 0.033 | 0.015 (1.00) |
| Jarvis | **0.804** | 0.793 ± 0.024 | 0.770 | 0.768 ± 0.015 | 0.036 (1.00) |

ALT-ref outputs vs the ALT clip itself: median 0.761 ± 0.019. vs Python Nano baseline (n=1 each, different sentence): Samantha 0.689 → 0.721 (+0.03), held-out 0.561 → 0.625 (+0.06); Jarvis 0.797 → 0.804 (≈ equal), held-out 0.758 → 0.770.
- **8 threads** (Jarvis, 3 sentences, 1 rep, `out/C-fp32-T8/`): synth RTF 0.80 / 0.74 / 0.77 (LM 4.1–4.7 s, decoder 1.5–1.9 s), RSS 2.6 GB. Box was shared, so this is not an idle-box number.
- **quantized variant FAILS** under cbx's ORT (ort 2.0.0-rc.10 prebuilt): `Load model ... conditional_decoder_quantized.onnx failed: This is an invalid model. In Node, ("/input_embedding/Gather_Quant", GatherBlockQuantized, "com.microsoft", -1) ... Error Unrecognized attribute: bits for operator GatherBlockQuantized` (all 21 quantized calls rc=1, logged in `logs/C-bench.jsonl`). It needs a newer ONNX Runtime. Not pursued past the cap. fp16/q4 not tried.

## Summary table (ECAPA cosine, same loader as t628u; n=15 = 3 sentences × 5 reps)

| Option × voice | vs ORIGINAL clip-store ref: median / mean ± sd | vs held-out window: median / mean ± sd | RTF (4 thr) | first audio | peak RAM | WER mean | license code / weights |
|---|---|---|---|---|---|---|---|
| Nano Python baseline (t628u, n=1) — Samantha | 0.689 | 0.561 | – | – | – | – | MIT / MIT |
| Nano Python baseline (t628u, n=1) — Jarvis | 0.797 | 0.758 | – | – | – | – | MIT / MIT |
| A OpenVoice v2 (Rust) — Samantha | 0.279 / 0.293 ± 0.046 | 0.212 / 0.225 ± 0.035 | 0.96 (+Piper) | ≈ full utterance (no streaming) | 1.36 GB | 0.037 | MIT / MIT |
| A — Samantha ALT ref | 0.282 / 0.295 ± 0.038 | 0.241 / 0.240 ± 0.017 | 0.98 | same | 1.36 GB | 0.054 | MIT / MIT |
| A — Jarvis | 0.369 / 0.374 ± 0.021 | 0.328 / 0.330 ± 0.021 | 0.98 | same | 1.34 GB | 0.058 | MIT / MIT |
| B Pocket TTS (Rust) — both voices | **BLOCKED (gated clone weights)** | BLOCKED | 0.13–0.17 (100M); 0.04–0.07 (18M) — built-in voice | 89–111 ms (100M T4); 30–54 ms (18M T4), streamed | 0.65 GB / 0.28 GB | – | MIT / CC-BY-4.0 (gated) |
| C Chatterbox Turbo ONNX fp32 (Rust) — Samantha | **0.721 / 0.719 ± 0.022** | 0.625 / 0.624 ± 0.028 | 1.08 (0.77 @ 8 thr) | ≈ full utterance (no streaming in cbx) + 5 s load per cold call | 2.57 GB | 0.012 | Apache-2.0/MIT / MIT |
| C — Samantha ALT ref | 0.667 / 0.664 ± 0.019 | **0.637 / 0.639 ± 0.033** | 1.11 | same | 2.57 GB | 0.015 | same |
| C — Jarvis | **0.804 / 0.793 ± 0.024** | 0.770 / 0.768 ± 0.015 | 1.10 | same | 2.57 GB | 0.036 | same |
| Real donor ceiling (held-out speech vs orig ref) | Samantha 0.784 · Jarvis 0.884 | – | – | – | – | – | – |

B pocket-tts RTF by threads (100M): T1 0.40–0.48, T2 0.23–0.30, T4 0.13–0.17; (18M): T1 0.067–0.072, T2 0.064–0.087, T4 0.044–0.067.

## Listening WAVs
- A: `out/A/{samantha-orig,samantha-ALT-src-17.84-29.25,jarvis-orig}/s{1,2,3}-r{1..5}.wav` (source Piper: `out/piper/s{1,2,3}.wav`)
- B (built-in voice `alba`, NOT a clone): `out/B/rtf-{english,english_18m_2026-10}-T{1,2,4}/s{1,2,3}.wav`
- C: `out/C-fp32/{samantha-orig,samantha-ALT-src-17.84-29.25,jarvis-orig}/s{1,2,3}-r{1..5}.wav`; best-scoring picks: Samantha `out/C-fp32/samantha-orig/s1-r1.wav` (0.764), Jarvis `out/C-fp32/jarvis-orig/s1-r2.wav` (0.824).
- Raw data: `out/scores-{A,C}.json`, `out/wer-{A,C,piper}.json`, `out/results.json`, `logs/*-bench.jsonl`, `logs/B-fa.jsonl`.

## Build notes / box changes
- No apt packages installed (apt unreachable). Created `bakeoff-t648/linkfix/libstdc++.so` symlink (used only via RUSTFLAGS). Downloads to shared caches: `~/.cache/huggingface/hub/models--ResembleAI--chatterbox-turbo-ONNX` (4.2 GB), `~/.cache/huggingface/cbx/voices/t648-*`, `~/.cache/ort.pyke.io`. Pocket weights (ungated, no-clone) in `bakeoff-t648/hf`.
- Option A's `-S` wrote `.tone` caches next to inputs (`out/piper/*.tone`, `out/A/*/target.tone`).

## Recommendation
1. **Pursue C (Chatterbox Turbo) as the clone path.** It is the only option that clears the Nano baseline: Samantha +0.03 vs the original ref (0.721 vs 0.689) and +0.06 on held-out (0.625 vs 0.561); Jarvis equal (0.804 vs 0.797). Intelligibility is the best of all (WER 1–4%). Drop A: OpenVoice tone conversion over LJSpeech Piper only reaches ~0.3.
2. **Speed is C's problem, not quality.** Through cbx it is slightly slower than real time on 4 threads (RTF ~1.1, ~0.77 on 8), with no streaming and a ~5 s cold load per call. Next steps: keep a warm worker; move to an ONNX Runtime that runs the quantized/q4 LM (the LM is ~70% of synth time; the current ORT rejects `GatherBlockQuantized bits`); add chunked or streamed decoding.
3. **Pocket TTS is the speed winner but unmeasured for identity.** It runs 6–7× real time on 4 threads with ~100 ms streamed first audio (18M model: ~20× and ~30–50 ms). It is worth one owner action: accept the `kyutai/pocket-tts` gate (CC-BY-4.0, auto-approval) and provide an HF token. Then rerun `tools/run_B.sh` with `--voice refs/*.wav`. I did not create an account.
4. **"Samantha above Jarvis" and "toward 1.0".** On this metric the gap is mostly the reference, not the engine. The real Amy Koenig, on a different passage, scores only 0.784 against her own clip-store reference; Alan scores 0.884 against his. As a fraction of that ceiling the C clones are already level: Samantha 0.721/0.784 = 92%, Jarvis 0.804/0.884 = 91%. 1.0 cannot be reached by any clone (only the reference file itself scores 1.0). Use a ceiling-normalised score or the held-out window as the target. To raise Samantha's absolute number, improve the reference rather than the model. The ALT ref generalises at least as well (held-out 0.637 vs 0.625, within sd) and scores 0.761 against itself. A cleaner and longer Samantha reference, or a fused multi-clip reference, is the most likely lever.
5. **Filtered corpus for a Piper fine-tune: technically yes with C, no with A.** All 45 C clips have WER ≤ 4% for Samantha (≤ 7.4% for Jarvis) and similarity 0.62–0.82, so a filter such as sim ≥ 0.70 vs ref + WER ≤ 5% keeps most of them. At RTF ~1 on 4 threads, 1–2 h of corpus per voice is an overnight CPU job. Caveats: (a) a fine-tuned Piper student will land *below* the teacher's similarity; (b) Chatterbox outputs carry the PerTh watermark and decoder artefacts; (c) **consent scope**: both consent.txt files say `scope: local-zero-shot-conditioning`. Training a Piper voice on the donors' (synthetic) voice is outside that literal scope and needs an owner decision before any corpus is generated.
