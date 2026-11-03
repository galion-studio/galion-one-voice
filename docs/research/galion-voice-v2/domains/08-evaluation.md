> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 8 — Evaluation & Benchmarking of Voice Agents (Galion Voice V2)

Constraint: the Galion presence harness **forbids engagement metrics** (session length, retention, DAU, "stickiness"). Everything below measures *quality of presence*: timing, listening, accuracy, and how natural the voice sounds. None of it rewards keeping a user on the line.

## 1. Landscape of benchmarks and harnesses

| Tool | What it measures | Why it matters for Galion | Source |
|---|---|---|---|
| **voicebench** (PyPI v0.2.0) | Response latency, TTFA, barge-in stop time and success rate, false barge-in, spurious response, talk-over, overlap, WER. Scenarios are YAML; it returns exit code 1 on a failed assertion; it has mock, WebSocket, LiveKit and Pipecat transports | Closest fit for CI gating. Timing comes from VAD on the recorded audio | https://pypi.org/project/voicebench/ |
| **voice-agent-bench** (todoforai) | Black-box, audio-only measures: voice→voice, barge-in stop, first *content* word (a "Hmm" filler doesn't count), EOT delay, STT first partial, TTS first audio, echo leakage, self-interruption | Stage breakdown, plus catching filler-word gaming | https://github.com/todoforai/voice-agent-bench |
| **voice-evals** (PyPI v0.2.1) | WER per call, E2E latency p50/p95/p99, stage TTFT/TTFA, barge-in with unconfirmed stops reported as censored, deterministic task checks (required facts, forbidden phrases) | Production regression gates without an LLM judge | https://pypi.org/project/voice-evals/ |
| **openbenchmarks-labs/voice-agent-latency** | TTFAB from dual-channel PSTN recordings, using a neutral carrier | Competitive phone baseline (below) | https://github.com/openbenchmarks-labs/voice-agent-latency |
| **tvbench** (ICT Innovations) | Caller-side opening latency, pacing, continuity, barge-in cut (first caller frame → last agent frame) | "Caller's seat" methodology | https://ictinnovations.com/caller-side-voice-agent-benchmark-open-source/ |
| **Full-Duplex-Bench v1 / v1.5** | Pause handling (Takeover Rate), backchannel timing, smooth turn-taking latency, interruption handling; v1.5 adds overlap behaviour and prosody, plus UTMOSv2 before and after overlap | Turn-taking quality beyond raw speed | https://arxiv.org/html/2503.04721 · https://arxiv.org/html/2507.23159v3 |
| **UTMOS / UTMOSv2** | Automatic naturalness MOS with no reference audio needed. VoiceMOS 2022 winner; UTMOSv2 placed 1st or 2nd on all 16 VMC 2024 metrics | Nightly MOS proxy. It's trained on English and doesn't judge content | https://arxiv.org/html/2204.02152v2 · https://github.com/sarulab-speech/UTMOSv2 · https://picovoice.ai/blog/measuring-tts-quality/ |

## 2. Competitive latency baseline (phone)
openbenchmarks leaderboard, 2026-08-01, TTFAB over PSTN:
- p50: Telnyx 1,296 ms, ElevenLabs 1,424 ms, Bland 1,520 ms, Vapi 1,558 ms, Retell 1,740 ms
- p95: ElevenLabs 1,768 ms up to Retell 2,259 ms

So the market band is **~1.3–1.9 s, with p95 around 2.3 s**. Methodology lessons:
1. Measure from recordings, never from a platform's own timestamps.
2. Disarm filler phrases, or count first content word separately.
3. Record endpointing config in a receipt for every run.

## 3. Metric definitions (Galion canonical)
- **TTFA**: user speech offset (VAD on the user track) → first non-silent agent audio. Report p50/p95/p99.
- **TTFCW**: time to first *content* word, so fillers don't game the number.
- **Barge-in stop time**: interruption onset → last agent audio frame. Also report success rate (share stopped in ≤ max_stop_ms), false barge-in rate (cough, "mm-hm") and spurious response rate.
- **Takeover rate on mid-utterance pauses** (Full-Duplex-Bench TOR).
- **WER**: Galion's ASR transcript against the scripted reference, normalised. Slice it by accent, noise and phone codec.
- **Naturalness MOS**: UTMOSv2 nightly, plus a human MOS/CMOS panel per release. Check UTMOS against the human scores on Galion's own languages.
- **Task/fact correctness**: required facts present and forbidden phrases absent. Deterministic checks only.
- **Explicitly excluded**: session duration, turns per session, return rate, any "engagement" score.

## 4. Proposed Galion eval suite ("presence-eval")

| Tier | When | Contents | Gate |
|---|---|---|---|
| T0 unit | Every PR | voicebench mock transport, deterministic virtual clock | Schema plus logic regressions |
| T1 scenario | Every PR, WebSocket/LiveKit | ~50 YAML scenarios: clean Q&A, 900 ms barge-in, cough, mid-sentence pause, backchannel, noisy café, 8 kHz codec | TTFA p50 ≤ 800 ms / p95 ≤ 1,200 ms (web); barge-in stop p95 ≤ 300 ms, success ≥ 95%; false barge-in ≤ 5%; TOR ≤ 10%; WER clean ≤ 6%, noisy ≤ 12% |
| T2 phone | Nightly | Neutral-carrier PSTN dual-channel calls (openbenchmarks method), ≥ 200 turns | TTFA p50 ≤ 1,000 ms, p95 ≤ 1,500 ms, i.e. beating the 1.3–1.9 s market band |
| T3 naturalness | Nightly / per release | UTMOSv2 on a fixed prompt set, plus post-barge-in re-entry MOS (FDB v1.5 style) | UTMOSv2 ≥ 4.0; drop after barge-in ≤ 0.1 |
| T4 human | Per release | 20+ raters, MOS and CMOS against the previous release, turn-taking naturalness | No significant regression (p < 0.05) |

Every run writes a receipt: model, ASR, TTS, endpointing and VAD config, transport, commit hash. Reports are JSON, Markdown and HTML.

All gate thresholds are first-draft proposals. Calibrate them against Galion V1 baseline runs before enforcing.

— Maciej Grajczyk
