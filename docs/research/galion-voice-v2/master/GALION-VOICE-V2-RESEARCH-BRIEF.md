> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is desk synthesis, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.
>
> Owner Maciej Grajczyk authorized this docs-only landing. Sentences below that say the tree is local-only, or "do not push", record the pre-landing state. Landing these documents does not ratify open decisions O1–O12 and does not authorize implementation.

# Galion Voice V2: Master Research Brief

**Owner:** Maciej Grajczyk · **Product:** Galion Voice · **Written:** 2026-11-03 (Europe/Zagreb, CET)
**Inputs:** ten domain briefs in `../domains/` (01–10) and `../eval/EVALUATION-V2-2026-11-03.md`.
**Method:** synthesis only. Nothing was re-researched, and no citation was re-fetched for this master. Domain claims are carried forward with their source tags, as `[Dnn]` or `[Dnn §x]`.
**Status:** local research tree only. **Do not push to GitHub** until Maciej approves.

---

## 0. Locks enforced here (these override any domain text)

| Lock | Rule in this brief | Domain text overridden |
|---|---|---|
| No GGML / whisper.cpp | Whisper-family engines are out of the product entirely, including faster-whisper on the live path. | **D10** recommends "whisper.cpp/faster-whisper int8 base/small" for the VPS and says AVX "helps ggml". **Rejected.** The D10 RTF numbers are only CPU-class context. |
| No in-tree ort / candle | No ONNX Runtime or candle linked into Galion crates. ONNX-based engines may only run as **separate processes**, and only once the owner confirms the sidecar scope (decision O1). | **D04 §6** suggests "Piper as a persistent in-process model (Python `PiperVoice`)". **Rejected.** Piper stays out-of-process. |
| Piper A21 out-of-process OK | Piper (voice A21, `en_US-ljspeech-medium`) runs as a **persistent, pre-warmed worker process** over stdin/stdout IPC. flite is the last-resort fallback. | None. D02, D04 and D05 agree once D04 is corrected. |
| ASR letter A = Vosk | Vosk via the `vosk` Rust crate, run as an out-of-process sidecar on the V2-S1 bus. Default flag `GALION_ASR=vosk` after the pass bar. | **D03 §6** prefers Deepgram Flux (cloud) for ingress. **Downgraded** to a design reference and optional eval yardstick, never the product path. |
| `paid_tiers=false` | Local/Plus/Ultra are **deployment/memory profiles**, never priced product tiers. No SKUs or prices in product docs. | D10 hosting prices are **infra cost notes for the owner only**. Keep them out of README and product docs. |
| `chrome_allowed=false` | No extra UI chrome, menus, emotion meters or mood badges. The one-dot orb plus listening/thinking/speaking and consent state is all the user sees. | None. D06 §2.9 already complies. |
| Consent deny-default | Retention, training, emotion inference and speaker ID are all **off** on a fresh install. Each opt-in is separate, revocable and logged without audio. | None. D06 and D07 agree. |
| Presence harness | No engagement metrics (session length, retention, turns/session, DAU). | None. D03, D06 and D08 agree. |
| Naming | "Galion" everywhere, with Maciej Grajczyk as owner and sign-off. No studio or company branding in product docs. | None. |

---

## 1. Executive summary

