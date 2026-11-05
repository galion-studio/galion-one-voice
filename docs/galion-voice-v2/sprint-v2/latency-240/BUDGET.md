# Galion One Voice: 240 ms latency budget (end of user speech -> first content audio)

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

Baseline (results/latency, 24 runs): p50 608.3 ms, p95 775.1 ms.

| Stage | Baseline p50 | Budget | Owner |
|---|---|---|---|
| End-of-speech detection (160 ms hang) | 130.2 | 60 | 04-pipeline |
| ASR final | 19.9 | 15 | 04-pipeline |
| ASR -> LLM request gap | 67.9 | 5 | 04-pipeline |
| Reasoning first token (in-house S0 on cores 2-5; baseline was Groq cloud, historical) | 284.7 (historical, Groq) | 120 | 05-local-model |
| First token -> first clause | 20.6 | 10 | 04-pipeline |
| TTS first audio (Piper warm) | 71.3 | 30 | 01-streaming-tts |
| Playback handoff | 0 | 0 | 01-streaming-tts |
| **Total** | **~595** | **240** | |

Clone lane (Samantha/Jarvis): 02-core-pinning and 03-clone-efficiency report their own first-clone-audio numbers.
Each agent appends a row to CONTRIBUTIONS.md: agent, stage, before p50/p95, after p50/p95, measured or estimate.
