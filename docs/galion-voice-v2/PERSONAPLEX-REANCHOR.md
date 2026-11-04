# V2 re-anchor: a PersonaPlex-style integrated dual-band model, with RL

Status: **plan only** (owner item 9, 2026-11-04 16:01 CET). No code, no weights, no training run.
Every number below is either measured on the sprint box (the source is named) or published by
NVIDIA or Kyutai (cited). Anything else is marked as **arithmetic**: an estimate derived from those
numbers, not a measurement.

## 1. What "PersonaPlex-style" means here

PersonaPlex is NVIDIA's 7B full-duplex speech-to-speech model, released 2026-01-15, built on Kyutai's Moshi
architecture and Moshiko weights:

- **Inputs:** three streams at once: user audio, agent text, and agent audio.
- **Codec:** Mimi turns audio into tokens at 24 kHz and 12.5 Hz.
- **Model:** a temporal transformer steps once per frame, and a depth transformer emits that frame's
  codebook tokens.
- **Persona prompt:** a "hybrid system prompt", a short voice sample on the agent-audio channel plus a role text on
  the agent-text channel. This gives zero-shot voice and role control.

Sources: arXiv 2602.06053, the HF card for `nvidia/personaplex-7b-v1`, and Moshi arXiv 2410.00037. The card lists
the NVIDIA Open Model License plus CC-BY-4.0, PyTorch on A100/H100, and Fisher English as training data. The paper also
uses a large synthetic dialogue set. The paper does **not** describe an RL stage, so the RL below is ours.

Galion already has the shape this needs:

- **Clock:** `LOCKS.toml [clock] frame_ms = 80`, which is exactly Mimi's 12.5 Hz.
- **Bands:** `[bands]` = `you` / `me` / `feeling`, `duplex = "full"`.
- **Frame loop:** `galion-heartbeat` runs one non-blocking `step()` per frame.
- **Existing crates:**
  - `galion-mimi`: real kernels, no weights, `IS_STUB = true`;
  - `galion-tensor`: own CPU runtime, `.gq` later;
  - `galion-student-backend`: slot only, `LoadError::NotPlaced`;
  - `galion-student/`: the distillation plan, PersonaPlex-7B teacher, owner pick S0 first.

"Dual-band" in this doc means the model consumes the **you** band (user audio) and produces the
**me** band (agent text plus agent audio tokens) in the same step. **feeling** stays Galion's own 8-byte
EmotionWire side channel (Rika), and the model does not replace it.

## 2. What changes vs today's cascade, and what stays

Today's live path (PRs #62–#69) is a cascade:

1. Vosk ASR;
2. endpoint (about 140–160 ms after end of speech);
3. LLM (cloud, or local Qwen2.5-1.5B in #69);
4. clause buffer;
5. Piper TTS;
6. a separate backchannel path.

| Piece | Cascade today | Re-anchored V2 |
|---|---|---|
| Listening | Vosk transcribes, then the endpointer decides the turn is over | The model reads Mimi codes of the you band every 80 ms. No hard endpoint. |
| Turn-taking, barge-in, backchannel | Rules (endpoint timer, barge-in gate, canned backchannel clips) | Learned behaviour of the model (when to start, yield, or say "mm-hm"), checked against the same locks: `barge_in_max_ms = 200` |
| Reply content | Text LLM, prompted per persona | The model's agent-text stream (Moshi's "inner monologue"), conditioned on the persona text prompt |
| Speaking voice | **Piper** (A21 pin), or the Chatterbox clone slot | **Piper stays the TTS engine** (owner ruling), see the decision below |
| Persona voice prompt | Chatterbox conditioning (PR #71 caches it) | Only if decision D1 = B. Under D1 = A, Piper's voice pack is the voice. |
| Emotion | Rika / EmotionWire, prosody to Piper and clone | Unchanged. The feeling band still drives prosody and gain. It becomes an extra conditioning input to the model (student `aux_heads = ["turn_state","affect"]` already planned). |
| Consent / locks | galion-consent gate, `reference_clips = false`, `[consent]` default deny | Unchanged. The model never sees a voice prompt the gate did not approve. |

**Decision D1 for Mate: who makes the sound?** A PersonaPlex-style model emits its own audio tokens,
which Mimi decodes. That conflicts with "Piper stays the TTS engine" unless we split roles:

- **A (recommended first, keeps the ruling):** the model decides *what* and *when*. It produces agent-text tokens plus
  turn and backchannel timing on the 80 ms clock. Piper renders the text clause by clause, as today. The
  model's audio head is trained (it helps the model learn timing) but is not played.
  - Gain: no endpoint wait, learned barge-in and backchannel.
  - Not gained: no sub-word overlap speech, and Piper TTFA (about 120–160 ms p50 in the loop, below) stays in the path.
- **B:** play the model's own Mimi audio, and keep Piper as the fallback lane (as `[formats.exceptions.a21_piper]`
  already says for clone). This changes the owner ruling, so it is Mate's call. It also needs Mimi weights
  (licence, A21) and the voice-prompt consent path.