1. **Galion's biggest gap is that it does not recognize speech.** V2-S4 sits at 5%: `GALION_ASR=interim` returns length buckets. Every naturalness, barge-in and latency gain depends on real streaming ASR. **Vosk (letter A) is the lowest-risk path that satisfies the locks.** It is streaming, emits partials, endpoints, runs on CPU, takes 16 kHz input, is Apache-2.0, has no GGML, and has a Rust crate [D01 §1.2, D05].
2. **Latency is mostly endpointing and buffering, not model speed.** Hosted vendors that claim under 500 ms measure **1.3–1.9 s p50** on real PSTN calls [D08 §2, D09 §2]. A streaming cascade where stages overlap gets total latency toward max(stage) instead of sum(stage) [D04 §1]. Galion can beat the market median on a CPU box if it owns the end-of-utterance (EOU) decision, starts the LLM speculatively, and flushes a short first clause to a warm Piper.
3. **Barge-in should be an out-of-band, priority cancel with epochs,** not something the heartbeat polls. Supersede-cancel already exists. The missing pieces are flushing the playout buffer and the telephony `clear`, a priority control lane on the V2-S1 bus, and a backchannel ladder so "mhm" never cancels [D03 §1, §5, §6].
4. **Timing matters more than voice quality for naturalness.** Human turn gaps average about 200 ms [D06 §1.1]. Keep Piper; make it persistent and clause-streamed; map dialogue acts to prosody parameters. Kokoro-82M is an optional later HQ voice sidecar [D02].
5. **Security has two must-fix items before anything is exposed further:** CORS `*` and missing WebSocket Origin checks (CSWSH), plus an unauthenticated ngrok tunnel. Fixing R1, R2 and R4 lifts V2-S3 from 68% to about 85% [D07 §3]. The EU AI Act Art. 50 AI disclosure applies from 2 Aug 2026, so it applies **now** [D07 §1].
6. **Measure from recorded audio, not from internal timestamps.** Adopt a tiered "presence-eval" suite (T0–T4) with an external PSTN probe. Without instrumentation none of the latency claims can be verified [D04 §8.1, D08 §4].
7. **Galion's honest position is the only local-first, MIT, branded voice-first product** among the players surveyed. Frameworks (Pipecat, LiveKit) are toolkits, and the hosted players are closed and cloud-bound [D09 §3]. Never publish unmeasured latency numbers.

**Build order (consistent with the eval §5):** V2-S1 bus (control and data lanes, epochs) → V2-S4 letter A Vosk harness → warm Piper and clause streaming (V2-S6) → instrumentation and presence-eval (V2-S11/S7) → security R1/R2/R4 (V2-S3) → speculation and partial commit → optional upgrades (B/C ASR, Kokoro, outbound backchannels).

---

## 2. Recommended stack (V2.0)

