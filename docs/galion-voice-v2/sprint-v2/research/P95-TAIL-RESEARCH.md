# P95 latency tail: research and reasoning (Galion One Voice V2 sprint)

Written 2026-11-03 (Europe/Zagreb, UTC+1). Read-only analysis; no repo files were changed.
Sources: `results/latency/full-loop.ndjson` (24 runs), `results/latency/latency.json`, and the code that produced them:
`galion-v2-live/src/bin/latency.rs`, `src/llm.rs`, `src/piper.rs`, `src/vad.rs`, `src/asr.rs`.
(Note: `repo/scripts/eval/piper_stream_ttfa.py` is the standalone t301 Piper TTFA probe. It does **not** produce
`full-loop.ndjson`; the full loop comes from the Rust `v2-latency` binary `galion-v2-live/src/bin/latency.rs`.)

Labels used below: **[measured-data]** = recomputed from the sprint ndjson; **[measured-today]** = measured on this box
today (shared 8-vCPU Xeon, other agents running, so contention is possible); **[source]** = published, linked;
**[estimate]** = my reasoning, not measured.

---

## 1. Stage decomposition, recomputed from the raw ndjson [measured-data]

Linear-interpolated percentiles (same method as the sprint `stats::pct`), n = 24.

| Stage | How derived from ndjson | p50 | p95 | min | max |
|---|---|---|---|---|---|
| EOS → endpoint | `eos_to_endpoint_ms` | 130.2 | 140.3 | 40.0 | 140.4 |
| ASR final increment | `eos_to_asr_final − eos_to_endpoint` | 19.9 | 32.7 | 11.1 | 39.3 |
| "Unlabeled gap" | `eos_to_first_token − llm_ttft − eos_to_asr_final` | 68.0 | 112.6 | 59.1 | 134.6 |
| LLM TTFT | `llm_ttft_ms` | 284.8 | 498.0 | 178.7 | 1823.1 |
| First token → first clause | `eos_to_first_clause − eos_to_first_token` | 20.6 | 44.1 | 2.8 | 146.0 |
| Piper TTFA in loop | `piper_ttfa_ms` (= first_content_audio − first_clause) | 71.3 | 181.9 | 25.6 | 195.8 |
| **Full loop** | `eos_to_first_content_audio_ms` | **608.3** | **775.2** | 405.5 | 2243.3 |
| Backchannel audio | `eos_to_backchannel_audio_ms` | 186.1 | 212.9 | 82.1 | 259.8 |

- The six stages sum exactly to the full loop (max reconstruction error 0.1 ms, rounding). The brief's numbers are confirmed.
- LLM TTFT is 46.8 % of the p50 loop (284.8 / 608.3), matching the owner's "~47 %".
- **Statistical caveat:** with n = 24, p95 is interpolated between the 3rd-worst (744.3, run 13) and 2nd-worst
  (780.5, run 9) runs: 0.15 × 744.3 + 0.85 × 780.5 = 775.1. **Two runs set the whole p95**, so moving one run moves p95 by ~30 ms.
  p99 (1906.9) is dominated by run 4. A p95 claim this fragile needs ≥ 200 runs before tuning decisions.

### 1a. What the 68 ms "unlabeled gap" actually is (from the code)

In `latency.rs::full_loop`, after the ASR final is received and **before the LLM request is sent**, the bench does:

```
p.drain();
p.say("Mm-hm.");                 // backchannel on the same Piper process
let bc = p.first_pcm(5000);      // wait for backchannel first PCM
p.collect_until_done(before_bc, 1, 5000);  // wait for Piper to finish AND the pipe to be idle 30 ms
// only now: let t_llm = now_ms(); llm::stream(...)
```

So the gap is **serial backchannel synthesis**, not ASR, network or prompt build:
- `eos_to_backchannel_audio − eos_to_asr_final` (backchannel first PCM after ASR final): p50 ~35 ms, range 28.9–104.3 ms.
- `gap − that` is **30.1–34.6 ms in all 24 runs**, which is the 30 ms `recv_timeout` idle window in `Piper::collect_until_done` plus <5 ms.
- Prompt build is negligible: JSON write to a temp file + `curl` spawn happen **inside** the timed `llm_ttft` window
  (`t_llm` is taken before `llm::stream`), not in the gap.