## 3. RL approach

Training order (extends `galion-student/configs/distill.toml`):

1. **Distil** (stages 0–5 as planned): KD from the teacher, then duplex KD, then persona KD, then Q8 QAT.
   - Teacher options: PersonaPlex-7B, or Moshi as the CC-BY fallback; the licence check is risk R1, already in `distill.toml`.
   - This stage is supervised and RL-free.
2. **Offline preference stage (DPO-style).** For each simulated dialogue context, sample several student
   continuations and rank them with the programmatic rewards below. Train on chosen/rejected pairs.
   - Cheap, stable, and no on-policy rollouts needed.
3. **On-policy RL (GRPO-style group-relative advantages, no learned critic).** Run full-duplex rollouts against the
   loopback virtual user (`scripts/eval/virtual-user.sh`, `v2-hard-loop.sh`), extended to speak over and pause
   like Full-Duplex-Bench scenarios.
   - Rewards come per 80 ms step where possible (turn events), and per turn for content.
   - A KL penalty to the stage-2 policy keeps speech quality from drifting.

Rewards. All are programmatic and logged per frame, and no reward is a model grading itself alone (EUREKA rule 8):

| Reward | Signal | Source of truth |
|---|---|---|
| Yield on barge-in | Agent audio or text stops within `barge_in_max_ms = 200` of user onset | LOCKS gate, frame timestamps |
| No takeover on pause | Agent does not start in a user pause inside a turn (Full-Duplex-Bench "pause handling" TOR) | Scenario labels |
| Smooth turn-taking | Start latency after real turn end, penalised when late or early | Scenario labels |
| Backchannel timing | Backchannels land in listener slots, not over content | Scenario labels |
| Content | Agent text matches the persona prompt, grounded, short (the persona 1–2 sentence limit, which Qwen-1.5B breaks today) | Rubric checks plus an outer human-calibrated anchor (EUREKA rule 8) |
| Text/audio agreement (D1 = B only) | ASR of the model's own audio equals its text stream | Vosk on box |
| Voice match (D1 = B only) | Speaker-embedding cosine to the **approved** reference | ECAPA, only when the consent gate passed |

Hard exclusions (`specs/presence.json`): **no reward** for engagement, retention, session length, dwell, or speech
elicited. A reward that correlates with them is a bug. The feeling band may condition behaviour, but no reward may
make the model steer the user's affect.

Data: synthetic dialogues generated with open models plus Piper, the same way PersonaPlex built its set
(arXiv 2602.06053 §3), and no NC data (`galion-student/configs/data.toml`). Fisher English (PersonaPlex's real-data
source) is LDC-licensed, so it needs counsel before use.

## 4. CPU feasibility (8-core AVX-512, 16 GB, no GPU)

Measured on this box:

- `galion-probe/local-baseline-grok-bot-vm.json`, 2026-11-02:
  - STREAM triad 13.1 GB/s single-thread, 98.0 GB/s best multi-thread;
  - runs 63.7 / 61.6 / 98.0 GB/s, mean 74.4, CV 27 %, so the G0 gate FAILs on stability;
  - 15.6 GiB RAM, no swap.
