# R1 — HORBM retrieval core

Perpetual Open MIT License. (c) 2026 Galion Labs.

Research note, agent R1 of the HORBM swarm. Written 2026-11-18. This is a design note. It is not an implementation, not a benchmark, and not an owner ruling.

HORBM here means Galion's Heuristic Open Retrieval Brain Model: a CPU-first, Rust-first store that replaces the retrieval half of Microsoft XiaoIce (Zhou, Gao, Li, and Shum, 2018) and feeds the Symphony / MnE V2 voice stack already in this repository. The reasoning half, including Eureka Moment Pipelines, is out of this note's scope except for the packet retrieval must hand them.

## How to read a sentence

Every claim in this note is one of three kinds.

- **Fact.** Someone said it. The sentence names who, the date, and the source. A number copied from a paper or a crate page is that author's number, measured on their hardware and their data. It is not a Galion measurement.
- **Opinion.** A recommendation by R1, dated 2026-11-18. It is not a measurement and not an owner decision.
- **Unknown.** The sources read for this note do not establish it.

Nothing in this file was timed. No model, dataset, or index was downloaded. No paid API was called. No audio was opened. Crate versions and download counts were read from crates.io and GitHub pages on 2026-11-18 and will move.

The copyright line on new files is an open question in the Symphony V2 plan (Q9, written 2026-11-03). This file uses the line already printed on the persona docs, `(c) 2026 Galion Labs`.

---

## 1. Recommendation

**Opinion (R1, 2026-11-18).** The voice diary is a hybrid of BM25 and one static int8 vector per consented card, scanned exactly, fused with reciprocal rank fusion, and prefetched while the person is still talking. A small heuristic knowledge graph expands the query only when that expansion still fits the same budget. Late interaction, SPLADE, product quantization, HNSW, DiskANN, and GraphRAG stay off the voice path. They are documented below so a later failure of the diary can pick one up with its cost already named.

The hot path publishes at most four hits on the blackboard the Symphony V2 plan already sketched (`MemoryHit`). Each hit carries who said it, when, the source, and whether it is a statement, an inference, an opinion, or unknown. Galion Research may spend the side band's three seconds on a heavier second pass. The voice does not wait.

The latency target for the diary, on one core, at the scale of a person's memory, is **≤5 ms p95**, and **0 ms on the critical path** when the prefetch has already landed. Five milliseconds is the whole ASR-to-reasoning gap in the 240 ms voice budget, so a miss spends that gap and nothing else. Five milliseconds is also about 2.4% of the cross-language mean gap between a question and its answer in human conversation (208 ms; arithmetic from Stivers et al., below). That is the sense in which the target is well under a real-time voice turn.

---

## 2. What XiaoIce retrieval was, in the authors' words

**Fact.** Li Zhou, Jianfeng Gao, Di Li, and Heung-Yeung Shum, "The Design and Implementation of XiaoIce, an Empathetic Social Chatbot." arXiv:1812.08989, first posted December 2018 (Microsoft Research also lists it as MSR-TR-2018-42, December 2018). Journal version: *Computational Linguistics*, 2020, ACL Anthology `2020.cl-1.2`. The quotations below are from the arXiv PDF fetched for this note on 2026-11-18.

Core chat has three candidate generators. Two of them are retrieval. The third is a neural response generator and is outside this note.

**Paired retrieval.** The paired database is query–response pairs from public internet conversations and, after the May 2014 launch, from XiaoIce's own chats: "more than 30 billion conversation pairs as of May 2018." "70% of XiaoIce's responses are retrieved from her own past conversations." Pairs are filtered for persona fit, empathy, PII, and mess, then "indexed using Lucene." At runtime the contextual query retrieves "up to 400 response candidates using keyword search and semantic search based on machine learning representations of the paired database," citing Zhang et al. (2016) and Wu et al. (2016).

Those two citations, as printed in Zhou et al.'s bibliography:

- Kai Zhang, Wei Wu, Fang Wang, Ming Zhou, and Zhoujun Li. "Learning distributed representations of data in community question answering for question retrieval." WSDM 2016.
- Bowen Wu, Baoxun Wang, and Hui Xue. "Ranking responses oriented to conversational relevance in chat-bots." COLING 2016.

