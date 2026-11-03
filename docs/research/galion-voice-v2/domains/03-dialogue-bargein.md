> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 03 — Dialogue Management, Turn-Taking, Barge-In (Galion Voice V2)
_Research agent 3/10 · 2026-11-03 (Europe/Zagreb, CET) · sources checked this run_

## 1. TL;DR
- Treat every turn as **two moments**: *speculate* (start LLM at medium confidence) and *commit* (release TTS audio on high confidence). Deepgram Flux formalizes this as `EagerEndOfTurn` → `EndOfTurn` / `TurnResumed` [D1][D2][D5].
- Barge-in = a **SystemFrame-priority cancel** that cancels the LLM, clears TTS, and **flushes the unplayed playout buffer**. Pipecat says "silent within roughly one audio write" [P3]. Galion's TurnWorker supersede-cancel already matches this. The missing piece is flushing the playout/jitter buffer on the phone leg.
- Backchannels ("uh-huh", "okay") must **not** cancel. Two ways to filter them: an acoustic classifier (LiveKit adaptive mode [L4]; Krisp VIVA in Pipecat [P3]) or gates on duration and word count (`min_duration` 0.5 s, `min_words`) [L1][L2]. Add **false-interruption recovery**: resume speech after 2 s of silence with no transcript [L2][L3].
- Silero VAD at 8 kHz phone audio: **256-sample / 32 ms frames**, <1 ms CPU per frame [S1][S2]. The speech-start signal comes after ~1 frame. The end signal waits `min_silence_duration_ms` (default 100 ms; 200–300 ms is better for conversation) [S1][S3].

## 2. Patterns surveyed
| System | Turn end | Barge-in | Backchannel handling | Speculation |
|---|---|---|---|---|
| **Deepgram Flux** | Integrated EOT model. `eot_threshold` default 0.7, `eot_timeout_ms` default 5000 [D3]. EOT latency 100–500 ms [D4]. Every EndOfTurn carries a `trigger` field (model/manual/timeout), and `ForceEndTurn` exists [D2] | Up to the app (StartOfTurn) | None built in | `eager_eot_threshold` 0.3–0.9 enables Eager/TurnResumed. The EOT transcript equals the Eager transcript, so it is cache-safe [D2][D3]. Saves ~100–200 ms at the cost of extra LLM calls [D1] |
| **Deepgram Voice Agent** | Same as Flux | Holds TTS until the turn is confirmed. Function calls are not gated unless `defer_until_eot` is set [D5] | — | Speculative window [D5] |
| **Pipecat** | Silero VAD `stop_secs=0.2` + Smart Turn v3 (local ONNX, last 8 s of audio). Silence fallback at `stop_secs` 3.0 [P1] | `InterruptionFrame` (SystemFrame) cancels LLM, TTS and transport queue. `interruptible=False` frames survive. Tools honor `cancel_on_interruption` [P3] | min-words strategy; Krisp VIVA model [P3] | Eager answer is voided via `UserTurnResponseCancelFrame` [P3] |
| **LiveKit Agents** | Turn-detector model or VAD/STT. `endpointing.min_delay` 0.5 s, `max_delay` 3.0 s, dynamic mode available [L1] | `interruption.mode` adaptive/vad [L1] | Adaptive audio model. `backchannel_boundary` (1.0, 1.0) s cooldown at turn edges [L2][L4] | `preemptive_generation` on by default. TTS preemption is off by default. Max 3 retries per turn [L1] |
| **Silero VAD** | `min_silence_duration_ms` 100, `speech_pad_ms` 30, threshold 0.5 with hysteresis at −0.15 [S1][S2] | Fast speech-start trigger | — | — |

