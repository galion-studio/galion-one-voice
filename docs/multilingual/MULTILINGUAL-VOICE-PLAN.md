# Galion Voice — Multilingual Voice Plan (V4)

**Owner:** W20-18 (ML design, assembly) · **Date:** 2026-11-02 (Europe/Zagreb, UTC+1) · **Status:** DRAFT for owner review, local only
**Inputs:** [GROK-VOICE-SWITCHING.md](GROK-VOICE-SWITCHING.md) (W20-16), [TOP-20-LANGUAGES.md](TOP-20-LANGUAGES.md) (W20-17), A18 cascade lane design (`v4/agents/A18-cascade-lane.md`), `crates/galion-cascade` (W20-10 stubs), `crates/galion-ml-voice` (W20-19 EN scaffold), `LOCKS.toml` `[formats]`.

> **Priority:** English first, then **Polish**. Croatian is the Tier-1 regional add.
> **Prototype today:** English only. Nothing in this plan adds a language to code until the owner signs it off.
> **Out of scope:** he-IL/Hebrew (per Wave-20 assignments), weight downloads, VPS work, any GitHub writes.
> **Tags:** `[S1]`/`[S2]` = xAI public docs (via W20-16). `[verify]` = model or language-coverage claim not checked live this turn; confirm against the model card and the A21 licence memo before it is relied on. Untagged = Galion design decision.

---

## 0. TL;DR

1. **Contract:** copy Grok Voice's *behaviour contract*, not its (unpublished) architecture. Auto-detect by default, a live-updatable BCP-47 `language_hint` that biases and never hard-locks, and session/turn-level language pins `[S1]`.
2. **Model approach:** **Stage A** = CPU cascade (ASR → small LLM → TTS) on `galion-tensor` for every non-EN language. **Stage B** (later) = **language tokens** (`<|en|>`, `<|pl|>`, …) in the dual-band student, so switching becomes a token decision inside the 80 ms heartbeat.
3. **Tiers:** T0 `en` (now) · T1 `pl` then `hr` · T2 the rest of the top 20, EU languages first (es, fr, de, pt, ru, then it).
4. **V4 integration:** Whisper-/Parakeet-class ASR in, Piper/SpeechT5/Pocket-TTS-class TTS out, SenseVoice-class affect/LID inbound only. **The 80 ms hot path stays the student.** The cascade is a fallback lane with its own gate profile.
5. **Backlog:** 6 phases, about 56–86 engineer-days (P0–P2) to get EN + Polish working on the cascade. Student language tokens depend on the training budget (H6) and the A21 licence memo.

---

## 1. Grok Voice language switching: what we copy

Full detail and sources are in [GROK-VOICE-SWITCHING.md](GROK-VOICE-SWITCHING.md). Summary:

| Behaviour | Grok Voice (public) | Galion decision |
|---|---|---|
| Default | Detects input language and replies in it, no config `[S1]` | Same: auto by default |
| Mid-conversation switch | "seamlessly switch languages mid-conversation" `[S2]`; code-switching `[S1]` | Switch at **turn boundaries** in the cascade (A18 hysteresis). Intra-turn switching only once the student has language tokens (Stage B) |
| ASR bias | `language_hint` (BCP-47), updatable mid-session; unknown codes silently fall back to auto; es/pt need a region `[S1]` | Same semantics: bias, never lock; unknown → auto; region required where our voices differ |
| Output pin | Session `instructions` or per-turn `response.instructions` `[S1]` | Session `lang_pin` + per-turn override in the sideband (A17) |
| Keyterms / pronunciation | `keyterms` (≤100) and a `replace` map before TTS `[S1]` | Per-language keyterm list + pronunciation map (proper nouns, "Galion") |
| Languages | 20+ listed; **Polish not listed** `[S1]` | PL gets first-class evaluation. There is no Grok PL baseline |
| Architecture / LID mechanism / sizes | **Not published** | Not assumed. We choose cascade vs. end-to-end on our own benchmarks |

**Galion-specific metric (no public equivalent):** *switch latency* = time from the first L2 phoneme to the first L2 audio frame out. Overall TTFA bar to beat: < 1 s `[S2]`.

---

## 2. Recommended small-model approach for Galion

### 2.1 Why two stages
- The primary path is the dual-band student on the 80 ms heartbeat (no VAD, no cascade). Training a multilingual student needs a GPU budget (H6), cleared weights (A21) and per-language data. None of these is available today.
- A CPU cascade can ship one language at a time using existing open ASR/TTS checkpoints, once licences clear. That gets PL working without retraining the student.
- The cascade also gives us **teacher data and evaluation sets** for the student's language tokens later.