**Unpaired retrieval plus knowledge-graph expansion.** The unpaired database is sentences from public lectures and news quotes, also indexed with Lucene. The query is expanded so the reply does not merely repeat the user. The graph is head–relation–tail triples joined from the paired data and Microsoft Satori (Satori "is seeded by Freebase"). A Satori triple is kept when the head and the tail "co-occur often enough in the same conversations." Their examples are (Beijing, Great Wall), (Einstein, Relativity), (Quantum Physics, Schrödinger's cat). The runtime steps they list are: identify topics in the query; retrieve "up to 20 most related topics" and score them with a boosted tree (Wu, Burges, Svore, and Gao, 2010, "Adapting boosting for information retrieval measures"); retrieve "up to 400" sentences. They say unpaired candidates are lower quality and broader coverage than paired ones, and longer than the neural generator's outputs.

**Ranker, shared by all three generators.** A boosted tree (Wu et al., 2010) ranks the pool. One feature family is "local cohesion," computed with Deep Structured Semantic Models. Zhou et al. point at Huang et al. (CIKM 2013, "Learning deep structured semantic models for web search using clickthrough data"), Shen et al. (CIKM 2014), and Gao et al. (EMNLP 2014). A candidate is then sampled from those above a threshold. The paper does not say the spoken reply cites the source of the retrieved sentence.

**Unknown.** Zhou et al. say "keyword search." They do not name BM25 and they do not name the Lucene version. BM25 is the probabilistic keyword model later systems mean by that phrase (Robertson and Zaragoza, *Foundations and Trends in Information Retrieval*, 2009), and it later became Lucene's default similarity. Whether the 2018 deployment used BM25 specifically is not stated.

**Opinion.** HORBM keeps four of their ideas and drops the scale and the silence about provenance.

| XiaoIce, 2018 | HORBM diary |
|---|---|
| Retrieve from the system's own past conversations (70% of replies, their figure, May 2018) | Retrieve from this person's consented diary. The 30 billion pairs are a product corpus, not one diary. |
| Lucene keyword index over query–response pairs | BM25 over the card text, in-process |
| A learned semantic index beside the keywords (Zhang 2016, Wu 2016) | One static int8 vector per card, exact scan |
| Up to 400 candidates, then a boosted tree | An internal pool on the order of a few dozen, then four published hits. The voice speaks a sentence, not a ranked list. The tree can live on the Research side band. |
| Unpaired Lucene over news and lectures, expanded by up to 20 KG topics | A small triple list built from the diary. World knowledge is Galion Research's side band, not the companion's pocket. |
| Persona filter before indexing | Consent bits and persona scope. A fact is not dropped because it fails to flatter the persona. |
| No source on the spoken candidate | Who, when, source, and epistemic kind on every hit |

---

## 3. Constraints already written down in this repository

These are facts about documents in the tree, not new measurements.

**Voice budget.** `docs/galion-voice-v2/sprint-v2/latency-240/BUDGET.md` sets 240 ms from end of user speech to first content audio. The ASR-to-reasoning gap inside that budget is 5 ms. The owner ruling dated 2026-11-04 20:45 CET, printed at the top of that file and of the Symphony V2 plan, puts a low-priority context/retrieval worker on core 0, with the bus, the heartbeat, audio, VAD, and the orchestrator. Cores 2–5 are in-house reasoning. Cores 6 and 7 are Piper. Clone workers do not run during a live call. The same ruling says the reasoning sector is a local S0 student, with no third-party LLM in the architecture.

**Heartbeat.** `LOCKS.toml`, locked 2026-11-02, sets the frame at 80 ms and the compute budget inside the frame at 60 ms p95, shared by audio, emotion, console, render, Rika, and the OS scheduler. `docs/galion-voice-v2/personas/GALION-RESEARCH.md` says the side band is not on that tick and "You never move a retrieval onto that tick."

**Memory shape the V2 plan already proposed.** `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`, written 2026-11-03, status PLAN. One SQLite file per user, FTS5, an int8 vector arena, consent on every write, prefetch on stable ASR partials. The plan's own read budget, marked there as an estimate, is embed ≤20 µs + scan ≤3 ms at 100k items + FTS5 ≤1 ms + fuse ≤0.1 ms, hence ≤5 ms p95, and 0 ms on the critical path when the prefetch hits. The plan's published hit is:

```rust
BusBody::MemoryHit{ hits: Arc<[Hit; 4]>, query_sig: u64, took_us: u32 }
pub struct Hit { item_id: u64, kind: Kind, score: f32, tokens: u16 }
```

The plan also states a contradiction with the owner table: its §5.1 assigns the context worker to core 7, while the owner table assigns core 7 to Piper Jarvis and retrieval to core 0. Both texts are in the same file. Which core wins is unknown. See open question 1.

**Locks on runtimes.** `LOCKS.toml` `[formats]`: `forbidden_runtimes = ["candle", "ort", "tract", "burn", "llama", "ggml"]`, with `candle` listed as `dev_oracle_only`. An owner exception (Maciej Grajczyk, t613s2992, 2026-11-04) allows the `ort` crate to load ONNX Runtime 1.31.1 inside `runtimes/galion-piper-rt` only, a separate process, sunset 2027-02-01. The exception says not to widen it. Consent defaults to deny (`sense`, `adapt`, `remember`, `share`).

**Personas.** Galion One (`personas/GALION-ONE.md`): the voice may not pretend yesterday's call is in its pocket; withdrawal wipes RAM history. Galion Research (`personas/GALION-RESEARCH.md`): keep speaking from local knowledge while a lookup is in flight; the side band's cap is 3 seconds (`galion-sideband`); separate what was found, what was inferred, and what is unknown. The Research voice was previously called Hermes. Hermes remains the access layer.

**MnE.** `tools/symphony/mne/README.md`: the integrator does not implement upstream memory, persona, or inner life. `--text` and `--pin` stand in for the contract. Retrieval sits in front of that text. It does not render audio.

**What the V2 plan already rejected, and on what evidence.** The 2026-11-03 plan prefers SQLite FTS5 over Tantivy at diary scale, exact int8 scan over HNSW under about 200k items (their estimate), and treats LanceDB and qdrant-edge as heavier than the hot path needs. It cites third-party timings for those choices. This note does not re-fetch those timing pages and does not adopt the timings as checked facts. The architectural conclusion below agrees with the plan, for the reasons in §6 and §8, and it fills the provenance hole the plan's `Hit` struct still has.

---

## 4. Recommended design

**Opinion (R1, 2026-11-18).** Three lanes, one record format.

### 4.1 Lane A — the voice diary (replaces paired Lucene)

Runs in-process on the context worker, on stable ASR partials, never inside `Heartbeat::step`.

1. **Lexical leg.** BM25 over the card text (Robertson and Zaragoza, 2009, for the model; the implementation choice is in §7). This is the leg that keeps names, dates, and rare words, which a single vector blurs. Luan, Eisenstein, Toutanova, and Collins (arXiv:2005.00181, submitted 1 May 2020, revised 16 February 2021) argue that a fixed-length vector has a capacity limit tied to document length and the margin to the next document, and they study sparse–dense hybrids for that reason. Karpukhin et al. (EMNLP 2020, arXiv:2004.04906) showed a dense dual encoder beating a Lucene BM25 baseline by 9 to 19 absolute points of top-20 accuracy on their open-domain QA sets. Both results are about their benchmarks. Together they are why the diary keeps both legs: dense retrieval misses exact strings, and BM25 misses paraphrase.
2. **Dense leg.** A static embedding: tokenize, look up a row, mean-pool. The Model2Vec line (Stephan Tulkens and Thomas van Dongen, 2024, MIT; GitHub `MinishLab/model2vec`, repository created 18 July 2024) is the class that fits a CPU voice turn, because it is a table rather than a Transformer forward pass. Their README claims models "up to 500 times faster" and "up to 50" times smaller than the sentence transformers they distill. That is their claim, not a Galion run. The `potion-base-8M` card (MinishLab, fetched 2026-11-18) says the model reaches 91.96% of `all-MiniLM-L6-v2` on the card's average (51.32 against 55.80) and is "orders of magnitude faster." `potion-retrieval-32M` is the retrieval-tuned sibling: the Mozilla `static-embeddings` mirror of its README states a width of 512 and a vocabulary of 63,091, and the Minish model table marks it English, distilled from `BAAI/bge-base-en-v1.5`, 32.3M parameters. The software citation on the 8M card says MIT. The 32M checkpoint's own license field was not opened.
3. **Quantization.** Symmetric int8, one scale per vector, stored as the V2 plan's `emb BLOB` plus `emb_scale`. Scalar int8 is a 4× cut against float32; binary is a 32× cut. Those ratios are the definitions used by Aamir Shakir, Tom Aarsen, and Sean Lee, "Binary and Scalar Embedding Quantization for Significantly Faster & Cheaper Retrieval," Hugging Face blog, 22 March 2024. They report, on their own 41-million-passage Wikipedia demo with a 1024-d model, that binary search kept up to about 96% of retrieval performance with a float rescore and about 92.5% without. Those percentages are theirs. They also write that int8 buckets depend on a calibration set. The V2 plan's per-vector scale is a simpler quantizer than their per-dimension min/max. Which one holds quality on diary text is unknown.
4. **Search structure.** Exact scan of a contiguous int8 arena. At 100,000 cards of 256 dimensions the payload is 100,000 × 256 = 25,600,000 bytes, about 26 MB. That arithmetic is the same one the V2 plan prints. Hierarchical graph indexes and disk indexes solve a larger problem than this (§6.4).
5. **Fusion.** Reciprocal rank fusion of the BM25 ranking and the cosine ranking (Gordon V. Cormack, Charles L. A. Clarke, and Stefan Büttcher, SIGIR 2009). The V2 plan sets the constant k to 60 and multiplies by a recency decay and a salience term. This note keeps that formula. The 2009 paper's own plotted constant was not re-read.
6. **Publish.** Four hits, the V2 `MemoryHit`, extended as in §4.4. A wider internal pool (on the order of 32) is enough to fuse. Four hundred candidates was XiaoIce's input to a tree ranker, not a voice context window. The V2 plan's context budget is ≤3 hits × about 40 tokens plus one Hermes note.

**Width choice, opinion.** Start at 256-d int8, the width the V2 plan already stores, using a small static table in that width (the 8M class). Move to the 512-d retrieval table only if a recall check on Galion dialogue fails. A 512-d scan moves twice the bytes. The V2 plan left the 32M width as an estimate; the Mozilla mirror confirms 512.

**In-tree embedder, opinion.** Rewrite the static model as a tokenizer plus a table plus a mean, as the V2 plan proposes, and keep `ort` and `candle` out of the voice binary. `LOCKS.toml` already forbids both on the shipped path. `fastembed` 7.7.1 (crates.io, updated 16 November 2026) defaults to the `ort` backend, so it is a Research-side tool, not a voice dependency.

### 4.2 Lane B — topic expansion (replaces unpaired Lucene and the Satori hop)

**Opinion.** Keep XiaoIce's co-occurrence idea. Drop Satori, the news-quote corpus, and the boosted tree on the hot path.

- A triple is `(head, relation, tail, count, support_ids, epistemic)`.
- `epistemic` is `said` when the person or the assistant uttered the sentence that supports it, and `inferred` when a heuristic extractor proposed it. An inferred triple is never spoken as a fact. Galion Research's three piles (found, inferred, unknown) are the speaking rule; this field is how retrieval supplies them.
- Expansion budget: at most one extra BM25 query, fed by a handful of tails (XiaoIce used up to 20; a voice slice cannot spend 20 extra probes and stay at 5 ms). If the extra query does not fit the remaining budget, publish the Lane A hits and mark the graph leg pending. The pending leg may finish on the side band.
- The extractor runs after the turn, off the critical path, and only when `remember` allows the write. It is a pattern extractor (names, dates, copulas), not an LLM. Quality of that extractor is unknown.

Microsoft GraphRAG is the 2024 form of "build a graph, then retrieve." Darren Edge and colleagues, arXiv:2404.16130 (v2 dated 19 February 2025; Microsoft Research lists the preprint as April 2024), use a large language model to extract entities, to summarize communities, and to map-reduce those summaries at question time. They evaluate global questions over corpora around one million tokens. The GitHub repository `microsoft/graphrag` (created 27 March 2024, MIT, about 36.6k stars on the page fetched 2026-11-18) warns that the project is largely in maintenance mode and is not taking feature work. That design needs an LLM at index time and at query time, and the query is a summarization over communities. It does not fit a non-LLM hot path or a 5 ms slice. The piece worth keeping is the compressed community page: a short gist of a cluster of cards, itself a card, so a broad question ("what have we talked about") hits a summary instead of a pile of turns. The gist is extractive (a sentence the person said, plus entity ids), and it is marked `inferred` where it compresses.

### 4.3 Lane C — Galion Research corpus (the three-second side band)

**Opinion.** Papers, Galion docs, and Hermes notes live here. The companion voice does not block on this lane. Galion Research may. The cap of 3 seconds is the persona document's, not a new number.

Allowed on this lane, still CPU-only, still out of the voice binary when the lock forbids the runtime:

- Tantivy for fielded BM25 (title, author, date, quotation). This is the Lucene-shaped index once the corpus is a library rather than a diary.
- A pure-Rust HNSW (`hnsw_rs`) or `usearch` if a measured exact scan no longer meets the lane's budget. No such measurement exists.
- A small Transformer encoder as a reranker over the four to thirty-two diary hits, in a separate process, only after a recall gate fails. `candle` is allowed as a dev oracle by `LOCKS.toml`. `ort` is not allowed in the voice workspace beyond the Piper exception.
- ColBERT/PLAID or SPLADE as an experiment with a named abort criterion: if the CPU time exceeds the 3 s cap on the Research corpus, the experiment stops. Their published operating point is web-scale, not a diary (§6.2, §6.3).

Not the default on this lane: a Qdrant or LanceDB server process, a DiskANN on SSD, or a GraphRAG map-reduce. They solve multi-tenant and billion-vector problems this brain does not have.

### 4.4 The record, and knowledge per KB

**Opinion.** A card is the unit of memory. Short on purpose, so a life of conversation stays a diary and not a heap of audio transcripts or raw web pages. Audio is out of this store entirely.

Fields, extending the V2 plan's `mem_item` rather than replacing it:

| Field | Role |
|---|---|
| `kind` | `persona_card`, `turn`, `fact`, `scene`, `doc_chunk`, `hermes_note`, as in the V2 plan |
| `text` | the gist the voice is allowed to say |
| `said_by` | the person, the assistant, an extractor, or a cited author |
| `said_at_ms` | when they said it; unknown stays null and the hit is marked unknown |
| `src` | where it came from (turn id, doc id, paper). The V2 plan already has `src TEXT` |
| `epistemic` | `said`, `inferred`, `opinion`, `unknown` |
| `consent_bits` | snapshot of sense/adapt/remember/share at write time |
| `persona_scope` | which voice may read it; `share` gates cross-persona reads |
| `simhash` | 64-bit near-duplicate key, already in the V2 plan |
| `emb`, `emb_scale` | int8 vector and its scale |

`Hit` on the bus gains `said_by`, `said_at_ms`, `epistemic`, and `src`, beside the existing `item_id`, `kind`, `score`, and `tokens`. Without those fields a Eureka pipeline cannot tell a fact from an opinion, and Galion Research cannot speak the three piles honestly. The V2 struct does not have them yet.

**Arithmetic, not a measurement.** Bytes per card for the vector alone:

| Encoding | Bytes per card | Against float32 256-d (1024 B) |
|---|---|---|
| float32, 384-d (MiniLM / bge-small class) | 1,536 | larger |
| float32, 512-d (`potion-retrieval-32M`) | 2,048 | larger |
| float32, 256-d | 1,024 | 1× |
| int8, 256-d, plus a 4-byte scale | 260 | about 4× smaller |
| binary, 256-d | 32 | 32× smaller |

ColBERTv2 stores a vector per token. Santhanam, Khattab, Saad-Falcon, Potts, and Zaharia (NAACL 2022; arXiv:2112.01488) encode each 128-d token vector as a 4-byte centroid id plus a 1-bit or 2-bit residual, "20 or 36 bytes per vector," against ColBERT's 256-byte 16-bit vectors, and they report a 6–10× smaller MS MARCO index (16 or 25 GiB against 154 GiB, including 4.5 GiB of inverted list). A 40-token gist at 20–36 bytes is 800–1,440 bytes of token vectors, before the inverted list, against 260 bytes for one int8 card vector. Late interaction spends the knowledge-per-KB budget on tokens. The diary spends it on cards.

A card's text gist is the other half of the budget. The V2 plan already caps a turn at one line and a doc chunk at about 200 tokens. Keeping the gist in that range, plus the 260-byte vector, is what "an experienced person with a huge diary" means in bytes: many small sourced cards, not a transcript warehouse. SQLite page overhead and the FTS5 index sit on top of this arithmetic. They were not measured here.

Matryoshka truncation is not a free extra compression for these tables. Kusupati et al. (arXiv:2205.13147, 2022; the method AdANNS cites) train a vector so that a prefix of its dimensions is itself a usable embedding. Aditya Kusupati and colleagues then show, in "AdANNS" (NeurIPS 2023), that IVF and product quantization can use a short prefix at the coarse stage and a longer one at the fine stage. The potion cards describe a different step: post-training PCA and re-regularization. Whether a prefix of a potion vector retrieves is unknown. Do not truncate one until that is tested.

### 4.5 What the hot path deliberately does not do

**Opinion.**

- It does not call a language model, including the S0 student. S0 is the reasoning sector on cores 2–5. Retrieval hands it a packet.
- It does not load `candle`, `ort`, `tract`, `burn`, `llama`, or `ggml` into the voice binary (`LOCKS.toml`).
- It does not read or write audio, including CC0 donor audio.
- It does not follow a network hop. `qdrant-client` is a gRPC client (crates.io: "Rust client for Qdrant"). A round trip, even to localhost, is a different budget from an in-process scan.
- It does not build or compact the index during the turn. Writes happen after the reply, and only with `remember`.

---

## 5. Options and trade-offs

**Opinion column is R1, 2026-11-18.** "Mature" is a reading of age, download counts, and dependents on 2026-11-18, not a quality award. Download counts are crates.io totals on that date.

| Option | Replaces in XiaoIce | Licence and maturity (fetched 2026-11-18) | Cost that matters on a CPU voice turn | Where HORBM uses it |
|---|---|---|---|---|
| SQLite FTS5 BM25, in-process (`rusqlite`, already the V2 plan) | Paired Lucene keyword leg | SQLite is public-domain; the V2 plan calls the bundled FTS5 MIT-compatible | One index the diary already has. Weak once the corpus is a library with many fields and writers. | Lane A lexical leg |
| Tantivy 0.27.0 (updated 2026-10-17; crates.io created 2016-08-11; about 22.5M downloads, 354 dependents). Pure Rust. README: inspired by Lucene. Quickwit is the distributed engine built on it. | Lucene itself | MIT (crates.io and the GitHub license badge) | A second index to build and to keep warm. Worth it when queries are fielded (author, date, quote) over a research corpus. | Lane C. Not a second hot-path index beside FTS5. |
| Static int8 vector, exact arena scan | Zhang/Wu semantic leg | No extra crate if the table is in-tree. Model2Vec software is MIT. | Sequential bandwidth. Honest recall: the scan is exact. Grows linearly with card count. | Lane A dense leg |
| `hnsw_rs` 0.3.5 (updated 2026-10-09; created 2020-01-13; about 1.40M downloads, 92 dependents) | Nothing in XiaoIce; this is the modern ANN | MIT OR Apache-2.0. Pure Rust port of Malkov and Yashunin. | Approximate. Graph walks are random memory access. Quantization is a distance you bring, not a shipped int8 index. Still pre-1.0. | Lane C, after an exact scan is measured too slow. First ANN to try, because it is pure Rust. |
| `usearch` 2.26.6 (updated 2026-10-31; created 2023-05-01; about 1.76M downloads, 72 dependents) | Same ANN job | Apache-2.0. Rust bindings over a C++ core. The Hugging Face quantization blog lists USearch as supporting binary embeddings. | Native i8 and binary, which `hnsw_rs` does not productize. A C++ core is a second toolchain inside a Rust-first brain. | Lane C alternative when int8 or binary HNSW is required and a pure-Rust distance is not enough. |
| `lancedb` 0.41.0 (updated 2026-10-29; created 2023-03-18; about 2.45M downloads, 102 dependents). MSRV 1.94. | A vector database, not a XiaoIce component | Apache-2.0 | Disk-oriented IVF/PQ store. A process and a format, not a function call. MSRV 1.94 is a high bar for this tree. | Not the diary. A candidate only if Lane C outgrows an in-process arena and needs a columnar on-disk table. |
| Qdrant server (`qdrant/qdrant`, GitHub created 2020-05-30, Apache-2.0, about 35.5k stars) and `qdrant-client` 1.20.0 (updated 2026-10-22; about 6.34M downloads) | Same | Apache-2.0. The server is Rust and mature as a product. | A service and a network hop. Filters and payloads are real features, and they are the wrong shape for a 5 ms in-process diary. | Not on the voice path. |
| `qdrant-edge` 0.9.0 (updated 2026-10-22; crates.io created 2026-03-03; about 25.5k downloads, 3 dependents) | In-process vector engine | Apache-2.0. Young. The crates.io page shows a large dependency list. The V2 plan says it includes BM25; this note did not open the API, so that claim stays the plan's. | An embedded database inside the voice binary, with a young major version. | Fallback if Lane A ever needs Qdrant's filter model in-process. Not the default. |
| `fastembed` 7.7.1 (updated 2026-11-16; created 2023-09-19; about 5.60M downloads, 337 dependents) | The semantic encoder | Apache-2.0. Default features pull `ort` and can download ONNX files. Optional `candle` features exist. The model list includes MiniLM, bge, e5, GTE, Arctic, EmbeddingGemma, and Qwen3. | Convenient encoder host. The default backend is a forbidden runtime on the voice path, and the crate's job is to fetch weights. | Research experiments only, with weights supplied offline, never on the voice hot path. |
| `candle-core` 0.11.1 (updated 2026-10-31; created 2023-08-02; about 12.0M downloads, 690 dependents) | A place to run a small encoder | MIT OR Apache-2.0. Hugging Face. Pure Rust. | `LOCKS.toml` forbids it in shipped voice and allows it as a dev oracle. | Offline re-embedding and a side-band rerank prototype. |
| `ort` 2.0.0-rc.15 (updated 2026-10-19; about 26.7M downloads, 717 dependents) | ONNX host for a small encoder | MIT OR Apache-2.0 (pyke.io). The 2.0 line is still a release candidate. ONNX Runtime itself is the library the Piper exception already pins, in another process. | Widely used, and locked out of the voice workspace except that Piper process. | Do not add a second `ort` for retrieval. |
| `rust-diskann` 0.4.7 (updated 2026-10-03; created 2025-09-30; 955 downloads, 0 dependents) | Billion-scale ANN on SSD | crates.io says MIT OR Apache-2.0; the README text says MIT. The LICENSE file was not opened. | DiskANN's purpose is data that does not fit in RAM (Jayaram Subramanya et al., NeurIPS 2019). A diary that fits in 26 MB of vectors does not need SSD graph search. The crate is young. | Not used. Named so the swarm does not adopt it by default. |

---

## 6. The 2018-to-now methods, and the CPU cost each one actually moves

### 6.1 Hybrid BM25 and dense retrieval

**Fact.** BM25 is the probabilistic relevance model surveyed by Stephen Robertson and Hugo Zaragoza (2009). Dense dual encoders encode the query and the document independently and score them by inner product; Karpukhin et al. (EMNLP 2020) is the paper that made that pattern the default open-domain retriever, against a Lucene BM25 baseline. Luan et al. (2020/2021) combine a sparse score and a dense score and also study multi-vector "attentional" document encodings, because one vector has limited capacity. Cormack, Clarke, and Büttcher (SIGIR 2009) combine ranked lists by reciprocal rank fusion without requiring the scores to share a scale. That property is why RRF is the right fuse for BM25 and cosine: the two scores are not comparable numbers.

**Opinion.** Hybrid is the paired-Lucene replacement. It is also the cheapest hybrid that still has an exact-match leg for names ("Mate's dog"), which a static vector is weak on. The V2 plan already flags that weakness as an open risk. No Galion recall number exists.

**CPU.** At diary scale the fuse is a sort of two short lists. The encode and the scan dominate. Adding the second leg is cheap next to a Transformer query encoder.

### 6.2 Late interaction: ColBERT and PLAID

**Fact.** Omar Khattab and Matei Zaharia, "ColBERT: Efficient and Effective Passage Search via Contextualized Late Interaction over BERT," SIGIR 2020 (published 25 July 2020, DOI `10.1145/3397271.3401075`). The query and the document stay as token vectors. The score is a late sum of maximum similarities (MaxSim). The cost they accepted, which Formal, Piwowarski, and Clinchant restate in the SPLADE paper (§2 of arXiv:2107.05720), is "storing embeddings for each (sub)term."

ColBERTv2 (Santhanam, Khattab, Saad-Falcon, Potts, and Zaharia, NAACL 2022) compresses each token vector to a centroid plus a 1-bit or 2-bit residual, 20 or 36 bytes, and reports the MS MARCO index dropping from 154 GiB to 16 or 25 GiB.

PLAID (Keshav Santhanam, Omar Khattab, Christopher Potts, and Matei Zaharia, arXiv:2205.09707v1, 19 May 2022; CIKM 2022, DOI `10.1145/3511808.3557325`) prunes with a centroid interaction before the full MaxSim. Their abstract states a latency cut of up to 7× on a GPU and 45× on a CPU against vanilla ColBERTv2, "tens of milliseconds on a GPU and tens or just few hundreds of milliseconds on a CPU," at scales up to 140 million passages. That is their figure, at web scale, on their engine.

**Opinion.** A few hundred milliseconds, or even a few tens, is the entire voice gap and more. Per-token storage also loses the knowledge-per-KB comparison in §4.4. Late interaction is a Lane C experiment with an abort at the 3 s cap, not a diary format. A Rust ColBERT engine was not found in the crate set this note was asked to map; absence from that set is not a proof that none exists.

### 6.3 SPLADE

**Fact.** Thibault Formal, Benjamin Piwowarski, and Stéphane Clinchant (Naver Labs Europe and Sorbonne), "SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking," arXiv:2107.05720 (2021). A BERT masked-language-model head predicts a weight for every vocabulary term. Sparsity regularization and a log saturation keep the vector sparse enough for an inverted index. The point, in their words, is to keep exact match and inverted-index retrieval, and to expand the document so vocabulary mismatch hurts less. SPLADE v2 is the follow-up preprint arXiv:2109.10086 (21 September 2021).

**CPU.** Two costs, both on the wrong side of a 5 ms voice slice. The query still runs a Transformer. The expanded query touches more postings than a raw BM25 query; the sparsity knob is how they trade that. XiaoIce's KG expansion is the symbolic ancestor of this idea (add related terms so the reply is not a parrot). Lane B does the expansion with a handful of stored tails and no BERT.

**Opinion.** SPLADE is the learned form of Lane B. It belongs in the same bucket as ColBERT: a side-band experiment, not the diary.

### 6.4 ANN structures: HNSW, IVF, DiskANN, product quantization

**Fact.**

- **HNSW.** Yu. A. Malkov and D. A. Yashunin, "Efficient and Robust Approximate Nearest Neighbor Search Using Hierarchical Navigable Small World Graphs." arXiv:1603.09320, 30 March 2016. Journal: *IEEE TPAMI*, April 2020, volume 42, pages 824–836, DOI `10.1109/TPAMI.2018.2889473`. A hierarchy of proximity graphs. Search starts at the top layer. The walk is random access over the vectors and the neighbor lists.
- **IVF and PQ.** Hervé Jégou, Matthijs Douze, and Cordelia Schmid, "Product Quantization for Nearest Neighbor Search," *IEEE TPAMI* 33(1):117–128, DOI `10.1109/TPAMI.2010.57`. A vector is split into sub-vectors, each replaced by a codebook id. Inverted-file indexes (IVF) probe a few coarse clusters and then scan those lists, often with PQ codes (asymmetric distance computation). FAISS is the library that made IVF-PQ the common implementation: Jeff Johnson, Matthijs Douze, and Hervé Jégou, "Billion-scale similarity search with GPUs," arXiv:1702.08734 (2017).
- **DiskANN.** Suhas Jayaram Subramanya, Devvrit, Rohan Kadekodi, Ravishankar Krishnaswamy, and Harsha Vardhan Simhadri, NeurIPS 2019, "DiskANN: Fast Accurate Billion-point Nearest Neighbor Search on a Single Node." A Vamana graph stored for SSD, for a set that does not fit in memory. Author order is the order on the NeurIPS PDF fetched for this note.
- **Matryoshka inside ANN.** AdANNS (NeurIPS 2023) uses a short prefix to pick IVF clusters and a longer prefix, or a heavier quantizer, to rescore. The savings assume the encoder was trained so prefixes work.

**Opinion.** A personal diary of 10⁵ to a few times 10⁵ cards of 256 int8 values sits in tens of megabytes. That is a sequential scan, which is exact and friendly to a single core's caches. HNSW becomes interesting when the scan's bandwidth no longer fits the 5 ms (or the 3 s, on Lane C). The V2 plan's crossover "about 200k items" is still their estimate. DiskANN and IVF-PQ start to pay when the set leaves RAM or when the vectors are wide float embeddings of a shared corpus. Starting the diary on either one spends complexity and recall on a problem the byte count does not pose. `rust-diskann` is not mature enough to be that start even if the byte count changes (955 downloads, 0 dependents, on 2026-11-18).

### 6.5 Binary, int8, Matryoshka

**Fact.** Covered in §4.1 and §4.4. Short form: int8 is 8 bits per dimension (4× against float32); binary is 1 bit (32×); Matryoshka is a training method that makes a prefix of the dimensions meaningful (Kusupati et al., 2022). The Hugging Face blog of 22 March 2024 is a practitioner's report, with a rescore step they attribute to Yamada, Asai, and Hajishirzi (2021). USearch and Qdrant are named on that blog as systems that can store the binary form.

**Opinion.** int8 is the diary's quantizer because it is a small, exact-scan-friendly cut and the V2 schema already has a place for the scale. Binary is the next cut if a recall check says the Hamming neighborhood is good enough, with a float or int8 rescore of the short list, which is the blog's own recipe. Matryoshka is unused until a model that was actually trained that way is chosen. Potion's PCA step is not that training.

### 6.6 Knowledge graphs and GraphRAG

**Fact.** XiaoIce's graph is co-occurrence-filtered Satori triples, used as query expansion, scored by a boosted tree, capped at 20 neighbors (§2). GraphRAG (Edge et al., 2024/2025) is an LLM-built entity graph plus hierarchical community summaries, queried by map-reduce (§4.2).

**Opinion.** Lane B is the XiaoIce graph, scaled down and labeled. GraphRAG's community summary is reused only as an extractive gist card. The LLM index build is out, because the brain is non-LLM on this path and the voice locks already ban third-party LLMs.

### 6.7 Small CPU embedding models

**Fact.** The models below are the ones the crates and cards actually name. Parameter counts are the publishers'. None of them were run here.

| Model | Who, date | Size they publish | Licence they publish | CPU character |
|---|---|---|---|---|
| Model2Vec / potion-base-8M and potion-retrieval-32M | Tulkens and van Dongen, 2024; MinishLab cards | 7.5M and 32.3M static parameters. Retrieval model: 512 × 63,091 (Mozilla mirror). | MIT for the software and for the citation block on the 8M card. 32M checkpoint field not opened. | Table lookup and a mean. The hot-path class. |
| MiniLM, and the sentence-transformers checkpoint `all-MiniLM-L6-v2` | Wenhui Wang, Furu Wei, Li Dong, Hangbo Bao, Nan Yang, and Ming Zhou, "MiniLM: Deep Self-Attention Distillation for Task-Agnostic Compression of Pre-Trained Transformers," arXiv:2002.10957 (submitted 25 February 2020), NeurIPS 2020. The checkpoint is the later UKP sentence-transformers distillation, listed by `fastembed`. | The V2 plan's model table lists that checkpoint as 22M parameters and 384 dimensions. | Apache-2.0, in that same table. | A real Transformer forward pass. The V2 plan cites third-party millisecond timings for it; those pages were not re-fetched. |
| `bge-small-en-v1.5` | BAAI C-Pack line. Shitao Xiao and colleagues, "C-Pack," arXiv:2309.07597 (September 2023). `fastembed` lists it as the default model. | 33M-class, 384-d, in the V2 plan's table. | MIT, in that same plan table. | Same Transformer cost class as MiniLM. Background re-embed or rerank, and only through a lock-safe host. |
| E5 small, GTE, Arctic-embed, EmbeddingGemma, Qwen3-Embedding | Named on the `fastembed` 7.7.1 page (2026-11-16). E5: Liang Wang and colleagues, arXiv:2212.03533 (2022). GTE: Zehan Li and colleagues, arXiv:2308.03281 (2023). | Not re-stated here. The Qwen3 sizes on that page go up to 8B. | Not re-checked model by model. | Anything with a Transformer block misses the voice slice. Qwen3 is also a third-party LLM-class encoder, which the 2026-11-04 owner ruling keeps out of the architecture. |

**Opinion.** The hot path embeds with the static table. A 22–33M Transformer is already the wrong order of cost for a partial that fires during speech, and it drags a forbidden runtime if it arrives through `fastembed` or `ort`. The useful job for bge-small or MiniLM is a rerank of a handful of hits on Lane C, in a process the locks allow.

### 6.8 Memory compression as a retrieval problem

**Fact.** XiaoIce's own-chat index is already a diary: by May 2018, 70% of replies came from past conversations (Zhou et al.). What they stored was filtered pairs, not raw logs, but a pair is still a full query and a full response, and they had billions of them. ColBERTv2's residual code (§6.2) is the academic compression result for late interaction, and it still leaves an order of magnitude more bytes per passage than one int8 vector. GraphRAG compresses by summarizing communities with an LLM.

**Opinion.** HORBM's compression is the card, not a quantizer trick. One sourced gist, one int8 vector, one simhash, and a triple only when the gist supports it. Scenes and community gists are cards that stand in for many turns, so a long friendship does not require the scan to touch every utterance. Binary quantization and Matryoshka are further cuts, available later, and they are not a substitute for refusing to store the raw transcript.

---

## 7. Latency budget

**Facts the budget is built from.**

- Product budget: 240 ms, end of speech to first content audio. The gap from ASR final to the reasoning request is 5 ms (`BUDGET.md`).
- Frame: 80 ms, with 60 ms of compute already shared by audio and the rest (`LOCKS.toml`). Retrieval is forbidden on that tick (`GALION-RESEARCH.md`).
- Prefetch window: the V2 plan runs retrieval when the stable prefix grows, and it cites a stability rule of two ticks, 160 ms.
- Human turn gap: Tanya Stivers, N. J. Enfield, and colleagues, including Stephen C. Levinson, "Universals and cultural variation in turn-taking in conversation," *PNAS* 106(26):10587–10592, 2009 (approved 28 April 2009). Across ten languages, the mean offset from a polar question to its answer is +208 ms. Each language's mode sits between 0 and +200 ms. The overall mode is 0 ms. The cross-language median is +100 ms. This is a measurement of human question–answer gaps, not of a machine, and it is restricted to polar questions.
- Research cap: 3 seconds (`GALION-RESEARCH.md`).

**Opinion (R1, 2026-11-18).** Targets for the diary, one core, on the order of 10⁵ cards. They are gates to measure later. This note did not measure them. The V2 plan's stage estimates (≤20 µs embed, ≤3 ms scan, ≤1 ms FTS5, ≤0.1 ms fuse) are the working split behind the 5 ms figure, and they remain estimates.

| Slice | Target | Why this number |
|---|---|---|
| Diary retrieval, p95, one core | ≤5 ms | Matches the V2 plan's G-MEM-1 estimate and the 5 ms gap. A miss may consume the gap. It may not consume the 120 ms reasoning budget or the 30 ms TTS budget. |
| Critical path at end of speech | 0 ms when the stable-prefix prefetch hit | The 160 ms stability window is the time the work is supposed to hide in. |
| Added at end of speech on a miss | ≤5 ms, and the worker aborts and publishes whatever it has if the deadline is inside the frame | The heartbeat's 60 ms is already spoken for. A late scan does not borrow it. |
| Graph expansion | Inside the same 5 ms, or deferred | Lane B is not a second turn. |
| Galion Research side band | ≤3 s, and the voice keeps the Lane A packet meanwhile | The persona cap. |
| Share of a human answer-gap | 5 / 208 ≈ 2.4% of the Stivers mean | The sense of "well under real time." The 240 ms product budget is the whole turn, not the retrieval slice. |

**Unknown.** Whether 5 ms p95 holds on the Galion box at 100k cards. The V2 plan marks the scan cost as an estimate derived from bandwidth, and it says the int8 scan was not measured. This note adds no measurement.

---

## 8. CPU bottlenecks, ranked, with the fix

**Opinion (R1, 2026-11-18).** Ranked by how likely each one is to break the 5 ms voice slice or the 80 ms frame, given the documents above. The rank is a judgment. The facts under each item are the cited ones.

1. **A Transformer query encoder on the voice path.** MiniLM, bge-small, E5, GTE, SPLADE, and ColBERT all run a BERT-class forward pass per query. The static model does not. The V2 plan already treats those as different orders of cost and cites outside timings for that; the orders, not the third-party milliseconds, are what this design uses. **Fix:** in-tree static table on Lane A. Encoder rerank only on Lane C, out of process, after a recall failure.

2. **Sharing the heartbeat core.** The owner table puts retrieval on core 0, which also runs the 80 ms frame. The persona text forbids retrieval on the tick. The V2 plan's §5.1 puts the context worker on core 7, which the owner table gives to Piper Jarvis. A scan inside `Heartbeat::step` will miss the 60 ms frame budget the moment the audio thread needs it. **Fix:** the worker may be pinned where the owner says, and it still must not run inside `step`. It is low priority, abortible, and it publishes `MemoryHit` or nothing. Core 6 and core 7 stay Piper during a live call. Resolving the core-7 contradiction is open question 1, not something this note decides.

3. **An ANN or a disk index for a diary that fits in cache.** HNSW walks neighbor pointers (Malkov and Yashunin). IVF-PQ adds codebook lookups (Jégou, Douze, and Schmid). DiskANN is an SSD design for a billion points (Jayaram Subramanya et al.). A 26 MB sequential int8 scan is the structure whose bytes are known. **Fix:** exact arena until a measurement, which this note does not have, shows the scan over the budget. First upgrade is `hnsw_rs`. `usearch` if the quantizer has to be native i8. DiskANN and LanceDB stay off the list until the corpus leaves RAM.

4. **Late interaction or SPLADE turned on as the diary.** Per-token vectors (20–36 bytes in ColBERTv2, before the inverted list) and a Transformer query. PLAID's authors' CPU figures at 140 million passages are tens to a few hundreds of milliseconds. **Fix:** not on Lane A. Lane C experiment with a 3 s abort.

5. **GraphRAG-style query-time summarization.** An LLM map-reduce over community summaries (Edge et al.) is neither non-LLM nor a 5 ms call, and the reference repository is in maintenance mode. **Fix:** extractive gist cards and a handful of labeled triples (Lane B). Expansion that does not fit is deferred, not dropped silently.

6. **A network or process hop.** `qdrant-client` and any HTTP memory server (the Open Mind notes in this repo describe a memory service) put a round trip on the slice. **Fix:** in-process FTS5 and the arena. The V2 plan already says no HTTP on the hot path. This note agrees.

7. **Cold text and index mutation on the turn.** The scan can know the ids while the gist is still a disk read, and a write lock from compaction stalls the next partial. **Fix:** the arena is resident; the gists for the current pool are touched by the prefetcher; inserts, simhash dedup, and triple updates run after the turn, with `remember`. Withdrawal wipes RAM (`GALION-ONE.md`, and `LOCKS.toml` `withdraw_wipes_ram_history`).

8. **The wrong quantizer, chosen for the ratio rather than the recall.** Binary's 32× cut is real as arithmetic, and the quality numbers attached to it are someone else's Wikipedia demo (Shakir, Aarsen, and Lee, 22 March 2024). Truncating a potion vector as if it were Matryoshka is unsupported (§4.4). **Fix:** int8 with an explicit scale. Revisit binary only with a Galion recall check. Do not truncate.

9. **Re-embedding every partial from scratch, including the unstable suffix.** **Fix:** the V2 rule, kept: embed when `stable_prefix_len` grows, not on every audio frame.

10. **A learned ranker over hundreds of candidates on the voice path.** XiaoIce's boosted tree (Wu et al., 2010) sits on top of 400+400 candidates plus a neural generator. That is a second model. **Fix:** RRF of two short lists, publish four. A tree, if it ever earns its keep, is Lane C.

Fusion itself is not on this list. RRF is a sum over ranks.

---

## 9. How the core meets the rest of HORBM

**Opinion, tied to the documents named.**

- **Short-term memory.** The current call's turns, in RAM, even when `remember` is deny. The V2 plan puts L0 turns in RAM for the session and on disk only with `remember`. Galion One may use that for the call that is happening, and may not claim a yesterday that was not stored. After withdrawal, RAM is gone.
- **Long-term memory.** Facts, scenes, and gist cards on disk, with `remember`. Scenes are the compression layer (§6.8).
- **Persona.** `persona_scope` on the card. Samantha and Jarvis are test slots. Galion One is the companion and reads the diary under consent. Galion Research reads the diary plus Lane C, and says which pile a sentence came from.
- **Galion Research and Galion One.** One store, two speeds. One publishes in ≤5 ms and keeps the voice alive. Research may add a Hermes note when the side band returns, as a later `MemoryHit` or `HermesNote`, which is the asynchronous pattern the V2 plan already describes. Research does not stall past 3 seconds.
- **MnE / Symphony TTS.** Retrieval produces text hits. MnE speaks text. The retrieval worker does not call the throat, the mouth, the nose, or the voice guard, and it does not open a buffer of samples.
- **Agentic sandboxes.** No sandbox trait was found in the files read for this note. The seam, until one exists, is the same `MemoryHit` bus: a sandbox may write a card only with the same consent snapshot and the same epistemic label, and it may read only cards its scope allows. A sandbox is not a way around `remember`.
- **Eureka Moment Pipelines.** Retrieval does not run them. It hands them a packet: the four hits, each with score, kind, who, when, source, epistemic label, and `took_us`. A pipeline that needs more evidence asks Lane C and lives with the 3 s cap. The Eureka spec for that packet was not found. `LOCKS.toml` cites `Q-EUREKA.Q15` only as a source of the consent defaults. The packet shape in §4.4 is a proposal.

---

## 10. Open questions

1. **Which core is the context worker?** Owner table of 2026-11-04: core 0, shared with the heartbeat. V2 plan §5.1: core 7. Core 7 is also Piper Jarvis in the owner table. This note refuses to run the scan inside the heartbeat step, and it does not pick the core.
2. **Does static-embedding recall hold on Galion dialogue,** especially names and short turns? The V2 plan lists this as unchecked. It is still unchecked. No dataset was built or downloaded here.
3. **Per-vector int8 scale versus per-dimension calibration.** The V2 schema has one scale. The 22 March 2024 quantization write-up says the calibration set changes int8 quality. Untested on this data.
4. **Are potion vectors safe to truncate?** Unknown. They are not identified as Matryoshka models by the cards read here.
5. **Language.** `potion-retrieval-32M` is listed as English. The tree's persona docs are in English, and the lab is in Europe/Zagreb. Whether the diary must retrieve other languages is unknown. A multilingual static model exists in the Minish table (`potion-multilingual-128M`, 128M parameters). Its dimension and its quality were not read.
6. **The 32M checkpoint licence field** was not opened. The project licence is MIT. Confirm the checkpoint before vendoring a table.
7. **Eureka packet schema.** Proposed in §4.4. No owner document read here defines it.
8. **Sandbox API.** Not found. The bus proposal in §9 needs an owner if sandboxes are a real caller.
9. **`qdrant-edge` BM25** is the V2 plan's statement. The API was not opened. **`rust-diskann`'s LICENSE file** was not opened; crates.io and the README disagree on the wording (MIT OR Apache-2.0 versus MIT).
10. **The 5 ms and 200k-item figures** remain the V2 plan's estimates. This note sets them as the target and does not confirm them.
11. **Lane B extractor quality** without a language model is unknown. If it is poor, the honest outcome is "no expansion," not a silent fall back to an LLM.
12. **World knowledge.** XiaoIce mixed unpaired news quotes into the same ranker as personal chat. Galion One's persona says the voice does not invent a clock or a weather report. Opinion: unpaired world facts are Lane C, not Lane A. That split is not an owner ruling.

---

## 11. What this note did not do

- No benchmark, no timing run, no index build, no model or dataset download, no paid API.
- No audio, and no CC0 donor material.
- No change to `LOCKS.toml`, the core map, the MnE crates, or the voice runtime.
- Third-party millisecond figures cited inside the 2026-11-03 Symphony V2 plan (Tantivy’s Wikipedia bench, LanceDB’s FAQ, usearch’s QPS table, MiniLM and bge ONNX timings, model2vec-rs microbenchmarks) were not re-fetched. They are not repeated here as checked facts.
- Crate download counts, star counts, and versions are a snapshot of pages fetched on 2026-11-18.

---

## 12. Sources

Repository documents, this tree:

- `LOCKS.toml`, `locked_at` 2026-11-02; Piper `ort` exception widened 2026-11-04, owner Maciej Grajczyk, decision t613s2992.
- `docs/galion-voice-v2/sprint-v2/latency-240/BUDGET.md`, owner ruling 2026-11-04 20:45 CET.
- `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`, written 2026-11-03.
- `docs/galion-voice-v2/personas/GALION-ONE.md`, `GALION-RESEARCH.md`.
- `tools/symphony/mne/README.md`.

Papers and primary write-ups:

- Zhou, Gao, Li, and Shum. "The Design and Implementation of XiaoIce, an Empathetic Social Chatbot." arXiv:1812.08989, December 2018. *Computational Linguistics*, 2020, ACL Anthology `2020.cl-1.2`.
- Zhang, Wu, Wang, Zhou, and Li. WSDM 2016. Distributed representations for question retrieval.
- Wu, Wang, and Xue. COLING 2016. Ranking chatbot responses.
- Wu, Burges, Svore, and Gao. 2010. "Adapting boosting for information retrieval measures." *Information Retrieval* 13(3):254–270.
- Huang, He, Gao, Deng, Acero, and Heck. CIKM 2013. Deep structured semantic models.
- Shen, He, Gao, Deng, and Mesnil. CIKM 2014. Convolutional DSSM.
- Gao, Pantel, Gamon, He, and Deng. EMNLP 2014. "Modeling interestingness with deep neural networks."
- Robertson and Zaragoza. 2009. "The Probabilistic Relevance Framework: BM25 and Beyond." *Foundations and Trends in Information Retrieval* 3(4):333–389.
- Cormack, Clarke, and Büttcher. SIGIR 2009. "Reciprocal Rank Fusion outperforms Condorcet and individual Rank Learning Methods."
- Karpukhin, Oğuz, Min, Lewis, Wu, Edunov, Chen, and Yih. EMNLP 2020. "Dense Passage Retrieval for Open-Domain Question Answering." arXiv:2004.04906.
- Luan, Eisenstein, Toutanova, and Collins. arXiv:2005.00181, submitted 1 May 2020, v3 16 February 2021. "Sparse, Dense, and Attentional Representations for Text Retrieval."
- Khattab and Zaharia. SIGIR 2020, 25 July 2020, DOI `10.1145/3397271.3401075`. ColBERT.
- Santhanam, Khattab, Saad-Falcon, Potts, and Zaharia. NAACL 2022. ColBERTv2. arXiv:2112.01488.
- Santhanam, Khattab, Potts, and Zaharia. PLAID. arXiv:2205.09707v1, 19 May 2022. CIKM 2022, DOI `10.1145/3511808.3557325`.
- Formal, Piwowarski, and Clinchant. arXiv:2107.05720 (2021). SPLADE. Follow-up: arXiv:2109.10086, 21 September 2021.
- Malkov and Yashunin. arXiv:1603.09320, 30 March 2016. *IEEE TPAMI* 42:824–836, April 2020, DOI `10.1109/TPAMI.2018.2889473`. HNSW.
- Jégou, Douze, and Schmid. *IEEE TPAMI* 33(1):117–128, DOI `10.1109/TPAMI.2010.57`. Product quantization.
- Johnson, Douze, and Jégou. arXiv:1702.08734 (2017). FAISS, "Billion-scale similarity search with GPUs."
- Jayaram Subramanya, Devvrit, Kadekodi, Krishnaswamy, and Simhadri. NeurIPS 2019. DiskANN.
- Kusupati et al. arXiv:2205.13147 (2022). Matryoshka Representation Learning. Cited here through AdANNS (NeurIPS 2023), which uses it.
- Shakir, Aarsen, and Lee. Hugging Face blog, 22 March 2024. "Binary and Scalar Embedding Quantization for Significantly Faster & Cheaper Retrieval." They attribute the binary rescore to Yamada, Asai, and Hajishirzi (2021).
- Wang, Wei, Dong, Bao, Yang, and Zhou. arXiv:2002.10957, submitted 25 February 2020. NeurIPS 2020. MiniLM.
- Xiao et al. arXiv:2309.07597 (September 2023). C-Pack / BGE.
- Wang et al. arXiv:2212.03533 (2022). E5.
- Li et al. arXiv:2308.03281 (2023). GTE.
- Edge, Trinh, Cheng, Bradley, Chao, Mody, Truitt, Metropolitansky, Ness, and Larson. arXiv:2404.16130, v2 19 February 2025; Microsoft Research lists April 2024. GraphRAG.
- Stivers, Enfield, Brown, Englert, Hayashi, Heinemann, Hoymann, Rossano, de Ruiter, Yoon, and Levinson. *PNAS* 106(26):10587–10592, 2009. Turn-taking gaps.
- Tulkens and van Dongen. Model2Vec, 2024, MIT, DOI `10.5281/zenodo.17270888`, https://github.com/MinishLab/model2vec. Cards: `minishlab/potion-base-8M`; retrieval width from the Mozilla `static-embeddings` mirror of `potion-retrieval-32M`.

Code and crate pages, all fetched 2026-11-18:

- https://github.com/quickwit-oss/tantivy — MIT. crates.io `tantivy` 0.27.0.
- https://github.com/unum-cloud/usearch — Apache-2.0 via crates.io `usearch` 2.26.6.
- crates.io `hnsw_rs` 0.3.5 — MIT OR Apache-2.0. Repository `jean-pierreBoth/hnswlib-rs`.
- crates.io `lancedb` 0.41.0 — Apache-2.0. https://github.com/lancedb/lancedb.
- https://github.com/qdrant/qdrant — Apache-2.0. crates.io `qdrant-client` 1.20.0, `qdrant-edge` 0.9.0.
- crates.io `fastembed` 7.7.1 — Apache-2.0. https://github.com/Anush008/fastembed-rs.
- crates.io `candle-core` 0.11.1 — MIT OR Apache-2.0. https://github.com/huggingface/candle.
- crates.io `ort` 2.0.0-rc.15 — MIT OR Apache-2.0. https://github.com/pykeio/ort.
- crates.io `rust-diskann` 0.4.7 — licence wording unresolved, see open question 9. https://github.com/jianshu93/rust-diskann.
- https://github.com/microsoft/graphrag — MIT, maintenance-mode warning on the README.