Consequence: the whole ~68 ms (p95 ~113 ms) is a bench sequencing artifact. Firing the LLM request at (or before) the ASR final,
with the backchannel in parallel on its own lane, removes it. Counterfactual p95 with gap = 0: **705.8 ms** (−69 ms) [measured-data, counterfactual].

### 1b. Other things the code shows

1. **LLM TTFT includes a fresh TLS connection every turn.** `llm.rs` spawns a new `curl --http1.1` per request (no keep-alive pool).
   [measured-today] from this box to the OpenRouter endpoint (public `/api/v1/models`, no key, 6 trials): TCP connect ~1.5–2 ms,
   TLS handshake done at 27–39 ms, first byte 89–105 ms on a fresh connection vs **18 ms** first byte on a reused connection
   (`curl` with 3 URLs on one handle). `curl` process spawn ~6 ms. So connection reuse plausibly saves **~35–80 ms per turn** [estimate:
   the /models endpoint is not the chat endpoint; the TLS part ~30 ms is solid].
2. **Provider pin:** the bench sends `provider.order = ["Groq"], allow_fallbacks = true`. OpenRouter's docs say a failed
   primary provider is retried on the next option and "a failed initial completion unsurprisingly adds latency", and that
   hard-coded `provider.order` disables its latency-aware reordering; they also offer `preferred_max_latency` (p90, 5-minute
   rolling window) for queue spikes ([OpenRouter latency guide](https://openrouter.ai/docs/guides/best-practices/latency-and-performance)).
   Run 4's 1823 ms is consistent with a queue spike or a retry, but the ndjson only logs the final provider ("Groq"), so **the
   cause of run 4 cannot be determined from the data**.
3. **The first-clause cap is effectively off.** `Clauser::push` cuts the first clause after >5 words only when
   `self.buf.ends_with(' ')`. Llama tokens stream with a *leading* space (" word"), so the buffer almost never ends in a space and
   the cap rarely fires. Evidence: run 12 spoke a 12-word first clause, run 21 an 11-word one, run 19 an 8-word one, all
   above the intended 5. [measured-data + code reading]
4. **Piper ONNX session is configured for load time, not tail latency.** The pinned Piper `2023.11.14-2` C++ hard-codes
   `GraphOptimizationLevel::ORT_DISABLE_ALL`, `DisableCpuMemArena()`, `DisableMemPattern()`, and leaves intra-op threads at the
   ORT default (all cores) ([piper.cpp @ 2023.11.14-2](https://github.com/rhasspy/piper/blob/2023.11.14-2/src/cpp/piper.cpp)).
   Piper also writes a sentence's audio only after the whole sentence is synthesized ([README, Streaming Audio](https://github.com/rhasspy/piper/blob/2023.11.14-2/README.md)),
   so TTFA grows with first-clause length. With no memory arena, every inference re-allocates. Whether that drives the variance
   is **untested** [estimate/hypothesis].
5. **Piper variance is not just length.** Same-text "Mm-hm." backchannel TTFA ranged 28.9–104.3 ms across runs. Piper TTFA vs
   first-clause word count correlates only r = 0.40, and 6-word clauses ranged 60.5–195.8 ms. Run 0 was **not** a cold spawn: the
   process had 3 warm-up clauses plus a backchannel first. The likely causes are CPU contention (Piper using all 8 cores by default
   while curl/Vosk/other box tenants run), plus ORT allocation [estimate]. The sprint's own core-pinning table shows 20-word p95 dropping
   from 185.9 ms (8 cores) to 152.2 ms (2 cores) with the same p50, which points the same way.
6. **Endpoint values are clip-deterministic.** EOS→endpoint is 40, 120 or 140 ms depending on the clip (`run % 4`), because
   "true EOS" uses a peak−35 dB threshold while the VAD uses an adaptive floor (≥ −48 dB), and the 160 ms hang counts from the
   VAD's end. It adds per-clip offset, not random tail.

## 2. How much of p95 each cause contributes [measured-data, counterfactual]

### Per-run excess over the p50 stage values (the runs that set p95/p99)

| Run | Total | Δ vs p50 608.3 | LLM | Piper | Gap | 1st-tok→clause | Endpoint | Dominant cause |
|---|---|---|---|---|---|---|---|---|
| 4 | 2243.3 | +1635 | **+1538** | −11 | −8 | +125 | +10 | Groq/OpenRouter spike (sets p99/max) |
| 9 | 780.5 | +172 | **+229** | −31 | +2 | −8 | −10 | LLM TTFT 513.9 (sets 85 % of p95) |
| 13 | 744.3 | +136 | +8 | **+125** | +21 | +3 | −10 | Piper 195.8 on 6 words (sets 15 % of p95) |
| 0 | 743.2 | +135 | +23 | **+117** | −1 | +1 | +10 | Piper 188.5 (first loop run) |
| 12 | 724.0 | +116 | +36 | **+73** | −7 | +23 | +10 | 12-word first clause (cap not firing) |

### Counterfactual p95 (replace one cause, recompute over all 24 runs)

| Scenario | p50 | p95 | Δp95 |
|---|---|---|---|
| Baseline | 608.3 | 775.2 | – |
| LLM TTFT set to its median for every run | 606.5 | 718.0 | −57 |
| Gap removed (LLM fired at ASR final, backchannel parallel) | 538.7 | 705.8 | −69 |
| LLM TTFT capped at 350 ms (idealized hedge/timeout fallback) | 597.0 | 744.1 | −31 |
| Piper TTFA capped at 60 ms (idealized pre-warm/short clause/tuning) | 577.0 | 767.1 | −8 |
| Gap 0 + LLM cap 350 + Piper cap 60 | 499.0 | 576.6 | −199 |
| …plus endpoint hang −60 ms (160 → 100 ms) | 439.0 | 516.6 | −259 |

Reading: because p95 sits on two runs, single fixes look small or even negative (setting one stage to its median can promote
another run into the p95 slot: Piper-at-median gives p95 794.8). Only **combined** fixes move p95 robustly. Ranked by
standard deviation excluding run 4: LLM TTFT 71.1 ms, Piper 44.2 ms, endpoint 41.7 ms (clip-driven), gap 17.9, t2c 10.9,
ASR 7.7. The caps above are idealized [estimate]; real hedging/tuning would not hit them exactly.

## 3. Published and measured numbers for alternatives

### 3a. Direct native Linux TTS (C/C++ libraries, no ONNX)

| Engine | Latency evidence | Quality / notes | Croatian | License |
|---|---|---|---|---|
| **espeak-ng (direct C API)** | [measured-today] in-process `espeak_Synth` (AUDIO_OUTPUT_SYNCHRONOUS) using the **same `libespeak-ng.so.1.52` that ships in our Piper pack**: init 2.6–4.6 ms; first audio callback < 1.1 ms; full synthesis of a 4-word clause 2.2 ms, 8-word 5.1 ms, 20-word 10.4 ms (RTF ≈ 0.002). Croatian voice `hr`: 2.1–9.6 ms full synth. 20 reps each, scratch code outside repo. | Formant synthesis, robotic ([TechShark comparison](https://techshark.io/tools/rhvoice/) calls it "Robotic / Formant"). Not cloneable. | Yes (`hr` voice present in espeak-ng-data; measured) | GPL-3.0-or-later ([README](https://github.com/espeak-ng/espeak-ng#license-information)) |
| **Flite (CMU)** | [source] 2001 paper: 20-word utterance starts audio in **45 ms**, 40 words 75 ms, 23 ms startup, 70.6× real time on a **500 MHz PIII** ([Black & Lenzo, SSW 2001](http://www.cs.cmu.edu/~awb/papers/ISCA01/flite.pdf); [festvox/flite README](https://github.com/festvox/flite)). On a modern Xeon it should be far lower [estimate]. | Diphone/clustergen voices, dated quality. | No Croatian voice found [not verified exhaustively] | BSD-like core, commercial use OK ([COPYING](https://github.com/festvox/flite/blob/master/COPYING)) |
| **Festival** | [source] Same paper: Festival **server mode could not get under 1 s** for a 20-word utterance (~2 s for 40 words); CLI startup 4–5 s; 13.4× real time on PIII. | Scheme runtime, heavy. Poor fit for a first-audio budget. | Unknown | Not verified |
| **RHVoice** | No published numeric latency or RTF found. Docs describe HTS statistical-parametric synthesis with "low CPU" and three quality levels, where lower quality gives shorter response time ([linuxadictos on 1.6.0](https://en.linuxadictos.com/rhvoice-the-open-voice-synthesis-system-reaches-its-version-1-6-0.html); [RHVoice repo](https://github.com/RHVoice/RHVoice/)). **Needs a local measurement.** | More natural than formant, less than neural ([RHVoice repo](https://github.com/RHVoice/RHVoice/)). | **Yes: Karmela (v4.5) and Marija (v4.1)** ([rhvoice.org/hr-voices](https://rhvoice.org/hr-voices/)); Marija released Jan 2026 ([announcement](https://www.freelists.org/post/slikom/Nove-verzije-RHVoice-glasova-i-novi-hrvatski-glas)) | Core LGPL-2.1+, **GPL-3+ when built with MAGE**. Many voices CC BY-NC-ND or personal-use-only ([License.md](https://github.com/RHVoice/RHVoice/blob/master/doc/en/License.md)). Croatian voice licenses not verified. |
| **Mimic 3 (legacy)** | [source] RTF < 0.05 on Ryzen 9 5950X, ~0.5 on Pi 4, ~1.3 on Pi 3 ([Mycroft docs](https://mycroft-ai.gitbook.io/docs/mycroft-technologies/mimic-tts/mimic-3)). | VITS + **ONNX** (not ONNX-free), Python. Archived; README points to Piper ([repo](https://github.com/MycroftAI/mimic3)). | No | AGPL-3.0 |

Key reasoning: a direct espeak-ng/Flite path removes **only the Piper slice** (p50 71 ms, p95 182 ms) and costs voice quality and
cloning compatibility. It does not touch the LLM TTFT, which is the biggest tail driver. Piper already uses espeak-ng only for
phonemization (<1–2 ms per clause from the measurement above); the ~40–190 ms is ONNX VITS inference plus pipe/process overhead
[estimate from the measurement split].

### 3b. Piper alternatives and ONNX-path tuning

| Option | Evidence |
|---|---|
| **sherpa-onnx (C/C++ with Rust crate)** | Rust `OfflineTts` supports Piper VITS and Kokoro ([docs.rs/sherpa-onnx](https://docs.rs/sherpa-onnx); [rust example](https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/vits_tts.rs)). Published RTF on Raspberry Pi 4, piper en_US-lessac-medium: 0.774 (1 thread) → 0.357 (4 threads); kokoro-en-v0_19 6.629 → 2.774 ([sherpa RTF table](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/rtf.html)). MacBook Pro, 1 thread: piper libritts_r-medium fp32 RTF 0.114, **int8 0.320 (slower)**, fp16 0.123; kokoro fp32 1.128 ([PR #2460](https://github.com/k2-fsa/sherpa-onnx/pull/2460)). This brings Piper models in-process (no child process or pipe), but still on ONNX Runtime. |
| **Kokoro-82M** | [source] On an 8-vCPU cloud Xeon, 16 GB, kernel 6.12.94+ (the same class as our box): kokoro-onnx fp32 RTF 0.1234, **time to first audio 1.416 s** for a 2-sentence input; int8 1.686 s; ~0.75–0.93 GB RAM ([OpenVoiceIndex](https://openvoiceindex.com/models/kokoro-82m/on/x86-cpu/)). Apache-2.0. **Far too slow for first audio on CPU** compared with Piper's 25–75 ms warm TTFA. |
| **Piper thread tuning** | Independent 2-core ARM measurement: piper fr_FR-siwis-medium RTF 0.197 (1 thread), 0.121 (2), **0.194 (4)**. More threads than cores hurts ([sherpa-onnx discussion #3968](https://github.com/k2-fsa/sherpa-onnx/discussions/3968)). The pinned Piper CLI cannot set intra-op threads. The sprint used `taskset` instead (20w p95 185.9 → 152.2 ms at 2 cores). |
| **`ort` crate (Rust ONNX Runtime)** | [pykeio/ort](https://github.com/pykeio/ort) would let us load the same `.onnx` with arena on, graph optimizations on and explicit intra-op threads. No published Piper-on-ort latency found. **Needs a local A/B** against the CLI's `ORT_DISABLE_ALL` and no-arena settings. |
| **Piper upstream status** | rhasspy/piper is archived. The successor OHF-Voice/piper1-gpl is **GPL-3.0-or-later** and embeds espeak-ng ([piper1-gpl v1.3.0](https://github.com/OHF-Voice/piper1-gpl/releases/tag/v1.3.0); [discussion #53](https://github.com/OHF-Voice/piper1-gpl/discussions/53)). **No Croatian Piper voice** in the official catalog; there is an open request ([issue #68](https://github.com/OHF-Voice/piper1-gpl/issues/68); [VOICES.md](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/VOICES.md)). |

### 3c. Local LLM on CPU instead of Groq

[measured-today] llama.cpp release b12076 (prebuilt CPU binary), `llama-server` with prompt cache on, Q4_K_M GGUFs, the bench's
exact system prompt, the 4 bench ASR texts, `max_tokens 12`, 1 cold request + 16 warm, client-side TTFT over localhost HTTP
streaming. Box load average was ~3.7 during the first block and rose to ~17 in the second (other agents compiling and quantizing),
so treat the second block as a **contention** measurement.

| Model (Q4_K_M) | Threads | Cold-cache first TTFT | Warm TTFT p50 | Warm p95 | Box load |
|---|---|---|---|---|---|
| Qwen2.5-0.5B-Instruct | 4 | 160 ms | **66 ms** | 96 ms | ~3.7 |
| Llama-3.2-1B-Instruct | 4 | 247 ms | **71 ms** | 90 ms | ~3.7 |
| Qwen2.5-1.5B-Instruct | 4 | 223 ms | **90 ms** | 155 ms | ~3.7 |
| Llama-3.2-3B-Instruct | 4 | 639 ms | 225 ms | 760 ms | 4–17 (contended) |
| Llama-3.2-1B-Instruct | 6 | 1635 ms | 408 ms | 2370 ms | 4–17 (contended) |
| Qwen2.5-0.5B-Instruct | 2 | 391 ms | 138 ms | 279 ms | 4–17 (contended) |

llama-bench (same box, load ~1.5–3.7): pp80 / tg16 tokens per second. Qwen2.5-0.5B: 416 / 69 (4 threads), 497 / 74 (8 threads).
Llama-3.2-1B: 371 / 26 (4 threads), 199 / 32 (8 threads, high variance). Qwen2.5-1.5B: 225 / 24 (4 threads).

Interpretation:
- The prompt cache matters. With the static system prompt cached, only the 8–28 user tokens are prefilled. That is why warm TTFT
  (66–90 ms) is far below cold-cache (160–250 ms).
- 1B-class local TTFT on an idle box is about 4× lower than Groq p50 (284.8) and has no network tail. **But the tail moves to CPU
  contention**: the same 1B model went to 408 ms p50 and 2.4 s p95 when the box was loaded. This matches published thread-trap
  data: on a 12-vCPU EPYC VPS, Qwen2.5-0.5B peaked at 8 threads (pp512 581–658 t/s) and generation collapsed to 0.5 t/s at 12
  threads ([workloftai thread-trap note](https://github.com/workloftai/ships/blob/main/examples/077-llama-cpp-threads-trap.md)).
  In production, LLM, Piper and Vosk on one 8-core host need explicit core budgets [estimate].
- Quality is the open risk. The samples include refusals from 0.5B ("I'm sorry, but I can't assist with that.") and an off-topic
  1B reply on the same prompts. Small models need a quality eval before they can replace llama-3.1-8b.
- Rust runtimes: mistral.rs v0.9.0 claims CPU decode at or above llama.cpp, but its published sweep is on **GB10 (aarch64)**
  with Qwen3-4B, Gemma4-E4B and LFM2.5-230M ([commit 486ade2](https://github.com/EricLBuehler/mistral.rs/commit/486ade2d8e8882e39e222a2deaf6989a6a3faef4)).
  x86 claims were not verified. A 2024 candle issue shows candle at 31.8 t/s vs llama.cpp at 51.3 t/s on Mistral-7B, but that
  was on **Metal, not CPU** ([candle #1939](https://github.com/huggingface/candle/issues/1939)). llama.cpp can also be linked
  from Rust via bindings (e.g. `llama-cpp-2`), with no per-turn process spawn.
- Groq published TTFT: I could not open Artificial Analysis directly (JS page). The search snippet cited ~0.91 s median TTFT for a
  10k-token-input workload ([AA Llama 3.1 8B providers](https://artificialanalysis.ai/models/llama-3-1-instruct-8b/providers)).
  That workload is not comparable to our ~80-token prompt. **Unverified.**

### 3d. Approaches to the tail (prior art)

- **Hedged requests**: send a backup after the primary exceeds ~p95 and take the first response. Google reports cutting Bigtable p99.9
  from 1,800 ms to 74 ms with 2 % extra requests ([The Tail at Scale, CACM](https://cacm.acm.org/research/the-tail-at-scale/)).
  Applied here, that means a second request (or a local fallback model) after ~350 ms without a token [estimate of threshold].
- **Speculative start on partial ASR**: a COLING 2025 study found that incremental response generation with voice-activity
  projection shortens response gaps, but faster responses were less accurate ([ACL Anthology 2025.coling-main.249](https://aclanthology.org/2025.coling-main.249/)).
  Production examples: Deepgram Flux "EagerEndOfTurn" starts generation on a medium-confidence turn end and cancels on resume
  ([Deepgram docs](https://developers.deepgram.com/docs/flux/voice-agent-eager-eot.mdx)). NVIDIA's voice-agent examples use
  stable interim transcripts ([SPECULATIVE_SPEECH_PROCESSING.md](https://github.com/NVIDIA/voice-agent-examples/blob/main/docs/SPECULATIVE_SPEECH_PROCESSING.md)).
  In our loop the ASR final arrives ~20 ms after endpoint, so speculation pays off mostly by overlapping the 160 ms hang [estimate].
- **OpenRouter knobs**: `preferred_max_latency`, latency-sorted routing, and dropping the hard `provider.order` ([OpenRouter guide](https://openrouter.ai/docs/guides/best-practices/latency-and-performance)).

## 4. Ranked levers (reasoning) [estimate unless marked]

1. **Fire the LLM at ASR final, with the backchannel in parallel**: −68 ms p50, p95 → ~706 (counterfactual). This is a bench/loop sequencing change.
2. **Persistent HTTP/2 or keep-alive connection instead of curl per turn**: TLS ~30 ms measured, plausibly 35–80 ms total.
3. **LLM tail control**: hedge at ~p90 to a second provider or a local 0.5–1B model, or route with `preferred_max_latency`. This targets
   runs 9 and 4, which set p95 and p99.
4. **Make the first-clause cap actually fire** (cut at the last whitespace once >N words). This targets runs 12 and 21 and shortens Piper TTFA.
5. **Piper determinism**: limit threads/cores per Piper (the sprint data shows lower p95 at 2 cores) and A/B ORT arena and graph
   optimizations via `ort` or sherpa-onnx. This targets runs 13 and 0.
6. **Endpoint hang 160 → ~100 ms**, only with false-endpoint (cut-in) measurement. It is a fixed-cost lever, not a tail lever.
7. Replacing Piper with espeak-ng direct saves at most the Piper slice and costs a lot of quality. Replacing Groq with a local 1B model
   cuts TTFT strongly on an idle box but needs core isolation and a quality eval.

## 5. Not verified / limitations
- Root cause of run 4 (1823 ms): the ndjson has no per-request provider or retry trace.
- RHVoice and Flite latency on this box: not measured (no packages installed; I did not build them). The Flite numbers are from 2001 hardware.
- Croatian RHVoice voice licenses, Festival license and Festival Croatian support: not checked.
- Groq/Artificial Analysis TTFT figures: page not readable. The snippet is unverified and not comparable.
- mistral.rs x86 CPU claims: not verified (the reddit/x86 claim was not opened).
- Local LLM numbers come from a shared box with other agents running. The first block was lightly loaded and the second heavily loaded.
  They are not a clean benchmark. Re-run on an isolated host.
- The `ort` crate or sherpa-onnx Piper A/B vs the Piper CLI: not run.
- The OpenRouter connection timing used the public `/api/v1/models` endpoint, not chat completions.
- Scratch files (espeak C probe, llama.cpp binary, GGUFs, TTFT client) are in `/workspace/p95-scratch/` on the box, outside the repo.
  They are not shipped code.

- Agent 05 (local model, hedge design, licenses, candle KV-reuse bench harness): see latency-240/05-local-model.md.
