# 05 Local Model & Stack Alternatives (LLM first token, budget 120 ms p50)

> **OWNER RULING 2026-11-04 20:45 CET (in force; supersedes everything below that names Qwen, Llama, llama.cpp, Groq or a cloud LLM).**
> No Qwen anywhere in the architecture, no fallback to it. Qwen, Groq and any cloud LLM are third party.
> The reasoning sector is fully local and in-house: the S0 student (~0.5B, `.gq`, W8A8) on `crates/galion-tensor`, with `CannedLlm` until trained weights exist.
> The S0 teacher / training source is an **open owner decision** (no third-party model is assumed).
>
> | core | role | threads |
> |---|---|---|
> | 0 | bus / heartbeat / audio I/O / VAD-endpoint / orchestrator + low-priority context/retrieval worker | 1 |
> | 1 | Vosk ASR | 1 |
> | 2-5 | in-house reasoning (S0 on galion-tensor; CannedLlm until weights) | 4 |
> | 6 | Piper Samantha | 1 |
> | 7 | Piper Jarvis | 1 |
> | — | clone workers | none during a live call |
>
> In code: `swarm/core-map-v3` (PR #68), `CoreRole::{Bus, Retrieval}` 0, `Asr` 1, `Reasoning` 2-5, `PiperSamantha` 6, `PiperJarvis` 7.
> Qwen / Llama / llama.cpp / Groq numbers below are **historical / superseded** measurements, kept for the record only.


**Status: STOPPED EARLY at 18:48 CET.** The box disk dropped to **11 GB free**. The parent's stop line is 12 GB. My downloads did not cause the drop: they were done by 18:41 CET, when the box still had 16 GB free. I made no new local measurements. Every number below is from another source or is labelled as an estimate.

## What was done
- Models in `models/` (3 downloads, all under 4 GB) plus 1 file made from one of them:
  - Qwen2.5-0.5B-Instruct Q8_0 (531 MB, Apache-2.0)
  - Llama-3.2-1B-Instruct Q8_0 (1.32 GB)
  - Llama-3.2-3B-Instruct Q4_K_M (2.02 GB)
  - Llama-3.2-1B Q4_K_M, re-quantized locally from the Q8_0 file (`*.from-q8.gguf`)
  - llama.cpp b12076 CPU binary (`models/llamacpp/`)
  - **Duplicates:** `/workspace/p95-scratch/` already holds `llama3b.gguf` (the same 2.02 GB file) plus 1B, 0.5B and 1.5B Q4_K_M. I did not know this when I downloaded. About 3.8 GB in `models/` duplicates scratch and could be reclaimed by the owner. I deleted nothing.
  - Partial leftover: `Llama-3.2-1B-Instruct-Q4_K_M.from-q8.gguf.partial.off`, about 268 MB, from an interrupted quantize run.
- **Rust bench, built but not timed:** `latency-240/05-bench/` (candle 0.9.2, CPU).
  - It uses vendored copies of candle's `quantized_llama` and `quantized_qwen2` with two patches:
    1. The causal mask now accepts an offset, so it is `(t, off+t)`. Upstream builds a `(t,t)` mask, which breaks a multi-token user prefill on top of a cached prefix.
    2. `Clone` is added to the qwen2 `ModelWeights`.
  - System-prompt KV reuse: prefill the 150-token system prefix once, keep a clone of the model as a snapshot (tensors are Arc'd, so the clone is cheap), then for each turn clone the snapshot and prefill only the user suffix.
  - Modes: `cold` (full prompt each turn) and `cached`. Output is JSONL per turn, with TTFT, decode tok/s and the reply.
  - Smoke test only: the model loaded and the system prefill ran (Qwen 0.5B, 2 threads). No timing run was done.
  - To run: `tools/run_candle.sh <gguf> <label> <llama|qwen2> <tokenizer.json>`, wrapped in `flock BENCH.lock`. Tokenizers are in `models/tok/`.
- **llama.cpp harness, ready:** `tools/run_llamacpp.sh` starts `llama-server` (6 threads, 1 slot) and runs `tools/server_ttft.py`.
  - The client is bench tooling only, not shipped. It sends the same 150-token system prompt and 12 voice turns, cold (`cache_prompt:false`) versus cached.
  - Not run: the BENCH.lock was held by other agents until the disk stop.
- mistral.rs was not built because of the time and disk limits.

## Existing measurements to use (research/P95-TAIL-RESEARCH.md, llama.cpp, warm cached system prompt, Q4_K_M)

| Model | Threads | Box load | Warm TTFT p50 | Warm TTFT p95 |
|---|---|---|---|---|
| Qwen2.5-0.5B | 4 | ~3.7 | 66 ms | 96 ms |
| Llama-3.2-1B | 4 | ~3.7 | 71 ms | 90 ms |
| Qwen2.5-1.5B | 4 | ~3.7 | 90 ms | 155 ms |
| Llama-3.2-3B | 4 | 4–17 | 225 ms | 760 ms |
| Llama-3.2-1B | 6 | 4–17 | 408 ms | 2370 ms |

- Cached prefill sizes: only the 8–28 user tokens are prefilled, so TTFT is driven by prefill throughput.
  - Measured prefill (pp80) at 4 threads: Qwen2.5-0.5B 416 tok/s, Llama-3.2-1B 371 tok/s, Qwen2.5-1.5B 225 tok/s.
- Quality, from the same source: 0.5B and 1B gave refusals and off-topic replies. **The quality eval was not re-run by me.**
- **Estimate:** the 3B model would need roughly 3× the 1B compute. That gives about 200+ ms warm TTFT even on a quiet box. 3B does not fit 120 ms on this CPU; it only works as a quality fallback.

## Recommendation (historical, superseded by the 20:45 ruling: no Qwen, no Llama, no cloud)
- **Primary:** keep the cloud 8B for quality. Run a **local Llama-3.2-1B or Qwen2.5-1.5B Q4_K_M** with the system prompt cached as a *hedge*.
  - Reserve cores for it, e.g. 2–4 cores set aside from the Chatterbox and Piper workers (coordinate with 02-core-pinning).
  - Without reserved cores, local p95 under load (2.4 s) is worse than Groq's.
- **Pick Qwen2.5-1.5B** (Apache-2.0) if the quality check favours it. Its warm TTFT p50 was 90 ms, inside the 120 ms budget.
- Expected contribution with reserved cores (estimate): LLM stage **p50 about 90 ms, p95 about 160 ms**, down from 284.7 / 498.0. Without reserved cores, budget the stage as unchanged.

## Hedged request design (local + cloud race) — historical, superseded (no cloud, no third-party model)
1. At endpoint (or on a stable partial transcript), fire **both** requests together:
   - the cloud request on a keep-alive connection (the curl-per-turn TLS cost is noted in P95-TAIL-RESEARCH item 1);
   - a local `llama-server` request on the OpenAI-compatible SSE API (`GALION_LLM_URL` already supports this), with `cache_prompt:true` and one slot per persona.
2. **Speak the first clause from whichever stream delivers it first. Do not switch streams in the middle of a reply.**
   - The winner owns the turn. Cancel the loser by closing its HTTP stream; llama-server stops decoding when the client disconnects.
3. Quality guard: if the local stream wins but its first clause fails a cheap check (refusal patterns such as "I can't" or "As an AI", or an empty or bracketed reply), drop it and wait for the cloud.
   - Cap that wait at 350 ms. P95-TAIL-RESEARCH estimates that a 350 ms cap alone gives p95 744 ms.
4. Variant B (fewer wasted local cycles): start the cloud request first and start local only if there is no cloud token after about 150 ms (roughly cloud p25–p40). This saves CPU but only fixes the tail, not the p50.
5. Rust implementation sketch: in `galion-v2-live/src/llm.rs`, add `stream_race(cfgs: &[LlmCfg], msgs) -> Receiver<LlmEv>`. It spawns both streams, forwards events from the first that emits a `Token`, and kills the other child. Each `LlmEv` gets a `source` tag for the logs.
6. Cost: local is free but takes 2–4 reserved cores during the call. Cloud is billed on every turn either way.

## Licenses
- **Llama 3.2:** Llama 3.2 Community License. Commercial use is allowed unless the licensee has more than 700M monthly active users. Requirements:
  - show "Built with Llama";
  - include the license and the Acceptable Use Policy;
  - names of derived models must start with "Llama".
  - The EU clause restricts the multimodal (vision) models only; the 1B and 3B text models are not affected. Source: llama.com/llama3_2/license.
- **Qwen2.5-0.5B, 1.5B and 7B:** Apache-2.0. **Qwen2.5-3B and 72B:** Qwen Research / Qwen license, which is not open commercial. Avoid 3B.
- **Phi-3.5-mini and Phi-4-mini:** MIT. Not measured; at 3.8B it is estimated slower than Llama-3B.

## TTS libraries
Already covered with measured and published numbers in research/P95-TAIL-RESEARCH.md (lines ~120–145), so I did not repeat them:
- espeak-ng direct C API: first audio under 1.1 ms, 4-word clause 2.2 ms, measured; GPL-3.0+.
- Flite: 45 ms for 20 words in a 2001 paper on a 500 MHz CPU; BSD-like license.
- RHVoice: no published latency; has Croatian voices; voice licenses are mostly NC-ND.

Not researched by me because of the stop: Festival, Mimic 3 (archived, ONNX VITS), and Rust TTS crates (piper-rs, sherpa-rs, kokoro via ort).

## Blockers
- Disk under 12 GB, so I stopped.
- BENCH.lock was contended by other agents.
- No local candle or llama.cpp timings, and no quality eval, were done by agent 05.

---
# UPDATE 19:01 CET: V2 is fully local (owner decision t608u, no Groq). Production checklist — historical, superseded at 20:45 CET

Disk is back to 45 GB free after the cleanup, so the earlier stop no longer applies. No new downloads were made: the queued runs reuse the GGUFs already in `/workspace/p95-scratch/`.

Queued benchmarks, all with the system prompt cached and 4 threads, waiting on a busy BENCH.lock with box load around 18:
- llama.cpp Qwen2.5-1.5B Q4_K_M
- llama.cpp Llama-3.2-1B Q4_K_M
- candle Qwen2.5-1.5B (Rust)

Results land in `05-bench/out/{llamacpp-qwen15-t4,llamacpp-l1b-t4,candle-qwen15-t4}.jsonl` and the `*.summary` files. Each per-turn row includes the reply text, which is the input for the quality check.

**Production model (historical, superseded; NOT the architecture): Qwen2.5-1.5B-Instruct Q4_K_M. Now: in-house S0 on galion-tensor, CannedLlm until weights.**
- TTFT p50 90 ms / p95 155 ms. Measured in P95-TAIL-RESEARCH with llama.cpp, 4 threads, box load ~3.7.
- Under heavy load and without pinning, TTFT is unknown. For comparison, the 1B model on 6 threads at load 4–17 was 408 / 2370 ms.

**Fallback model (historical, superseded; no fallback to any third-party model): Llama-3.2-1B Q4_K_M, 71 / 90 ms** (same source). It is faster, but it had quality problems: refusals and off-topic replies.

Before / after for this stage:
- Before: Groq 8B, 284.7 / 498.0 ms.
- After: 90 / 155 ms, measured on a lightly loaded box and not yet re-verified under the production load.

| # | Item | Pass criterion | Status |
|---|---|---|---|
| 1 | Model chosen | in-house S0 (`.gq`, W8A8, galion-tensor); teacher/training source = open owner decision | **OPEN** (historical PASS was Qwen2.5-1.5B, superseded) |
| 2 | Model on box, under 4 GB, no new download | file present | **PASS** (`p95-scratch/qwen15.gguf`, 1.12 GB) |
| 3 | RAM budget | model + KV for 2 personas under 2 GB | **PASS** (estimate: 1.1 GB weights; KV f16 = 28 layers × 2 (K,V) × 2 kv-heads × 128 dim × 2 B ≈ 28.7 KB/token, so ≈ 57 MB per 2k-context slot, ≈ 115 MB for 2 personas) |
| 4 | Warm TTFT p50 ≤ 120 ms with cached system prompt | measured | **PASS** (90 ms, P95-TAIL-RESEARCH, load ~3.7) |
| 5 | Warm TTFT p95 ≤ 160 ms | measured | **PASS (marginal)** (155 ms, same run) |
| 6 | TTFT holds under the real call load (Chatterbox + Piper running) on reserved cores | p95 ≤ 200 ms with `taskset` on 4 cores | **FAIL / not run** (the only loaded data, 1B unpinned, is 408 / 2370 ms) |
| 7 | Core reservation agreed with 02-core-pinning | written core map, e.g. LLM 0-3, clone 4-7 | **FAIL** (not agreed; clone workers currently want all 8 cores) |
| 8 | Reply quality eval | ≥ 90 % on-topic, ≤ 5 % refusals, ≤ 25 words, no brackets or emojis, 12+ turns | **FAIL / pending** (queued run collects replies; 0.5B/1B had failed) |
| 9 | Rust-first runtime | candle (Rust) TTFT within 1.5× of llama.cpp | **PENDING** (candle harness built with system-prompt KV reuse and the mask patch; not yet timed) |
| 10 | System-prompt KV reuse per persona | one slot per persona (`-np 2` or 2 servers), cache hit on every turn | **PARTIAL** (llama-server `cache_prompt` measured on 1 slot; 2-persona setup not done) |
| 11 | Wired into the live path | `GALION_LLM_URL` = local OpenAI-compatible endpoint; no cloud traffic | **FAIL** (config not changed. Note: `GALION_LLM_PROVIDER=""` still sends `provider:{order:[""]}`; this needs a code fix to send nothing) |
| 12 | Persistent connection (no curl spawn per turn) | keep-alive client, or reqwest in-process | **FAIL** (owned jointly with 04-pipeline) |
| 13 | Server supervision | started at call boot, warm-up request, health check, restart on crash | **FAIL** |
| 14 | Stall fallback (now that there is no cloud) | if no token within 250 ms, say a prepared line or backchannel (04-pipeline) and keep waiting | **FAIL** (design only; replaces the cloud-race design above, which is now obsolete) |
| 15 | Generation config | max_tokens ~40, stop tokens, temperature tuned for short replies | **PARTIAL** (bench uses 32–40 tokens, greedy; live path still 60–70 at temperature 0.7) |
| 16 | Code on branch `swarm/lat240-local-model` with PR, no merge | branch pushed | **FAIL** (no repo change yet) |

**Completion: 5 pass + 2 partial (counted as 0.5 each) of 16 items = 6/16 = 37.5 %.**

The biggest risks are items 6, 7 and 8. With no cloud, the LLM stage's p95 depends entirely on core isolation from the clone lane, and the reply quality of a 1.5B model is not yet verified.
