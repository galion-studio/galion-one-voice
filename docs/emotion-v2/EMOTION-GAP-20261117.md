# Samantha emotion gap: scorer vs voice (17 Nov 2026)

Galion Voice — Perpetual Open MIT License. Europe/Zagreb.


Tue 17 Nov 2026, 18:20–18:45 CET. Read-only. Sources: existing score JSON and reports in galion-one-voice main `56312f7`
mainly
`tools/cloning-v2/scorers/emotion/results/baseline_{samantha,jarvis}.json`,
`tools/cloning-v2/emotion-blend/analysis/FINDINGS.md`,
`tools/cloning-v2/scorers/universal/results/{samantha,jarvis}-universal/RESULTS.json` and `lines_*.json`,
`tools/cloning-v2/scorer-probe/results-persona/COMPARISON.md`, `tools/cloning-v2/merge-v2/REPORT.md`,
`docs/emotion-v2/EMOTION-V2-FOUNDATION-SPEC.md` §7.
No audio was opened, copied or moved. Nothing was trained. All recomputations below use only logged class posteriors and logged per-emotion scores (python arithmetic on the JSON).
The per-line log behind the exact 0.491/0.461 laptop figures was **not found**, so §1 reconstructs the scorer behaviour from the same scorer's logged posteriors on the 12 own-clone-analysis lines.

## Short answer
- **Most of the gap is the scorer and its single reference, not the voice.** On the 4-class SUPERB gate path, Samantha's *own real recording* (7 windows of the original) scores **lower** against her single reference than her clones do: category_blend **0.424 (original voice) vs 0.461 (clones)**.
- That path has a **hard mathematical cap of 0.778** for any render with P(sad)=0, even with a perfect embedding (0.70 × (1 − 0.318) + 0.30). The gate is 0.83. It cannot be passed by a normal-sounding render.
- The two scorers disagree on the **same merge-v2 renders**: SUPERB says 7/12 lines are *angry*; emotion2vec+ says **0/12 angry** (10 neutral, 2 happy; max P(angry) 0.000).
- On the universal (emotion2vec+/CLAP/EfficientAT) scorer the voice-side gap is real but small and concentrated in **two emotions: love (0.307) and aggressive (0.556)**. Fixing just those two to the original-voice level gives **0.881** (passes 0.83).

## 1. 4-class SUPERB gate path (the 0.491 / 0.461 series)
Formula `0.70*category_blend + 0.30*emb_sim`, category_blend = histogram intersection of the 4-class posterior (neu/hap/ang/sad) vs ONE whole-file reference.

Samantha whole-file reference posterior (baseline_samantha.json): **neu 0.349, hap 0.214, ang 0.119, sad 0.318**. Jarvis reference: neu 0.848, hap 0.140, ang 0.009, sad 0.003.

| Comparison (logged posteriors) | category_blend | em @ emb 0.561* | em @ emb 1.0 |
|---|---:|---:|---:|
| A. Samantha ORIGINAL voice, 7 × 2.5 s windows vs whole-file ref | **0.424** | 0.465 | 0.596 |
| B. Samantha clones t01–t12 vs whole-file ref | **0.461** | 0.491 | 0.623 |
| C. Clones vs mean of her own windows (neu .557 hap .083 ang .347 sad .012) | 0.692 | 0.652 | 0.784 |
| D. Clones vs best-matching single original window | **0.832** | 0.751 | 0.883 |
| Jarvis clones vs his whole-file ref | 0.822 | — | — |

*emb_sim is not in these logs; 0.561 is the value implied by em 0.491 and cat 0.461. It is shown only to place rows on the same scale.

Per-class shortfall of the clones vs the Samantha reference (mean over 12 lines): **sad 0.307**, hap 0.149, neu 0.083, ang 0.000. So ~2/3 of the categorical loss is the reference's sad mass.
Why that sad mass is a scorer artefact: the whole recording gives P(sad)=0.318 but its own 2.5 s windows average **P(sad)=0.012** (FINDINGS.md). SUPERB's sad depends on clip length/context, not on the voice.
Cap: with P(sad)=0 in the render, max category_blend = 1 − 0.318 = 0.682 → **max emotion_match 0.778**. To reach 0.83 needs category_blend ≥ 0.757 even at emb 1.0.
Normalisation (0.491 → 0.461, −0.030): loudness/room changes cannot remove the sad term; it is a reference problem, not a room problem (spec §7.1 agrees).
4-class blind spot: SUPERB has only neu/hap/ang/sad, so 8 of 12 V2 sectors (Calm, Amused, Excited, Tender, Disappointed, Fearful, Surprised, Disgusted) have no class at all.
Angry lean: SUPERB tops for Samantha clones: ang 7, neu 3, hap 2 (baseline) and ang 7, neu 4, hap 1 (merge-v2). Her own original windows also top ang 3 of 7 (15.0–17.4 s window: ang 0.971). emotion2vec+ on the same merge-v2 files: angry 0/12.

