> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 01: ASR/STT and streaming transcription. Galion Voice V2 research brief

**Owner:** Maciej Grajczyk · **Product:** Galion Voice · **Sector:** V2-S4 Real EN ASR (eval puts it at about 5%)
**Written:** 2026-11-03 (Europe/Zagreb) · **Agent:** domain-01 research agent
**Hard constraints:** no GGML / whisper.cpp (LOCKS forbid ggml containers) · Rust, local-first, CPU · phone path is 16 kHz mono PCM → transcript · policy recommends letter **A = Vosk** · today `GALION_ASR=interim` returns a **length bucket, not recognition**.

---

## 1. Top findings (2024–2026)

1. **Galion's limiting factor is streaming latency, not raw WER.** What users notice is the delay from *end of speech* to *final transcript*, plus how stable the partials are. Moonshine and Deepgram both set the target at **under 200–300 ms after speech ends** [1][7]. Batch engines that start from scratch on every call (Whisper) miss that target on CPU by 1–2 orders of magnitude [1].
2. **Vosk (Kaldi, Apache-2.0) is still the lowest-risk way to hit the LOCKS.** It is truly streaming, emits partial hypotheses, detects endpoints, runs on CPU, works offline, and accepts 16 kHz PCM natively. It also has a Rust crate (`vosk`, a safe FFI wrapper around `libvosk`) with `accept_waveform` / `partial_result` / `final_result` [3][4]. It contains no GGML. Its weakness is accuracy on open-vocabulary conversational speech compared with 2025 neural models. With a **grammar-constrained** vocabulary it is excellent: partial words appear about **40 ms median** after the word ends [5].
3. **`utter` (pure-Rust Vosk-model decoder, 2026)** loads stock Vosk model directories with **no C toolchain and zero dependencies**. It decodes 40 ms blocks and returns partials, alternatives, per-word stability and endpoints. It matches the Vosk wheel at 91.8% vs 91.5% on Speech Commands, and the finals agree on 99.2% of clips [5]. That fits Galion's Rust-native style well. It is very young (0.0.x), so treat it as an **A′ variant** to benchmark against `vosk-rs`, not as the default.
4. **Moonshine Streaming (Useful Sensors / moonshine-ai, 2025–26)** is the best accuracy-per-CPU option for English. Results from [1][2]:
   - Medium: 245M params, **6.65% WER** (Open ASR avg, better than Whisper Large v3's 7.44%), **269 ms** latency on Linux x86.
   - Small: 123M params, 7.84% WER, **165 ms**.
   - Tiny: 34M params, 12% WER, **69 ms**.

   It runs on ONNX Runtime with a C++ core: no GGML, but it is a C++ dependency. Encoder/decoder KV caching plus speculative reuse of the previous hypothesis means partials cost very little. **Strongest candidate for a later letter upgrade (B or C).**
5. **NVIDIA NeMo / Parakeet.** Parakeet-TDT-0.6B-v2 tops the Open ASR leaderboard at **6.05% WER** [8]. It is an *offline* model with a 600M-param GPU/PyTorch stack. The streaming variants are the *cache-aware FastConformer* models, with 80 / 480 / 1040 ms lookahead [9][10] (plus the newer Nemotron streaming RNNT). They are excellent on GPU and heavy for a Rust CPU box. **Reference baseline only**, unless they are exported to ONNX via sherpa-onnx.
6. **sherpa-onnx (k2-fsa)** has an official Rust streaming example using a Zipformer transducer: 100 ms chunks, `is_ready → decode → endpoint reset` [11]. Zipformer chunk-16 gives about **320 ms** algorithmic latency, and int8 is about 1.5–1.8× faster with near-neutral accuracy [12]. No GGML. **A credible "letter C" Rust-ONNX path**, closer to Kaldi-level engineering than Moonshine.
7. **Whisper / faster-whisper.** whisper.cpp is **banned** (GGML). faster-whisper (CTranslate2) is technically not GGML, but Whisper is a poor fit for streaming:
   - It uses a fixed 30 s window and recomputes everything on every call [1].
   - Streaming wrappers (whisper_streaming with LocalAgreement-2) average about **3.3 s** English latency even on an A40 GPU [13].

   **Reject** for the live phone path. It could only be an *offline* re-transcription or QA tool, and that adds a Python dependency.
8. **Kyutai STT (2025)** is natively streaming with a Rust server. The 1B EN/FR model has a fixed **500 ms delay**; the 2.6B model has 2.5 s [14]. It is GPU-oriented. **Watch, don't adopt** for CPU-local.
9. **Cloud APIs (comparison only, which fits local-first):**
   - **Deepgram Nova-3:** under 300 ms transcript latency [7].
   - **Deepgram Flux:** STT with built-in end-of-turn (EoT). About 260 ms EoT, 100–500 ms from speech end to `EndOfTurn`, plus `EagerEndOfTurn` for speculative LLM start [7][15].
   - **AssemblyAI Universal-Streaming:** about 300 ms word emission, **immutable** partials, semantic endpointing tunable via `min_turn_silence` 160–800 ms [16][17].
   - **Coval** (independent tester, but the result is published on Deepgram's own blog) measured Flux about 0.9 s ahead of AssemblyAI at P50 time-to-first-token [18].

   **Lesson for Galion:** the industry has moved to **ASR plus turn detection in one stage, with stable partials.** Copy the event shape (`Partial`, `Final`, `EagerEndOfTurn`, `TurnResumed`), not the vendor.
10. **Web Speech API** is not usable as the phone-path engine:
    - Chrome sends audio to a server by default [19].
    - On-device recognition (`processLocally = true`) arrived in Chrome desktop 139–142, but is not supported on Chrome Android or WebView, needs per-language packs, and is still marked experimental [20][21].
    - It also only runs inside a browser tab, so it can't handle 16 kHz telephony PCM on the server.

    It is fine only as an **orb/browser demo helper**, never as Galion's recognizer.

## 2. Partial hypotheses and WER vs latency: what to design for

- **Partial stability matters as much as WER for barge-in.**
  - Kaldi/Vosk partials can be revised up to the endpoint [3].
  - AssemblyAI sells immutable partials [16].
  - `utter` exposes a per-word "will this word hold" probability [5].

  Galion should feed downstream logic (intent, barge-in, eager LLM) only on **stable prefixes**, using either LocalAgreement-style prefix agreement or a stability threshold.
- **The latency budget for the phone path (target):** about 20 ms frames in, partials every 100–200 ms, final ≤ 300 ms after the endpoint, EoT ≤ 500 ms. Endpointing silence is the largest single component: Vosk's default rules and AssemblyAI's 400 ms "balanced" setting [17] both make it so. Make it a **config knob per dialogue state** (raise it while the caller dictates numbers).
- **Rough Pareto front for CPU English (2026):**

  | Engine | Approx. WER | Latency after speech ends | Fit |
  |---|---|---|---|
  | Vosk small-en (0.15, ~40 MB) | worse | lowest | good for grammar/commands |
  | Vosk en-us-0.22 (large) | mid | low | good for open speech |
  | Zipformer (sherpa-onnx) | better | about 320 ms chunk | Rust-ONNX path |
  | Moonshine Small/Medium | best (6.6–7.8%) | 165–269 ms on x86 | best accuracy per CPU |
  | Whisper | ok | seconds | **not real-time on CPU** |

## 3. Adopt / reject for Galion

| Option | Verdict | Why |
|---|---|---|
| **Vosk via `vosk-rs` (letter A)** | **ADOPT now** | Streaming, partials, endpointing, 16 kHz, CPU, Apache-2.0, no GGML, has a Rust crate. Run it as an **out-of-process sidecar** on the V2-S1 bus, so a `libvosk` crash can't take down PhoneLane. |
| `utter` (pure-Rust Vosk decoder) | **EVALUATE (A′)** | Same models with no C toolchain, plus a partial-stability API. Too young (0.0.x) to be the default. |
| Moonshine Streaming (Small/Medium) | **NEXT (B/C candidate)** | Best WER and latency per CPU. ONNX Runtime C++, no GGML. Check the license and LOCKS wording for ONNX Runtime first. |
| sherpa-onnx Zipformer (Rust API) | **EVALUATE (C)** | Rust example exists, int8 CPU, endpointing included. A good tensor-path candidate. |
| NeMo Parakeet / FastConformer streaming | **Reference only** | Top accuracy, but heavy on GPU/PyTorch. Use only as an offline WER oracle when scoring Galion test sets. |
| Kyutai STT | **Watch** | Rust server and natively streaming, but GPU-centric. 500 ms delay at 1B. |
| whisper.cpp / any GGML | **REJECT (LOCKS)** | Forbidden. |
| faster-whisper / whisper_streaming | **REJECT for live path** | 30 s window and seconds of streaming latency. At most an offline QA re-transcriber, outside the product. |
| Deepgram / AssemblyAI | **Compare only** | Use their event models and turn knobs as design references, and optionally as a WER/latency yardstick in eval. Not local-first. |
| Web Speech API | **REJECT for phone path** | Server-based by default, no Android on-device support, browser-only. At most a demo-orb toy. |
| `GALION_ASR=interim` | **Sunset** | It is a length bucket, not recognition. Rename it to `placeholder` and delete it once A passes the experiment below. |

## 4. Concrete next experiment: letter A (Vosk)

**Goal:** replace `interim` with real recognition on the phone path, and measure it honestly.

1. **Models:** `vosk-model-small-en-us-0.15` (~40 MB) and `vosk-model-en-us-0.22-lgraph` (~128 MB, runtime-graph, still CPU-light), from alphacephei.com/vosk/models [6]. Pin their SHA256 in the repo manifest and do **not** vendor them into containers until the LOCKS review is done.
2. **Harness:** a Rust bin `galion-asr-vosk` using the `vosk` crate [3]. Its loop:
   - read 16 kHz s16le PCM from stdin or the bus in **20 ms frames (320 samples)**;
   - call `accept_waveform`;
   - emit `{"type":"partial"|"final","text","t_ms"}` NDJSON;
   - keep one recognizer per call, and reset it after each final.

   Add `set_words(true)` for word timings. Feature-flag it as `GALION_ASR=vosk`.
3. **A′ side-by-side:** the same harness behind a trait (`trait Asr { fn feed(&mut self, pcm:&[i16]) -> Vec<AsrEvent>; }`), with the `utter` backend fed the identical model directory.
4. **Test set:**
   - about 50 real Galion phone utterances, recorded with consent at 8 kHz and upsampled to 16 kHz, plus at 16 kHz native;
   - 50 LibriSpeech test-clean clips;
   - 20 command-style phrases from the canned intent table.
5. **Metrics:**
   - WER (normalized: lowercase, no punctuation);
   - **first-partial latency**, **final-after-endpoint latency**, RTF, CPU% and RSS;
   - **partial stability** (share of partial words that survive into the final).

   These mirror the GOESB streaming profile [4].
6. **Grammar mode:** rerun the intent phrases with a Vosk grammar (the intent-table vocabulary plus `[unk]`) and compare intent accuracy against free decoding.
7. **Pass bar (proposed):**
   - final ≤ 300 ms after the endpoint at p50 and ≤ 600 ms at p95 on the target CPU;
   - RTF < 0.3 on one core;
   - phone-utterance WER ≤ 20% free decoding (small model) or ≤ 12% (lgraph);
   - intent accuracy ≥ 95% in grammar mode.

   If it passes, flip the default from `interim` to `vosk`. If WER fails, record the result and run the same harness on Moonshine Small or sherpa-onnx Zipformer (letters B/C).
8. **Output:** `eval/asr-letterA-<date>.md` with the table, plus raw NDJSON logs. No push without Maciej's approval.

## 5. Sources

1. Moonshine vs Whisper (latency/WER table, 30 s window critique): https://moonshine-voice.readthedocs.io/en/latest/moonshine-vs-whisper/
2. Moonshine benchmarks (latency definition): https://moonshine-voice.readthedocs.io/en/latest/using/benchmarks/ · repo https://github.com/moonshine-ai/moonshine · Pi forum post https://forums.raspberrypi.com/viewtopic.php?t=396227
3. `vosk` Rust crate docs: https://docs.rs/vosk/latest/vosk/
4. GOESB Vosk streaming profile (metrics incl. first-partial latency, partial stability): https://www.goesb.com/profiles/vosk-medium-en-streaming
5. `utter`, a pure-Rust Vosk-model streaming recognizer: https://docs.rs/crate/utter/latest
6. Vosk models list: https://alphacephei.com/vosk/models · Vosk API: https://github.com/alphacep/vosk-api
7. Deepgram, Measuring STT latency (Nova-3, Flux, latency table): https://developers.deepgram.com/docs/measuring-streaming-latency
8. Parakeet-TDT-0.6B-v2 model card (6.05% WER, Open ASR): https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2
9. NeMo ASR models / cache-aware streaming: https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/asr/models.html
10. NeMo cache-aware streaming mic demo (80/480/1040 ms lookahead): https://github.com/NVIDIA/NeMo/blob/main/tutorials/asr/Online_ASR_Microphone_Demo_Cache_Aware_Streaming.ipynb
11. sherpa-onnx Rust streaming Zipformer example: https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/streaming_zipformer_microphone.rs
12. Streaming Zipformer chunk/latency notes: https://huggingface.co/kangkyu/icefall-asr-ko-streaming-zipformer-72m
13. Whisper-Streaming / LocalAgreement (3.3 s latency): https://arxiv.org/html/2307.14743 · https://github.com/ufal/whisper_streaming · faster-whisper https://github.com/SYSTRAN/faster-whisper
14. Kyutai STT: https://kyutai.org/stt/ · https://github.com/kyutai-labs/delayed-streams-modeling/
15. Deepgram vs AssemblyAI vs Speechmatics (Flux EoT, eager EoT cost): https://nextlevel.ai/blog/deepgram-vs-assemblyai-vs-speechmatics/
16. AssemblyAI Universal-Streaming launch: https://www.assemblyai.com/blog/introducing-universal-streaming
17. AssemblyAI turn-detection tuning: https://www.assemblyai.com/docs/streaming/getting-started/optimizing-accuracy-and-latency
18. Coval / Flux benchmark (vendor-published): https://deepgram.com/learn/coval-validates-flux-no-tradeoff-between-latency-and-interruption
19. MDN, Using the Web Speech API: https://developer.mozilla.org/en-US/docs/Web/API/Web_Speech_API/Using_the_Web_Speech_API
20. MDN, `processLocally`: https://developer.mozilla.org/en-US/docs/Web/API/SpeechRecognition/processLocally
21. MDN BCD PR (Chrome 142, no Android): https://github.com/mdn/browser-compat-data/pull/27901 · caniuse https://caniuse.com/mdn-api_speechrecognition_processlocally

*Caveats:* the latency figures are vendor-reported on different hardware and with different definitions. Only the letter-A harness above produces numbers comparable across Galion options. The Vosk model sizes and WERs come from the alphacephei model page as best recalled and were not re-fetched this run, so verify them when you download.

— Maciej Grajczyk