| Layer | V2.0 choice | Process model | License | Later or optional | Rejected |
|---|---|---|---|---|---|
| Telephony ingress | Twilio Media Streams, 8 kHz μ-law → decode → resample to 16 kHz s16le in-process (rubato or speexdsp) [D05, D04 §5] | in-tree Rust | Apache/MIT, BSD | speexdsp AEC for barge-in echo [D05] | ffmpeg spawned per turn [D04 §5] |
| VAD | **libfvad (WebRTC VAD)**: tiny C library, no ML runtime [D05] | in-tree (C FFI) | BSD-3 | Silero VAD (32 ms frames, under 1 ms CPU [D03]) as an **ONNX sidecar only if O1 = yes** | — |
| ASR | **Vosk, letter A** via the `vosk` crate. Models `small-en-us-0.15` (~40 MB) and `en-us-0.22-lgraph` (~128 MB), SHA256-pinned [D01 §4] | **out-of-process sidecar** (`galion-asr-vosk`, NDJSON) so a libvosk crash cannot take down PhoneLane | Apache-2.0 | A′ `utter` (pure-Rust Vosk decoder, 0.0.x, benchmark only); B/C Moonshine Small/Medium or sherpa-onnx Zipformer, **ONNX sidecar only if O1 = yes** [D01 §3] | whisper.cpp/GGML (lock); faster-whisper and whisper_streaming (~3.3 s latency, 30 s window); Web Speech API (server-based by default, browser-only); Kyutai STT (GPU, watch only); Parakeet/NeMo (offline WER oracle only) [D01] |
| Turn detection | Galion owns EOU in TurnWorker: VAD silence + **stable-partial agreement** + per-dialogue-state silence knob (longer while the caller dictates numbers) [D01 §2, D04 §3] | in-tree | — | Smart Turn v3 (open, CPU, ONNX) as a semantic turn model **sidecar if O1 = yes** [D03, D09] | LiveKit turn-detector in core (LiveKit Model License, not MIT) [D09 §5] |
| Dialogue / barge-in | `TurnEpoch` + `reply_id` on every frame; stale-drop; `Cancel{epoch}` on a priority control lane; backchannel ladder; false-interrupt resume [D03 §4–6] | in-tree (V2-S1 bus) | — | Acoustic backchannel classifier; VAP-timed outbound backchannels behind a flag [D06] | Cancel coupled to the 80 ms heartbeat [D03 §3] |
| Reply / LLM | **CannedLlm** (intent router) as the deterministic and offline path; MockLlm echo removed from the phone default [eval §4] | in-tree | — | OpenAI-compatible HTTP endpoint (e.g. vLLM) behind a trait, out-of-process [D05]; `.gq` student (V2-S9) | Anything GGML-based (llama.cpp) [D05] |
| TTS | **Piper A21 `en_US-ljspeech-medium`**, persistent worker (`--json-input --output-raw`), pre-warmed at boot, clause-chunked; **flite** fallback [D02, D04 §5] | **out-of-process** (also isolates espeak-ng GPL-3) | Piper MIT (rhasspy, pinned) or GPL-3 (piper1-gpl); espeak-ng GPL-3 | Kokoro-82M HQ voice sidecar after MOS A/B [D02]; experimental Piper ONNX chunked decode for long first clauses [D04 §5] | XTTS-v2 (CPML non-commercial), StyleTTS2 in prod, espeak-ng as a voice [D02] |
| Prosody | Dialogue act → `length_scale` / `noise_scale` / `noise_w` / `sentence_silence`; 160–180 wpm, 10–15% slower for numbers and confirmations; 150–250 ms pause before key information; **0 fillers by default** [D02, D06 §2.7–2.8] | in-tree mapper | — | Utterance-initial filler (max 1 per turn) for a social persona only | Emotion inference without consent [D06] |
| Orb / render | JS spring orb driven by TTS RMS/viseme events; CPU `galion-render` baseline [D05] | client | in-house | wgpu GPU path behind a feature flag [D05] | Server video, GStreamer/VAAPI (skip for V2) [D10] |
| Edge | Loopback app (`127.0.0.1`), **one** public edge (remove the duplicate cloudflared/ngrok), ngrok with auth for dev → Caddy with ACME TLS on a VPS later [D07, eval §3] | — | — | Regional host colocated with the Twilio region (IE1 for EU) [D04 §6] | Two simultaneous tunnels; ngrok in production [D04, D10] |
| Hosting | Stay on the current CPU VPS and measure steal time. Move to bare metal when p95 jitter is steal-driven or there are more than 2–3 concurrent sessions [D10] | — | — | NPU/edge boxes (RK3588 etc.) as an R&D spike behind the same ASR/TTS traits [D10] | Pricing or SKUs in product docs (`paid_tiers=false`) |

**Interface rule:** `trait Asr { fn feed(&mut self, pcm:&[i16]) -> Vec<AsrEvent>; }` and `trait TtsBackend` (stdin JSON / stdout PCM). The ASR event shape copies the industry pattern, not any vendor: `Partial{text, stable_prefix, t_ms}`, `Final`, `EagerEndOfTurn`, `EndOfTurn`, `TurnResumed` [D01 §1.9]. Downstream logic (intent, barge-in, eager LLM) only consumes **stable prefixes** [D01 §2].

---

## 3. Latency targets (reconciled)

The domain briefs used different start and end points. This brief fixes three distinct measurements and never mixes them.

- **Pipeline:** user stopped speaking (server VAD) → first media frame sent.
- **Mouth-to-ear (M2E):** external probe, from the caller's speech end to the first non-silent inbound audio, over PSTN.
- **Web:** the same as M2E, over a WebSocket or browser transport.

