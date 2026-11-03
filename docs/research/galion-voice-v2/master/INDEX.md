> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is desk synthesis, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.
>
> Owner Maciej Grajczyk authorized this docs-only landing. Sentences below that say the tree is local-only, or "do not push", record the pre-landing state. Landing these documents does not ratify open decisions O1–O12 and does not authorize implementation.

# Galion Voice V2: Research Index

Master brief: [GALION-VOICE-V2-RESEARCH-BRIEF.md](GALION-VOICE-V2-RESEARCH-BRIEF.md) · 2026-11-03 (Europe/Zagreb) · local only, not pushed.

| # | Domain file | One-line finding | Lock note in master |
|---|---|---|---|
| 01 | [domains/01-asr-stt.md](../domains/01-asr-stt.md) | Streaming latency and partial stability matter more than raw WER. Vosk (letter A) via the `vosk` crate as an out-of-process sidecar is the lowest-risk lock-compliant path; Moonshine and sherpa-onnx Zipformer are B/C candidates. | Web Speech and Whisper rejected; B/C need O1 |
| 02 | [domains/02-tts-prosody.md](../domains/02-tts-prosody.md) | Keep Piper out-of-process (also isolates espeak-ng GPL-3), make it persistent and clause-streamed with prosody parameter mapping; Kokoro-82M is an optional HQ sidecar; reject XTTS (CPML). | Consistent |
| 03 | [domains/03-dialogue-bargein.md](../domains/03-dialogue-bargein.md) | Speculate/commit turns with epoch cancel tokens on a priority control lane; backchannel ladder (ignore/duck/yield); barge-in stop ≤ 250 ms p95; heartbeat off the cancel path. | Flux-first ingress downgraded to a reference |
| 04 | [domains/04-latency-streaming.md](../domains/04-latency-streaming.md) | Overlapping stream stages; PSTN mouth-to-ear target p50 ≤ 900 / p95 ≤ 1,300 ms; instrument first; partial commit, first-clause flush and off-tick audio push are the biggest levers. | In-process PiperVoice overridden → out-of-process |
| 05 | [domains/05-oss-vs-v1.md](../domains/05-oss-vs-v1.md) | Replace interim ASR with Vosk, adopt libfvad VAD, keep CannedLlm, Piper A21, flite, the orb and the CPU render; ONNX tools only as sidecars pending the owner's lock ruling. | Raises O1 and O2 (URLs not re-fetched) |
| 06 | [domains/06-naturalness.md](../domains/06-naturalness.md) | Timing beats voice quality (human gap ~200 ms); inbound backchannels never take the turn; outbound backchannels and fillers off by default; emotion inference deny-default per AI Act Art. 5(1)(f). | Consistent with chrome_allowed=false |
| 07 | [domains/07-security-privacy.md](../domains/07-security-privacy.md) | Fix CORS `*`, WS Origin/CSWSH and the unauthenticated ngrok tunnel (V2-S3 68% → ~85%); ephemeral RAM turns; AI disclosure (Art. 50) now; no voiceprints or cloning; GDPR/AZOP paperwork. | Consistent |
| 08 | [domains/08-evaluation.md](../domains/08-evaluation.md) | Tiered "presence-eval" (T0–T4) measured from recorded audio; market phone TTFA band 1.3–1.9 s p50; engagement metrics excluded; gates are drafts to calibrate on V1. | Gates reconciled with D03/D04 |
| 09 | [domains/09-competitive.md](../domains/09-competitive.md) | Vendor sub-500 ms claims measure 1.0–1.9 s on real calls; endpointing is the hidden latency; Galion is the only local-first, MIT, voice-first product in the set. | Some sources vendor-authored |
| 10 | [domains/10-hardware-edge.md](../domains/10-hardware-edge.md) | Stay on the CPU VPS and measure steal time; move to bare metal on jitter or >2–3 sessions; NPU is R&D only; skip GStreamer/VAAPI; profiles are not tiers. | whisper.cpp/faster-whisper advice **void** (GGML lock); provider OVH vs Hetzner unverified |
| — | [eval/EVALUATION-V2-2026-11-03.md](../eval/EVALUATION-V2-2026-11-03.md) | V2 program at 36%; V2-S4 ASR at 5% and V2-S1 bus at 12% are the critical gaps; next build is the S1 bus plus letter A prep. | Source of the sector baselines |

Seller snapshot, same date: [SELLER-EVAL-V5-2026-11-03.md](../eval/SELLER-EVAL-V5-2026-11-03.md). Domain rows 01–10 are the source briefs. LOCKS.toml overrides any conflicting text in them.

— Maciej Grajczyk
