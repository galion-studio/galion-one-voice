> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 4: Real-Time Latency Optimization & Streaming Pipelines (Galion Voice V2)

_Researched 2026-11-03. Sources listed at the bottom, numbered [n]._

## 1. Target and why it matters
- UX bands: <500 ms is aggressive; 500–800 ms is a sound production target; 800–1500 ms is noticeable; >1500 ms feels sluggish [5].
- Twilio's Nov-2025 median benchmarks: mouth-to-ear turn gap target **1,115 ms** (upper limit 1,400); platform turn gap 885 ms (upper limit 1,100) [6].
- The competitive brief (domain 09) found phone stacks often run 1.0–1.9 s EOU→TTFA. **Galion V2 goal: p50 ≤ 900 ms and p95 ≤ 1,300 ms mouth-to-ear on PSTN.** That beats the field median and sits under Twilio's upper limit.
- Blocking pipelines land at 1000–2000 ms+. Streaming pipelines land at 400–800 ms, because stages overlap and total latency approaches max(stage) instead of sum(stage) [2].

## 2. Latency budget by stage (EOU → first audible byte)
| Stage | Industry target | Galion V2 budget (phone) | Notes |
|---|---|---|---|
| Endpointing / EOU decision | 150–200 ms [1] | 160–240 ms (2–3 heartbeats at 80 ms) | The 80 ms tick quantizes this. A decision can lag up to one tick. |
| ASR finalization | 150–250 ms [1]; Twilio STT target 350 ms [6] | ≤150 ms *effective* | Use the stable interim transcript and don't wait on the final flag (§4). |
| LLM TTFT (to first clause) | 200–450 ms [1]; Twilio 375 ms [6] | ≤350 ms | Keep RAG and guardrail calls off the critical path. They add 300–800 ms [1]. |
| TTS first playable chunk | 150–300 ms [1]; 100–200 ms streaming [2] | ≤150 ms (warm Piper, short first clause) | Piper synthesizes a whole sentence/clause at a time [7][9], so the first chunk scales with first-clause length. |
| Network + jitter/playout | 50–100 ms [1]; buffer 50–150 ms on stable links [4] | ≤150 ms | ngrok adds a public hop (§6). |
| **Total** | ~700 ms good case [1] | **~800–1,050 ms** | |

Pipecat-style accounting covers only the pipeline, from user-stopped-speaking to bot-started-speaking. It leaves out both media legs [3], so track the platform number and the mouth-to-ear number separately.

## 3. Streaming ASR
- Feed the stable interim hypothesis into the next stage before the final flag fires [1].
- Vendor endpointing defaults vary a lot. Deepgram's `endpointing` defaults to 10 ms. Pipecat Silero `stop_secs` is 0.2 s. LiveKit's `min_delay` is 0.5 s (0.3 s with its turn detector) [3]. Galion should own the EOU decision in TurnWorker and treat vendor finals as confirmation, not as the trigger.

## 4. Partial commit (speculative turn start)
Pattern: once the interim transcript is stable (unchanged for N ticks) and VAD silence passes about 160 ms, **speculatively** start the LLM on that transcript. Hold the TTS output until EOU is confirmed.
- If the final transcript matches the speculative prefix (normalized), commit and play. This saves roughly the ASR finalization and part of TTFT.
- If the user resumes speaking or the final differs materially, cancel the LLM stream (abort the HTTP stream) and discard any queued TTS. This costs only tokens.
- Commit in two tiers: (a) "LLM may start" at around 2 ticks of silence; (b) "audio may play" at EOU confirmed, around 2–3 ticks plus the semantic turn check.
- This follows the "stream every stage; never wait for a full unit" rule. Buffering any stage is where most 300–800 ms overruns come from [1].

## 5. Streaming TTS with Piper
- Piper's VITS model works on whole utterances, not true incremental audio. It splits text into sentences and yields each one when ready [7][9]. `--output_raw` / `--output-raw` streams PCM as each sentence finishes [7][8].
- **Short first clause:** prompt the LLM to open with a short clause of 3–8 words and flush it to Piper at the first `,;.!?` or after about 8 words. Splitting word by word ruins prosody [9].
- Piper is fast once warm. Measured RTF on 4 CPU cores is 0.037–0.052 (ljspeech-medium) [10], so a 1.5 s clause takes about 60–80 ms to synthesize. Cold model load dominates otherwise. In one report, keeping the process open cut a long utterance from 14 s to 4 s on weak hardware [7].
- An experimental ONNX streaming path splits the encoder output and decodes it chunk by chunk, logging first-chunk latency [11]. It's worth evaluating for long first sentences.
- Output format: synthesize at the voice's native rate and resample/encode to 8 kHz μ-law in-process with a streaming resampler. Avoid shelling out to ffmpeg per turn.

