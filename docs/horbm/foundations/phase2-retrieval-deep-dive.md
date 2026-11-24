# Phase 2 retrieval deep dive

Phase 2 from the HORBM R4 note (`docs/horbm/research/R4-cpu-realtime-integration.md` on PR #207, branch `cursor/horbm-r4-cpu-realtime-2180`, not merged). Owner Q1, settled 2026-11-18, option A: HORBM retrieval answers first on the hot path. A small local S0 model fills gaps marked as guesses later. Galion Research (the Rust engine in `obsidian-open-mind` crates `hermes-rs` and `hermes-gateway`) handles hard questions off the tick with a 3 second cap. This phase does not call Galion Research, does not load a model, and does not speak.

`LOCKS.toml` and `SOUL.md` are unchanged.

## Gate

With `remember` denied, the cell is empty and nothing is on disk. A 1,000-row fixture returns in the test, untimed.

Proving command, from the repository root:

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo test -p horbm-retrieval -- --test-threads=1
```

The test is `crates/horbm-retrieval/tests/phase2_gate.rs`, `remember_denied_cell_empty_nothing_on_disk_and_fixture_returns`. It does not read a clock. `cell.generation` must be at least 1, so an untouched empty cell is not a pass. The 1,000 rows are inserted into the RAM index. The query for `t0042` returns `fixture.row.0042`. `HORBM_MEMORY_DB` and `revisions.sqlite` stay absent. The same run loads the committed two-card fixture and checks its `galion-shard-manifest/1` digest.

A second test in that file grants `remember`. The cell then holds the anchor card. Phase 2 still does not create a database file. Disk writes of session memory are phase 7.

## What this phase does not build

| Phase | Left alone |
|---|---|
| 1 | Domain 04 stamps on the cascade |
| 3 | Mapped int8 arena, scalar dot, SIMD, prefault |
| 4 | Heuristic pick, spoken clause, Piper |
| 5 | Eager end-of-turn, epoch rollback, barge-in |
| 6 | Galion Research side band, 3 second cap test |
| 7 | Learn pass, pack swap, withdrawal deletes on disk |
| 9 | Timed eval, recall@5, p95 |

Swarm 4 calls `horbm_retrieval::query`. This crate does not choose a spoken line.

## What already existed in galion-one-voice

| Path | What it is | Works | Stub or gap |
|---|---|---|---|
| `horbm/crates/horbm` | One turn: empathy, then cards, then the research gateway, then `I don't know.` | `KnowledgeCards` reads JSON, checks `galion-shard-manifest/1` / `card-bundle/1`, covering token lookup. Tie is a miss. | `Brain::open(memory_db, knowledge_dir, cases)` ignores `memory_db` and `cases`. Comment says `Stack::open` would create `revisions.sqlite`. |
| `voice-stack/horbm-bridge` | 7881 card engine. Path-depends on `horbm`. | Same covering lookup, plus a legacy token-count `CardBrain` for markdown. | `GALION_HORBM_MEMORY` is documented and not opened. |
| `voice-stack/local-server/src/brain.rs` | Picks the knowledge directory. | `GALION_HORBM_KNOWLEDGE`, else `G:\galion\horbm\knowledge` when that folder exists, else the example kettle card. | Does not open `horbm-memory`. |
| `scripts/windows/Sync-HorbmKnowledge.ps1` | Copies the public card pack. | Sparse checkout of `v2/data/cards` and `v2/shards/manifest.json` into `G:\galion\horbm\knowledge`. | Cards are not committed here. |
| `crates/galion-consent` | Clone-time reference-wav gate. | `evaluate`, `clone_gate`. | Not the session `remember` bit. |
| `galion-emotion` `band.rs`, `store.rs` | In-band consent bits and affect persistence. | `REMEMBER = 1 << 2`. `AffectStore` starts unconsented. Clearing `REMEMBER` wipes the baseline. | Affect only. No card index. |
| `crates/galion-openmind` `seam.rs` | Optional `turns.jsonl`. | `GALION_OPENMIND_REMEMBER` unset means deny. | Not the knowledge schema. |
| `crates/galion-cascade` `offline.rs` | BM25 over desk files, std only. | A fallback helper. | Not the HORBM brain. |
| `crates/galion-tensor` | Scalar GEMV, SIMD behind features. | Default build is scalar. | No FTS. Phase 3 may use it. This phase does not. |
| Root `Cargo.toml` | Voice workspace. | `horbm` is excluded so the root `cargo tree` stays clear of that subgraph. | No `rusqlite` anywhere before this phase. |
| `scripts/check-locks.sh` | Lock file gate. | `[consent]` bits stay `"deny"`. | Does not scan crate names. |
| `scripts/check-no-forbidden.sh` | Forbidden runtimes. | `candle`, `ort`, `tract`, `burn`, `llama`, `ggml`. | Scoped to `crates/galion-tensor` by default. |

No lock-free snapshot cell existed on the voice path. The orchestrator uses `tokio::sync::watch` and atomics. Phase 2 uses `std::thread` and `arc-swap`, not tokio.

## horbm-memory (reuse)

Repository `galion-studio/galion-knowledge`, package `v2/memory`, name `horbm-memory`, MIT, rust-version 1.83, edition 2021. Merged in PR #16 (`d5c4177b4aa61592b491080f90d76b1ec6ef4690`, 2026-11-18, "Add hybrid voice memory with a 10 ms query gate"). Current master used here is `761a7f225304fc010379219e5b88c6669dd48f40` (2026-11-19, PR #35). That tree still contains the PR #16 stack: FTS5, pins, revisions, session summaries, and the int8 leg.

The repo root is a Next.js app. It has no workspace `Cargo.toml`, so a git dependency on the repository does not resolve `horbm-memory`. The crate is vendored at `third_party/horbm-memory` and excluded from the voice workspace. The only edit is a re-export of `index_tokens`, `is_stopword`, and `tokens` from the private `text` module. Upstream tests are not part of `cargo test --workspace`.

Public surface that matters:

| API | Behaviour |
|---|---|
| `parse_cards`, `Card`, `Card::gist`, `Card::lexical_body` | Knowledge card JSON. `source` is the first source title and URL. |
| `Store::open_in_memory` | Shared-cache memory URI, then a background job thread. |
| `Store::open` / `Stack::open` | Creates the database file, parent directories, and `knowledge_dir/revisions.sqlite`. |
| `Store::search` / `Stack::query` | FTS5 BM25 fused with one int8 vector per row (hashing embedder, 256-d, AVX2 behind runtime detection with a scalar fallback). At most 4 hits. |
| `items_fts` | `title, summary, body, said_by, source`, `tokenize = 'unicode61'`. |

`v2/docs/MEMORY_STACK.md` records a release measurement on 2026-11-18, 200,000 generated cards, `Stack::query`: selective cold p95 4,722 µs, and 6,043 µs with a writer running. Broad p95 is about 8.4 ms. That clock is not this phase. Phase 2 is untimed. Those numbers are also not a measurement of the 1,506-card pack.

Phase 2 does not call `Stack::open` or `Store::search`. `Stack::open` would write `revisions.sqlite` even when `remember` is denied. `Store::search` always runs the int8 scan, which is phase 3 ("No vectors yet"). The retrieval crate uses the card parser, the tokenizer, and the same `items_fts` column list. The vector columns are not created. `Connection::open_in_memory` plus `temp_store=MEMORY` is the database. After the rows are inserted the connection is set `query_only`.

## openmind-memory (reuse the FTS shape, not the server)

Repository `galion-studio/obsidian-open-mind`, crate `crates/openmind-memory`, read at `0b0a898695b0525b179b5dd791bbfa8c281b7ffc`. MIT. Description: SQLite FTS5 BM25 recall, offline, std HTTP, an API stand-in for the memory gateway Galion Research uses.

| Piece | Role |
|---|---|
| `src/store.rs` `SCHEMA_SQL` | `l0_conversations`, `l1_records`, `l0_fts`, `l1_fts`. Not knowledge cards. The file says vec0 tables are never created. |
| `search_l1_fts` | `bm25(l1_fts) AS rank` … `ORDER BY rank ASC`. In-process. |
| `src/fts.rs` `build_fts_query` | Quoted tokens joined with `OR`. `bm25_rank_to_score` maps a negative BM25 rank into `(0, 1]`. |
| `src/http.rs`, `src/main.rs`, `src/api.rs` | The HTTP server. The voice path must not call it. |

`horbm-memory` `text.rs` already says its query is that quoted-OR shape without the CJK dictionary. Phase 2 keeps that shape and the same score map. It does not depend on `openmind-memory`: the crate's `Cargo.toml` path-depends on `galion-common`, `galion-feature-pin`, `nerve-client`, and `zagreb-time`, so it is not a standalone git dependency, and linking it would pull the HTTP binary into the voice build. No code in `crates/horbm-retrieval` opens a socket.

## Knowledge store format

The laptop pack is the directory `Sync-HorbmKnowledge.ps1` writes:

- `G:\galion\horbm\knowledge\manifest.json` copied from `v2/shards/manifest.json`
- `world/`, `horbm/`, and `diary/` when those card directories exist

`manifest.json` is schema `galion-shard-manifest/1`. `core_pack.payload` is `card-bundle/1`. The digest, from `v2/docs/SHARDS.md` and `horbm/crates/horbm/src/cards.rs`, is every `*.json` except `manifest.json`, `catalog.json`, and `schema.json`, sorted by relative path with `/` separators:

```text
<relative path utf-8>
<newline>
<raw file bytes>
<newline>
```

On the public pack at the pin above, that digest is `5dfa2a00ed3e64e079accae8f5a8ffb05cd2ffd37f7141bd6d7979fb3a60b44f`, 3,060,127 bytes, `card_count` 1506. A loader run against that layout returned `cards: 1506`, `manifest: sha256 ok`.

Environment:

| Variable | Use |
|---|---|
| `GALION_KNOWLEDGE_HOME` | First choice. If set and not a directory, the run fails. It does not fall through. |
| `GALION_HORBM_KNOWLEDGE` | Second choice. Same rule. This is the name the 7881 server already uses. |
| `G:\galion\horbm\knowledge` | Used only when it is already a directory and neither variable is set. Nothing is created. |
| `HORBM_MEMORY_DB` | Stamped and not opened. With `remember` denied the file is not created. If it already exists, length and mtime must stay the same. |

`--remember` on the binary is the only grant. No environment variable turns `remember` on. Default is deny, matching `LOCKS.toml`:

```toml
[consent]
default = "deny"
remember = "deny"
withdraw_wipes_ram_history = true
```

The gate test reads that section and fails if `remember` is no longer `"deny"`.

The committed text fixture is `crates/horbm-retrieval/fixtures/mini/` (two JSON cards plus a manifest). The 1,000-row fixture is built in the test, in RAM, and is not a file.

## Gaps and risks

- The 7881 path still answers with the covering lookup in `horbm`. Phase 4 is what should call `query`. Two matchers will disagree until that wire exists. Swarm 4 should use `RetrievalHit`, not `CardBrain`.
- `Store::search` remains in the vendored crate and still fuses vectors. Calling it from a later phase would skip the phase 3 gate. This crate does not call it.
- The vendored tree can drift from `galion-knowledge` master. The pin is `761a7f225304fc010379219e5b88c6669dd48f40`.
- `rusqlite` with `bundled` compiles SQLite from the amalgamation. Windows needs the MSVC C toolchain. Linux needs a C compiler. There is no system-sqlite feature and no network in the tests.
- A pack whose `manifest.json` does not match the card bytes fails closed. A partial copy of `G:\galion\horbm\knowledge` will not load.
- `openmind-memory` CJK n-grams are not in this index. The card pack is the Latin `unicode61` index `horbm-memory` already writes.
- Phase 2 never writes, even when `remember` is granted. A granted cell is RAM only. Phase 7 owns the learn file.
- `arc-swap` is the lock-free cell. It is MIT or Apache-2.0. It is not a forbidden runtime. `cargo tree -p horbm-retrieval` does not contain `candle`, `ort`, `tract`, `burn`, `llama`, `ggml`, `tokio`, or `openmind-memory`.
- The vendored embedder has AVX2 behind runtime detection and a scalar `dot_i8`. This phase does not call it. No `-C target-cpu=native` is set for this crate.

## What was built

`crates/horbm-retrieval` is a member of the root workspace.

| Module | Job |
|---|---|
| `fts.rs` | RAM `items` + `items_fts`. Insert token text from `index_tokens`. Query with a quoted OR and `bm25(items_fts)`. At most 4 hits. |
| `knowledge.rs` | Read-only pack loader and manifest check. |
| `consent.rs` | `SessionConsent::default` is deny. |
| `cell.rs` | `ArcSwap` snapshot. Readers do not lock the index. |
| `thread.rs` | Std thread `horbm-retrieval`. Denied publish stores an empty snapshot and does not query. `withdraw` clears a granted cell. |
| `run.rs` | Load, optional RAM rows, query, publish, then stamp `memory.db`, `revisions.sqlite`, and their `-wal` / `-shm` sidecars. |
| `src/bin/horbm-retrieval.rs` | Laptop entry. Prints hits and `disk_written: no`. |
| `tests/phase2_gate.rs` | The gate. |
| `fixtures/mini/` | Two committed cards and a manifest. |

Swarm 4 API:

```rust
pub struct RetrievalHit {
    pub card_id: String,
    pub clause: String,
    pub source: String,
    pub score: f32, // higher is better; SQLite bm25() is ordered ascending before this map
}

pub fn query(index: &FtsIndex, stable_prefix: &str) -> Result<Vec<RetrievalHit>>;
```

`stable_prefix` is the ASR text that has stayed unchanged. The function does not publish the cell and does not write a file. `run_retrieval` is the thread wrapper the binary and the gate use. `MAX_HITS` is 4, the same cap as `horbm_memory::MAX_HITS`.

## Local proof

`cargo test -p horbm-retrieval -- --test-threads=1` passed (unit tests, the gate, the grant-still-writes-nothing test, and the withdraw test).

A run against the public pack layout (1,506 cards, manifest checked) with `remember` denied:

- `cell: empty`, `cell_generation: 1`
- `query: newtonian gravitation` returned `world.physics.newtonian-g` first
- `memory_db: not created`, `revisions: not created`, `disk_written: no`
- a query token that is not in the pack returned `query_hits: 0` and an empty cell

That run is not the gate. The gate is the RAM fixture test named above.

## Laptop test

From `G:\galion\work\galion-one-voice`. The first command builds the crate and runs the gate. The second reads the real pack. It does not pass `--remember`.

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo test -p horbm-retrieval -- --test-threads=1
```

```powershell
Set-Location G:\galion\work\galion-one-voice
$env:GALION_KNOWLEDGE_HOME = "G:\galion\horbm\knowledge"
$env:HORBM_MEMORY_DB = "G:\galion\horbm\memory.db"
cargo run -p horbm-retrieval -- --query "newtonian gravitation"
```

A pass on the second command prints `remember: deny`, `cell: empty`, `disk_written: no`, and `memory_db: not created` when that file was absent (or `memory_db: unchanged` when it was already there). `cards` follows the synced pack (1,506 on the pack this note checked). The Newtonian constant card should be among the hits when that pack is the current galion-knowledge core. The process must not create `G:\galion\horbm\memory.db` or `G:\galion\horbm\knowledge\revisions.sqlite`.
