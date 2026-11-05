# TTS stack alternatives: skeleton for the owner to fill in

> Template only. Headings, prompts, empty tables and open questions. No answers or recommendations are filled in.
> Supporting research (numbers and sources) is in `P95-TAIL-RESEARCH.md` in this folder.

---

## 0. Context and current baseline

_Prompt: what the stack is today and what "good enough" means for this decision._

### 0.1 Current stack (reference)

| Layer | Current choice |
|---|---|
| Endpointing | Energy VAD, fixed 160 ms silence hang |
| ASR | Vosk streaming sidecar |
| LLM | llama-3.1-8b-instruct via OpenRouter, provider Groq |
| TTS | Piper en_US-ljspeech-medium, piper 2023.11.14-2, onnxruntime 1.14.1, espeak-ng phonemizer |
| Host | 8-core Xeon (AVX-512), 16 GB RAM |

### 0.2 Baseline numbers (reference, 24 runs)

| Metric | p50 (ms) | p95 (ms) | Notes |
|---|---|---|---|
| Full loop: end of speech → first content audio | 608.3 | 775.1 | |
| End of speech → endpoint | 130.2 | 140.3 | fixed 160 ms hang |
| ASR final (increment after endpoint) | 19.9 | 32.7 | |
| Gap: ASR final → LLM request | 67.9 | 112.6 | |
| LLM TTFT | 284.7 | 498.0 | range 178.7–513.9; one spike 1823.1 (run 4) |
| First token → first clause | 20.6 | 44.1 | |
| Piper TTFA in loop | 71.3 | 181.8 | |
| Piper cold spawn → first PCM | 334.3 | 440.3 | |
| Piper warm TTFA (1w / 4w / 8w / 20w) | 25.9 / 42.3 / 74.2 / 131.9 | – | p50 |

Tail runs: 9 (LLM 513.9), 13 (Piper 195.8, 6 words), 0 (Piper 188.5, first run), 12 (12-word clause, Piper 144.2), 4 (LLM spike 1823.1).

### 0.3 Targets

| Metric | Target | Why |
|---|---|---|
| Full-loop p50 | | |
| Full-loop p95 | | |
| Full-loop p99 / max | | |

### Open questions
1. What p50 / p95 targets count as "done" for V2?
2. How many runs (and which clips/languages) make a trustworthy p95 measurement?
3. Is the host fixed at 8 cores / 16 GB, or can the hardware change?
4. Which languages must ship (English only, or also Croatian)?
5. Is voice cloning a hard requirement for the TTS path that speaks first?

---

## 1. Direct Linux TTS libraries vs Piper/ONNX

_Prompt: could a native C library called from Rust via FFI (no ONNX Runtime) replace or supplement Piper, and at what cost?_

### 1.1 Candidates

_Prompt: list each engine to evaluate and how it would be called from Rust._

- espeak-ng (direct C API)
- RHVoice
- Festival
- Flite
- Others: _(fill in)_
- Baseline for comparison: Piper (ONNX)

### 1.2 Comparison

| Engine | Latency (TTFA / RTF) | Quality | Voices / Croatian support | Cloning compatibility | License | Rust FFI effort |
|---|---|---|---|---|---|---|
| Piper / ONNX (baseline) | | | | | | |
| espeak-ng direct | | | | | | |
| RHVoice | | | | | | |
| Festival | | | | | | |
| Flite | | | | | | |
| _other_ | | | | | | |

### 1.3 Where it would sit in the pipeline

_Prompt: full replacement, first-clause-only fast voice, backchannel/filler only, or fallback when Piper is slow?_

### 1.4 Measurement plan

_Prompt: which texts, which metric (TTFA, RTF, p95), on which host, with what CPU budget._

| Test | Engine | Text / length | Runs | Result |
|---|---|---|---|---|
| | | | | |