| Metric | p50 | p95 | Basis / reconciliation |
|---|---|---|---|
| Pipeline EOU → first frame | ≤ 650 ms | ≤ 1,000 ms | Sum of the D04 §2 stage budgets minus the speculation saving |
| **PSTN M2E (TTFA)** | **≤ 900 ms** (stretch ≤ 800) | **≤ 1,300 ms** | D04 §1. Stricter than the D08 T2 draft (1,000/1,500). Beats the 1.3–1.9 s market band [D08 §2] and sits under Twilio's 1,400 ms upper limit [D04 §1]. Use D08's 1,000/1,500 as the **initial CI gate**, then tighten to 900/1,300 after V1 baseline calibration. |
| Web M2E (TTFA) | ≤ 800 ms | ≤ 1,200 ms | D03 §3 and D08 T1 agree |
| Naturalness stretch (gap) | ≤ 500 ms | — | D06 §2.1. Aspirational; humans average about 208 ms |
| TTFCW (first *content* word) | report separately | — | Fillers must not game TTFA [D08 §3] |
| **Barge-in stop** (onset → bot silent) | — | **≤ 250 ms** (design); CI gate ≤ 300 ms with ≥ 95% success | D03 design target vs D08 gate; same intent, the gate gives margin |
| ASR first partial | ≤ 200 ms after word start | — | D01 §2 (partials every 100–200 ms) |
| ASR final after endpoint | ≤ 300 ms | ≤ 600 ms | D01 §4 letter-A pass bar |
| ASR RTF | < 0.3 on one core | — | D01 §4 |
| LLM TTFT to first clause | ≤ 350 ms | — | D04 §2 |
| TTS first PCM (warm Piper, short first clause) | ≤ 150 ms | — | D02 and D04 agree |
| Network + playout | ≤ 150 ms | — | 50–150 ms buffer; never pre-buffer 500 ms [D04 §6] |
| Canned ack fallback trigger | if no LLM token by ~300–600 ms | — | D06 says 300 ms, D04 says 600 ms. **Start at 500 ms and tune in the harness.** The ack must be content-neutral and contain no emotion words. |
| False-interrupt resume | 1.5–2.0 s with no transcript → resume from the last unspoken sentence | — | D03 |

**Heartbeat reconciliation:** D04 budgets EOU as "2–3 ticks of 80 ms", while D03 says the heartbeat must never be on the cancel path. **Resolution:** EOU, Cancel, Duck and Commit are **events** pushed on the V2-S1 control lane. The 80 ms heartbeat is only for liveness and reconciliation and must never stall (eval §5.1). Audio output is event-driven and never tick-aligned (D04 §6 saves up to 80 ms). "2 ticks ≈ 160 ms" stays as a *silence duration* parameter, not a scheduling dependency.

**Top latency levers (expected saving, per D04 §8):** partial commit and speculative LLM start (150–300 ms) · first-clause flush to Piper at punctuation or ~8 words (100–400 ms) · warm Piper (removes 0.3–1 s cold spikes) · off-tick audio push (≤ 80 ms) · regional host instead of ngrok (unmeasured) · RAG and guardrails off the critical path (300–800 ms avoided).

---

## 4. Security and privacy musts

These must be in place before the edge is widened or a VPS goes live. Gate tests come from D07 §3.