- llama-bench, Qwen2.5-1.5B Q4_K_M (1.12 GB file), 2026-11-04 16:04 CET:
  - decode 21.7 ± 1.3 tok/s at 4 threads, and 22.9 ± 15.7 at 8 threads;
  - prefill 303.6 tok/s at 4 threads, 112.5 at 8 threads.
- Same model in the live loop on all 8 cores, 2026-11-04 (#69 doc):
  - decode p50 18–24 tok/s;
  - Piper first audio while the LLM decodes, p50 117–161 ms.

**Arithmetic** from those numbers (not measured):

- **Effective bandwidth.** CPU decode reads the whole weight set per step. Qwen-1.5B Q4 at about 22 tok/s × 1.12 GB
  is roughly 25 GB/s effective, about a third of the probe's mean triad.
- **Model sizes.** A frame step must finish 12.5 times per second, and `[probe] realtime_headroom = 2.0` asks for 25.
  - **PersonaPlex-7B is not feasible on this box.** BF16 weights are about 16 GB (the card says 8B params), which does
    not fit next to the OS. At int8 (about 8 GB), 12.5 steps/s needs about 100 GB/s before the depth transformer,
    which is 4× the effective rate measured above.
  - **Student S0** (`student.toml`: 16 layers, d_model 1536, depth 3 × 512): roughly 0.5 B params in the temporal
    transformer, so about 0.5 GB at int8. At 25 GB/s effective that is about 50 steps/s for the temporal part.
    - This is the first size worth measuring.
    - The depth transformer adds one small step per codebook per frame, and its cost must be measured, not assumed.
  - **S1** (about 1 B) at int8 sits near the 25 steps/s headroom line on this estimate. **S2** does not fit the headroom.
- **Sharing the box.** The model replaces Vosk and the LLM but (under D1 = A) not Piper. The #69 re-measure shows
  that 8 LLM threads on 8 cores slow Piper (p95 262 vs 166 ms at 4 threads). A core split has to be part of
  the gate.
- **Training is not feasible on this box.** KD and RL need GPUs. This needs a budget decision (D3).

## 5. Milestones (each closes only on a measured gate written to `symphony/SELF-EVAL.md`)

| M | Deliverable | Gate |
|---|---|---|
| M0 | This plan, plus owner decisions D1–D4 | Mate signs off |
| M1 | Turn-event harness: Full-Duplex-Bench-style scenarios (pause, backchannel, smooth turn, interruption) on the loopback virtual user, scored for **today's cascade** as the baseline | Baseline TOR and latency numbers recorded for the cascade, on the box |
| M2 | `galion-mimi` + `galion-tensor` run a weight-loaded (licence-cleared) Mimi encode, and a random-init S0 temporal + depth step in Rust, timed | Measured S0 step time on 8 cores with Piper running: ≥ 25 steps/s, or S0 is cut |
| M3 | Distilled S0 (stages 0–5) off-box, exported as safetensors / `.gq` | Beats nothing yet. Must reproduce the teacher's turn events on M1 within a set tolerance |
| M4 | DPO stage, then GRPO stage, with the reward table above | Better than M3 and the cascade on M1, `barge_in_max_ms` holds, presence bans hold, outer human anchor agrees |
| M5 | D1 = A in the live loop: S0 drives text and timing, Piper speaks, cascade as fallback lane | Live HARD-LOOP: eos→first content audio and barge-in no worse than the cascade's measured numbers |
| M6 | (only if D1 = B) model audio plays, Piper is the fallback | Voice match on approved refs only, consent gate holds |

## 6. Decisions for Mate

- **D1:** Who makes the sound (A: Piper, keeps the ruling; B: model audio, Piper fallback).
- **D2: Teacher licence.** NVIDIA Open Model License (PersonaPlex) vs Moshi as the CC-BY fallback. Fisher data needs counsel.
  Mimi weights need A21 clearance.
- **D3: GPU training budget.** The box cannot train. No figure is given here because none was measured or quoted.
- **D4: Lock conflict in today's path.** `[formats]` forbids `llama`/`ggml` runtimes and GGML containers. The local
  Qwen GGUF via llama-server in #69 is outside that lock. It stays a measurement/fallback experiment unless Mate
  writes an exception. The re-anchored student would run on `galion-tensor` with safetensors / `.gq`, inside the lock.