## 6. Warm processes and transport
- Run Piper as a persistent in-process model (Python `PiperVoice`) or a long-lived server, one per voice and preloaded at boot. Never spawn it per turn [7][10].
- Hold connections open: keep LLM HTTP/2 connections and ASR WebSockets alive across turns. Prewarm on call `connected`, not on first speech [5].
- Run a dummy synth and a 1-token LLM call at startup and on idle. Keep cold-start samples in their own column [10].
- **ngrok edge:** use it for development only. It adds a public-internet hop and leaves tail latency to chance. Twilio's guidance is to colocate STT/LLM/TTS near the media edge that serves the call. What matters is the distance from Twilio's ingress to your services [6][12]. Production should run as a regional host (e.g. near IE1/Dublin for EU numbers, or US1) [12].
- TurnWorker off-tick is correct. Don't gate audio output on the 80 ms heartbeat. Push TTS frames to the Media Stream as soon as they exist. Keep the tick only for decisions (EOU, barge-in).
- Playout buffer: start sending at the first chunk. Don't pre-buffer 500 ms; a 50–150 ms buffer is enough on stable links [4].

## 7. Measuring TTFA
Keep three events separate: first response byte, first playable audio, and first sound heard. Headers (WAV/HTTP) aren't audio [10]. TTS TTFB is not TTFA [3].
Per turn, with one `turn_id`, log these monotonic timestamps:
`t_vad_silence_start, t_eou_decided, t_asr_final, t_llm_req, t_llm_first_token, t_first_clause_flushed, t_tts_first_pcm, t_first_media_frame_sent, (t_first_audio_heard via external probe)`.
- Report p50/p95 per region and per hour. Investigate whichever stage moves when p95 moves [3].
- Measure from outside the pipeline with an external caller probe: a synthetic call that plays a WAV and times the gap from its own speech end to the first non-silent inbound audio. Reconcile that against internal stamps [3].
- Report the first turn (session setup) separately. It's usually the longest [3].

## 8. Galion-specific recommendations (priority order)
1. **Instrument first.** Add the §7 stamps plus an external PSTN probe running 20 calls nightly, and set up a p50/p95 dashboard. Without this, nothing else is verifiable.
2. **Partial commit:** start the LLM speculatively on a stable interim at about 2 ticks (160 ms) of silence, gate audio on EOU, and cancel on resume. Expected saving: 150–300 ms.
3. **First-clause flush to Piper** at punctuation or about 8 words, with a prompt instruction for a short opener. Expected saving: 100–400 ms on long first sentences.
4. **Warm Piper in-process,** preloaded and prewarmed, with in-process μ-law resampling. This removes cold spikes.
5. **Push audio off-tick:** the TTS→Media Stream writer should be event-driven, never tick-aligned. Saves up to 80 ms.
6. **Leave ngrok for production** and move to a regional VM colocated with the Twilio region. The tail-latency gain is unmeasured until step 1 is in place.
7. **Keep the critical path clean:** do RAG/tools before EOU (speculatively) or after the first clause. Run guardrails in parallel, never in series [1].
8. **Filler only as a fallback:** if the LLM's first token isn't back by about 600 ms, play a short cached acknowledgement ("Mm-hm,") from a pre-synthesized clip.

## Sources
1. AWS Builder Center, "Five Rules That Keep a Voice Agent Inside Its Latency Budget" https://builder.aws.com/content/3JDEQi9AVukeX7JbxagjF0eROCf/five-rules-that-keep-a-voice-agent-inside-its-latency-budget
2. LiveKit, "Sequential pipeline architecture for voice agents" https://livekit.com/blog/sequential-pipeline-architecture-voice-agents
3. rtcanary, "Voice Agent Latency: How to Measure Time to First Audio" https://rtcanary.com/blog/voice-agent-latency/
4. ElevenLabs, "Voice agent latency optimization" https://elevenlabs.io/blog/voice-agent-latency-optimization
5. Bearworks, "Sub-500ms Voice Agents" https://www.bearworks.com/post/sub-500ms-voice-agents-stt-llm-tts-latency
6. Twilio, "Core Latency in AI Voice Agents" https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents
7. rhasspy/piper issue #25, "Streaming the output" https://github.com/rhasspy/piper/issues/25
8. wip.tf, "My self-hosted TextToSpeech setup" https://wip.tf/posts/tts-setup/
9. KoljaB/RealtimeTTS issue #339 https://github.com/KoljaB/RealtimeTTS/issues/339
10. Mahimai, "TTFA vs TTFB: Measuring TTS Latency" https://mahimai.ca/learn/100x/tts/ttfa-vs-ttfb
11. Piper docs, infer_onnx_streaming.py https://tderflinger.github.io/piper-docs/codereading/python/piper_train/infer_onnx_streaming/
12. Twilio, "Understanding Edge Locations" https://www.twilio.com/docs/global-infrastructure/understanding-edge-locations

— Maciej Grajczyk