| # | Must | Gate |
|---|---|---|
| M1 | **CORS exact allow-list** (loopback, the single tunnel host, later the Caddy host). No wildcard with credentials. | A foreign Origin gets 403 |
| M2 | **WebSocket Origin check** plus a short-lived per-session token in the first WS message (not in the URL) | CSWSH proof-of-concept fails |
| M3 | **Loopback bind only.** Refuse `0.0.0.0` unless an env flag is set and auth is on. | Startup assertion |
| M4 | **ngrok auth** (OAuth restricted to the owner email, or Basic Auth plus IP restriction), inspector off, reserved domain, kill when idle. **One** public edge only. | Unauthenticated curl gets 401 or a redirect |
| M5 | **Ephemeral turns:** PCM in RAM only, zeroised at turn end or barge-in; no PCM or transcripts in logs (lengths and latency only); tmpfs if files are needed; `RLIMIT_CORE=0` | grep test on logs |
| M6 | **Consent deny-default:** retention, training, emotion and speaker ID all off. Each opt-in separate, revocable, and logged as timestamp + version with no audio. Mic only on explicit user action, with a visible indicator. | A fresh-install config test shows everything off |
| M7 | **AI disclosure** on the first turn, audible plus visible (HR/EN), per AI Act Art. 50(1), which applies from 2 Aug 2026 | Acceptance test |
| M8 | **No voice cloning or voiceprints in V2.** Licensed stock voices only, with a voice-licence manifest (check the A21 MODEL_CARD; some Piper voices are CC-BY-NC). Evaluate TTS watermarking before 2 Dec 2026. | Manifest present |
| M9 | **Emotion inference:** none by default. Acoustic signals (loudness, rate, overlap) may be used for turn and volume control only and are never labeled as emotion. Opt-in coarse inference is hard-blocked in workplace and education deployments (AI Act Art. 5(1)(f)). | Config and deployment-type test |
| M10 | **Transcripts are untrusted input.** Tools are allow-listed; side-effecting tools are `defer_until_commit` and need confirmation; idempotent reads are `cancel_on_interrupt` | Red-team prompts |
| M11 | **GDPR paperwork:** RoPA, HR+EN privacy notice, DPIA (likely needed), processor list with SCC/TRA for ngrok and any non-EU processor, and a 72 h breach runbook for AZOP | Docs present |
| M12 | **GPL isolation:** espeak-ng and piper1-gpl ship only as separate executables, with a source offer, listed in the licence file | Licence file |
| M13 | Caddy with ACME TLS, HSTS and the existing CSP once SSH/VPS access unblocks | SSL Labs A, WS through the proxy |

Expected re-score (D07): M1+M2+M4 brings V2-S3 from 68% to ~85%; M13 brings it to ~95%.

---

## 5. Evaluation suite outline: "presence-eval"

Canonical metrics (D08 §3): TTFA (p50/p95/p99), TTFCW, barge-in stop time and success rate, false barge-in rate, spurious response rate, takeover rate on mid-utterance pauses (TOR), WER sliced by accent, noise and codec, UTMOSv2 plus human MOS/CMOS, and deterministic task-fact checks. **Excluded:** session duration, turns per session, return rate, any engagement score, and emotion-label telemetry.

| Tier | When | Contents | Draft gate (calibrate on the V1 baseline first) |
|---|---|---|---|
| T0 unit | Every PR | voicebench mock transport, virtual clock, bus/epoch stale-drop tests | Schema and logic regressions |
| T1 scenario | Every PR | ~50 YAML scenarios: clean Q&A, mid-sentence barge-in, cough, backchannel ("mhm", HR "da/aha"), correction in the first second, pause, noisy café, 8 kHz codec | Web TTFA ≤ 800/1,200; barge-in p95 ≤ 300 ms with ≥ 95% success; false barge-in ≤ 5%; **zero cancels on lexicon backchannels**; TOR ≤ 10%; WER clean ≤ 6% / noisy ≤ 12% (**applies to B/C engines; letter A uses the §5.1 bar**) |
| T2 phone | Nightly | External PSTN probe with dual-channel recordings (openbenchmarks method), ≥ 200 turns (D04 suggests 20 calls per night); first turn reported separately | PSTN TTFA 1,000/1,500 initially → 900/1,300 target |
| T3 naturalness | Nightly / per release | UTMOSv2 on a fixed script plus post-barge-in re-entry MOS; Piper vs Kokoro A/B | UTMOSv2 ≥ 4.0; drop after barge-in ≤ 0.1 (validate UTMOS against humans; it is English-trained) |
| T4 human | Per release | 20+ raters, MOS/CMOS against the previous release, turn-taking naturalness; outbound-backchannel flag A/B | No significant regression (p < 0.05) |

