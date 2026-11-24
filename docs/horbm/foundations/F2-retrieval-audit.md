# F2 — Retrieval audit and the phase-2 cell

Perpetual Open MIT License. (c) 2026 Galion Labs.

Lead F2 of the HORBM foundations swarm. This note audits the retrieval pieces that already exist and records the stable API the int8 arena (F3) and the heuristic pick (F4) consume. The code that implements the API is `horbm/crates/horbm/src/retrieve.rs`. `Brain::turn` speaks the phase 4 pick in `horbm-brain` and does not read this cell.

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A design constant already in a named file. |
| **[M]** | A measurement already written down. This session did not repeat it. |
| **[C]** | A published source, named with who said it and when. |
| **[T]** | A target. Not a measurement from this session. |
| **[R]** | Read from code or a schema on the date in the sentence. Not a timing. |

## What this phase ships

R4 phase 2, from the unmerged note `docs/horbm/research/R4-cpu-realtime-integration.md` §11 on draft PR #207 (branch `cursor/horbm-r4-cpu-realtime-2180`): a retrieval thread, FTS5 in-process, a RAM fixture, consent deny by default, a cell publish, no vectors yet. The gate in that table: with `remember` denied, the cell is empty and nothing is on disk; a 1,000-row fixture returns in the test, untimed. **[T]**