### 2.2 Stage A — CPU cascade (first; covers PL/HR/T2)
```
mic 24 kHz ─► 16 kHz ─► LID (first ~1.5 s, 2 agreeing windows) ─► ASR (Parakeet-/Whisper-class, streaming)
          ─► endpoint (cascade-local) ─► small multilingual LLM (~0.5–1.5B, int8, streaming)
          ─► phrase chunker ─► TTS per-language voice pack (Piper/Pocket-TTS-class) ─► 80 ms frames
inbound affect: SenseVoice-class ─► AffectStore (consented) ─► TTS style + LLM tag
```
- **Runtime:** everything runs on `galion-tensor` (int8 GEMV, conv/KV ring), weights safetensors → `.gq`. No foreign runtime and no legacy container formats (LOCKS `[formats]`, G2). Reference toolkits are offline dev oracles under `tests/oracle/` only.
- **Routing (`cascade-routes.toml`, per A18):** explicit locale > `language_hint` > LID. `en` → student if healthy, else cascade EN. `pl`, `hr` (+`bs`/`sr` share models, hr voice), EU → cascade. Unsupported → EN with an honest spoken notice; never fake fluency.
- **Budget (A18):** onset p50 ≤ 800 ms, p95 ≤ 1.2 s; barge-in ≤ 200 ms (mandatory); separate `presence.cascade.json` gate profile. The primary p50 ≤ 300 ms gate is **not** loosened.
- **Rika:** not claimed on the cascade (`rika_armed=false`); Rika-lite behind a flag (A18 §6).

### 2.3 Stage B — language tokens in the student (later)
- Reserve language-control tokens in the student's text/control vocabulary now, matching the W20-19 scaffold format: `<|en|>`, later `<|pl|>`, `<|hr|>`, … (`LanguageId::token()` in `crates/galion-ml-voice`).
- **Input side:** a tiny LID head on the inbound band (a few K–100K params, int8) emits a per-window language posterior every 80 ms. Hysteresis: 2 agreeing windows before the student's state flips.
- **Output side:** the active language token conditions generation (prefix token + FiLM-style conditioning, the same mechanism as affect). `language_hint` and `lang_pin` become logit biases on the token, never hard masks, except a pin.
- **Training (blocked on H6/A21):** KD from the Stage-A cascade (teacher transcripts + TTS audio) plus real PL speech. Add one language at a time; gate each on EN no-regression (EN WER and presence gates must not move).
- **Cost on the hot path:** one LID head + one conditioning vector per frame. Target ≤ 3 % of the 80 ms frame budget, the same envelope as affect.
- **Exit criterion for Stage B per language:** student PL beats cascade PL on onset latency, and stays within an agreed WER/MOS delta. Until then PL stays on the cascade.

### 2.4 Candidate components (all `[verify]` + A21)
| Slot | Candidates | EN | PL | HR | Notes |
|---|---|---|---|---|---|
| ASR | Parakeet-class multilingual (FastConformer TDT/CTC) | yes | likely (EU multilingual checkpoint) `[verify]` | likely `[verify]` | Streaming-friendly; preferred for EU |
| ASR | Whisper-class (small/base, multilingual) | yes | yes `[verify]` | yes `[verify]` | Built-in LID + language tokens (a design precedent); heavier; chunked streaming |
| Affect / LID inbound | SenseVoice-Small-class | yes | **no / weak** `[verify]` | no `[verify]` | Use for **affect** (and EN/CJK LID) only; do not rely on it for PL LID |
| TTS | Piper-class (VITS) | yes | yes (several PL voices) `[verify]` | yes `[verify]` | Small, CPU-fast, per-language voice packs; best PL/HR bet |
| TTS | SpeechT5-class | yes | no (EN-trained) `[verify]` | no | EN fallback / research only |
| TTS | Pocket-TTS-class (~100M, streaming 24 kHz) | yes | not at release `[verify]` | no `[verify]` | Best EN streaming quality; multilingual later |
| LLM | small multilingual instruct LM, ~0.5–1.5B int8 | yes | must be evaluated on PL `[verify]` | `[verify]` | One LM for all routes; licence per A21 |
| LID | Whisper-style LID or a tiny dedicated head | — | needed | needed | Owned in-house long-term (matches "own the stack" `[S2]`) |

No voice cloning (H). Weights are not vendored (`weights_vendored = false`).

---

## 3. Language tiers (top 20)

From [TOP-20-LANGUAGES.md](TOP-20-LANGUAGES.md) (speaker counts are approximate, Ethnologue-style, ±10–20 %):

