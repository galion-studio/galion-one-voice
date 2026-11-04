# Local Qwen2.5-1.5B in the live path (sprint item 5)

Measured 2026-11-04 11:43–12:02 CET on the sprint box, under `BENCH.lock`, core map v3
(llama-server b12076 on cores 2-5 with `-t 4 -c 4096 --parallel 2`; everything else on 0,1,6,7).
Model: `qwen2.5-1.5b-instruct-q4_k_m.gguf`, loopback port 18080. No cloud key was set.

## What changed

- `GALION_LLM_URL=http://127.0.0.1:…/v1/chat/completions` now selects the in-process
  keep-alive client in `src/local.rs` (loopback hosts only: 127.0.0.1, localhost, [::1]).
  No curl process and no temp file per turn. No API key needed. Dropping the receiver shuts
  the socket, which cancels the generation on barge-in. `GALION_LLM_CLIENT=curl` keeps the old path.
- The request body for local has `cache_prompt: true` and no `provider` / `usage` / `reasoning` fields.
- Bug fix: `GALION_LLM_PROVIDER=""` used to send `provider:{order:[""]}`. Empty entries are now
  dropped. `""` means no provider block at all. Unset means Groq for cloud, nothing for local.
  Test: `empty_provider_env_sends_no_provider`, which also checks `warm::Prompt`.
- The client drops pooled sockets idle for 4 s or more (`GALION_LLM_KEEPALIVE_MS`).
  llama-server answers `Keep-Alive: timeout=5`.
- The `Done` provider tag carries llama-server's own timings
  (`local cache_n=36 prompt_n=22 prompt_ms=202.8 tok_s=23.4`).
- `v2-latency` / `v2-call` prewarm the prompt cache with each persona's system prompt (1 token).
  `GALION_LAT_CLIPS` overrides the latency clip list.

## Numbers (v2-latency --only-loop, clips amy / alan / pdx, ms)

| client | n | llm_ttft p50 / p95 | eos→first content audio p50 / p95 |
|---|---|---|---|
| keep-alive (runs 1+2) | 36 | 184.8 / 254.7 | 522.2 / 658.5 |
| per-turn curl (runs 1+2) | 36 | 195.0 / 259.0 | 540.6 / 629.9 |
| keep-alive + idle drop (run 3) | 18 | 195.0 / 275.5 | 542.0 / 658.8 |

Honest read: on loopback the transport is **not** the cost. The difference between the clients
is within run-to-run noise (about 10 ms at p50). TTFT is llama-server time: run 3's server-reported `prompt_ms`
is 122–279 ms against client TTFT of 126–283 ms. Even fully cached turns (`prompt_n=1`) take 122–186 ms.
In `v2-latency` every turn is more than 5 s after the last one, so the server has already closed the socket
and no turn reused one. The gain is fewer processes, no key, and clean cancellation, not lower latency.

## Re-measured on the full core budget (owner item 8)

Measured 2026-11-04 16:04–16:14 CET under `flock latency-240/BENCH.lock`. There was **no taskset**: llama-server,
Vosk, and Piper share all 8 vCPUs (AVX-512, 15 GB RAM). The binary was the release build of this PR's head 40098a8
with `CARGO_TARGET_DIR=repo/target`, using the legacy piper runtime. Each config ran `v2-latency --only-loop --runs 30`.
Input clips u1–u6 were synthesised with Piper ljspeech (no donor or celebrity voice). The server ran
`-c 4096 --parallel 2` with `-t` as shown. No cloud key was set. A background host process used about half
a core for the whole run. Raw files: `results/llm-8core/` on the sprint box.

Raw llama.cpp (`llama-bench -p 64 -n 64 -r 5`, nothing else running; tok/s mean ± sd):

| threads | prefill pp64 | decode tg64 |
|---|---|---|
| 8 | 112.5 ± 23.6 | 22.9 ± 15.7 |
| 4 | 303.6 ± 6.2 | 21.7 ± 1.3 |

In the loop (ms; tok/s is llama-server's own `tok_s` per turn; n = 30 turns each):

| server `-t` | llm first token p50 / p95 | decode tok/s p50 (p5) | Piper first audio while LLM decodes p50 / p95 | eos→first content audio p50 / p95 |
|---|---|---|---|---|
| 8 | 235.3 / 381.6 | 21.4 (12.0) | 160.7 / 262.3 | 723.5 / 1118.1 |
| 6 | 212.4 / 319.2 | 23.6 (18.5) | 117.4 / 210.3 | 642.8 / 793.1 |
| 4 | 215.2 / 375.2 | 18.2 (16.3) | 116.5 / 165.8 | 677.7 / 888.5 |

Here's what the numbers say:

- **8 threads on 8 cores is the worst setting.** Prefill drops to a third of 4 threads and swings widely, and
  decode p5 falls to 12 tok/s. The LLM threads fight Piper, Vosk, and the background process. Piper's first
  audio gets slower too (p95 262 ms against 166–210 ms).
- **6 threads** gave the best eos→first content audio (p50 643 ms, p95 793 ms) and the best decode rate.
- **4 threads** gave Piper the most headroom (p95 166 ms).
- Each config has only 30 turns on one box, so the 6-vs-4 gap at p50 is within noise. At p95 it is not.
- These numbers sit above the 11:43 CET core-map-v3 numbers (4 pinned threads). The clips, the competing load, and
  the pinning all differ, so the two tables are not comparable.
- Choosing a thread count for the live config is an owner decision. The default in this PR is unchanged.

## Two-persona call (v2-call --lane piper, 120 s)

| run | turns | llm_ttft p50 / p95 | endpoint→first audio p50 / p95 | endpoint→first content clause p50 / p95 |
|---|---|---|---|---|
| call 1 (11:53 CET) | 22 | 263.3 / 1011.0 | 59.8 / 172.5 | 671.2 / 1316.9 |
| call 2 (12:00 CET) | 35 | 725.7 / 1314.9 | 60.0 / 641.1 | 1149.8 / 2290.3 |

Both personas share the one 4-thread server (`--parallel 2`), so overlapping turns queue. That is the
p95 and call 2's p50.

## Reply quality (honest)

Single turns are coherent and on topic (e.g. "The traveler felt the warmth of the cloak and realized
that the warmth was the true strength, not the wind."). It sometimes refuses oddly ("I'm sorry, but I
can't access or read articles…"). In the two-persona call the first turn is good but too long (it ignores the
1–2 sentence limit). By turn 4–6 both personas slide into assistant boilerplate and echo each other
("How can I help you today?" → "Let's continue our conversation." ×10). The 1.5B model is not good enough
for persona conversation. It is fine as an offline / fallback brain or for short backchannel-style replies.