**5.1 Letter-A Vosk experiment (first eval to run, from D01 §4).** Use a `galion-asr-vosk` bin with 20 ms frames (320 samples), NDJSON partial/final output, `set_words(true)`, and the A′ `utter` backend behind the same trait.
- **Test set:** ~50 consented Galion phone utterances (8 kHz upsampled to 16 kHz, plus native 16 kHz), 50 LibriSpeech test-clean clips, and 20 intent phrases.
- **Pass bar:**
  - final after endpoint ≤ 300 ms p50 and ≤ 600 ms p95;
  - RTF < 0.3;
  - phone WER ≤ 20% for the small model or ≤ 12% for lgraph;
  - grammar-mode intent accuracy ≥ 95%.
- **Report:** partial stability.
- **On pass:** flip the default `interim` → `vosk`, rename `interim` to `placeholder`, and sunset it.
- **On WER fail:** rerun the same harness on B/C, subject to O1.
- **Output:** `eval/asr-letterA-<date>.md`.

**Receipts:** every run records the model, ASR, TTS, endpointing/VAD config, transport and commit hash. Measure from recordings; never trust platform timestamps; disarm fillers [D08 §2].

**Per-turn stamps (D04 §7):** `t_vad_silence_start, t_eou_decided, t_asr_final, t_llm_req, t_llm_first_token, t_first_clause_flushed, t_tts_first_pcm, t_first_media_frame_sent, t_first_audio_heard (probe)`, all under one `turn_id`.

---

## 6. Sector mapping to V2-S1..S13

Baseline percentages are from `EVALUATION-V2-2026-11-03.md`.