| Tier | Languages | Path | Gate to enter |
|---|---|---|---|
| **T0 — now** | English (`en-US`/`en-GB`) | Student primary + cascade EN fallback | Already the prototype |
| **T1 — next** | **Polish (`pl-PL`)** first, then Croatian (`hr-HR`; `bs`/`sr` share models) | Cascade (Stage A), then student language token (Stage B) | ASR/TTS/LLM licence cleared (A21); PL eval set + EN↔PL switch tests T1–T6 pass |
| **T2a — EU** | Spanish (`es-ES`/`es-419`), French, German, Portuguese (`pt-BR`/`pt-PT`), Russian; Italian as the EU add | Cascade with Piper-class voices | Per-language WER/MOS + switch test pass |
| **T2b — global** | Mandarin (`zh-CN`), Hindi, Arabic (`ar-SA`, `ar-EG`; RTL), Bengali, Indonesian, Urdu (RTL), Japanese, Vietnamese, Turkish, Marathi, Telugu | Cascade; voice coverage varies | As T2a + RTL/bidi handling where needed |
| **T2c — low priority** | Nigerian Pidgin, Hausa, Swahili (borderline) | Only if open ASR/TTS quality is adequate | Honest "not supported" notice until then |

Overlap with Grok Voice's listed set `[S1]`: most of T2a/T2b. **Polish and Croatian are not on Grok's list**, so our T1 has no external baseline.

The one-dot UI (`[ui]` lock) has no text, so RTL affects only logs/console and TTS text normalisation, not the dot.

---

## 4. V4 multi-lab integration

### 4.1 Where each lab's model sits
| Direction | Component | V4 crate / slot | Hot path? |
|---|---|---|---|
| In | Whisper-class / Parakeet-class ASR | `galion-cascade::Asr` (stub today: `MockAsr`) | **No**, cascade only |
| In | SenseVoice-class affect (+ EN/CJK LID hint) | `galion-cascade::EmotionIn` → `galion-emotion` AffectStore (consent-gated) | No. Advisory, ≤ 3 % budget |
| Mid | small LLM | `galion-cascade::Llm` | No |
| Out | Piper / SpeechT5 / Pocket-TTS-class | `galion-cascade::Tts` | No |
| Both | Student (dual-band V2V) | `galion-student-backend`, `galion-heartbeat` | **Yes: the 80 ms hot path stays the student** |
| Control | `LanguageId`, `<|xx|>` tokens, `LangRouter` | `galion-ml-voice` (EN-only today) | Router decision is O(1) per turn |
| Side | Hermes sideband: `language_hint`, `lang_pin`, keyterms | `galion-sideband` (A17) | No (3 s cap) |

### 4.2 Rules
1. **The hot path is the student.** No third-party model runs inside `step()` on the 80 ms heartbeat. Multilingual on the hot path arrives only through Stage-B language tokens in the student.
2. **The cascade is a fallback lane** (cargo feature `cascade`, off by default). It is selected by language coverage, probe CPU floor, or student health (A18 §1). It still ticks every 80 ms so presence/sideband behave the same way.
3. **One runtime:** all imported models are ported to `galion-tensor`; external toolkits only as offline oracles (G2, LOCKS `[formats]`).
4. **Affect is inbound-only and consent-gated** (`persist_requires = "AffectStore<Consented>"`; prohibited purposes stay locked).
5. **Honest gates:** cascade gets `presence.cascade.json`; G6 not claimed on the cascade; primary gates unchanged.

### 4.3 Switching sequence (Stage A, EN → PL)
1. User turn starts in PL. LID windows 1–2 agree on `pl` (≈ 160–1500 ms).
2. If the session runs on the student (EN), the router hands the **next turn** to the cascade PL route; the current EN reply finishes or yields on barge-in (≤ 200 ms).
3. ASR biased by `language_hint` if set; LLM prompted with `<|pl|>`; TTS uses the PL voice pack.
4. Back to EN: two agreeing EN windows → next turn returns to the student.
5. Log switch latency (first PL phoneme → first PL audio frame) to the console feed.

---

## 5. Phased backlog with effort estimates

Estimates are engineer-days (ed) for one engineer familiar with the codebase; ranges cover port difficulty. They assume no weights are downloaded until A21 clears; "port" phases start only after that.

