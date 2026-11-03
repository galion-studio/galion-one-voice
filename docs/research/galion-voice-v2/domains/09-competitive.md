> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 9 — Competitive Landscape (Galion Voice V2)
_Research date: 2026-11-03 (Europe/Zagreb). Galion Voice = local-first, MIT, open code. paid_tiers=false: no paid tiers documented as product._

## 1. Landscape map
| Player | Type | Open? | Architecture | Latency (claimed vs measured) |
|---|---|---|---|---|
| Vapi | Hosted orchestrator, BYO STT/LLM/TTS | Closed | Cascade of chained third-party APIs | Claims sub-500 ms; measured median 1,558 ms / p95 2,008 ms on phone [1]; 1.56 s median [2] |
| Retell AI | Hosted platform | Closed | Cascade + proprietary turn-taking model [4] | 1,740 ms median [1]; 1.04 s median phone / 0.76 s browser [2] |
| Bland AI | Hosted, all-in billing | Closed | Own stack, "Pathways" scripted flows | Claims <400 ms (or <200) [3]; measured 1,520 ms median [1], 1.86 s [2] |
| ElevenLabs Agents / Eleven Realtime | Hosted | Closed | **Cascaded** STT→turn→LLM→TTS tuned end-to-end; OpenAI-Realtime wire-compatible WebSocket [5][6] | 1,424 ms phone median [1]; 403–439 ms p50 post-VAD [7] |
| OpenAI Realtime (GPT Realtime 2.1) | Hosted API | Closed | Native speech-to-speech; Go media frontend over WebRTC, tools on async RPC path [8] | 693–783 ms p50 post-VAD [7] |
| LiveKit Agents | Framework + WebRTC SFU | Apache-2.0 (turn model under LiveKit Model License) [10] | Server-side agent joins WebRTC room; mix STT/LLM/TTS or realtime models; semantic audio turn detector (v1 hosted, v1-mini local CPU) [9] | Self-measured; turn detector shortens 0.5–3.0 s default endpoint wait [9] |
| Pipecat (Daily) | Framework | Open source (BSD-2) [11] | Frame-based composable pipeline, transport-agnostic (WebSocket/WebRTC/Daily/LiveKit) [11] | Silero VAD + local Smart Turn v3 ONNX on CPU, default since v0.0.102 [12] |
| Daily | WebRTC transport/cloud | Closed infra, open Pipecat | Hosts Pipecat Cloud | — |

## 2. Key findings
- **Advertised latency is unreliable.** Every hosted vendor's sub-500 ms claim measures ~1.0–1.9 s end-to-end on real phone calls [1][2][3]. Independent rankings disagree with each other (Retell 1st in [2], last in [1]) — configuration (endpointing timer!) dominates [1].
- **Endpointing is the hidden latency.** Vapi shipped 0.4–1.5 s and Retell 1,000 ms default waits [1]; LiveKit defaults 0.5–3.0 s without semantic turn model [9].
- **Cascade is winning back.** ElevenLabs' fastest product is cascaded, not fused, chosen for transparency (text at each step enables guardrails/PII redaction/debugging) [6]. Cascades reach ~540–550 ms p50 post-VAD [7].
- **Open frameworks own the turn-detection commons:** Smart Turn v3 (fully open, CPU) [12] vs LiveKit v1-mini (local CPU, custom model license) [9][10].
- **De-facto wire protocol:** OpenAI Realtime events over WebSocket; ElevenLabs cloned it [5].

## 3. Where Galion fits
Only local-first + MIT + branded voice-first product in the set; frameworks (Pipecat/LiveKit) are toolkits, not products; hosted ones are closed and cloud-bound. Galion's honest edge: zero network hops for STT/TTS → can beat 1.3 s phone medians on-device if endpointing is tuned.

## 4. Patterns to steal (patterns, not APIs)
1. **Split media fast path from logic** — audio loop never blocks on tools; tools via async boundary (OpenAI [8]). Also: rewrite of hot path out of Python asyncio gave p95 = old p50 [8] → supports a Rust/Go media core.
2. **VAD + semantic turn model + silence fallback** (Pipecat [12], LiveKit [9]). Use Smart Turn v3 (open) as default; pin short endpoint (~0.1–0.2 s) once turn model is confident.
3. **Measure TTFAB from recorded audio, p50 + p95 + tail ratio + discarded turns** (Openbenchmarks [1]) — adopt as Galion's eval metric; never publish vendor-style claims.
4. **Keep the cascade observable** — text at every stage for debugging/guardrails [6].
5. **Composable frame pipeline + pluggable transports** (Pipecat [11]).
6. **Optional OpenAI-Realtime-compatible event schema** at the edge so existing clients can talk to Galion locally [5] (compatibility shim, not dependency).
7. **Model fallback** (full → mini local) mid-session (LiveKit [9]).
8. **Barge-in/talk-over tests** in eval suite [2][3].

## 5. Avoid
- Chaining 4–5 remote APIs (latency stacking under concurrency) [4].
- Publishing unmeasured latency numbers [3].
- Turn models with restrictive licences in an MIT core (LiveKit Model License) [10] — keep optional.

## Sources
[1] https://openbenchmarks.com/voice-agent-latency/voice-agent-latency-comparison
[2] https://cozzo.io/benchmarks/vapi-vs-retell-vs-bland
[3] https://www.visionnexera.com/insights/vapi-vs-retell-vs-bland-ai-for-production-voice-agents-2026-engineering-comparison
[4] https://www.retellai.com/blog/vapi-vs-bland (vendor-authored)
[5] https://elevenlabs.io/docs/eleven-agents/guides/openai-realtime-compatibility
[6] https://elevenlabs.io/blog/realtime
[7] https://sprag.ai/benchmarks?tab=agentic (vendor-run benchmark; Sprag ranks itself 1st)
[8] https://openai.com/index/continuous-voice-interaction-with-gpt-live/
[9] https://docs.livekit.io/agents/logic/turns/turn-detector/
[10] https://github.com/livekit/agents/
[11] https://github.com/pipecat-ai/pipecat
[12] https://docs.pipecat.ai/api-reference/server/utilities/turn-detection/smart-turn-overview

— Maciej Grajczyk