| Sector | Now | What the research says to do | Domains |
|---|---:|---|---|
| **V2-S1 Process bus** | 12% | Typed bus with a **control lane** (Cancel/Duck/Commit/EOU, priority, preempts) and a **data lane** (audio/text). Every frame carries `TurnEpoch`, `reply_id` and `spoken_cursor`. Stale-drop. ASR and TTS sidecars attach over the bus. **Build first.** | 03, 04, 01, 02 |
| **V2-S2 Orb / graphic engine** | 55% | Keep the spring orb; drive it from TTS RMS/viseme. Move it into `galion-render` (CPU baseline; wgpu behind a flag). No emotion visuals (`chrome_allowed=false`). | 05, 06, 10 |
| **V2-S3 Secure edge** | 68% | M1–M4 now (~85%), M13 when the VPS unblocks (~95%). One public edge. | 07, 10 |
| **V2-S4 Real EN ASR** | 5% | Letter A Vosk sidecar plus the §5.1 experiment; A′ `utter` side-by-side; B/C only after O1. | 01, 05 |
| **V2-S5 Reply intelligence** | 28% | CannedLlm stays the deterministic floor; remove the MockLlm echo from phone; speculative LLM start on stable partials; trait for an out-of-process OpenAI-compatible endpoint; RAG and tools off the critical path. | 03, 04, 05 |
| **V2-S6 Speech runtime** | 42% | Persistent pre-warmed Piper A21 worker, clause chunker (punctuation or ~8 words), in-process μ-law resampling, flush the queue (don't kill the process) on barge-in, prosody mapper, phrase cache, flite fallback. Kokoro later. | 02, 04, 06 |
| **V2-S7 CI/CD track** | 62% | Wire presence-eval T0/T1 into PR gates and T2/T3 nightly; TTFA p50/p95 in CI; licence-manifest check. Auto-deploy is still blocked on SSH. | 08, 04 |
| **V2-S8 Branding & docs** | 78% | Refresh the stale local README (it still describes the StubAsr/MockLlm era). Never call `interim` "recognition". No tier pricing. Publish only measured latency. | 09, eval |
| **V2-S9 Student / Open Mind** | 10% | Out of scope for this research. Any student model must avoid GGML and in-tree ort/candle, and must sit behind the reply trait. | 05 |
| **V2-S10 Gates R0–R3** | 18% | Map the gates to presence-eval tiers and the security musts (e.g. R1 ≈ T1 + M1–M4, R2 ≈ T2 nightly green, R3 ≈ M13 + T4). Still blocked on SSH. | 07, 08 |
| **V2-S11 Presence** | 38% | Spec phone barge-in: backchannel ladder (<150 ms ignore, 150–400 ms duck −12 dB, >400 ms or ≥2 non-lexicon words yield), edge cooldowns, false-interrupt resume, presence-safe metrics only. | 03, 06, 08 |
| **V2-S12 Consent / Rika on bus** | 32% | Put consent state into the bus envelope. Rika lean-in means **pacing only** (faster pickup, shorter first sentence, follow-up question) with no warmth escalation. Counterbalancing: mirror at most one step, then return to neutral. Deny-default (M6, M9). | 06, 07 |
| **V2-S13 Multilingual hold** | 95% | Keep the EN-only hold. When it lifts: per-locale gap calibration (±250 ms between languages), HR backchannel lexicon, HR disclosure line, Vosk HR model availability check. | 06, 03, 07 |

---

## 7. Open owner decisions (Maciej Grajczyk)

| # | Decision | Why it matters | Recommended default |
|---|---|---|---|
| **O1** | Does "no in-tree ort" allow ONNX Runtime **inside a separate sidecar process** (as Piper already does internally)? | Decides Silero VAD, Smart Turn v3, Moonshine, sherpa-onnx Zipformer and Kokoro. | Allow it for out-of-process sidecars only, matching the Piper A21 precedent. Nothing in-tree. |
| O2 | Piper build: pin MIT `rhasspy/piper` (archived) or move to GPL-3 `OHF-Voice/piper1-gpl`? | Licence posture. Both are fine out-of-process. | Pin the MIT release now; re-evaluate on security fixes. |
| O3 | Audit the A21 voice MODEL_CARD licence | Some Piper voices are CC-BY-NC or research-only. ljspeech is public domain. | Confirm and record it in the voice manifest. |
| O4 | Vosk model choice: small-en (~40 MB) vs en-us-0.22-lgraph (~128 MB) default | Accuracy vs RAM/CPU trade-off | Decide from §5.1 results. Grammar mode for intents either way. |
| O5 | Is cloud ASR (Deepgram/AssemblyAI) allowed **in eval only** as a yardstick? | Local-first purity; audio transfer outside the EU (AZOP precedent) | Allowed only on synthetic or consented test audio, with a DPA. Never on the product path. |
| O6 | Outbound backchannels and fillers: ship off in V2.0? | Bad timing sounds like interruption | Off by default; flag plus T4 A/B later. |
| O7 | Hosting provider for `51.38.137.193`: OVH or Hetzner? Trigger to move to bare metal? | D10 flags 51.38.0.0/16 as an OVH range, so the Hetzner pricing may not apply | Verify the provider and measure steal time first. |
| O8 | Production edge: Caddy on VPS vs Cloudflare Tunnel; which single tunnel to keep now | The double edge is redundant risk; transfer and DPA implications | Keep ngrok with OAuth for now and remove cloudflared. Caddy when SSH unblocks. |
| O9 | AI Act Art. 50(2) watermark: adopt an AudioSeal-class mark before 2 Dec 2026? | Compliance window; robustness on 8 kHz is unknown | Run an eval spike; decide by mid-Nov. |
| O10 | DPIA and legal review (AZOP list, call-centre emotion carve-out) | Legal certainty | Commission a review before any opt-in emotion feature. |
| O11 | Approve pushing this research tree to GitHub | It is currently local only | **Do not push** until approved. |
| O12 | Optional OpenAI-Realtime-compatible event shim at the edge | Client compatibility vs scope | Defer past V2.0. |

---

## 8. Contradictions resolved and caveats

### 8.1 Domain contradictions resolved here
- **D10 vs locks:** D10's "use whisper.cpp/faster-whisper int8" recommendation is void. Only its CPU-class RTF context and hosting comparison are kept. Treat D10's Kokoro-on-NPU number (~800 ms TTFA) as NPU-specific. It does not contradict D02's CPU estimate, but it does argue against Kokoro on shared vCPU.
- **D04 vs locks:** in-process Python `PiperVoice` is replaced by a persistent out-of-process Piper worker. The latency rationale (warm, preloaded, no per-turn spawn) still holds.
- **D03 Flux-first ingress vs local-first:** Galion's own Vosk + VAD + stable-partial EOU replaces it. Flux's Eager/EndOfTurn/TurnResumed *pattern* is adopted.
- **VAD choice:** D03 assumes Silero; D05 says adopt libfvad and trial Silero. **libfvad for V2.0**; Silero after O1.
- **Sample rates:** D03 runs VAD at 8 kHz and D01 feeds 16 kHz. Ingress is 8 kHz μ-law from telephony → VAD can run at 8 kHz (libfvad supports it) → resample to 16 kHz for Vosk.
- **Piper licence:** D05 says pin MIT rhasspy; D02 notes the GPL-3 successor. Both are compatible out-of-process (O2).
- **Latency numbers:** three measurement points are defined (§3). Design targets (D03/D04) and initial CI gates (D08) are kept separate.
- **Ack timing:** 300 ms (D06) vs 600 ms (D04) → start at 500 ms and tune.

### 8.2 Citation and verification caveats
No citation was re-verified for this master. The domain briefs vary in rigor:
- **D01:** the Vosk model sizes and WERs are "as best recalled, not re-fetched". The `utter` crate (0.0.x, 2026) and its accuracy figures rest on one docs.rs page. Moonshine, Deepgram and AssemblyAI latencies are vendor-reported on different hardware and with different definitions. The Coval/Flux result is vendor-published.
- **D02:** the engine-table speeds (Kokoro "~RT to 3–5× RT", Piper TTFA ≤150 ms) are estimates without per-claim citations.
- **D03:** the PSTN jitter-buffer size, the Twilio `clear` message (not fetched) and the 150/400 ms duck thresholds are design assumptions.
- **D04:** relies heavily on blogs (AWS Builder, Bearworks, rtcanary, Mahimai) and GitHub issues. Piper RTF figures come from user reports.
- **D05:** all URLs are "canonical repo URLs written from domain knowledge, not re-fetched". Verify the licence files before locking.
- **D06:** mostly peer-reviewed sources. S13 (prosody uncanny valley) is a master's thesis. The call-centre carve-out (S17) is a regulator consultation summary, not law.
- **D07:** no legal review. The DPIA need is assumed. ngrok data location and inspector retention are unverified. The AZOP fine figures should be rechecked before citing externally.
- **D08:** tool versions (voicebench v0.2.0, voice-evals v0.2.1) and the 2026-08-01 openbenchmarks leaderboard figures were not re-checked. All gates are first-draft.
- **D09:** several sources are vendor-authored or self-ranking (Retell blog [4], Sprag [7]), and independent rankings contradict each other (Retell first in one, last in another). Product and version names (e.g. "GPT Realtime 2.1") are unverified.
- **D10:** the ngrok guidance is uncited. Hetzner prices and stock-limited SKUs change often. The VPS provider is uncertain (OVH vs Hetzner).
- **Eval file:** the completion percentages are judgment-weighted, not measured.

**Rule:** no number in this brief goes into public product docs until a Galion-owned harness run (§5) reproduces it.

---

## 9. Top recommendations (ranked)

1. **Build the V2-S1 bus with a priority control lane and `TurnEpoch` stale-drop.** It unblocks the ASR sidecar, barge-in and speculation.
2. **Ship letter A Vosk as an out-of-process sidecar,** run the §5.1 experiment, and retire `interim` once it passes.
3. **Make Piper A21 a persistent, pre-warmed, clause-streamed worker with flite fallback,** with event-driven audio push (never tick-aligned).
4. **Instrument first:** per-turn stamps, an external PSTN probe and presence-eval T0–T2. Targets: PSTN TTFA ≤ 900/1,300 ms and barge-in ≤ 250 ms p95.
5. **Close the security gaps:** CORS allow-list, WS Origin check plus session token, ngrok OAuth with a single edge, ephemeral RAM turns, first-turn AI disclosure, and consent deny-default verified by test.

*Sign-off pending: Maciej Grajczyk.*