## 3. Latency budgets (phone)
Targets, built from the cited components:
- **Barge-in stop (user speech onset → bot audio silent): ≤ 250 ms p95.** Breakdown: VAD onset 32–64 ms (1–2 frames) [S1], plus a debounce gate of ~120–200 ms (phone-tuned; shorter than LiveKit's 0.5 s `min_duration` because phone callers expect quick yield), plus cancel propagation (one 80 ms heartbeat tick at most, ideally an event push), plus the telephony buffer flush (PSTN jitter buffer ~40–60 ms).
- **Response (user stop → first bot audio): ≤ 800 ms p50, ≤ 1.2 s p95.** Breakdown: Flux EOT 100–500 ms [D4], minus the speculation saving of ~100–200 ms [D1], plus LLM time-to-first-token, plus TTS time-to-first-byte.
- **False-interrupt resume window:** 1.5–2.0 s [L2].
- The 80 ms heartbeat must **not** be on the cancel path. Cancel is an out-of-band event. The heartbeat is only a liveness/reconcile fallback.

## 4. Backchannel vs true barge-in
Decision ladder, cheapest test first:
1. Ignore VAD speech under ~150 ms. Treat 150–400 ms as **"duck"**: lower TTS gain by −12 dB and keep playing. Over 400 ms, or STT words ≥ 2 and not on the backchannel lexicon, counts as **yield**.
2. Lexicon: uh-huh/mm/yeah/okay/right/da/aha (HR+EN).
3. Optional acoustic classifier later (LiveKit adaptive pattern [L4]).
4. Edge cooldown, per [L4]: in the first ~1 s of bot speech, any speech yields (it is likely a correction). Speech in the last ~1 s of bot speech counts as the user's next turn, not as overlap.
5. False interruption: if a yield produces no transcript within 2 s, resume from the last unspoken sentence [L2][L3].
Backchannels are never logged as "engagement" signals. This is consistent with the presence harness ban on engagement metrics.

## 5. Cancel tokens
- One `TurnEpoch` (monotonic u64) per conversation turn. Every LLM stream, TTS job, tool call and audio chunk carries `epoch` + `reply_id`.
- Supersede = `epoch += 1`, then broadcast `Cancel{epoch}` at priority. Consumers drop any frame where `frame.epoch < current`. This is a stale-drop that does not depend on cancel delivery, and it mirrors Pipecat's discard of queued frames [P3].
- Mark tool calls with side effects `defer_until_commit` (cf. Deepgram `defer_until_eot` [D5]). Mark idempotent reads `cancel_on_interrupt` [P3].
- Track `spoken_cursor` (the last played word/sentence, from TTS timestamps) so context only records what the caller actually heard (Pipecat's context-truncation behavior [P3]).

## 6. Recommended design — Galion PhoneLane
States: `LISTENING → SPECULATING → COMMITTED/SPEAKING → (DUCKED) → YIELDED → LISTENING`.
1. **Ingress:** 8 kHz μ-law → Silero VAD (256-sample frames, threshold 0.5, min_silence 200 ms, pad 100 ms) runs in parallel with streaming STT. Prefer Flux with `eager_eot_threshold=0.5`, `eot_threshold=0.75`, `eot_timeout_ms=3000`. Open-source fallback: Silero + Smart Turn v3 ONNX [P1].
2. **Speculation:** on Eager/Smart-Turn "likely complete", start the LLM under a new epoch while holding the TTS output gate (Deepgram's gating model [D5]). Pre-synthesize the first sentence only if the TurnResumed rate is under 20%. On `TurnResumed`, cancel and bump the epoch.
3. **Commit:** on EndOfTurn, open the gate. Flush the first audio frame within 20 ms.
4. **Barge-in:** VAD onset while SPEAKING runs the §4 ladder. The duck happens locally in the media worker (≤ 1 frame). On yield, the media worker clears the playout queue, sends a telephony clear (e.g. Twilio Media Streams `clear` / the SIP equivalent), and emits `Cancel{epoch}` on the V2-S1 process bus at priority.
5. **Bus (V2-S1):** add two lanes, `control` (priority: Cancel, Duck, Commit) and `data` (audio/text). Make control preempt data. No heartbeat coupling.
6. **Metrics (presence-safe):** only latency/quality metrics: `bargein_stop_ms`, `eot_ms`, `false_interrupt_rate`, `turn_resumed_rate`, `talk_over_ms`. No engagement metrics.
7. **Tests:** a scripted harness with 8 kHz recordings covering mid-sentence interrupts, backchannels, coughs/noise, and corrections in the first second. Assert stop ≤ 250 ms p95, zero cancels on lexicon backchannels, and a resume after false interrupts.

## Sources
- [D1] https://developers.deepgram.com/docs/flux/voice-agent-eager-eot
- [D2] https://developers.deepgram.com/docs/flux/state
- [D3] https://developers.deepgram.com/docs/flux/configuration
- [D4] https://developers.deepgram.com/docs/measuring-streaming-latency
- [D5] https://developers.deepgram.com/docs/voice-agent-speculative-replies
- [P1] https://docs.pipecat.ai/api-reference/server/utilities/turn-detection/smart-turn-overview
- [P3] https://docs.pipecat.ai/pipecat/fundamentals/interruptions
- [P4] https://docs.pipecat.ai/api-reference/server/utilities/turn-management/user-turn-strategies
- [L1] https://docs.livekit.io/agents/logic/turns/tuning/
- [L2] https://docs.livekit.io/reference/agents/turn-handling-options/
- [L3] https://docs.livekit.io/agents/logic/turns/
- [L4] https://docs.livekit.io/agents/logic/turns/adaptive-interruption-handling/
- [S1] https://deepwiki.com/snakers4/silero-vad/5.1-vaditerator-for-streaming
- [S2] https://github.com/snakers4/silero-vad/blob/master/src/silero_vad/utils_vad.py
- [S3] https://docs.switchboard.audio/extensions/silero-vad/

_Unverified/assumed: PSTN jitter-buffer size, the Twilio `clear` message (not fetched this run), and the 150/400 ms duck thresholds (design choices to tune with the harness)._

— Maciej Grajczyk