| Phase | Item | Effort | Depends on | Output |
|---|---|---|---|---|
| **P0 — Contract & scaffolding (now, EN-only)** | P0.1 Sideband fields `language_hint`, `lang_pin`, `keyterms`, pronunciation map (types + tests) | 2–3 ed | A17 | `galion-sideband` types |
| | P0.2 `LangRouter` contract: explicit > hint > LID; unknown hint → auto; hysteresis 2 windows (EN-only impl, multi-lang tests behind `#[cfg(test)]` fakes) | 2–3 ed | W20-19 | `galion-ml-voice` |
| | P0.3 Reserve `<|xx|>` token range in student vocab spec (doc + config, no training) | 1 ed | A8 | student config |
| | P0.4 Switch-latency metric + console feed field | 1–2 ed | A11 | `galion-console` |
| | P0.5 `presence.cascade.json` gate profile (EN) | 2 ed | A13, W20-10 | specs + runner |
| **P1 — EN cascade real (blocked on A21)** | P1.1 Port Parakeet-class EN ASR to `galion-tensor` + oracle tests | 8–12 ed | A21, A4/A5, A7 writer (G5) | `Asr` impl |
| | P1.2 Port Piper- or Pocket-TTS-class EN TTS | 6–10 ed | A21 | `Tts` impl |
| | P1.3 Small LLM on `galion-tensor` (int8, streaming) | 10–15 ed | A21, KV ring | `Llm` impl |
| | P1.4 Endpointing + phrase chunker + barge-in ≤ 200 ms | 3–5 ed | P1.1–1.3 | cascade loop |
| | P1.5 Onset gate on probe-class CPU (p50 ≤ 800 ms) | 2–3 ed | R0 (VPS) for real numbers | gate result |
| **P2 — Polish on cascade (T1)** | P2.1 PL eval set: ~2 h read + conversational, consented; EN↔PL switch set T1–T6 | 5–8 ed | owner/data consent | `specs/fixtures/pl/` |
| | P2.2 LID head (EN/PL first) + hysteresis wiring | 4–6 ed | P0.2 | LID impl |
| | P2.3 PL ASR (same multilingual Parakeet-/Whisper-class checkpoint) + WER eval | 3–5 ed | P1.1, A21 | WER report |
| | P2.4 PL TTS voice pack (Piper-class) + MOS/intelligibility check | 4–6 ed | P1.2, A21 | voice pack |
| | P2.5 LLM PL quality check + PL keyterms/pronunciation map | 2–4 ed | P1.3 | eval note |
| | P2.6 Add `LanguageId::Pl` + `<|pl|>` (owner sign-off) | 1 ed | P2.1–2.5 | code |
| **P3 — Croatian + affect** | P3.1 HR route (`hr`, `bs`/`sr` share) — ASR/TTS/LLM evals | 5–8 ed | P2 | route |
| | P3.2 SenseVoice-class affect port (inbound, consent-gated) | 5–8 ed | A21, A9 | `EmotionIn` impl |
| **P4 — T2a EU languages** | Per language (es, fr, de, pt, ru, it): eval set + voice pack + switch tests | 3–5 ed **each** (~18–30 total) | P2 pipeline | routes |
| **P5 — Student language tokens (Stage B)** | P5.1 KD data from cascade (EN+PL) | 5–8 ed | P2, H6 | dataset |
| | P5.2 Train/adapt student with `<|en|>`/`<|pl|>` + LID head conditioning | 15–25 ed + GPU | H6, A21, A8 | student s1 |
| | P5.3 EN no-regression + PL student-vs-cascade gate | 3–5 ed | P5.2 | gate result |
| **P6 — T2b/c global** | Per language as P4; RTL normalisation for ar/ur | 3–6 ed each | P4 | routes |

**Totals (rough):** P0 ≈ 8–11 ed (unblocked now) · P1 ≈ 29–45 ed · P2 ≈ 19–30 ed · P3 ≈ 10–16 ed · **EN+PL on cascade (P0–P2) ≈ 56–86 ed; through HR + affect ≈ 66–102 ed**, plus P4 ≈ 18–30 ed. P5 depends on GPU budget.

### 5.1 Blockers carried
- **A21 counsel** — weights/licence clearance for every ASR/TTS/LLM/affect checkpoint (blocks P1+).
- **G5 / `.gq` writer** — needed to load ported weights (A7/W20-14).
- **R0–R3 VPS** — real latency numbers on target CPU (P1.5).
- **H6 GPU budget** — Stage B.
- **Data consent** — PL eval recordings.

### 5.2 Open owner decisions
1. Confirm Croatian as T1 after Polish (W20-17 proposal).
2. Confirm turn-boundary-only switching for the cascade (no intra-utterance code-switch until Stage B).
3. Confirm Piper-class as the PL TTS default vs. waiting for a multilingual Pocket-TTS-class model.
4. Whether to run the T1–T6 switch tests against Grok Voice as an external reference (needs an API key; PL is informational only).

---

## 6. Evaluation (per language)
- ASR WER on own eval set; TTS MOS + intelligibility (ASR round-trip WER); LLM reply-language accuracy (replies in the user's language unless pinned).
- Switch tests T1–T6 (GROK-VOICE-SWITCHING §4.1) + switch latency.
- Presence: cascade profile onset p50/p95, barge-in ≤ 200 ms.
- EN no-regression on every language add.

*No code, weights, or locale packs were added by this document. Local only; no git push.*

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
