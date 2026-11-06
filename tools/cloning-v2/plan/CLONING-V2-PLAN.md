# Cloning V2 Plan — own-samantha / own-jarvis

**When:** 2026-11-05 evening Europe/Zagreb (CET)  
**Source of truth:** `/workspace/swarm/voice-v2-sprint/own-clone-analysis/REPORT.md` (+ `scores/`, `benchmark_manifest.json`)  
**Engine under test:** `cbx` Chatterbox Turbo ONNX fp32, voices `own-samantha` / `own-jarvis`  
**This document lives at:** `/workspace/swarm/voice-v2-sprint/cloning-v2/plan/CLONING-V2-PLAN.md`  
**Parallel workers (do not conflict):** ten workstreams under `cloning-v2/` — `plan`, `hf-samantha`, `similarity`, `pipeline`, `emotion-score`, `emotion-fix`, `ecapa`, `articulation`, `mos-wer`, `scoreboard` (create dirs as needed).  
**Steering (owner):** articulation / mouth-expression P0 with Samantha HF (t734u); lightning-fast **Rust-native** (t735u); **ten parallel Rust workstreams**, **emotion = P0 #1** scoring+fixes in Rust (t736u).

---

## 0. Hard constraints (non-negotiable)

1. **Read-only originals.** Do not modify:
   - `Samantha.wav` / `Jarvis.wav` (owner source uploads)
   - `clip-store-own/*/original.wav`