The owner ruling of 2026-11-18, option A, is the one this phase follows: HORBM answers first, on the HORBM engine. That ruling is written up as the target design in `galion-knowledge` `v2/docs/MEMORY_PARITY_SPEC.md` §4 and shipped in `horbm-memory` (PR #16, merge `d5c4177b`). The spec's hard line is: Rust first. Do not reinvent the retrieval engine. Do not call `openmind-memory` over HTTP from the voice path. Do not vendor the Node or Python pipeline. **[D]**

R4 named `openmind-memory`'s schema as the dependency and said not to use its HTTP server. The parity spec, which is the later owner choice, says the voice path uses `horbm-memory` in-process instead. This phase links `horbm-memory`. It does not link `openmind-memory`. The schema audit below is so F3 and F4 can see what was not ported.

`LOCKS.toml` and `SOUL.md` are not edited. `SOUL.md` is not in this repository.

## Research notes read

| Note | Where | What it asks of retrieval |
|---|---|---|
| R1 | branch `cursor/horbm-r1-retrieval-core-690c`, `docs/horbm/research/R1-retrieval-core.md` | BM25 plus one static int8 vector per consented card, RRF, at most four hits on a `MemoryHit` cell, who / when / source / epistemic kind on every hit. The 5 ms p95 is a target. **[T]** |
| R2 | branch `cursor/horbm-r2-ranking-eureka-906f`, `docs/horbm/research/R2-ranking-generation-eureka.md` | The pick replaces XiaoIce's GRU-RNN and boosted tree. It runs beside the heartbeat, not inside `Heartbeat::step`. |
| R3 | branch `cursor/horbm-r3-memory-persona-3e32`, `docs/horbm/research/R3-memory-persona-empathy.md` | Persona and empathy are filters and a mood point. They are not a second index. |
| R4 | PR #207, not merged | Phase 2 as above. The heartbeat only reads a cell. Retrieval is its own std thread. |

None of R1–R4 are on `main`. This file is the foundations note that is.

---

## 1. `horbm-memory`

**Path.** `galion-studio/galion-knowledge`, package `horbm-memory`, sources in `v2/memory`. Master at the time of the audit was `761a7f225304fc010379219e5b88c6669dd48f40`. The voice pin is the git revision in `horbm/crates/horbm/Cargo.toml` (repository root manifest, `lib` path `v2/memory/src/lib.rs`). PR #16 merged `d5c4177b` (hybrid voice memory, `user_version` 5).

**Real behaviour.** `Store` is one SQLite file, `PRAGMA user_version` 5. **[R]** `v2/memory/src/store.rs`. Tables: `items`, `items_fts` (FTS5, `unicode61`), `items_fts_vocab`, `archive`, `card_revisions`, `gaps`, `session_turns`, `session_turns_fts`, `session_meta`, `memory_jobs`, `memory_revisions`, `meta`. `items.layer` is `semantic`, `episode`, or `diary`. Pins, importance, extractor kind, and `current` are columns. `emb` is a 256-d int8 blob (`EMB_DIM` 256) because the column is `NOT NULL`. **[D]**

`Store::open_in_memory` uses a shared-cache URI `file:horbm-mem-<pid>-<n>?mode=memory&cache=shared`. **[R]** `Store::open` creates a file and parent directories. `Stack::open` also creates the knowledge directory and `revisions.sqlite`. The voice thread calls neither of those. It calls `open_in_memory`.

`Store::search` and `Store::search_memory` fuse FTS5 with an exact scan of those int8 blobs and the RAM ring (`SessionRing` cap 32), reciprocal rank fusion `k = 60`, then truncate to `MAX_HITS` 4. **[D]** `Store::search_lexical` is the same path with the int8 scan omitted. The voice thread calls `search_lexical`. Upsert still writes the blob, because the schema requires it. That is storage, not a second arena.

`Stack::query` searches memory and knowledge as two lists, applies the covering rule (every content token of the query is in the haystack; an empty term list misses), and on a knowledge miss sets the answer to `I don't know` and enqueues a gap. **[R]** `v2/memory/src/stack.rs`. The voice thread does not call `Stack::query`, so it does not create `revisions.sqlite` and it does not enqueue gaps. It repeats the covering predicate on the lexical hits and speaks `UNKNOWN_REPLY` (`I don't know.` with the period the voice crate already uses). **[D]** `horbm/crates/horbm/src/route.rs`.

`push_turn` updates the ring and enqueues extract, transcript, and summary work on a background worker. `remember` pins a fact. The crate comment says consent is given by use and there is no withdrawal command. **[R]** `v2/memory/src/lib.rs`. That disagrees with `LOCKS.toml`. The voice thread does not call `remember` or `push_turn`. Withdrawal is `RetrievalThread::set_remember(false)`, which drops the in-memory store.

**Tests.** `v2/memory/tests/stack.rs`, `tests/memory_a.rs`, `tests/whole.rs`, and unit tests in the crate. `whole.rs` uses 40 rows in debug and 200,000 in release. **[R]** The release numbers in PR #16, not re-run here: selective p95 6.0 ms and broad p95 8.3 ms at 200,000 cards with a background writer. **[M]** A new unit test covers `search_lexical` on one synthetic card.

**Status.** Library and CLI, MIT, `rust-version` 1.83, `rusqlite` 0.32 with the bundled amalgamation (no system SQLite). CPU only. The int8 dot has an AVX2 path behind `target_arch = "x86_64"` and a scalar fallback. **[R]** `v2/memory/src/embed.rs`. This phase does not add SIMD.

**Gaps.** No public FTS-only method existed before `search_lexical`. `Stack::open` writes into the card directory, so the voice must not use it on a read-only pack. The crate does not know `LOCKS.toml`. Persona filter and `share` are not columns.

## 2. Cards, manifest, shards

**Path.** Cards: `galion-knowledge` `v2/data/cards` (1,506 JSON files on `761a7f2`). Shards: `v2/shards` (160 card JSON files plus per-shard `shard.json`). Manifest: `v2/shards/manifest.json`. Schema: `v2/data/schema.json`. The laptop copy is built by `scripts/sync-horbm-knowledge.sh` and `scripts/windows/Sync-HorbmKnowledge.ps1`. Default Windows directory: `G:\galion\horbm\knowledge`. **[D]** `horbm::WINDOWS_KNOWLEDGE_DIR`. Env: `GALION_HORBM_KNOWLEDGE`.

**Real behaviour.** A card is one JSON object. Required fields in `schema.json`: `id`, `title`, `section`, `category`, `body`, `claim_type` (`fact`, `interpretation`, `opinion`, `contested`), `sources`, `evidence_strength`, `said_by`, `as_of_date`, `status`, `contested_views`, `related`. **[D]** `horbm-memory` `card.rs` accepts the same object, plus `summary`, `tags`, aliases, and a few research aliases (`text` for `body`, `gist` for `summary`). The id pattern is a slug. Section is `world`, `horbm`, or `diary` in the outline; the schema allows another word and files the card by folder.

The manifest schema string is `galion-shard-manifest/1`. `core_pack.payload` is `card-bundle/1`. `core_pack.card_count` is 1,506. `core_pack.bytes` is 3,060,127. **[D]** Many shard entries are catalogs: `card_count` 0 and the empty-blob sha256. `first-aid` is seeded (57 cards). The manifest note says the sha256 is the card-bundle digest, not a SQLite file hash, and that shard files are not edited in place.

`horbm::KnowledgeCards::load` walks `*.json`, skips `manifest.json`, `catalog.json`, and `schema.json`, checks the manifest when present, and builds an in-memory covering index. It does not create directories and does not open SQLite. **[R]** `horbm/crates/horbm/src/cards.rs`. Tie on the smallest haystack is a miss. That loader is the 7881 path. This phase does not replace it.

The retrieval thread, when granted and `fixture_rows` is 0, calls `Store::ingest_dir` on the card directory. That reads JSON into the RAM diary. It does not write the directory.

**Tests.** `horbm/crates/horbm/tests/cards.rs` and the unit tests in `cards.rs`. `tests/pack_bench.rs` runs only when `GALION_HORBM_KNOWLEDGE` is a real pack. The new ignored test `real_card_folder_is_read_and_not_written` ingests that directory into RAM and asserts the directory listing is unchanged.

**Status.** The pack on master is the card source. The voice loader is read-only and already matches the covering rule. The retrieval thread's folder path is opt-in.

**Gaps.** Ingesting 1,506 cards into RAM SQLite duplicates the index `KnowledgeCards` already built. The two answers can differ: the loader breaks ties by refusing, `search_lexical` returns the FTS order after the covering filter. F4 should treat them as two cells, not one merged ranking. Shard SQLite (`ScaleIndex`) is not opened.

## 3. Truth gate

Two different gates share the name.

**Publish gate.** **Path.** `galion-knowledge` `v2/tools/truth-gate`. **Behaviour.** A card is publishable when `evaluate` reports no hard failure. The gate does not invent a source, raise an evidence label, or delete a card. **[R]** `v2/tools/truth-gate/src/lib.rs`. Hard codes read from `check.rs`: `invalid_json`, `schema`, `id_filename`, `duplicate_id`, `section_mismatch`, `banned_source`, `ip_address`, `bad_url`, `strength_without_sources`, `none_with_sources`, `evidence_above_ceiling`, `fact_without_real_url`, `strong_without_real_url`, `contested_without_views`, `unsourced_has_sources`. Warnings stay in the report. **Tests.** `v2/tools/truth-gate/tests/rules.rs`. CI job `v2-truth-gate.yml` runs `cargo test --locked` and then the gate on `v2/data/cards` and `v2/shards`. **Status.** The checked-in report `v2/docs/truth-gate-report.md` is dated 2026-11-18 and says 436 cards, 0 hard failures, 214 warnings. **[M]** The card directory at the same commit has 1,506 JSON files. The report is older than the pack. This session did not re-run the gate.

**Query gate.** Every content token of the ask must appear in the hit haystack. Stopwords are dropped first (`index_tokens`). An ask that is only stopwords misses. **[D]** Implemented in `Stack::query` and copied as `covers` in `retrieve.rs` so the voice path does not have to call `Stack::open`. A miss publishes `answer = "I don't know."` and no hits. That is not the publish-time truth gate.

**Gaps.** The query path does not run `truth-gate`. A card that would fail the publish gate is still retrievable if it is in the folder. The stale report means a reader of `truth-gate-report.md` does not see the current pack.

## 4. `openmind-memory` (obsidian-open-mind)

**Path.** `galion-studio/obsidian-open-mind` `crates/openmind-memory`, read from `main` on 2026-11-24. **[R]**

**Real behaviour.** SQLite, `PRAGMA user_version` 4. **[D]** `store.rs` `SCHEMA_VERSION`. Tables in `SCHEMA_SQL`: `embedding_meta`, `l1_records` (content, type, priority default 50, scene, session, timestamps, `metadata_json`), `l0_conversations` (session, role, `message_text`, `recorded_at`, integer `timestamp`), FTS5 `l1_fts` and `l0_fts`. v3 adds an index on `l0_conversations.timestamp`. v4 adds `idx_l0_session_ts`. The file header says vec0 tables are never created and the live gateway reports `embeddingService: false`. Recall is HTTP (`src/http.rs`, `src/engine.rs`), one thread per connection. The parity spec records `busy_timeout` 5,000 ms and no published p95. **[D]** `MEMORY_PARITY_SPEC.md` §1.2–1.3.

**Tests.** The crate's own tests, including a user_version stamp test around `store.rs`. Not run in this session.

**Status.** This is the Galion Research long-term memory gateway. The voice crate does not depend on it. `horbm-memory` kept the FTS5 idea and did not copy the L0/L1 schema, because that schema has no provenance fields the voice hit requires. **[D]** parity spec §5.

**Gaps.** No in-process API the voice can call without also taking the HTTP types and the workspace crates (`galion-common`, `nerve-client`, and the rest). Linking it would fight the parity spec. `share` and persona files live over there and are not in the cell.

## 5. How voice links it in-process

**What was already here.** `horbm::Brain::open(memory_db, knowledge_dir, cases)` ignores `memory_db` and `cases` and loads `KnowledgeCards`. **[R]** `horbm/crates/horbm/src/brain.rs`. On `main` at `8caa52a`, that turn then asks `horbm-brain` for one stored clause (first piece, rest, side-band mark). That retriever is the committed fixture, `horbm_brain::Retriever`, not this cell. `galion-sam-horbm` always depends on `horbm` by path. Feature `horbm` only compiles `HorbmEngine`. The seam crate `voice-stack/horbm-bridge/seam/horbm` is not the dependency. `CannedLlm` is `crates/galion-cascade/src/canned.rs`. It is not on the 7881 card path.

`crates/galion-heartbeat` `Heartbeat::step` must not block or allocate. There is no `MemoryHit` variant on the console bus. The Symphony plan sketched `BusBody::MemoryHit { hits, query_sig, took_us }`. **[D]** That sketch is not compiled. This phase publishes its own cell in the `horbm` crate so F4 can read it without a bus change.

Consent in the lock: bits `sense`, `adapt`, `remember`, `share`, default deny, `remember = "deny"`, `withdraw_wipes_ram_history = true`. **[D]** `LOCKS.toml` `[consent]`. `crates/galion-consent` is the clone-time clip gate, not this bit. The emotion wire has a `remember` bit (`galion-emotion` `consent_bits::REMEMBER`). The retrieval thread does not read that wire yet.

**What this phase adds.** `RetrievalThread` in `horbm::retrieve`. One std thread named `horbm-retrieval`. No async runtime, no HTTP, no socket. When `remember` is false the thread does not call `Store::open_in_memory`. When it is true the thread opens an in-memory store, loads either `fixture_rows` synthetic cards or, if that count is zero, ingests `knowledge_dir`. Asks call `search_lexical`, apply `covers`, and publish at most 4 hits. `set_remember(false)` drops the store and publishes an idle cell.

The dependency is a git revision of `galion-knowledge`, not a copied tree and not a path that only exists on one laptop.

**Tests.** `horbm/crates/horbm/tests/retrieve.rs`. Denied: cell stays idle after an ask, including when `fixture_rows` is 1,000, and the scratch directory, the process cwd, and the temp directory gain no sqlite or `horbm-mem` file. Granted: the 1,000-row fixture returns `sample-742` for `m742`, a nonsense token is `I don't know.`, `ask` without waiting still publishes, withdraw clears the cell, and a later ask stays idle. Untimed. **[T]** for the 5 ms figure, which this test does not claim.

**Status.** Built. Not read by `Brain::turn`, the 7881 server, or `Heartbeat::step`. The phase 4 pick on that turn is a separate fixture retriever.

**Gaps.** See the five below. Also: an outstanding `Arc` from `cell()` is not chased on withdraw. Drop it when the bit falls.

---

## 6. Stable API for F3 and F4

Module `horbm::retrieve`. Re-exported from `horbm`. Do not depend on `horbm_memory` from F3 or F4. The diary crate can change; this surface is the contract.

```rust
pub const MEMORY_HIT_CAP: usize = 4;

pub struct MemoryHit {
    pub item_id: String,   // card id, or ep-* / turn-* ; not the plan's u64
    pub pool: HitPool,     // Knowledge or Memory
    pub title: String,
    pub text: String,      // gist
    pub said_by: String,
    pub date: String,
    pub source: String,
    pub claim_type: String,
    pub score: f32,
    pub pinned: bool,
}
impl MemoryHit {
    pub fn clause(&self) -> &str; // text, or title when text is empty
}

pub enum CellState { Idle, Miss, Hits }

pub struct MemoryHitCell {
    pub state: CellState,
    pub epoch: u64,
    pub turn_id: u64,
    pub query: String,
    pub hits: Vec<MemoryHit>, // len <= MEMORY_HIT_CAP
    pub answer: String,       // "" or "I don't know."
    pub took_us: u32,
}
impl MemoryHitCell {
    pub fn is_empty(&self) -> bool; // Idle, no query, no answer, no hits
}

pub trait RetrievedCells: Send + Sync {
    fn cell(&self) -> Arc<MemoryHitCell>;
    fn try_cell(&self) -> Option<Arc<MemoryHitCell>>;
}

pub trait Int8Scorer: Send + Sync {
    fn score(&self, item_id: &str, query: &str) -> Option<i32>;
}

pub struct RetrievalConfig {
    pub remember: bool,
    pub knowledge_dir: Option<PathBuf>,
    pub fixture_rows: usize,
}

pub struct RetrievalThread;
impl RetrievalThread {
    pub fn spawn(config: RetrievalConfig) -> Result<Self, RetrieveError>;
    pub fn ask(&self, text: impl Into<String>, turn_id: u64, epoch: u64);
    pub fn ask_wait(&self, text: impl Into<String>, turn_id: u64, epoch: u64) -> Result<(), RetrieveError>;
    pub fn set_remember(&self, grant: bool) -> Result<(), RetrieveError>;
    pub fn last_error(&self) -> Option<String>;
}
```

`RetrievalThread` implements `RetrievedCells`.

**F4 (heuristic pick).** `horbm-brain` already picks one fixture clause inside `Brain::turn` and does not call this cell. A later wiring can read `cell()` or `try_cell()` off the heartbeat thread. `Idle` means do not speak a retrieved clause (deny, withdraw, or no ask yet). `Miss` means speak `answer` (`I don't know.`). `Hits` means pick one `clause()`. This cell does not truncate to two words; `horbm-brain` does that on its own clause. A newer `epoch` replaces an older one. An in-flight ask is dropped when a consent command is queued. Do not call `ask` inside `Heartbeat::step`.

**F3 (mapped int8 arena).** `crates/horbm-keys` is the mapped arena on `main`. This thread does not link it and does not call `Int8Scorer`. Do not scan vectors inside the retrieval thread. `search_lexical` already skipped the diary's hashing scan. Key the arena by `MemoryHit.item_id` as a string. `score` returning `None` means no row. SIMD stays behind the existing `galion-tensor` features, with a scalar fallback. This crate is `forbid(unsafe_code)`.

**Consent.** `RetrievalConfig::default().remember` is false. `from_env` / `from_flags` grant only when the flag trims to exactly `grant`. `GALION_HORBM_KNOWLEDGE` is the directory when set, even if it does not exist yet (`spawn` then returns the diary error). When the variable is unset, `G:\galion\horbm\knowledge` is used only if it already exists. `fixture_rows > 0` skips the directory. `set_remember(false)` is the withdraw for `withdraw_wipes_ram_history`.

**Epoch.** `accept_cell` keeps the published cell when the incoming epoch is lower, unless the publish is a withdraw or a denied ask (those force idle).

## 7. Windows build

Rust 1.85 or newer, matching `horbm`'s `rust-version`. The git dependency compiles bundled SQLite, so the laptop needs the MSVC C++ build tools once (`cl.exe`). No model download, no audio file, no API key.

From the `galion-one-voice` repo root:

```powershell
cargo test --manifest-path horbm/Cargo.toml -p horbm --test retrieve
cargo test --manifest-path horbm/Cargo.toml -p horbm
```

Against the real card folder (read-only; the test checks the directory listing does not change):

```powershell
$env:GALION_HORBM_KNOWLEDGE = "G:\galion\horbm\knowledge"
$env:GALION_HORBM_REMEMBER = "grant"
cargo test --manifest-path horbm/Cargo.toml -p horbm --test retrieve -- --ignored --nocapture
```

Leave `GALION_HORBM_REMEMBER` unset to keep the lock default. The cell stays empty and the folder is not opened.

Clippy, the same bar as `.github/workflows/voice-stack-horbm-router.yml`:

```powershell
cargo clippy --manifest-path horbm/Cargo.toml -p horbm --all-targets -- -D warnings
```

## 8. Top 5 gaps

1. `Brain::turn` speaks the `horbm-brain` fixture pick and still does not read `MemoryHitCell`. The 7881 bridge follows that turn. The diary cell and the fixture retriever are not joined.
2. `crates/horbm-keys` is the mapped int8 arena, and this thread does not call it. `Int8Scorer` still has no implementor here. The diary still stores a 256-d int8 blob per row because the column is `NOT NULL`. Those bytes are not scanned on this path.
3. `remember` is not read from the emotion-wire bit. The grant is `RetrievalConfig` or `GALION_HORBM_REMEMBER=grant`. Withdraw on the thread drops the RAM store; it does not yet see `EmotionWire`.
4. The card folder and the retrieval thread are two indexes. `KnowledgeCards` answers the current turn. The thread ingests the same JSON into RAM SQLite only when granted. They can disagree on ties. `ScaleIndex` shard files are not opened, and `Stack::open` must stay unused on the read-only pack because it writes `revisions.sqlite`.
5. `share`, persona scope, and the learn pass are not in the cell. `openmind-memory` still owns the Research HTTP schema (`user_version` 4, L0/L1, no provenance). The checked-in truth-gate report (436 cards, 2026-11-18) does not describe the 1,506-card pack. Query-time covering is not the publish-time truth gate.