## 2. Universal scorer (0.774 merge-v2 / 0.751 coherent)
Formula as RESULTS.json; category_blend = strength-weighted mean of per-emotion scores; targets come from her original recording.

| Emotion (backend, target strength) | ceiling (original vs itself) | merge-v2 | coherent | em lost (merge-v2) |
|---|---:|---:|---:|---:|
| neutral (emotion2vec+, 1.0) | 0.962 | 0.969 | 0.965 | 0.005 |
| laughter (EfficientAT, 0.274) | 0.917 | 0.854 | 0.925 | 0.007 |
| yelling (EfficientAT, 0.665) | 0.861 | 0.938 | 0.923 | 0.007 |
| logic (CLAP zero-shot, 0.500) | 0.852 | 0.925 | 0.897 | 0.006 |
| **aggressive** (emotion2vec+, 0.999) | 0.820 | **0.556** | 0.529 | **0.073** |
| **love** (CLAP zero-shot, 0.811) | 0.785 | **0.307** | 0.291 | **0.093** |
| emb_sim | 1.000 | 0.881 | 0.831 | 0.036 |
| **emotion_match** | **0.904** | **0.774** | **0.751** | gap to gate 0.056 |

Recomputed from the logged scores (matches the logged 0.7738 / 0.7509 exactly):
- love → ceiling only: **0.838** (pass). aggressive → ceiling only: 0.817. Both: **0.881**. Drop love from the persona: 0.843.
- Coherent: love → 0.817, both → 0.865.
- The love and aggressive targets are the **same 7.5–10.0 s window** of the original: emotion2vec+ calls it angry 0.999, CLAP calls it love 0.81. The scorer asks the clone to be both at once from one window.
- The newer love/seductive detector (COMPARISON.md) drops merge-v2 love to 0.061 and em to 0.687, so love depends heavily on which zero-shot prompt bank is used → a scorer-side weakness.
- Jarvis passes (0.880 / 0.890) partly because his persona has only 3 targets (neutral, yelling, logic), all easy; Samantha has 6 including the two hard ones.

## 3. How much is scorer vs voice
| Gap | Size | Scorer / reference | Voice |
|---|---:|---|---|
| SUPERB path, 0.491 → 0.83 | 0.339 | **≈ 0.26–0.34.** Her real voice scores below her clones on this path (A < B); single-ref vs best-window alone moves cat 0.461 → 0.832 (row D); cap 0.778 sits under the gate | ≈ 0–0.08 (not separable here; the original voice itself fails) |
| Universal, 0.774 → 0.83 | 0.056 | love term is unstable across prompt banks (0.307 vs 0.061) and shares a window with aggressive | **aggressive 0.556 vs 0.820 on the original** and love 0.307 vs 0.785 are real renders missing that moment (emotion2vec+ angry 0.000 on all 12 clone lines) |
| Gate unreachable by design | — | 4-class head + single mixed reference | — |
Bottom line: on the SUPERB series almost all of the gap is the scorer/reference. On the universal scorer, Samantha is 0.056 short, and that is mostly the voice failing to reproduce one angry/intense window, plus an unstable love detector.