2. **Current clones are backups.** Keep `own-samantha` / `own-jarvis` (profiles + any shipped synth outputs) as baselines. V2 work writes new profile names, conditioned refs, or post-process paths — never overwrite the accepted V1 clones in place until owner promotes them.
3. **Laptop approved as GPU instance (2026-11-05 20:08 CET, Slavik via Mate).** Own-voice recordings and clones may be copied to galion-laptop under `~/workspace/galion/cloning-v2/` only (RTX 4060, cbx with CUDA). No other off-box copies (no email/Drive/external). Development and scoring code stays on this box; heavy rendering runs on the laptop. Persona targets: `cloning-v2/personas/`.
4. **No Piper training on own-voice.** Consent scope is **local zero-shot conditioning only**. Do not add own-voice clips to `piper-finetune/` corpora, do not fine-tune Piper on Samantha/Jarvis, do not export student models from own refs. Piper remains CC0-only until explicit new consent.
5. **Scope = analysis recommendations + owner steering (emotion P0#1, articulation+HF P0, Rust-native ×10).** No unrelated refactors, no new personas, no watermark product decisions beyond verifying PerTh presence, no changing consent text. No Python in the hot path for pipeline/scoring/fixes/emotion (see §0A / §3).
6. **Coordinate across ten parallel workstreams** (§3). Plan-only writes stay under `cloning-v2/plan/`. Do not write into another stream’s directory.
7. **Rust-native hot path.** Production V2 pipeline, **emotion score+fix**, ECAPA, articulation, MOS/WER/hygiene, scoreboard, ref prep, and similarity selection are Rust crates (see §0A). Python is **offline cross-check only** (legacy SQUIM/SER/ASR) until replaced; **SER for V2 must be ONNX in `galion-emotion`**, not Python transformers.
8. **Priority order:** **Emotion (P0 #1)** → mouth/articulation + Samantha HF (P0) → similarity/ECAPA + pipeline hygiene → MOS/WER/scoreboard integration. Emotion scoring and emotion fixes are first-class Rust work, not a Python afterthought.

---

## 0A. P0 — Rust-native stack + ten workstreams (emotion = #1)

Owner mandates: lightning-fast **Rust** (t735u); **ten parallel streams** with **emotion as TOP priority** for both scoring and fixes (t736u); mouth + HF remain P0 (t734u).

### Priority stack

| Rank | Theme | Why |
|---|---|---|
| **P0 #1** | **Emotion** — `galion-emotion` + `cbx-emo` | Clones miss intended affect (Sam SER `ang` 8/12; emotional lines t06/t07 never land `hap`); must score + fix in Rust |
| **P0** | **Mouth / articulation** — `galion-artic` | All-scopes match: sound, pronunciation, mouth expression (audio proxies) |
| **P0** | **Samantha HF** — `refprep-rs` in hf-samantha | Largest identity/MOS driver; fricatives need bandwidth |
| **P0** | Shared Rust plumbing — `cbx-v2`, `galion-ecapa`, `galion-board`, … | Hot path speed + agreement benches |

### Ten parallel workstreams ↔ crates

| # | Workstream dir | Crate / focus | Owns |
|---|---|---|---|
| **1** | `cloning-v2/plan/` | *(docs)* | This plan, phonetic scripts, priority/success criteria |
| **2** | `cloning-v2/hf-samantha/` | **`refprep-rs`** (Sam HF path) | Conditioned Sam refs: denoise, click, loudness, estimate-only HF shelf; never originals |
| **3** | `cloning-v2/similarity/` | **`simsel-rs`** | A/B selection, multi-clip pack schema, promote `own-*-v2` using board metrics |
| **4** | `cloning-v2/pipeline/` | **`cbx-v2`** | Synth loop: fp32 pin, rep-penalty, seed, limiter/de-click, soxr resample; hooks for `cbx-emo` |
| **5** | `cloning-v2/emotion-score/` | **`galion-emotion`** | **P0 #1 scorer:** ONNX SER (neu/hap/ang/sad or equiv), **A/V/D** (arousal/valence/dominance) regressor or proxies, acoustic emotion proxies (F0 contour stats, energy bursts, spectral tilt); JSON for board |
| **6** | `cloning-v2/emotion-fix/` | **`cbx-emo`** | **P0 #1 fixes:** exaggeration / emotion conditioning knobs, sampling (temp/top_p or best available under Turbo), **emotion-aware best-of-N** (generate N, pick by `galion-emotion` + ECAPA guardrails) |
| **7** | `cloning-v2/ecapa/` | **`galion-ecapa`** | ECAPA-TDNN ONNX/`ort` cosine vs ref; half-vs-half ceiling helpers; fast batch |
| **8** | `cloning-v2/articulation/` | **`galion-artic`** | MCD-DTW, F1–F3, vowel-space, artic. rate, fricative centroid, plosive bursts, phoneme durations |
| **9** | `cloning-v2/mos-wer/` | **`galion-hygiene`** (name flexible) | Artifacts/peaks/clicks; F0/rate/energy; WER via Rust ASR binding or wasm/onnx if feasible; MOS — prefer ONNX MOS if available, else call out Python SQUIM as **offline cross-check only** |
| **10** | `cloning-v2/scoreboard/` | **`galion-board`** | Aggregate JSON scoreboard; before/after tables; wall-time benches; Rust↔Python agreement report; promote/no-promote gate |

> Note: earlier plan referred to a monolithic `galion-score`. **Split** into `galion-emotion`, `galion-ecapa`, `galion-artic`, `galion-hygiene`, orchestrated by `galion-board`. A thin `galion-score` meta-crate/CLI that shells to these is OK if board owns the schema.

### Emotion scorer detail (`galion-emotion`) — P0 #1

| Signal | Implementation target |
|---|---|
| Categorical SER | ONNX export of a small ER model (labels neu/hap/ang/sad or dataset-native); **not** Python `transformers` in hot path |
| Continuous A/V/D | ONNX regressor if available; else acoustic proxies mapped to arousal/valence/dominance (document mapping) |
| Acoustic proxies | F0 median/range/slope; energy dyn range + bursts; spectral tilt / HF ratio; speaking & articulation rate |
| Line-aware checks | t06/t07 expect higher valence / `hap`-leaning; Jarvis default `neu` high-confidence OK |
| Output | Per-clip top label + conf, A/V/D, proxy vector; match-ref flag |

### Emotion fixes detail (`cbx-emo`) — P0 #1

| Lever | Notes |
|---|---|
| Exaggeration / conditioning | Wire any Chatterbox emotion/exaggeration conditioning the ONNX graph allows; document if Turbo export lacks it |
| Sampling | Prefer non-argmax where export allows; else **best-of-N** with fixed seeds / decoder noise draws |
| Emotion-aware best-of-N | N=4..8 candidates; rank by emotion target distance + ECAPA ≥ (baseline−0.02) + artic. MCD not worse than +10% |
| Rep-penalty interaction | Score emo metrics in the 1.0 vs 1.2 A/B (Sam emotional lines) |
| Guardrails | Never promote emo win that tanks ECAPA >0.03 or WER > baseline+0.05 |

### Python offline cross-check only

| Tool | Rule |
|---|---|
| Legacy superb SER / transformers | Cross-check `galion-emotion` only; **not** V2 gate |
| SQUIM | Offline until ONNX MOS lands in `galion-hygiene` |
| vosk WER | Offline / optional until Rust ASR path exists |

### Rust-native success criteria (all streams)

| Criterion | Target |
|---|---|
| Hot-path languages | Synth + all scores + refprep + simsel + emo fix: **100% Rust** on promote path |
| Emotion agreement | `galion-emotion` categorical label agreement with Python SER cross-check **≥ 80%** on n=24 (12×2) where Python conf >0.5; A/V/D Pearson **≥ 0.7** vs proxy/Python if available |
| ECAPA agreement | `galion-ecapa` vs prior Python ECAPA **±0.01** |
| Articulation agreement | MCD ±5% rel; formants ±10%; artic rate ±5% |
| Hygiene | Clicks ±1; peaks exact within 0.1 dB |
| Wall-time | Full board (both voices × 12 lines, all metrics) **≥5×** vs `own-clone-analysis` Python path; single-utterance emotion score **< 200 ms** warm CPU target where feasible |
| Emotion product | Sam `ang` ≤ 4/12; t06/t07 valence↑ vs V1; Jarvis stays predominantly `neu` |

---

## 1. Discovery inventory (every finding from the analysis)

### 1.1 Similarity (ECAPA cosine vs own reference, n=12)

| Voice | Mean ± sd | Min…max | Median | Half-vs-half ceiling | Gap to ceiling |
|---|---|---|---|---|---|
| own-samantha | **0.707 ± 0.030** | 0.662…0.761 | 0.719 | **0.828** | ~15% relative (largest headroom) |
| own-jarvis | **0.733 ± 0.053** | 0.610…0.800 | 0.739 | **0.725** | mean already ≈/slightly above short-half ceiling |

- Prior CC0 Turbo fp32 bakeoff means (different refs): Sam 0.719, Jar 0.793.
- Prior single-line fp32 spots: Sam 0.740, Jar 0.805.
- Worst lines: Samantha t07 (emotional praise) 0.662; Jarvis t10 (numbers/phone) 0.610.
- **Inference:** Samantha is identity-limited by the reference; Jarvis is already near its short-clip self-similarity ceiling — further gains need richer refs / harder-line coverage, not just decoder tweaks.

### 1.2 Intelligibility (vosk-small WER)

| Voice | Mean ± sd | ≤10% fraction |
|---|---|---|
| Samantha | 0.050 ± 0.068 | 0.75 |
| Jarvis | 0.018 ± 0.033 | 1.00 |

Outliers are mostly soft function-word confusions (“open”→“up in”, “Hmm”→“he”, “four”→“for”), not wholesale garbling. WER is not the primary bottleneck.

### 1.3 Naturalness (torchaudio SQUIM)

| Voice | Clone subjective MOS | Ref MOS | Gap |
|---|---|---|---|
| Samantha | **3.73 ± 0.18** | 4.33 | −0.60 |
| Jarvis | **4.14 ± 0.15** | 4.55 | −0.41 |

Objective PESQ means: Sam 3.91, Jar 3.72. Clones sit ~0.4–0.6 MOS below their own references.

### 1.4 Artifacts (synth outs, 24 kHz mono s16)

| | Samantha | Jarvis |
|---|---|---|
| Peak dBFS mean | −1.13 (4/12 hit 0) | −2.05 (1/12 hit 0) |
| Clipping samples / clicks (Δ>\|0.35\|) | 9 / **32** (5 clips) | 4 / **39** (8 clips) |
| Internal ≥300 ms gaps | 7/12 | 4/12 |

Hot peaks + no limiter → occasional 0 dBFS hits. Clicks are common on both personas.

### 1.5 Prosody / expression (clone mean vs reference)

| Metric | Samantha clone → ref | Jarvis clone → ref |
|---|---|---|
| F0 median | **201 → 178 Hz** (↑ ~1.1 st) | **143 → 129 Hz** (↑ ~1.8 st) |
| F0 sd (st) | 3.74 → 3.11 | 3.58 → 3.31 |
| F0 range (st) | 17.8 → 18.3 | 14.5 → 14.8 |
| Energy dyn range | **16.4 → 22.3 dB** (compressed) | **10.0 → 13.2 dB** (compressed) |
| Speaking rate | **3.49 → 2.82 w/s** (~+24% faster) | **3.31 → 2.63 w/s** (~+26% faster) |

Emphasis pattern: F0 *range* similar to refs, but loudness flattened and rate systematically +15–25%. Samantha clones sound less like the soft intimate “Her” energy (higher + faster).

### 1.6 Emotion (superb/wav2vec2-base-superb-er; noisy on TTS) — **V2 moves this to Rust P0 #1**

| | Ref top | Clone mix (n=12) | Match ref |
|---|---|---|---|
| Samantha | neu (0.34, weak) | **ang 8**, neu 3, hap 1 | 3/12 |
| Jarvis | neu (0.84) | neu 12 | 12/12 |

Intended emotional lines t06/t07 did not land `hap` on either voice. Samantha `ang` tags are likely SER reacting to Turbo spectral tilt / sharper transients, not true anger — still a usability signal that Samantha synths skew bright/aggressive vs the intimate ref.

### 1.7 Recording environment (from `original.wav`; inferences only)

**Samantha (44.1 kHz stereo → dual-mono):**
- Dual-mono (L≈R, corr=1.000); NF **−51.9 dBFS**; peak −1.93 / RMS −21.0 dBFS; 0 clips on original.
- Bandwidth: f90≈937 Hz, **f99≈2.7 kHz**; only **~0.4%** energy >4 kHz; hf/mid ≈ 0.001 (**severe HF loss**).
- Mild rumble; no clear mains hum; RT60 unclear (dry / close / short tail).
- Predicted: small dry space or very close mic; **HF-limited chain** (capsule/foam/EQ/breathy delivery). Confidence high on HF limit; low on exact room.

**Jarvis (48 kHz stereo, near-mono):**
- Near-mono (corr=0.990); NF **−81.7 dBFS**; peak −0.62 / RMS −23.4; rumble +11.7 dB vs mid quiet.
- Bandwidth: f90≈2.0 kHz, **f99≈7.2 kHz**; **~6%** energy >4 kHz.
- Proximity LF ratio 0.57 (classic close mic); RT60 proxy **~0.85 s** (low–medium confidence).
- Predicted: quiet room, close cardioid/condenser on-axis, light untreated return.

**Do clones carry the room?** No. Clones are always mono 24 kHz Turbo outs; they do not keep dual-mono, Jarvis’s −82 floor, or Samantha’s exact HF hole. Mild reverb/noise identity can leak via the speech encoder; measured clone floors/RT60 are decoder artefacts.

### 1.8 Reference prep issues

- Single **16–17 s** scripted character monologue per persona (not phonetically balanced; not multi-style).
- Consent path: full upload → mono 24 kHz s16, **no denoise, no normalisation**.
- Samantha `reference.wav` introduced a few large sample jumps (**3 clicks vs 0 on original**) — likely linear resample path.
- Profile: `~/.cache/huggingface/cbx/voices/own-*.cbxvoice`, fp32, locked to repo/revision/dtype; **no multi-ref blend** in shipped cbx.

### 1.9 Pipeline bottlenecks (ranked as in REPORT §4)

1. Samantha input HF bandwidth (largest identity/MOS driver for Sam).
2. Hot peaks / no normalize on both refs → synth 0 dBFS hits.
3. Single scripted ref (no multi-clip / phonetic coverage) → hard lines (numbers, emotional) suffer.
4. Turbo few-step decoder + **argmax** LM + **repetition_penalty 1.2** + no seed/CFG/exaggeration/decoder-step control; CLI default fp16 is a footgun (profiles correctly force fp32).
5. Linear resample / s16 clamp / no limiter / no de-click in `audio.rs`; 3 silence tokens appended before decode.
6. **PerTh** Python package present but **not called from Rust cbx** (watermark may be absent).
7. Piper constraints (own-voice training not consented; tiny kept hours; no GPU) — out of V2 execute scope except “do not train.”

### 1.10 Hardcoded cbx limits (source)

- `TARGET_SAMPLE_RATE = 24000`
- LM sampling = `argmax_i64` only
- `repetition_penalty` default **1.2**
- `SILENCE_TOKEN = 4299` ×3
- No seed, no CFG, no exaggeration, no Turbo decoder-step knob
- Decoder ONNX still has RandomNormalLike/RandomUniformLike → non-reproducible waveforms across runs

### 1.11 Articulation / mouth-expression gap (owner P0, t734u)

**Goal:** the clone must match across **all scopes: sound, mouth movement, pronunciation, mouth expression**.

**Measurement reality:** owner recordings and clone outs are **audio-only**. There is **no video**, so **visual lip-sync is not measurable** this sprint. “Mouth movement / mouth expression” is proxied **acoustically** (same physical articulators leave formant, burst, and duration signatures):

| Acoustic proxy | What it stands in for |
|---|---|
| **MCD-DTW** on synth vs ref forced to the **same transcript** | Overall spectral / articulatory match (lower = closer mouth shaping over time) |
| **Formants F1–F3** per vowel (means ± sd) | Tongue height/backness / lip rounding |
| **Vowel-space area** (F1–F2 convex hull) | Articulatory working space (under- vs over-articulation) |
| **Articulation rate** (phones or syllables / sec, excl. pauses) | How fast the mouth moves — distinct from raw word rate |
| **Consonant clarity** — fricative spectral centroid; plosive burst strength/timing | Lip/teeth/tongue constriction precision |
| **Phoneme durations** (esp. vowels, stops, fricatives) | Timing of mouth gestures / pronunciation |

**V1 status:** analysis reported WER, F0, word rate, energy — **not** MCD-DTW, formants, vowel-space, or consonant bursts. Treat articulation baselines as **to-be-measured in P0 by `galion-artic`** (board-aggregated) on V1 WAVs under `own-clone-analysis/wav/`.

**Linked bottlenecks:** Sam HF loss blunts fricative centroids; +20–25% speaking rate and compressed energy flatten mouth-expression dynamics; single scripted refs lack phoneme coverage for stable F1–F3 / duration stats.

---

## 2. Suggestion inventory (every REPORT §5 item → plan action)

| # | Suggestion | Expected gain | Effort | V2 disposition |
|---|---|---|---|---|
| S1 | Re-record Samantha with more HF / presence (or gentle transparent HF restore + denoise on a **new** consent-scoped clip) | High (Sam sim + MOS) | Medium (new take) | **P0 owner-recording** + optional **P1 HF workstream** estimate-only restore on conditioned copy |
| S2 | Multi-clip / phonetically richer reference pack (neutral + questions + numbers + soft emotional); rebuild profile when approved | High on hard lines | Medium | **P1 needs-owner-recording**; similarity workstream prepares pack schema + rebuild recipe |
| S3 | Light input conditioning before encode (−3 LUFS / true-peak −1.5 dBTP, gentle denoise, click repair) on a **new** ref — never silently mutate clip-store originals | Medium | Low–medium | **P0/P1 pipeline** + HF |
| S4 | Keep fp32 forever for premium path; pin dtype in wrappers | High vs fp16 (already measured) | Trivial | **P0 pipeline** |
| S5 | A/B `repetition_penalty` 1.0 vs 1.2 on emotional/long lines | Low–medium (prosody/rate + **emotion**) | Trivial | **P0 simsel + emotion-score/fix** |
| S6 | Optional full Chatterbox (non-Turbo) or Nano Python path for offline premium renders | Potentially medium–high naturalness | High | **P2 pipeline** (evaluate only; do not replace live Turbo path without owner OK) |
| S7 | Post-process synth: true-peak limiter, mild de-click, optional slight rate/F0 match toward ref | Low–medium artifacts/MOS | Low | **P0/P1 cbx-v2 + hygiene** |
| S8 | Piper student only after own-voice consent + much more audio + GPU | Latency not quality | High | **Blocked** — document only; no training |
| S9 | Expose seed / sampling / exaggeration in cbx (upstream or fork) | Medium for expressive lines | Medium eng | **P0 #1 `cbx-emo`** (pull forward from P1/P2) |
| S10 | Verify PerTh on cbx outs; add explicitly if product requires Resemble watermark | Compliance not quality | Low | **P1 pipeline** |
| S11 | **Articulation / mouth-expression match** (acoustic proxies; all-scopes) | High | Medium | **P0 `galion-artic`** (equal to Sam HF; after emotion #1) |
| S12 | **Rust-native ten-crate split** (`cbx-v2`, `cbx-emo`, `galion-emotion`, `galion-ecapa`, `galion-artic`, `galion-hygiene`, `galion-board`, `refprep-rs`, `simsel-rs`) | High | Medium–high | **P0 §0A** |
| S13 | **Emotion score in Rust** (`galion-emotion`: ONNX SER + A/V/D + proxies) | **Highest** | Medium | **P0 #1 stream 5** |
| S14 | **Emotion fixes in Rust** (`cbx-emo`: exaggeration/conditioning, sampling, emotion-aware best-of-N) | **Highest** | Medium–high | **P0 #1 stream 6** |

---

## 3. Workstream map — ten parallel Rust agents

| # | Dir | Crate | P0 role | Must not |
|---|---|---|---|---|
| 1 | `plan/` | — | Finish & maintain this doc + scripts | Edit other streams’ code/audio |
| 2 | `hf-samantha/` | `refprep-rs` | Sam HF conditioned copies; feed emo/artic | Touch `original.wav` / V1 profile |
| 3 | `similarity/` | `simsel-rs` | Select/promote using board (emo-weighted) | Piper train; delete V1 |
| 4 | `pipeline/` | `cbx-v2` | Fast synth; expose hooks for `cbx-emo` | Python synth loop |
| 5 | `emotion-score/` | **`galion-emotion`** | **P0 #1** ONNX SER + A/V/D + proxies | Depend on Python transformers at runtime |
| 6 | `emotion-fix/` | **`cbx-emo`** | **P0 #1** exaggeration/conditioning, sampling, emo best-of-N | Ignore ECAPA/WER guardrails |
| 7 | `ecapa/` | `galion-ecapa` | Identity cosine; ceilings | Block emo stream; mutate originals |
| 8 | `articulation/` | `galion-artic` | Mouth proxies (MCD, formants, …) | Claim visual lip-sync |
| 9 | `mos-wer/` | `galion-hygiene` | Peaks/clicks/F0/rate; WER/MOS paths | Gate on Python SQUIM in hot path |
| 10 | `scoreboard/` | `galion-board` | Aggregate, benches, promote gate | Silent promote without emo metrics |

**Conflict rule:** write only under your dir (+ shared read-only `own-clone-analysis/`, clip-store originals). Shared crate APIs negotiated via `galion-board` JSON schema dropped in `scoreboard/schema/`.

---

## 4. Prioritized steps

### P0 — do now (Emotion = #1; mouth + HF also P0; all Rust)

| ID | Step | Workstream | Concrete fix | Measurable success |
|---|---|---|---|---|
| **P0.E1** | **Stand up `galion-emotion`** (ONNX SER + A/V/D + acoustic proxies) | **5 emotion-score** | Export/run ER ONNX via `ort`; proxy features; JSON schema for board | Scores all 24 V1 clips; Sam `ang` rate reproduced within reason; no Python at runtime |
| **P0.E2** | Emotion baseline vs refs + line targets (t06/t07 valence/`hap`) | **5 emotion-score** | `baseline/emotion_v1.json` | Checked in; gaps listed for Sam ang-skew + flat emo lines |
| **P0.E3** | **Stand up `cbx-emo`** — exaggeration/conditioning + sampling hooks on `cbx-v2` | **6 emotion-fix** + 4 pipeline | Wire knobs or document Turbo gaps; seed/N candidates | Can produce N≥4 variants per line |
| **P0.E4** | **Emotion-aware best-of-N** ranked by `galion-emotion` with ECAPA/artic guardrails | **6 emotion-fix** + 3/7/8 | N=4..8; promote only if emo↑ and ECAPA ≥ base−0.02 | Sam `ang` ≤4/12; t06/t07 valence↑; Jarvis `neu` preserved; ECAPA no >0.03 drop |
| **P0.E5** | Score rep-penalty 1.0 vs 1.2 with **emotion metrics** (not only ECAPA) | 5 + 6 + 3 | A/B table on t06–t09 | Winner chosen on emo+ECAPA jointly |
| P0.A1 | Articulation baseline via **`galion-artic`** on V1 WAVs | **8 articulation** | MCD-DTW, F1–F3, vowel-space, rate, fricatives, bursts, durations → `baseline/articulation_v1.json` | Gaps listed |
| P0.A2 | Close artic. gap (rate toward ref; HF estimate without killing intimacy; emo best-of-N also ranks MCD) | 8 + 6 + 2 | Optimize MCD/formants without ECAPA regression | §5 artic targets |
| P0.A3 | Phonetically balanced owner script (done under plan/scripts; refine if needed) | **1 plan** + owner | `plan/scripts/PHONETIC-BALANCED-PACK.md` | Covers vowels/fricatives/stops/numbers/soft emotion |
| P0.H1 | Sam HF conditioned copies via **`refprep-rs`** (equal P0 with mouth) | **2 hf-samantha** | Denoise/click/loudness + estimate-only HF; label synthetic | f99 report; no claim of true >3 kHz recovery |
| P0.R1 | Scaffold **`cbx-v2`**, **`galion-ecapa`**, **`galion-hygiene`**, **`galion-board`**, **`simsel-rs`**, **`refprep-rs`** | 4,7,9,10,3,2 | Build on box; board schema | One WAV scored end-to-end through board |
| P0.R2 | Wire board aggregation + ≥5× bench vs Python analysis path | **10 scoreboard** | `galion-board --bench` | Wall-time table in `scoreboard/NOTES.md` |
| P0.R3 | `refprep-rs` loudness/click/soxr for both personas (copies only) | 2 + 4 | Rust-only conditioned refs | Originals byte-identical |
| P0.1 | Pin fp32 on all premium own-voice synth wrappers / CLI invocations | pipeline | Force `--dtype fp32` (or equivalent) everywhere own-* is called; add guard that fails loud on fp16 | Spot ECAPA does not regress to ~0.54 Sam fp16 cliff; wrapper test asserts dtype |
| P0.2 | True-peak limiter + mild de-click on **synth outputs** (and optionally on conditioned ref copies) | pipeline | Post-process: true-peak ≤ −1.5 dBTP; repair Δ>\|0.35\| clicks; never write back to originals | Peak 0 dBFS hits → **0/12** both voices; click count ↓ ≥50% vs baseline 32/39 |
| P0.3 | A/B `repetition_penalty` **1.0 vs 1.2** on t06–t09 | 3 simsel + 6 cbx-emo + 5 emotion-score | Seeded/best-of runs; log **emotion**, ECAPA, rate, F0, hygiene | Winner on emo+ECAPA; no WER↑; noted in similarity + emotion-fix |
| P0.4 | Loudness-normalize **conditioned ref copies** to ≈ −3 LUFS / −1.5 dBTP before any V2 profile rebuild | pipeline + HF | New files under workstream dirs only (e.g. `conditioned/reference_v2.wav`) | Cond. peak ≤ −1.5 dBTP; RMS/LUFS in target band; originals byte-identical |
| P0.5 | Gentle denoise + click-clean on conditioned Samantha ref copy (estimate HF restore only if reversible / documented) — **P0 equal priority with articulation** | HF | Transparent denoise; optional mild HF shelf **estimate** — label clearly as synthetic presence, not recovered content; prefer restores that help **fricative centroids** without fake air | Cond. NF better than −52 dBFS without musical-noise; f99 report before/after; **do not claim recovery above ~3 kHz**; artic. fricative centroid moves toward Jarvis-like clarity only if estimate is documented |
| P0.6 | Baseline freeze | all | Copy/symlink inventory of V1 profiles + score snapshot from `own-clone-analysis/scores/summary.json` into each workstream `baseline/` | V1 ECAPA/MOS numbers reproducible; V1 profiles untouched |

### P1 — next (may need owner recording or small eng)

| ID | Step | Workstream | Concrete fix | Measurable success |
|---|---|---|---|---|
| P1.1 | **Owner re-record Samantha** with more HF/presence **and** clearer articulation (quiet room, decent condenser; intimate character OK but brighter; follow P0.A3 phonetic script) | needs-owner-recording | New consent-scoped upload → new clip-store entry; rebuild `own-samantha-v2` via `cbx-v2`/`refprep-rs` | f99 **≥ ~6–7 kHz**; energy >4 kHz **≫ 0.4%**; Sam ECAPA **≥ 0.78** (stretch ≥ 0.80); SQUIM gap ≤ 0.35; MCD-DTW and vowel-space improve vs V1 baseline |
| P1.2 | Multi-clip **phonetically balanced** reference pack (neutral, question, numbers, soft emotional, **vowel/consonant coverage**) for both personas | similarity + articulation | Pack layout + `simsel-rs` rebuild recipe; checklist from P0.A3 | Pack ≥4 clips / persona (≥60 s total preferred); Jarvis t10 ECAPA **≥ 0.70**; Sam min **≥ 0.70**; stable per-vowel F1–F2 estimates (n≥5 tokens/vowel where possible) |
| P1.3 | Optional slight post-synth rate / F0 match toward ref (time-stretch / pitch shift within small bounds) | pipeline + articulation | Target rate within ~5% of ref w/s; F0 median within ~0.5 st of ref; keep WER | Sam rate ≤ 3.0 w/s (vs 3.49); F0 median within 0.5 st of 178 / 129 Hz; ECAPA not regress >0.02 |
| P1.4 | Improve energy dynamic range (less brickwall feel) via gentler limiting + optional mild expansion toward ref | pipeline + hygiene | Dyn range closer to ref (Sam ~22 dB, Jar ~13 dB) without new peaks | Sam energy dyn range ≥ 19 dB; Jar ≥ 12 dB; still 0× 0 dBFS |
| P1.5 | Reduce Samantha SER `ang` skew (spectral tilt / transient softness) | HF + expression | Soften excessive HF boost if any; prefer presence restore that matches intimate tone | Clone `ang` tags ≤ 4/12; match-ref emotion frac ≥ 0.5; do not over-brighten |
| P1.6 | Verify PerTh watermark on cbx outs; document presence/absence; add call only if product requires | pipeline | Hex/scan or perth detect on sample WAVs; optional explicit perth apply post-synth | Written verify result; if required, 12/12 watermark-detectable without MOS drop >0.1 |
| P1.7 | Replace linear resample in conditioned-ref path with high-quality resampler (soxr/sinc) to avoid ref clicks | pipeline | Samantha conditioned ref clicks → 0 (original had 0) | Cond. ref click count = 0; no new HF imaging artefacts |
| P1.8 | Expose or fork-wrap `repetition_penalty` + optional seed for reproducible A/B | pipeline + emotion-fix | CLI/env flags documented in pipeline notes | Same text × same seed → bit-similar where decoder allows; A/B tables checked in |

### P2 — later / optional

| ID | Step | Workstream | Concrete fix | Measurable success |
|---|---|---|---|---|
| P2.1 | Evaluate full Chatterbox (non-Turbo) or Nano Python offline premium path | pipeline | Side-by-side n≥6 lines; do not swap default live path | Document ΔECAPA / ΔMOS / latency; promote only if Sam ECAPA ↑ ≥0.03 and latency acceptable offline |
| P2.2 | Upstream/fork leftovers beyond `cbx-emo` Turbo limits | emotion-fix + pipeline | Only if Turbo export blocks P0.E3 | Emotional lines improve further without WER↑ |
| P2.3 | Piper student on own-voice | — | **Out of scope until new consent + GPU + hours of audio** | N/A — blocked |
| P2.4 | Room/mic matching (carry Jarvis RT60 / Samantha intimacy) | — | Not recoverable as true room clone from Turbo; optional light convolution only if owner wants aesthetic, not identity | Aesthetic A/B only; must not hurt ECAPA |

---

## 5. Success criteria (sprint-level)

Re-run the same 12-line harness (`own-clone-analysis` texts/protocol) after V2 changes. **Emotion metrics are gate #1.**

| Metric | V1 baseline | V2 target (stretch) |
|---|---|---|
| **Emotion (P0 #1)** Sam SER `ang` count | 8/12 | **≤ 4/12** via `galion-emotion` |
| **Emotion** Sam match-ref label frac | 0.25 | **≥ 0.50** |
| **Emotion** t06/t07 valence or `hap`-lean | flat / no hap | Both lines valence **> V1** and ≥ one `hap`/`neu` not `ang` |
| **Emotion** Jarvis `neu` dominance | 12/12 | **≥ 10/12 `neu`** (keep assistant affect) |
| **Emotion** Rust vs Python SER agree | — | **≥ 80%** on high-conf clips |
| Samantha ECAPA mean | 0.707 | **≥ 0.76** without new recording; **≥ 0.80** with better Sam recording (ceiling 0.828) |
| Jarvis ECAPA mean | 0.733 | **≥ 0.75** and min ≥ 0.68 (esp. t10); respect ~0.725 short-half ceiling without longer refs |
| Samantha SQUIM MOS | 3.73 (ref 4.33) | ≥ 3.95; gap to ref ≤ 0.45 |
| Jarvis SQUIM MOS | 4.14 (ref 4.55) | ≥ 4.25; gap to ref ≤ 0.35 |
| Peak 0 dBFS hits | Sam 4/12, Jar 1/12 | **0/12** both |
| Click events | 32 / 39 | ≤ 50% of baseline |
| Speaking rate vs ref | +24% / +26% | within **+10%** of ref after expression pass |
| F0 median vs ref | +23 / +14 Hz | within **~0.5–1 st** |
| Samantha SER `ang` | 8/12 | ≤ 4/12 |
| WER | 0.05 / 0.018 | no regression (Sam ≤0.07 mean, Jar ≤0.03) |
| Originals | — | byte-identical / mtime unchanged |
| Piper own-voice | — | still not trained |
| **MCD-DTW** (clone vs ref, same transcript) | *(measure in P0.A1)* | **≤ 85% of V1 baseline** MCD (lower better); stretch ≤ 75% after owner phonetic re-record |
| **F1–F3 per vowel** (abs mean error vs ref) | *(P0.A1)* | Mean abs error **≤ 10%** of ref formant (F1/F2 primary); F3 best-effort under Sam HF limit |
| **Vowel-space area** (F1–F2 hull) | *(P0.A1)* | Within **±20%** of ref area (avoid collapsed “mumble” space) |
| **Articulation rate** | *(P0.A1)* | Within **±10%** of ref (alongside word-rate target) |
| **Fricative centroid / plosive bursts** | *(P0.A1)* | Centroid bias vs ref **reduced ≥30%**; burst presence recall improves vs V1 notes |
| **Phoneme duration RMSE** | *(P0.A1)* | **≤ 85% of V1** RMSE vs ref align |
| Visual lip-sync | N/A (no video) | **Not scored** — document as out of scope |
| Rust↔Python metric agreement | — | ECAPA ±0.01; MCD ±5% rel; formants ±10%; artic rate ±5%; clicks ±1 (§0A) |
| Score wall-time (full board, 12×2) | Python analysis path | **`galion-board` ≥5×** faster (excl. downloads) |
| Emotion score latency | — | **< 200 ms**/utt warm CPU target |
| Hot path | Mixed / Python analysis | **`cbx-v2` + `cbx-emo` + `galion-{emotion,ecapa,artic,hygiene,board}` + `refprep-rs` + `simsel-rs` only** |

---

## 6. Limited by original recordings

These limits **cannot** be fully fixed by pipeline or decoder work alone:

1. **Samantha HF content above ~3 kHz cannot be truly recovered** — only estimated/hallucinated. Original f99 ≈ 2.7 kHz and ~0.4% energy >4 kHz. Any “HF restore” on a conditioned copy is a **synthetic presence estimate**, not restoration of lost mic/chain information. True fix = **re-record** (P1.1).
2. **Single 16–17 s clips** per persona. Half-vs-half ceilings (Sam 0.828, Jar 0.725) and hard-line failures (Jarvis numbers 0.610, Samantha emotional 0.662) reflect thin phonetic/style coverage. Multi-clip packs need **owner recording**.
3. **Scripted character reads** (Her-style intimate monologue; Jarvis assistant announcement) — not casual chat, **not phonetically balanced**. Prosody, SER, **and articulation stats** (unstable F1–F3 / few fricative tokens) stay biased until owner records the P0.A3 phonetic pack.
3b. **Mouth expression is audio-proxied only.** No video ⇒ no true lip-sync / facial expression score. Acoustic formants/MCD can disagree with perceived “mouth acting” in edge cases; say so in reports.
3c. **Samantha HF loss caps consonant clarity metrics** (fricative centroids rely on energy ≫3 kHz). Articulation P0 can measure and partially improve timing/formants; it cannot invent missing HF detail in the original.
4. **Samantha noise floor (−52 dBFS)** and dual-mono HF-limited chain set a ceiling on encoder cleanliness even after gentle denoise.
5. **Turbo architecture** (few-step decoder, argmax LM, fixed export) caps naturalness/expression vs a full generator; optional non-Turbo path is P2 and offline-only.
6. **Consent boundary:** local zero-shot conditioning only — no Piper distillation of these voices, so latency-oriented student quality is unavailable for own-voice.
7. **Room identity** (Jarvis RT60 ~0.85 s, Samantha dry intimacy) is not faithfully reproduced by mono 24 kHz Turbo outs; do not promise environmental cloning.

---

## 7. Explicit non-goals (this sprint)

- Mutating `clip-store-own/*/original.wav` or owner `Samantha.wav` / `Jarvis.wav`
- Overwriting V1 `own-samantha` / `own-jarvis` without owner promotion
- Shipping anything to external channels (the laptop GPU instance is approved)
- Piper fine-tune / corpus expansion with own-voice
- Changing consent documents or claiming new consent
- Work outside analysis recommendations + owner steering on articulation/Rust (no new personas, no unrelated cbx refactors)
- Claiming visual lip-sync or facial match without video
- Python in the V2 promote / hot path (offline SQUIM/legacy SER/ASR cross-check only; **V2 SER is `galion-emotion` ONNX**)
- Shipping emotion fixes that skip `galion-board` promote gates

---

## 8. Coordination checklist for parallel agents

1. Read this plan before writing code or audio.
2. Write only under your workstream directory (+ read-only use of `own-clone-analysis/` and clip-store originals).
3. Put conditioned refs, A/B WAVs, and score dumps under your dir’s `out/` or `conditioned/`.
4. Name any new profiles `own-samantha-v2` / `own-jarvis-v2` (or dated suffix) — leave V1 intact.
5. After local success, drop a short `NOTES.md` in your workstream with before/after numbers.
6. Do not merge into live default voice until owner reviews V2 scores against §5 (incl. articulation + Rust agreement benches).
7. Prefer crates in §0A; if you prototype in Python, **port before promote** (emotion especially).
8. Stream 5/6 own emotion; 8 owns mouth; 2 owns Sam HF — do not skip P0.E1–E4 or P0.A1 / P0.H1.
9. `galion-board` is the only promote authority; emo-weighted rank required.

---

## 9. Traceability

| Plan section | Analysis source |
|---|---|
| §1.1–1.6 scores | REPORT §1–2; `scores/summary.json` |
| §1.7 env | REPORT §3; `scores/ref_env_analysis.json`, `recording_env.json` |
| §1.9–1.10 bottlenecks | REPORT §4 |
| §2 suggestions | REPORT §5 |
| Benchmark lines | `benchmark_manifest.json`, `text/t01..t12.txt` |
| §0A Rust-native + 10 streams | Owner steering t735u + **t736u** |
| §1.11 / P0.A* articulation | Owner steering t734u (audio-only mouth-expression proxies) |
| P0.E* emotion | Owner steering **t736u** (emotion TOP; Rust score+fix) |

---

## 10. Owner-recording brief (articulation + HF)

When the owner can record again, ask for **both** personas (Samantha priority):

1. **Gear / room:** quiet room; condenser or good headset mic; no heavy foam/low-pass; leave headroom (~−6 dBFS peaks); 48 kHz mono or stereo OK.
2. **Samantha HF:** brighter presence than current f99≈2.7 kHz take; keep intimate tone.
3. **Phonetically balanced pack** (read naturally, not hyper-articulated unless marked):
   - Vowels: minimal pairs / carrier sentences covering /i ɪ e æ ɑ ɔ ʊ u ə ɚ/
   - Fricatives: /s ʃ f θ v z/ in initial/medial/final
   - Stops: /p t k b d g/ with clear bursts
   - Numbers + phone-style strings (fixes Jarvis t10 gap)
   - Soft emotional + neutral + question (existing categories)
4. **Length:** prefer **≥60–90 s** usable speech / persona across clips (multiple short clips OK).
5. **Consent:** same local zero-shot conditioning scope; no Piper training.
6. **No video required** for this sprint’s metrics (lip-sync remains unscored).

---

## 11. Plan finish checklist (stream 1)

- [x] All analysis discoveries inventoried (§1)
- [x] All suggestions mapped (§2) including emotion S13/S14
- [x] Ten Rust workstreams named with crates (§0A, §3)
- [x] Emotion = P0 #1 with scorer + fixer steps (P0.E1–E5)
- [x] Mouth/articulation + Samantha HF = P0
- [x] Success criteria include emotion gates + Rust agreement + wall-time (§5)
- [x] Limited-by-recordings + hard constraints stated
- [x] Owner phonetic pack at `plan/scripts/PHONETIC-BALANCED-PACK.md`
- [x] Non-goals / coordination / traceability updated for t736u

*End of Cloning V2 plan (finished for t734u/t735u/t736u). **P0 #1 = emotion** (`galion-emotion` + `cbx-emo`); mouth + HF P0; ten Rust workstreams. Respect §0 / §0A / §6.*
