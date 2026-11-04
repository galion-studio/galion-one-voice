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
