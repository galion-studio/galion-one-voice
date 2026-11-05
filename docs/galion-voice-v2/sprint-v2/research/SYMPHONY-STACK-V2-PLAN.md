# Symphony Stack V2: plan for Galion One Voice

> **OWNER RULING 2026-11-04 20:45 CET (in force; supersedes everything below that names Qwen, Llama, llama.cpp, Groq or a cloud LLM).**
> No Qwen anywhere in the architecture, no fallback to it. Qwen, Groq and any cloud LLM are third party.
> The reasoning sector is fully local and in-house: the S0 student (~0.5B, `.gq`, W8A8) on `crates/galion-tensor`, with `CannedLlm` until trained weights exist.
> The S0 teacher / training source is an **open owner decision** (no third-party model is assumed).
>
> | core | role | threads |
> |---|---|---|
> | 0 | bus / heartbeat / audio I/O / VAD-endpoint / orchestrator + low-priority context/retrieval worker | 1 |
> | 1 | Vosk ASR | 1 |
> | 2-5 | in-house reasoning (S0 on galion-tensor; CannedLlm until weights) | 4 |
> | 6 | Piper Samantha | 1 |
> | 7 | Piper Jarvis | 1 |
> | — | clone workers | none during a live call |
>
> In code: `swarm/core-map-v3` (PR #68), `CoreRole::{Bus, Retrieval}` 0, `Asr` 1, `Reasoning` 2-5, `PiperSamantha` 6, `PiperJarvis` 7.
> Qwen / Llama / llama.cpp / Groq numbers below are **historical / superseded** measurements, kept for the record only.


Status: **PLAN, not an implementation.** Written 2026-11-03, about 18:50 Europe/Zagreb (CET), for owner request pt602u.
Research was read-only. Nothing was committed, pushed, deployed, restarted, downloaded, built or installed. The only file written is this one.
Licence line for the code this plan proposes: GOATSI Inc. "open perpetual MIT" (see open question Q9 about the copyright line already in the tree).

**How to read the numbers.** Each number carries one of four tags:

- **[M]** measured on this box (a file path is given).
- **[C]** cited from a web source (a link is given).
- **[D]** a design constant taken from the tree (a file and line are given).
- **[E]** an estimate. Every estimate is marked as one. An estimate is not a measurement, and the gates in §6 exist to replace each one.

Repository paths are relative to `/workspace/swarm/voice-v2-sprint/repo`, the `swarm/merge-all-prs` branch at 5e7a923, unless a path is absolute.

---

## 0. The idea in one paragraph

The symphony champion line is "a conversation is one heartbeat, not a pipeline" (`docs/galion-voice-v2/symphony/README.md:10`). V2 of the stack keeps that line and adds three things.

1. **One blackboard.** The existing typed bus becomes a blackboard with lanes, not a broker (`docs/galion-voice-v2/IPC.md:27-92`, `crates/galion-console/src/bus.rs:325`).
2. **One mind with several voices.** Samantha, Jarvis and Galion One are persona *slots* over a single local reply model. Each slot has a pinned KV prefix, prosody and a voice reference. Galion Research (Hermes) is the slow cortex on the side band, never on the tick (`GALION-VOICE-V2.md:706-722`, `crates/galion-sideband/src/lib.rs:21`).
3. **One memory.** A single local store holds everything: SQLite FTS5 plus int8 vectors, with consent gating every write. A prefetcher reads it while the person is still talking, so at end-of-speech the context is already in RAM and mostly already in the KV cache.

The 240 ms target is reachable on the 8-core box only if almost no work is left at end-of-speech. That means:

- speculative prefill on stable partials;
- a persona prefix that is already in KV;
- a 1–2 word first clause;
- warm, pinned Piper;
- no cloud;
- no live neural cloning on the content path. Cloning moves to "clone at rest", see §5.4.

---

## 1. What already exists (inventory)

| Piece | Where | State |
|---|---|---|
| 80 ms heartbeat: 12.5 Hz, 24 kHz, 1920 samples/frame; never blocks or allocates; late frame = pause | `crates/galion-heartbeat/src/lib.rs:1-44`, `LOCKS.toml:21-27` | in tree [D] |
| Three bands (you / me / feeling); EmotionWire 8 B/frame | `LOCKS.toml:30-36` | in tree [D] |
| One-percent account: ≤800 µs per tick; p99 measured 3.2 µs (ratio 249) | `crates/galion-symphony/src/lib.rs:1-45`; `docs/galion-voice-v2/symphony/SELF-EVAL.md:577-610` | PASS on the build host [M] |
| Typed bus envelope `{id, turn_id, t_ms, cancel}`, `try_send`, bounded queues, no broker | `docs/galion-voice-v2/IPC.md:27-37, 81-92`; `bus.rs:325` (`BusBody`: MicLevel, Utterance, PartialAsr, ReplyText, SpeechPcm, OrbState, Consent, BargeIn, Persona) | in tree |
| Persona switch body, single latest-wins slot, `turn_id=0` pin rule, empathy gate e≥0.65 for 8 frames | `docs/galion-voice-v2/SAMANTHA-V2.md:169-308` | spec plus body in tree |
| Side band to Hermes: `ToHermes`/`FromHermes`, `RETRIEVAL_CAP` 3 s, `NoKnowledgeReason` | `crates/galion-sideband/src/lib.rs:21-92`. Line 3 says the C5 gateway and C6 memory surface are still open. | types only |
| Nerve (Open Mind, loopback 8670, NDJSON, 64-line RAM ring) vs cortex (Hermes) | `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md:18-25, 77-81, 125-197, 207-229` | spec plus crate |
| galion-tensor: no third-party runtime; AVX-512 int8 dot (bit-exact); W8A8 gemv with per-row scales; `KvRing` (preallocated, zero allocation per frame; `push`/`clear` only); `.gq` reader | `crates/galion-tensor/src/{lib.rs:1-41, simd.rs:6-10,489, gemv_i8.rs:29-79, kv.rs:13-90, gq.rs}` | in tree; no trained student weights yet |
| Reply intelligence: S0 ≈0.5B `.gq` W8A8 student on galion-tensor (owner YES), cores 2-5; CannedLlm until trained weights (in-house, not a third-party fallback) | `GALION-VOICE-V2.md:341-381` | planned |
| Hermes in the live reply path (`GALION_LLM=hermes`, loopback :8642, 3 s timeout) | PR #23, open | not merged |
| Live dual-band call: Samantha ↔ Jarvis. Duck at 80 ms, pause at 200 ms, yield after 800 ms voiced; cached cloned backchannels; history is the last 12 messages | `/workspace/swarm/voice-v2-sprint/galion-v2-live/src/bin/call.rs:132-157, 248-302` | working prototype (cloud LLM) |
| Open Mind memory: Node `memory-tencentdb` (RSS 614 MB, **no embeddings**, FTS5 BM25) → Rust port `openmind-memory` (rusqlite FTS5, 1.9 MB musl, 9 routes); tiers L0 JSONL turns / L1 records / L2 scene_blocks / L3 persona.md | `obsidian-open-mind` `docs/ws-memory.md`, `docs/memory-api.md`, `docs/memory-schema.md` (user_version 4) | Rust port exists; read via gh |
| openmind-compact hybrid BM25 + SimHash + recency: BM25 ≈0.7 ms per 10k tokens, ≈7 ms per 100k tokens median | `/workspace/swarm/plans/research-context-compaction.md` (table at lines 98-106; KV methods at line 67) | [M] in-house |
| Seam 4 vector store candidates: MindFox, clawmem, memori (FTS5 + 384-d + RRF + decay), veclite | `docs/galion-voice-v2/symphony/PUZZLE-PIECES.md:277-339` | survey |

**Prior symphony designs.**

- Found:
  - `docs/galion-voice-v2/symphony/*`: README, V3-PLAN, PUZZLE-PIECES, EUREKA, SELF-EVAL, STAGING-CPU, PLAN-PLUS, PLAN-ULTRA, ONE-PERCENT.
  - `docs/galion-voice-v2/SAMANTHA-V2.md` §4.
  - `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`.
  - `docs/research/galion-voice-v2/master/GALION-VOICE-V2-RESEARCH-BRIEF.md`.
  - `/workspace/shared/galion-plan-20261101/DESIGN.md`.
  - `/workspace/swarm/plans/{galion-os.md, cortex.md, research-context-compaction.md}`.
- Absent:
  - The GitHub repo `galion-studio/galion-symphony-v2` does not resolve.
  - `/workspace/galion-symphony-v2/V3-PLAN.md`, which `V3-PLAN.md:18` cites, does not exist.
  - `VOICE-SYMPHONY.md`, `PROJECTS.md`, `BOTS-AND-HANDOFF.md` and `GALION-OVERVIEW.md` were not found anywhere. `REPORT.md` §0 says the same.

---

## 2. Interconnect: personalities, voice pipeline, blackboard

### 2.1 Who is who (roles, not processes)

| Persona | Role | Runs where | Hot-path? |
|---|---|---|---|
| **Galion One** | Product voice. Default slot. | Local reply model, slot `g1` | yes |
| **Samantha** | Internal test voice: warmth, feeling band. Picked when the empathy weight e holds ≥0.65 for 8 frames and `sense` is granted (`SAMANTHA-V2.md:279-308`). | Local reply model, slot `sam` | yes |
| **Jarvis** | Internal test voice: tasks and commands. Picked for command intent. | Local reply model, slot `jar` | yes |
| **Galion Research (Hermes)** | Cortex: analysis, tools, long documents. Product voice (`symphony/README.md:41`). | Side band. Off device or a separate process, ≤3 s cap. | **never on the tick** |

Design decision: **personas share one model and differ by prefix, prosody and voice.** Running three concurrent LLM processes on 8 cores would repeat the clone-lane failure. Two Nano workers pushed live RTF from 0.55 to 2.11 (`REPORT.md` §4/§6 [M]). Persona = `{system-prefix KV snapshot, prosody/wash bias, voice ref, backchannel clip set}`. A switch swaps the slot. It does not restart the lane (`SAMANTHA-V2.md:212-221`).

"Talking to each other" means three concrete things:

1. In a dual-band call (Samantha ↔ Jarvis, `call.rs`), the two lanes *hear* each other's band, and the **Conductor** grants the floor (§2.3).
2. All personas **read and write the same blackboard and memory**, each scoped by `persona_scope`. Jarvis can read a fact Samantha learned if consent `share` is granted. It is denied by default (`LOCKS.toml:72-86`).
3. Hot personas can **ask the cortex**. A slot posts `ToHermes`. The answer arrives as `FromHermes` and lands on the blackboard as a `MemoryHit` for a *later* clause or turn. This is the MoshiRAG asynchronous pattern: keep talking with filler or coarse content, then inject the retrieved text when it arrives ([C] https://github.com/kyutai-labs/moshi-rag, https://kyutai.org/blog/2026-04-30-moshi-rag/).

### 2.2 Blackboard = the bus, with lanes

The blackboard is not a new broker. It is the existing `BusMsg` envelope (`IPC.md:27-37`) plus a few new bodies, grouped into three lanes with fixed semantics:

| Lane | Semantics | Bodies (existing + **new**) | Capacity |
|---|---|---|---|
| **Control** (priority) | Latest-wins single slot. Read every tick. Never blocks. | BargeIn, Consent, Persona, OrbState, **Cancel{epoch}**, **Floor{owner, since_ms}** | 1 per body |
| **Stream** (ordered) | Bounded ring. Drop-oldest for PCM; stale-drop where `turn_id < cancel`. | MicLevel, PartialAsr, Utterance, ReplyText, SpeechPcm, **Backchannel{clip_id}** | as `IPC.md:81-92` |
| **Context** (blackboard cells) | Versioned cells keyed by `(session, turn_id)`. Writers replace, readers take an `Arc` snapshot. | **CtxSlab**, **MemoryHit**, **HermesNote**, **TurnSummary** | 4 live turns |

Proposed data contracts. Rust shapes, all `Copy` or `Arc` so readers never clone big buffers:

```rust
// envelope unchanged: BusMsg { id, turn_id, t_ms, cancel, body }
pub struct Epoch(pub u32);                       // bumps on TurnResumed / barge-in yield
pub enum PersonaId { G1, Sam, Jar, Other(u16) }

BusBody::Cancel   { epoch: Epoch }                                   // priority lane
BusBody::Floor    { owner: PersonaId, since_ms: u32, why: FloorWhy } // conductor decision
BusBody::Backchannel { who: PersonaId, clip_id: u16 }                // cached PCM, no LLM
BusBody::PartialAsr  { text: Arc<str>, stable_prefix_len: u16, eager_eot: bool } // add stable_prefix_len/eager_eot
BusBody::CtxSlab  { slab: Arc<CtxSlab> }                             // the assembled prompt tail
BusBody::MemoryHit{ hits: Arc<[Hit; 4]>, query_sig: u64, took_us: u32 }
BusBody::HermesNote { text: Arc<str>, src: SrcRef, age_ms: u32 }     // from FromHermes, ≤80 tokens
BusBody::TurnSummary { who: PersonaId, text: Arc<str> }              // ≤1 line, for other personas

pub struct Hit { item_id: u64, kind: Kind, score: f32, tokens: u16 }
pub struct CtxSlab {
    persona: PersonaId, epoch: Epoch,
    prefix_slot: u8,             // which KV snapshot to fork from
    kv_pos_committed: u32,       // tokens already in KV (persona + history + speculative)
    tail_tokens: SmallVec<[u32; 96]>, // only what still needs prefill
    budget_used: u16,
}
```

The heartbeat rule stays: `Heartbeat::step` **reads** the control lane only and never decides (`SAMANTHA-V2.md:203-207`, `galion-heartbeat/src/lib.rs:1-28`). Context assembly, retrieval and the conductor run on the turn worker and the context worker (§5).

### 2.3 Conductor (floor control, backchannels, barge-in)

The Conductor is a small state machine on the turn worker. It is the merge of `call.rs` behaviour with domain 03 of the research brief:

`LISTENING → SPECULATING → COMMITTED/SPEAKING → (DUCKED) → (PAUSED) → YIELDED → LISTENING` (`docs/research/galion-voice-v2/domains/03-dialogue-bargein.md:46-53`).

- **Barge-in ladder.**
  - Speech under 150 ms is ignored.
  - At 80 ms voiced, duck the gain (call.rs uses 0.35; domain 03 suggests −12 dB).
  - At ≤200 ms, pause (the lock is `barge_in_max_ms 200`, `LOCKS.toml:39-56`).
  - Yield after more than 400–800 ms voiced, or ≥2 words that are not on the backchannel lexicon.
  - Measured live: onset→duck 80.1/80.7 ms, onset→silence 200.1/200.3 ms ([M] `results/call-live/call-summary.json`).
- **Backchannels never cancel.** The lexicon "mhm, yeah, okay" is filtered. False-interrupt resume follows `call.rs:262-263`.
- **Outbound backchannels and fillers** play from the cached clip store (Backchannel body). Measured: endpoint→first audio 60/332 ms with filler/backchannel; cached backchannel 186/213 ms in the full loop ([M] `REPORT.md` §3/§6).
- **Interjection.** Jarvis may cut into a long run-on (`call.rs:295-302`), but only through `Floor` and never over a pinned turn.
- **Cancel** carries `epoch`. Every worker drops stale epochs before work and again before publish. That is the existing `turn_id < cancel` rule, extended (`IPC.md:59-72`).
- **Full-duplex later.** Moshi-class models give a theoretical 160 ms (80 ms Mimi frame + 80 ms acoustic delay) and about 200 ms in practice on an L4 GPU ([C] https://github.com/kyutai-labs/moshi). PersonaPlex 7B reports 0.170 s turn-taking and 0.240 s interruption response on GPU ([C] https://huggingface.co/nvidia/personaplex-7b-v1). Both need a GPU. On this CPU box the cascade plus Conductor is the path; the PersonaPlex relay stays a remote option (`obsidian-open-mind docs/persona-plex-v2-design.md`, max_sessions 1).

### 2.4 Cortex hand-off (Galion Research)

- Trigger: an Intent tier that needs research, or the local model emits a `<research>` control token. The slot posts `ToHermes{turn, intent, affect}` and keeps speaking: "Let me check that."
- The cap is 3 s (`RETRIEVAL_CAP`). On timeout, or on `NoKnowledge{reason}`, the slot falls back to filler or an honest "I don't know yet" (`sideband/src/lib.rs:85-92`).
- The answer becomes a `HermesNote` cell, enters the next clause's tail, and is written to memory as an L1 record when `remember` is granted.
- Observability goes through nerve events. New kinds are proposed: `symphony.ctx_miss`, `symphony.prefetch_hit`, `symphony.kv_rollback`, `symphony.cortex_timeout`, next to `voice.frame_miss` and the others (`OPENMIND-NERVE-CORTEX.md:77-81`).

---

## 3. Vector storage

### 3.1 What to store

The tiers map to Open Mind's L0–L3 so Hermes and Voice share one store.

| Kind | Tier | Example | Written when | Consent |
|---|---|---|---|---|
| `persona_card` | L3 (static) | Samantha / Jarvis / G1 system prompt and style rules | build time | none (not user data) |
| `turn` | L0 | one utterance or reply, ≤1 line | each turn | **RAM only** in the session; disk only if `remember` (`V3-PLAN.md:260-276`) |
| `fact` | L1 | "Mate's dog is called …", preferences | the extractor runs after the turn, off the critical path | `remember`; `share` for cross-persona reads |
| `scene` | L2 | a summary of a call or session | at session end | `remember` |
| `doc_chunk` | L1 | user documents, Galion docs, ≤200 tokens each | ingest | user action |
| `hermes_note` | L1 | cortex answer with source | on arrival | `remember` |
| `clip_ref` | – | backchannel/filler PCM ids per persona (clip store) | build time | none |

Rule from `personas/GALION-ONE.md:51`: the voice "may not pretend yesterday's call is in your pocket." Without `remember` there is no cross-session recall, and the persona must say so.

### 3.2 Schema

Use one SQLite file per user, with the openmind-memory schema bumped to `user_version 5`. It already uses rusqlite with bundled FTS5, which is MIT-compatible.

```sql
CREATE TABLE mem_item(
  id INTEGER PRIMARY KEY, kind INTEGER, persona_scope INTEGER,  -- 0 = all
  created_ms INTEGER, last_used_ms INTEGER, salience REAL,
  consent_bits INTEGER,           -- snapshot of sense/adapt/remember/share at write
  text TEXT, tokens INTEGER,
  simhash INTEGER,                -- 64-bit, dedup and near-dup (openmind-compact)
  emb BLOB,                       -- int8[256] (static model) 
  emb_scale REAL,                 -- per-vector symmetric scale (gemv_i8 quantize_row_i8)
  src TEXT);
CREATE VIRTUAL TABLE mem_fts USING fts5(text, content='mem_item', content_rowid='id');
```

At start-up the vectors are copied into one contiguous `Vec<i8>` arena (N×256) plus a `Vec<f32>` of scales, so the scan never goes through SQLite. Encryption at rest is open question Q5.

### 3.3 Embedding model (locks-aware)

The locks forbid candle, ort, tract, burn, llama and ggml in shipped Voice (`LOCKS.toml:89-96`), so fastembed-rs (ort) and candle are out for the shipped path.

| Option | Params / dim | CPU cost per query | Licence | Fits locks? |
|---|---|---|---|---|
| **Model2Vec static, potion-base-8M** (query and turn embedding) | 7.5M / 256 | model2vec-rs 11.8 µs for a short text, 19.8 µs medium; statembed 1.7 µs ([C] https://astrabert.github.io/statembed/); 8k samples/s single-thread in the older README ([C] https://github.com/MinishLab/model2vec-rs) | MIT ([C] https://huggingface.co/minishlab/potion-base-8M) | **Yes.** It is a tokenizer plus a table lookup plus a mean. Rewrite it in-tree (about 300 LoC [E]) and store the table as `.gq`. |
| potion-retrieval-32M | 32M / 512 (dim [E], verify) | about 2–4× the 8M cost [E] | MIT family [E: verify] | Yes, same mechanism |
| bge-small-en-v1.5 encoder ported to galion-tensor | 33M / 384 | ONNX int8: 3.9 ms p99 on 2 cores ([C] https://pypi.org/project/synaptoroute/0.1.0/), 12 ms p50 ([C] https://github.com/frandi/trovec/commit/092484986a48dcd9441bed3512204712f8f9d75b) | MIT | Only as a native `.gq` port (BERT-small on gemv_i8). Use for background re-embedding and rerank, never on the hot path. |
| all-MiniLM-L6-v2 | 22M / 384 | 3.5 ms for 4 words, 24 ms for 128 words on Ryzen 2700 ([C] https://github.com/shuttie/embed-benchmark) | Apache-2.0 | Same as above |

Quality trade-off: the authors say potion-base-32M reaches about 92% of MiniLM quality at about 70× the speed ([C] https://news.ycombinator.com/item?id=44021883). **Recommendation:** use static embeddings on the hot path, because they are cheap enough to run on *every* partial transcript. Hybrid BM25 makes up for the quality gap on names and facts. Upgrade to a native bge-small reranker only if the recall gate (§6, G-MEM-2) fails.

### 3.4 Index and quantization

- **Default: exact brute-force int8 scan** with galion-tensor `dot_i8_for(Avx512)` (`simd.rs:489`). It has no index build and no dependency, and it is exact.
  - Cost: N×256 B. At 10k items that is 2.6 MB, which fits in the L2/L3 cache. At 100k items it is 26 MB.
  - Estimate: 10k ≈ 0.1–0.3 ms, 100k ≈ 1.5–3 ms on one core. This is bandwidth-bound at about 10–20 GB/s per core [E]. The `LOCKS.toml:59-69` probe floor is bw_min 8 GB/s, which gives ≤3.3 ms at 100k [E].
  - For comparison: a 384-d dot in Rust measured 28 ns ([C] khive-hnsw https://docs.rs/crate/khive-hnsw/latest/source/docs/benchmarks.md), i.e. 10k ≈ 0.28 ms single-threaded even in f32.
- **Above about 200k items:** HNSW.
  - `hnsw_rs` is pure Rust and MIT/Apache ([C] https://crates.io/crates/hnsw_rs).
  - `usearch` is Apache-2.0 C++ with a Rust binding and native i8. It reports 274,653 QPS for 256-d i8 at 98.9% recall@1 on 64 Graviton3 cores ([C] https://github.com/unum-cloud/usearch/blob/main/BENCHMARKS.md).
  - Rust single-query HNSW: 116 µs median on 5k×384 ([C] khive-hnsw).
  - Not chosen for V2 because a personal instance rarely passes 100k items [E].
- **Lexical:** FTS5 BM25 (already in openmind-memory). Measured in-house BM25 ≈0.7 ms per 10k tokens [M]. Tantivy reports p50 312 µs and p99 7.8 ms on Wikipedia ([C] https://tantivy-search.github.io/bench/), so it is not needed at this scale.
- **Rejected for the hot path:**
  - LanceDB: Apache, but IVF_PQ is about 10–13 ms per query ([C] https://docs.lancedb.com/faq/faq-oss).
  - qdrant-edge: Apache-2.0, embedded with BM25 ([C] https://crates.io/crates/qdrant-edge). Bigger dependency tree than needed; it stays a fallback.
- **Fusion:** score = RRF(BM25 rank, cosine rank, k=60) × recency decay (half-life 7 days [E]) × salience, with SimHash dedup (Hamming ≤3). This follows the memori pattern (`PUZZLE-PIECES.md:277-339`) and openmind-compact.
- **Read budget:** embed (≤20 µs) + scan (≤3 ms at 100k) + FTS5 (≤1 ms) + fuse (≤0.1 ms) ⇒ **≤5 ms p95 end to end [E]**. It runs on partials, so it is **0 ms on the critical path** when the prefetch hits.

---

## 4. Per-turn context loading

### 4.1 What the model needs, and the token budget

The current prototype sends the system prompt plus the last 12 messages every turn, with no cache reuse (`call.rs:155-157`). The bench prompt for Samantha is about 150–200 tokens [E] (`latency-240/05-bench/tools/prompts.json`).

| Segment | Tokens (budget) | Tier | Changes |
|---|---|---|---|
| Persona card and safety style (per slot) | ≤300 | **hot**, KV snapshot | never during a session |
| Session-pinned user facts (top 5 at session start, consent `remember`) | ≤120 | hot, appended once | per session |
| Dialogue history (append-only) | ≤480 (about 12 turns × 40) | hot, already in KV | +1 turn per turn |
| Scene line: floor, e, persona, other persona's TurnSummary | ≤30 | warm | per turn |
| Memory hits (≤3 × 40) + HermesNote (≤80) | ≤200 | warm (prefetched) | per turn |
| Current utterance | 10–40 | stream | per turn |
| **Total context** | **≤1,150, cap 1,536** | | |
| **Tokens that still need prefill at end-of-speech** | **target ≤16** | | |

### 4.2 Ordering for prefix reuse (append-only KV)

Order: `[persona card][pinned facts][history … ][scene + hits][utterance][assistant tag]`.

The KV is **append-only within a session**. Hits and the scene line for turn *n* stay in the history, so turn *n+1* never invalidates the prefix. When the window reaches the cap, compact *while the bot is speaking*, off the critical path:

- keep the persona prefix (attention sinks, StreamingLLM [C] https://arxiv.org/abs/2309.17453);
- replace the oldest turns with a one-line TurnSummary;
- re-prefill into a second KvRing and swap.

H2O ([C] https://arxiv.org/abs/2306.14048) and SnapKV ([C] https://arxiv.org/abs/2404.14469) are later options. They need attention-score hooks in `attention.rs`.

Persona snapshots: precompute KV for g1, sam and jar at boot. A switch forks from that snapshot and replays the shared history tail. Estimate for 0.5B-class shapes (historically sized on Qwen2.5-0.5B; S0 shapes are Galion's own) (24 layers, 2 KV heads, head_dim 64, f32 KvRing): 24 KB per token, so a 300-token card is about 7 MB per persona and a 1,536-token window is about 37 MB [E]. Replaying history on a switch costs about 480 tokens of prefill, roughly 1 s at 400–600 tok/s [E]. So **switch at turn boundaries only, and pre-warm the likely target slot in the background** when e first passes 0.65 (8 frames = 640 ms of warning, `SAMANTHA-V2.md:291-296`).

Where to cite the external engines (they are dev reference only, see §5.5): llama.cpp `cache_prompt`, `--cache-reuse` and `/slots/{id}?action=save|restore` ([C] https://github.com/ggml-org/llama.cpp/blob/HEAD/tools/server/README.md, https://github.com/ggml-org/llama.cpp/discussions/13606); mistral.rs sequence-level prefix cache on CPU, `--prefix-cache-n` default 16 ([C] https://docs.mistralrs.dev/guides/perf/throughput-tuning/).

### 4.3 Hot, warm and cold tiers

| Tier | Medium | Contents | Read cost |
|---|---|---|---|
| Hot | KV in RAM | persona prefix, pinned facts, history, speculative utterance prefix | 0 (already computed) |
| Warm | RAM arena | int8 vector arena, FTS5 pages in the page cache, last 64 turns ring, clip store PCM, HermesNotes | ≤5 ms [E] |
| Cold | encrypted SQLite on disk | all L0–L3 history, docs | 1–20 ms [E]; only the prefetcher touches it, never the turn worker |

### 4.4 Prefetch and speculative prefill on partials

1. On each `PartialAsr`, when `stable_prefix_len` grows (Vosk partials; stability = unchanged for 2 ticks, 160 ms, `domains/04-latency-streaming.md:31-33`), the **context worker** embeds the stable prefix (about 12 µs), runs hybrid retrieval (≤5 ms), and publishes `MemoryHit`.
2. The **LLM worker**, idle while the user talks, prefills `[scene + hits][stable prefix tokens]` into the KV and records a checkpoint position.
3. On `EagerEndOfTurn` it may decode the first 1–2 tokens behind the closed TTS gate (Deepgram-style speculate/commit, `domains/03-dialogue-bargein.md:9,48`).
4. At `EndOfTurn`, only the delta tokens plus the assistant tag are prefilled, which is the ≤16-token target.
5. On `TurnResumed`, or a final transcript that differs from the prefix, roll the KV back to the checkpoint and bump the epoch.
   - **Gap:** `KvRing` has only `push`/`clear` (`kv.rs:48-55`). It needs `truncate_to(pos)` and `snapshot()/fork()`. That is a small, allocation-free change [E].
6. Expected saving per the research brief: 150–300 ms ([C] internal brief `master/GALION-VOICE-V2-RESEARCH-BRIEF.md:93`, which cites Deepgram/LiveKit).

---

## 5. Pure-local target on the 8-core Xeon (AVX-512, 16 GB, no GPU)

### 5.1 Core and RAM allocation (live call; isolate with cpusets)

| Core | Lane | Threads | RAM (RSS) | Basis |
|---|---|---|---|---|
| 0 | Heartbeat, bus, audio I/O, VAD/endpoint, SER, nerve client, Conductor | 1 (RT priority) | 50 MB [E] | ≤1% engine measured at p99 3.2 µs per tick [M] |
| 1 | ASR: Vosk small-en streaming (later Earshot VAD + native ASR) | 1 | 300 MB [E] | 0.40 vCPU per session (`STAGING-CPU.md:56-64` [D]); ASR final 19.9 ms [M] |
| 2–5 | Reply LLM (S0 ≈0.5B `.gq` student, W8A8) | 4 | weights 0.54 GB at q8 (1.07 B/param, `LOCKS.toml:59-69` [D]) or 0.29 GB at q4; KV and snapshots ≈100 MB [E] | decode is bandwidth-bound; 4 threads to saturate [E] |
| 6 | TTS: warm Piper, one process per active voice, 1 thread | 1 | 150 MB per voice [E] | 1 core is fastest: 37.2 ms vs 45 ms on 8 cores (`REPORT.md` §3 [M]) |
| 7 | Context worker (embed, retrieve, memory writer, compaction), side-band client, clip store | 1 | 100 MB (8M static table ≈30 MB at f32 [E], arena 26 MB at 100k, page cache 32 MB) | §3.4 |
| — (idle only) | Clone lane (Chatterbox Nano) for offline clip and corpus generation | 6 when no call is live | 1.5–3 GB [E] | offline RTF 0.55 with 8 threads [M]; live RTF 2.11 with two workers [M] |
| — | OS, page cache, headroom | – | ≥3 GB | |
| **Total live** | | 8 | **≈1.3–1.6 GB [E]** | well under 16 GB. Room for a second session on Plus-class RAM, not on CPU. |

### 5.2 How it fits 240 ms (end of speech → first content audio)

| Stage | Baseline p50 [M] | Budget [D] | V2 mechanism | V2 estimate p50 [E] |
|---|---|---|---|---|
| Endpoint | 130.2 | 60 | Semantic/eager EOT on stable partial + 80 ms acoustic hang instead of 160 ms | 60–90 |
| ASR final | 19.9 | 15 | Final overlaps the hang; downstream uses the stable prefix | 0–15 |
| Gap | 67.9 | 5 | In-process channels instead of HTTP/JSON; context is already assembled (`CtxSlab`) | 1–5 |
| LLM TTFT | 284.7 (Groq cloud, historical) | 120 | Local 0.5B with the prefix in KV, ≤16 delta tokens prefilled, plus one decode step | 45–90 |
| First clause | 20.6 | 10 | First clause = 1–2 word reaction ("Oh," "Mm,") already demanded by the persona prompt; flush at the first comma | 20–50 (1–2 decode steps) |
| TTS first PCM | 71.3 | 30 | Warm pinned Piper, 1–2 word clause: 25.9/35.2 ms for 1 word, 33.2/43.3 for 2 words [M] | 26–35 |
| **Total** | **608.3 / 775.1 p95** | **240** | | **≈150–285 p50 [E]** |

How the LLM estimate was derived [E]:

- llama.cpp on a 12-core Xeon E5-2680 v4 (AVX2, 2016) runs Llama-3.2-1B Q4_K_M at 330 tok/s prefill and 65 tok/s decode ([C] http://jitllm.org/docs/reference/benchmarks/). The same page reports Llama-3.2-1B TTFT of 11 ms for a 48-token prompt on a V100.
- A 0.5B model on 8 AVX-512 cores should give about 400–600 tok/s prefill and 50–90 tok/s decode [E]. So 16 tokens of prefill ≈ 27–40 ms and one decode step ≈ 11–20 ms.
- **Without** the prefix cache and speculation, a 700-token prompt would be about 1.2–1.8 s, which does not fit. That is why §4 is the whole game.

**Honest reading.**

- p50 can land under 240 ms. p95 probably will not on day one: endpoint variance plus decode jitter plus box load.
- Piper on the loaded box measured 128 ms for 1 word at load average 14–17 (`latency-240/01-data/affinity-loaded.ndjson` [M]), so **gates must run on an isolated box**.
- Fallback that already works: the cached backchannel at endpoint, 186/213 ms [M]. It keeps *perceived* latency under 240 ms while content follows.

### 5.3 The clone lane does not fit live

Measured: live endpoint → first cloned clause 3,259 / 5,179 ms; Nano 4.4 s p50 per clause; 943 ms minimum (`results/call-live/call-summary.json` [M]). Even a single pinned Nano worker at RTF 0.55 [M] needs about 0.5–1 s for a 1–2 s clause, so it can never meet 30 ms.

### 5.4 Clone at rest

- **Now:** pre-generate per-persona clip sets (backchannels, fillers, interjections, frequent openers) with Nano while idle, as `call.rs:132` already does.
- **Next:** use Nano offline to synthesise a 1–2 h corpus per persona voice [E], then train a Piper-class voice (VITS) on it. Live TTS stays at about 30 ms, and the timbre approaches the clone. ECAPA similarity today is 0.64–0.79 [M]; the target is ≥0.65 against the donor.
  - Both Piper and Chatterbox run under lock exceptions with sunset 2027-02-01 (`LOCKS.toml:102-136`). The long-term path is a native `.gq` TTS on galion-tensor.

### 5.5 Lock conflict (must be decided)

The 05-local-model agent is building a **candle 0.9 + GGUF** bench (`latency-240/05-bench`) and fetching Llama-3.2 GGUF plus llama.cpp b12076 (`/workspace/swarm/voice-v2-sprint/models/`).

- `LOCKS.toml:89-96` forbids candle (dev oracle only), llama and ggml, and forbids GGUF in Voice.
- At 18:45 CET the build had failed with 37 errors (`05-bench/build.log`), and the `out/*.jsonl` files were empty. There are **no local TTFT numbers yet**.
- Licensing: Llama-3.2 ships under the Llama Community License, not MIT/Apache. Qwen2.5-0.5B-Instruct is Apache-2.0 ([C] https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/blob/main/LICENSE) — historical note; Qwen is out of the architecture.

Proposal:

- **(a)** Treat llama.cpp/candle results as a *dev oracle only*. They set the target number for the student and never ship.
- **(b)** Ship the S0 student as `.gq` on galion-tensor. Teacher / training source: **open owner decision**; no third-party model (Qwen is out per the 20:45 ruling).
- **(c)** If the owner wants local conversation before the student exists, add a time-boxed `[formats.exceptions.v2_llm_oracle]` that mirrors `a21_piper`, sunset 2027-02-01, loopback only. This is owner decision Q1.

---

## 6. Phased build plan and gates

The latency agents are 01-streaming-tts, 02-core-pinning, 03-clone-efficiency, 04-pipeline and 05-local-model (`latency-240/BUDGET.md`). Their `CONTRIBUTIONS.md` has no rows yet.

| Phase | Build | Gate (measured, n≥24, isolated box) | Depends on |
|---|---|---|---|
| **P0 Decide and measure** (days 0–2) | Owner calls on Q1–Q4; a local LLM oracle number; cpuset isolation recipe | G0: oracle TTFT for a 16-token delta with a cached 700-token prefix is recorded [M]; locks decision written | 05 (oracle), 02 (pinning) |
| **P1 Blackboard lanes** | `Cancel{epoch}`, `Floor`, `Backchannel`, `CtxSlab`, `MemoryHit`, `HermesNote`, `TurnSummary` bodies; `PartialAsr.stable_prefix_len/eager_eot` | G1: one-percent still PASS (p99 <80 µs); stale-drop test; zero allocations in `step` | V2-S1 bus (`GALION-VOICE-V2.md:167-199`) |
| **P2 Memory store** | openmind-memory schema v5 embedded as a crate (no HTTP on the hot path); in-tree static embedder (`.gq` table); int8 arena scan; RRF fusion; consent gating | G-MEM-1: retrieve p95 ≤5 ms at 100k items. G-MEM-2: recall@5 ≥0.8 on a 200-question personal fact set [E target]. G-MEM-3: with `remember` denied, nothing on disk (`V3-PLAN.md:260-276`) | Open Mind (openmind-memory, Hermes `/recall` compatibility) |
| **P3 KV mechanics** | `KvRing::truncate_to/fork/snapshot`; persona prefix snapshots; append-only assembler; compaction during playback | G-KV: switch persona at turn boundary ≤1 turn of delay; rollback correct (bit-exact vs fresh prefill) | galion-tensor |
| **P4 Local reply model** | S0 0.5B `.gq` student W8A8 (or exception Q1c); speculative prefill on partials; 1–2 word first clause | G-LLM: TTFT p50 ≤120 / p95 ≤180 ms from EndOfTurn with prefix hit; first-clause ≤50 ms | 05-local-model, V2-S5 |
| **P5 Pipeline** | Endpoint 60 ms (eager EOT), in-process gap, warm pinned Piper | G-240: end of speech → first content audio p50 ≤240, p95 ≤320 (V2.0), then p95 ≤240 (V2.1); barge-in silence ≤200 ms; zero cancels on lexicon backchannels | 04-pipeline, 01-streaming-tts, 02-core-pinning |
| **P6 Dual-band local call** | Conductor on the blackboard; Samantha ↔ Jarvis fully local; TurnSummary cross-reads under `share` | G-CALL: the 120 s call reproduces the `call-summary.json` metrics with a local LLM; endpoint→first audio p50 ≤240 | P4, P5 |
| **P7 Clone at rest** | Nano clip sets per persona (idle only); corpus → Piper-class voice | G-CLONE: ECAPA ≥0.65 vs donor; TTFA for 1 word ≤35 ms; no clone process during live calls | 03-clone-efficiency, LOCKS exceptions |
| **P8 Cortex** | Side band wired (PR #23 path), HermesNote injection, 3 s cap, nerve kinds | G-CTX: a research question gets filler ≤240 ms and the answer clause ≤3 s, or an honest NoKnowledge; zero tick misses | PR #23, Galion OS / Open Mind job (nerve, Hermes gateway C5, memory surface C6) |
| **P9 Hardening** | Encrypted store, per-user admin, presence-safe metrics, Galion OS Voice profile packaging | Security R1/R2/R4 rows; Galion OS Voice profile boots one dot with Hermes off the 80 ms path | Galion OS job |

---

## 7. Risks

1. **Lock conflict** (candle/GGUF/llama.cpp vs `LOCKS.toml:89-96`), and the Llama licence. Mitigation: §5.5.
2. **No local LLM TTFT measured yet.** Every LLM number in §5.2 is an estimate. The S0 student does not exist, and its quality at 0.5B is unproven for persona voice.
3. **Box contention.** Other agents push the load average to 7–17. Piper slows 3–5× under load [M]. Gates without cpuset isolation will be noise.
4. **Endpoint at 60 ms risks false cut-offs.** Eager EOT needs a TurnResumed rate <20% before pre-synthesis (`domains/03-dialogue-bargein.md:48`).
5. **Decode-speed coupling.** On CPU the first clause depends on decode tok/s (Groq hid this at 20.6 ms). A first clause longer than 2 words breaks the budget.
6. **Persona switch cost** (history replay about 1 s [E]). Mitigated by turn-boundary switching and pre-warm, but a forced mid-turn switch will be slow.
7. **Static embeddings miss paraphrases.** Hybrid BM25 helps. A reranker is the fallback (cost 4–12 ms [C]).
8. **Clone quality vs latency.** Clone at rest may lose timbre. Exceptions sunset 2027-02-01.
9. **Disk** is 11 GB free at 18:46 CET (down from 16 GB). The model fetches by agent 05 are filling it.
10. **Memory leakage.** Cross-persona reads and memory on disk must obey consent bits; the `share` default is deny.

## 8. Open questions for the owner

- **Q1** (closed by the 20:45 ruling) Local LLM before the `.gq` student: no third-party model; CannedLlm until S0 weights exist.
- **Q2** Teacher / training source for S0: **open owner decision**. Qwen and Llama are excluded (third party).
- **Q3** Embedding: accept static Model2Vec (MIT) as an in-tree `.gq` table on the hot path?
- **Q4** Memory on disk: is `remember` opt-in per session or per fact? Does `share` (cross-persona) default to deny, as the locks say?
- **Q5** Encryption at rest: SQLCipher-style page encryption, or an age-encrypted blob plus RAM-only index? Where does the key live (OS keyring vs passphrase)?
- **Q6** Are persona switches allowed only at turn boundaries (recommended)?
- **Q7** Clone at rest: is training a Piper-class voice on Nano-generated audio acceptable under the CC0 donor terms and the a21/v2s14 exceptions?
- **Q8** Is the V2.0 gate p95 ≤320 ms acceptable, with p95 ≤240 ms deferred to V2.1?
- **Q9** Licence header: the tree says "Perpetual Open MIT License (c) 2026 Maciej Grajczyk" (e.g. `symphony/STAGING-CPU.md:3`); the brief says GOATSI Inc. "open perpetual MIT". Which copyright line goes on new files?
- **Q10** Is the 64-turn RAM ring (nerve) the same as the memory L0 ring, or separate?

## 9. Not verified

- The `galion-symphony-v2` repository and its local mirror are absent. `VOICE-SYMPHONY.md`, `PROJECTS.md`, `BOTS-AND-HANDOFF.md` and `GALION-OVERVIEW.md` were not found.
- There are no local LLM TTFT numbers: the agent 05 bench failed to compile and its outputs were empty at 18:45 CET. No numbers are in `CONTRIBUTIONS.md`.
- Static-embedding retrieval quality on Galion data, and the dimension and licence of potion-retrieval-32M, were not checked.
- Int8 scan throughput on this box was not measured. Running it would have meant building, and builds were not allowed.
- The VPS (Titanaxe) was not accessed, by rule.
- The KV size per token assumes 0.5B-class shapes (historically Qwen2.5-0.5B; S0 shapes TBD) and f32 KvRing.