## 4. Ranked fixes (reuse existing open pieces; Rust glue only)
1. **Stop gating on the single-reference SUPERB float; gate per sector with emotion2vec+.** Already designed in `tools/cloning-v2/emotion-match --manifest` (per-sector, 0.83 per sector). Scorer: emotion2vec+ (https://github.com/ddlBoJack/emotion2vec, weights `emotion2vec/emotion2vec_plus_large` on HF; **licence "other" = FunASR model licence, not MIT/Apache — needs Mate's licence check**). Run it from Rust via `ort` (https://github.com/pykeio/ort, Apache-2.0) the same way `scorers/emotion` already runs SUPERB ONNX. Effect: removes the 0.778 cap and the length artefact. Cost: ONNX export + one Rust loader. No new model.
2. **Real per-sector references (12×5 capture kit #115).** No code; needs Slavik's consent + capture. Without these, every scorer compares to a mixed clip.
3. **Use an emotion-controllable open TTS for the missing moments (aggressive/intense, love/tender)** instead of hoping zero-shot cloning carries them:
   - Chatterbox (https://github.com/resemble-ai/chatterbox, MIT; weights MIT) — **already run** as cbx ONNX on Titan :7880. Its `exaggeration`/`cfg_weight` controls are exactly the aggressive knob; spec §3.6 already lists them. Cheapest step: sweep exaggeration on the existing worker (needs consent for own-voice refs).
   - Zonos v0.1 (https://github.com/Zyphra/Zonos, Apache-2.0 code + weights) — emotion vector conditioning (happiness/anger/sadness/fear…) on a cloned speaker. Not run yet. Python/PyTorch upstream; Rust would mean ONNX export via ort — not available upstream, so evaluation only.
   - CosyVoice 2 (`FunAudioLLM/CosyVoice2-0.5B`, Apache-2.0 weights) — instruct-style emotion ("speak angrily"). Not run yet.
   - Orpheus TTS (https://github.com/canopyai/Orpheus-TTS, Apache-2.0) — emotive tags (<laugh>, <sigh>), runs via llama.cpp-style GGUF. Not run yet.
   - Avoid: F5-TTS weights (CC-BY-NC-4.0), IndexTTS2 (custom licence), Seed-VC (GPL-3.0).
4. **Fix the love detector instead of designing a new one:** score love/tender with the dimensional valence/arousal path or emotion2vec+ "happy"+low arousal, not a CLAP prompt bank. Note: the current dimensional model `audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim` is **CC-BY-NC-SA-4.0 (non-commercial)** — flag it; replace or limit to research.
5. **Emotional voice conversion as a post-step only if 3 fails:** OpenVoice V2 (https://github.com/myshell-ai/OpenVoice, MIT; weights MIT) for tone colour over an emotional base voice. Not run.
6. **Stop room/mic normalisation as an emotion fix** (measured −0.030). Keep it only for capture hygiene.
7. Make scoring hosts real: SUPERB/WER/ECAPA weights on the scoring host (emotion-sweep gate is "unmeasured" 0/4); laptop `galion-ecapa` illegal-instruction fix. ECAPA from SpeechBrain (https://github.com/speechbrain/speechbrain, Apache-2.0) via ONNX + ort.

## 5. Symphony brick list (best open working component per layer)
Linux build status = upstream ships/supports Linux builds; "tested here" only where we run it. Laptop: RTX 4060 Laptop 8 GB, driver 610.91 (nvidia-smi checked 18:40 CET).

| Layer | Best existing open brick | Licence | We run it? |
|---|---|---|---|
| Speech-to-text | Vosk (https://github.com/alphacep/vosk-api) small-en-us-0.15 | Apache-2.0 | **Yes**, live :7878 (`GALION_ASR=vosk`) |
| Speech-to-text (streaming) | Moonshine Tiny Streaming (https://github.com/moonshine-ai/moonshine) | repo licence NOASSERTION on GitHub — check | **Yes**, `galion-stt` on Titan |
| Speech-to-text (accuracy) | whisper.cpp (https://github.com/ggml-org/whisper.cpp) | MIT | No (Whisper-tiny in galion-voice-cpu candle code, not deployed) |
| LLM reply | Hermes agent (https://github.com/NousResearch/hermes-agent) + hermes-rs gateway, DeepSeek V4.1 Flash via River | MIT (agent) | **Yes**, live (`GALION_LLM=hermes`) |
| LLM local | llama.cpp (https://github.com/ggml-org/llama.cpp) | MIT | Not in the voice path |
| Emotion (SER, categorical) | emotion2vec+ large | FunASR model licence ("other") — check | Yes, box Python universal scorer only; not Rust, not in a gate |
| Emotion (SER, 4-class gate) | superb/wav2vec2-base-superb-er | Apache-2.0 | Yes, Rust `galion-emotion` (ort) on the box |
| Emotion (dimensional) | audeering msp-dim | **CC-BY-NC-SA-4.0** | Yes, in `galion-emotion` — licence problem |
| Emotion (non-verbal) | EfficientAT mn10_as ONNX | MIT | Yes, box scorer |
| TTS live | Piper (https://github.com/rhasspy/piper, archived) + `galion-piper-rt`, en_US-ljspeech-medium | MIT (archived; successor OHF-Voice/piper1-gpl is GPL-3.0 — do not switch) | **Yes**, live :7878 (`GALION_SPEECH=a21`) |
| TTS with emotion control / cloning | Chatterbox Turbo (https://github.com/resemble-ai/chatterbox) | MIT | **Yes**, cbx :7880 (worker only, never serves calls) |
| TTS emotion candidates | Zonos v0.1 / CosyVoice 2 / Orpheus | Apache-2.0 | No |
| Voice conversion | OpenVoice V2 | MIT | No |
| Quality gate: WER | whisper.cpp or Vosk as ASR for WER | MIT / Apache-2.0 | `galion-wer` exists; absent on sweep host |
| Quality gate: speaker | SpeechBrain ECAPA-TDNN | Apache-2.0 | `galion-ecapa` exists; illegal instruction on laptop CPU |
| Runtime | ort (ONNX Runtime for Rust), candle | Apache-2.0 | Yes (galion-emotion, galion-piper-rt use ORT; galion-voice-cpu uses candle) |
| Hardware | nvml-wrapper (GPU), sysinfo (CPU/RAM) | Apache-2.0 / MIT | Not wired; laptop GPU available |

### 5a. North Star framing: our own PersonaPlex in Rust (Mate t1351u)
Target = full-duplex voice + real emotion + real recognition, live tested. Bricks in that order:

| North Star layer | Brick | Licence (code / weights) | Rust? | Status here |
|---|---|---|---|---|
| Full-duplex speech-to-speech base | Kyutai Moshi (https://github.com/kyutai-labs/moshi), Rust/candle backend, `kyutai/moshiko-candle-q8` | Apache-2.0 (Rust) / **CC-BY-4.0** (attribution OK, commercial OK) | Yes (candle) | A6 is bringing it up on the laptop; not run by A3 |
| Persona/role + voice prompt on the duplex base | NVIDIA PersonaPlex (https://github.com/NVIDIA/personaplex), built on Moshi | MIT (code) / `nvidia/personaplex-7b-v1` weights licence "other" (NVIDIA) — **needs licence check** | Upstream is Python; Rust path = load into Moshi candle backend (same architecture) — not verified | Not run |
| Streaming recognition (Rust) | Kyutai STT `kyutai/stt-1b-en_fr-candle` (https://github.com/kyutai-labs/delayed-streams-modeling) | Apache-2.0 / CC-BY-4.0 | Yes (candle) | Not run; today Vosk + Moonshine are live |
| Streaming TTS fallback (Rust) | Kyutai TTS `kyutai/tts-1.6b-en_fr` | Apache-2.0 / CC-BY-4.0 | Yes (candle) | Not run; today Piper is live |
| Emotion sensing (user side, in the loop) | emotion2vec+ via ort | FunASR model licence — check | via ort | Python only today |
| Emotion expression (Samantha side) | Chatterbox exaggeration (running), Zonos/CosyVoice2 as candidates | MIT / Apache-2.0 | ONNX via ort (Chatterbox already) | cbx running |
| Live test gate | per-sector emotion2vec+ ≥ 0.83, WER ≤ 1.8%, ECAPA > 0.7057, plus Swarm 10 heartbeat/barge-in bars | — | Rust harness exists (S11) | heartbeat FAIL on stub frames |

Implication for the emotion gap: on a Moshi/PersonaPlex base the emotion comes from the model's own audio tokens, so the gate must score full-duplex output per sector with emotion2vec+; the single-reference SUPERB float (§1) cannot measure it at all.

Not checked on the laptop this pass: builds of Zonos, CosyVoice 2, Orpheus, OpenVoice, emotion2vec ONNX export. Each needs a pinned version and a laptop test before it counts as a brick.

## Needs Mate's card / decisions
- Licence calls: emotion2vec+ (FunASR model licence), Moonshine, audeering msp-dim (non-commercial, already in use).
- Consent: any exaggeration sweep or new emotion TTS on Samantha's own-voice refs needs Slavik's signed consent (training-data-generation not needed for scoring-only, but own-voice rendering is consent-gated).
- Whether to change the gate from the single-reference SUPERB float to per-sector emotion2vec+ (threshold stays 0.83).

---

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