### Open questions
1. Is a robotic or parametric voice acceptable for any part of the reply (e.g. first clause, backchannel)?
2. Can licenses like GPL-3, LGPL with GPL-3 MAGE, or CC BY-NC-ND voices ship in this product?
3. Does mixing two voices (fast engine, then Piper) in one reply sound acceptable?
4. How much of the p95 tail would this remove, given Piper is only one slice of the loop?
5. Do we need Croatian TTS now, and which engines offer a usable Croatian voice?
6. Who owns building and maintaining the C FFI bindings?

---

## 2. Local LLM options to replace llama-3.1-8b on Groq

_Prompt: can a local model on this CPU cut first-word latency and its variance without hurting reply quality?_

### 2.1 Candidates

| Model | Runtime | TTFT target | RAM | Quality | License |
|---|---|---|---|---|---|
| | | | | | |
| | | | | | |
| | | | | | |
| | | | | | |

_Prompt: runtimes to consider, e.g. llama.cpp (C API or Rust bindings), candle, mistral.rs._

### 2.2 Deployment shape

_Prompt: full replacement, first-sentence-only local model plus cloud continuation, or local fallback when the cloud is slow?_

### 2.3 CPU budget

| Component | Cores | Notes |
|---|---|---|
| LLM | | |
| Piper / TTS | | |
| ASR | | |
| Headroom | | |

### 2.4 Quality evaluation plan

_Prompt: which prompts, which judge, what pass bar vs llama-3.1-8b._

### Open questions
1. What reply quality is acceptable vs llama-3.1-8b (persona, refusals, coherence)?
2. Can the LLM share an 8-core host with Piper and ASR without moving the tail into CPU contention?
3. Is a hybrid acceptable (local first sentence, cloud for the rest)?
4. What RAM budget is available for model weights plus KV cache?
5. Which model licenses are acceptable for shipping?
6. Must the LLM speak Croatian?

---

## 3. Other approaches to the p95 tail

_Prompt: for each lever, the expected gain, the risk, and the effort._

| Approach | Targets which stage | Expected p50 gain | Expected p95 gain | Risk / side effect | Effort | Decision |
|---|---|---|---|---|---|---|
| Speculative start on partial ASR | | | | | | |
| First-clause cap | | | | | | |
| Warm connections / prompt prebuild | | | | | | |
| Endpoint hang tuning | | | | | | |
| Hedged requests | | | | | | |
| Filler strategy | | | | | | |
| Piper pre-warm | | | | | | |
| _other_ | | | | | | |

### 3.1 Speculative start on partial ASR
_Prompt: when to fire, when to cancel, cost of wasted requests._

### 3.2 First-clause cap
_Prompt: max words before the first clause is spoken; where to cut._

### 3.3 Warm connections / prompt prebuild
_Prompt: keep-alive or persistent client, what can be built before end of speech._

### 3.4 Endpoint hang tuning
_Prompt: hang value vs cut-in rate; fixed or adaptive._

### 3.5 Hedged requests
_Prompt: hedge threshold, second target (provider or local model), cost cap._

### 3.6 Filler strategy
_Prompt: when a filler/backchannel plays, which phrases, and how it hands over to content._

### 3.7 Piper pre-warm
_Prompt: what to pre-warm (process, session, first clause), when, and CPU cost._

### Open questions
1. Which levers are cheap enough to try first?
2. What cut-in (false endpoint) rate is acceptable if the hang gets shorter?
3. Is paying for duplicate LLM calls (hedging / speculation) acceptable, and up to what cost per turn?
4. Should fillers count as "first audio" for the latency goal, or only content audio?
5. Should the bench be changed so the backchannel no longer blocks the LLM request, before comparing levers?

---

## 4. Decision log and open questions

### 4.1 Decision log

| Date | Decision | Options considered | Rationale | Owner | Revisit by |
|---|---|---|---|---|---|
| | | | | | |

### 4.2 Experiments to run

| Experiment | Hypothesis | Metric | Owner | Status | Result |
|---|---|---|---|---|---|
| | | | | | |

### 4.3 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| | | |

### Open questions
1. Which single metric is the go/no-go for the TTS and LLM choice?
2. Who signs off on license implications?
3. What is the deadline for the V2 latency decision?
4. Which numbers need re-measuring on an isolated host before deciding?
5. What would make us revert a change?
