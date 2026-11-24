# F-integration — phases 2, 3, and 4

Read-only synthesis of the HORBM foundations, then the wiring this branch adds.
Owner: Mate Grajczyk. Nothing in this note merges a pull request.

## What was read

| Source | Ref | SHA | State |
|---|---|---|---|
| R4 plan, PR #207 | `cursor/horbm-r4-cpu-realtime-2180` | `dd902bbcfe34e23f63745a19fc2fed8e468a3b09` | Closed, not merged. Not on `main`. |
| Phase 3 int8 arena, PR #249 | `main` | `204fd8cd6e399d0e1cced23c595214be94de1f57` | Merged. `crates/horbm-keys`. |
| Phase 4 brain, PR #252 | `cursor/horbm-phase4-brain-70b3` | `6eb04d2980f39b83bcf4d4a0fec2c74e34b3611c` | Open. Base was `4759d419`, before the arena landed. |
| Retrieval cell, PR #253 | `cursor/f2-retrieval-thread-3cd7` | `de26ba67db0d0eba83a0e66f1630b71da9e19d72` | Open. Clean against `204fd8cd` before this branch. |
| Diary face, galion-knowledge PR #36 | `cursor/horbm-memory-git-face-3cd7` | `974e594444448597741372fef578142c46d28566` | Open. Not merged. Voice PR #253 pins this SHA. |
| Alternate phase 2, PR #251 | `cursor/horbm-phase2-retrieval-24dc` | `5b69340344a839149b120ceba761f5ad5108a9f1` | Open. Superseded by #253. Not in this branch. |

`LOCKS.toml` gains a `[horbm]` block. Consent values are the same. `SOUL.md` is still not in this tree: nothing here reads it, and the spoken miss stays the existing persona clause.

## Q1 option A

R4 §13 asks whether HORBM is the hot-path brain, with S0 deferred, or whether the 2026-11-04 ruling keeps a student decode on every chat turn. The note does not letter the choices. Option A, as given for this work, is the first one: **HORBM retrieval answers first. A small S0 model is not on this path.**

That matches the split R4 §1 already wrote. The hot path is a stored clause. Galion Research stays off the tick, and only for Lookup and Action marks. Phase 4 does not post those marks (`posted: false`). Phases 2–4 do not download weights and do not put a third-party model on the turn.

R4’s own banner says the 2026-11-04 ruling wins until the owner says otherwise. On 2026-11-24 the owner opened `LOCKS.toml` for architecture edits. `[horbm]` records option A: `hot_path = "retrieval"`, `student_on_tick = false`, miss `I don't know.`, and the only grant string `GALION_HORBM_REMEMBER=grant`. `[consent]` is unchanged. `scripts/check-locks.sh` checks the new keys.

## What each phase actually is

R4 §11:

| Phase | Contract | Gate |
|---|---|---|
| 2 | FTS in-process, RAM fixture, `remember` deny by default, publish a cell. No vectors. | Denied: cell empty, nothing on disk. 1,000-row fixture returns, untimed. |
| 3 | Mapped int8 arena, scalar dot as the oracle, SIMD only through `galion-tensor`, prefault at open. | Bit-exact against scalar on a fixed fixture. No timing claim. |
| 4 | Heuristic pick where `CannedLlm` answers a chat turn. First clause at most two words when the fixture allows a cut. Piper unchanged. | A test turn produces the fixture clause. No network. No audio file of a person. |

The retrieval thread is one std thread. It does not run inside `Heartbeat::step`. That step does not allocate and does not block. No tokio `full` on this path. The 5 ms scan and the 1 ms cell read in R4 §4.3 are targets, not gates.

Consent stays the lock: `[consent] remember = "deny"`, `default = "deny"`, `withdraw_wipes_ram_history = true`. `adapt` is not the learn bit. Learn is phase 7.

## Three surfaces, and where they disagree

### PR #253 is the retrieval contract

`horbm::retrieve` (re-exported; the module is private):

- `RetrievedCells::{cell, try_cell}`. `try_cell` is `None` only when the lock is busy. An empty diary is `Some` of an idle cell.
- `CellState::Idle` — deny, withdraw, or not yet asked. `query`, `hits`, and `answer` stay empty. This is not the miss sentence.
- `CellState::Miss` — granted ask, nothing covered, or the diary returned an error. `answer` is exactly `I don't know.` (`UNKNOWN_REPLY`). `hits` is empty.
- `CellState::Hits` — at most `MEMORY_HIT_CAP` (4). `answer` is empty. Speak `MemoryHit::clause()`, which is `text`, or `title` when `text` is empty.
- `Int8Scorer::score(&self, item_id: &str, query: &str) -> Option<i32>`. `None` means no row. Nothing in #253 calls it. Asks use `Store::search_lexical`, which does not scan the diary’s 256-wide hashing arena.
- `RetrievalThread::ask` does not wait. `ask_wait` does. Do not call either from `Heartbeat::step`.
- `GALION_HORBM_REMEMBER=grant` is the only grant. `"yes"` and `"true"` deny.
- `fixture_rows > 0` loads `synthetic_card` into `Store::open_in_memory` and does not read the card directory. Index 742 is item id `sample-742` and the token `m742`.

The git pin is knowledge `974e5944`, the head of unmerged PR #36, not `master` `761a7f22`. `master` has no root manifest, no `search_lexical`, and no public `index_tokens`. Cargo resolves that git dependency from the knowledge repository root. PR #36 must stay reachable at that SHA. This branch does not merge it.

`Brain::turn` on #253 does not read the cell. The 7881 bridge does not either.

### PR #252 is the speaking brain

- `horbm-brain`: in-memory pick over `fixtures/clauses.json`. The kettle clause is `The kettle is on the hob.` The lead is `The kettle`. The miss is `I don't know.` (`horbm-say-i-dont-know`).
- `GALION_LLM=horbm` selects `HorbmChat`, which answers from that fixture and does not read `remember`.
- `Brain::turn` on #252 picks a knowledge-card gist. A card miss can still call the research port on a deep question. The 7881 process copies `reply` only. `first_clause` does not reach the mouth.
- `RetrievalAdapter` and `KeyAdapter` are function pointers. The features `retrieval` and `keys` compile a status string. They do not path-depend on a crate. #252’s summary is right: the plugs were not linked, because those crates were not on that branch.

`merge-tree` of #252 onto `main` at `204fd8cd` conflicts only in the root `Cargo.lock` (both sides insert a package before `ident_case`: `horbm-keys` on main, `horbm-brain` on #252). No other file conflicts with main. Merging #252 onto #253 also conflicts in `horbm/Cargo.lock` and `horbm/crates/horbm/src/lib.rs` (the re-exports). This branch keeps both.

`galion-cores` affinity now compiles on non-Unix targets: `sched_setaffinity` is Unix-only, and elsewhere `apply_affinity` leaves the inherited mask. That was the first error under `--features cascade` on `x86_64-pc-windows-gnu`. The speech workers are still next.

The laptop failure named for #252 is the fourth command:

```text
cargo test -p galion-cascade --features cascade horbm_chat_turn_is_the_fixture_clause_and_does_not_speak
```

`--features cascade` compiles `galion-ml-voice/speech` and `galion-asr-vosk`. Those modules use `std::os::unix` (`AsRawFd`, `CommandExt::pre_exec`) and Unix `poll` / `dlopen` / `sched_setaffinity` in `galion-cores`. On `x86_64-pc-windows-msvc` the test does not compile, so it does not run. The other three #252 commands do not enable `cascade`. The fixture JSON treats `\r` as whitespace. This branch does not rewrite the Piper or Vosk workers. The laptop section below runs the same horbm chat assertion without that feature, and records the cascade command as still Unix-bound.

### PR #249 is the arena, and it does not speak

`KeyArena` on main:

- File magic `HKB1`, version 1, 64-byte header, keys at offset 64. `KEY_DIM` is 32. The header field `k` allows other widths, including 256. No per-key scale. The compare is the `i32` dot.
- Clause id and source id are `u64`, addressed by row `usize`. There is no string column and no `item_id`.
- Consent is one allow bit per row. `dots_into` ignores it. `dots_allowed_into` skips a denied row and packs scores without row indexes.
- Dots call `galion_tensor::simd::dot_i8_for`. Default features stay scalar. SIMD is `simd-avx2` / `simd-avx512` / `simd-neon`. No `target-cpu=native` in this crate.
- Unix maps the file. Windows uses the `.gq` heap read. `open` prefaults before it accepts the header.

There was no `Int8Scorer` on main. A slug such as `sample-742` is not the arena’s `u64`. Parsing the string as an integer is the wrong key: F2 says the id is not the plan’s `u64`.

### PR #251 is the other phase 2

Root crate `horbm-retrieval` plus a vendored `third_party/horbm-memory` pinned at knowledge `761a7f22` (no `search_lexical`). The real function is `query(index: &FtsIndex, stable_prefix: &str) -> Result<Vec<RetrievalHit>>`, not the one-line signature in the PR body. A denied publish leaves the cell empty, but `query` can still return hits. The miss is an empty list, not `I don't know.`

`merge-tree` against #253 shares no paths. The APIs cannot both be the call Swarm 4 makes. #253 is the one F3 and F4 name (`Idle` / `Miss` / `Hits`, `clause()`, string `item_id`, `Int8Scorer`). This branch does not merge #251.

A throwaway merge of #251 onto `204fd8cd` did not fail `cargo test -p horbm-retrieval -- --test-threads=1` (6 passed) or `cargo test -p horbm-keys` (7 passed, scalar and with the SIMD features). The “1 failing test” was not reproduced on that merge. Vendored package tests under `third_party/horbm-memory` are outside the workspace. `whole::addon_shard_manifest_matches_shard_manifest` panics on a missing `../shards/manifest.json` before its assertion, and `whole::imports_the_real_pack_and_keeps_every_field` expects `../data/cards`. Those paths were not vendored. A workspace test run does not execute them. No fix was applied to #251.

## What this branch wires

Base of the branch: `main` at `204fd8cd` (PR #249). The branch then merges #253 (`de26ba67`) and #252 (`6eb04d29`) and adds the integration commits. GitHub cannot stack one PR on two sibling parents. The pull request’s base is `main`. Do not merge #252, #253, knowledge #36, or this pull request from this work.

`Brain::turn`, when a `RetrievalThread` is attached:

| Cell | Speech | What it does not do |
|---|---|---|
| `Hits` | `hits[0].clause()`, source `retrieve:{item_id}` | Does not call research or the card index. |
| `Miss` | `I don't know.` | Does not fall through to a card that would have hit, and does not call research. |
| `Idle` | Card gist if one hits, otherwise the committed fixture (`The kettle is on the hob.` for the kettle line) | Does not open the diary. `fixture_rows` is ignored while denied. |

No thread keeps the #252 card path unchanged, including `Brain::open` on a missing directory (the miss stays `I don't know.`, and the directory is not created). The two-word lead still comes from `split_clause`. The 7881 mouth still speaks `reply`. `first_clause` is on `TurnOut` for the tests.

The turn calls `ask_wait` so the reply is the cell from that ask. That wait is the chat turn. It is not `Heartbeat::step`.

`CardEngine` (the 7881 bridge) spawns `RetrievalThread::from_env` and points `knowledge_dir` at the directory it already opened. Grant is only `GALION_HORBM_REMEMBER=grant`. Spawn failure leaves the card brain in place and prints `horbm retrieval:`.

`StringKeyArena` binds one string per HKB1 row. The file format is unchanged. `score_scalar_text` and `score_selected_text` encode the query with `encode_query_i8` (a portable FNV mix, not the diary embedder) and dot that byte row. A missing id and a consent-denied row return `None`. `horbm` implements `Int8Scorer` for `StringKeyArena` by calling `score_selected_text`. The retrieval thread still does not call the trait, and it still does not scan the diary’s hashing arena.

`horbm-brain`’s function-pointer adapters stay a fixture seam. They are not the production call.

## Gaps left open

- Knowledge PR #36 is unmerged. The voice crate will not build if `974e5944` disappears.
- Phase 6 still does not post the side-band mark.
- `first_clause` is not what 7881 speaks. The spoken string is `reply`.
- The cascade feature’s Unix speech workers still do not compile on Windows. See the laptop section.
- PR #251 is not fixed and is not included.
- The string table is not stored in `HKB1`. A caller binds it. There is still no per-key scale.

## Gate run on this branch

Linux, rustc 1.85.0. No audio file was read. The granted turn’s reply was `MemoryHit::clause()` for item `sample-742` and contained `marker m742`. The source was `retrieve:sample-742`. The denied turn’s reply was `The kettle is on the hob.` with source `fixture:kettle`, and the cell stayed `Idle`. The scorer’s `i32` matched the scalar dot of `encode_query_i8` on the fixed HKB1 fixture, including with `simd-avx2,simd-avx512` selected. A denied row and a missing id were `None`.

```text
cargo test -p horbm-keys string_item_scorer_is_bit_exact_with_scalar_on_the_fixture
cargo test -p horbm-keys --features simd-avx2,simd-avx512 string_item_scorer_is_bit_exact_with_scalar_on_the_fixture
cargo test --manifest-path horbm/Cargo.toml -p horbm --lib a_granted_turn_returns_the_fixture_clause_from_the_cell
cargo test --manifest-path horbm/Cargo.toml -p horbm --lib a_denied_turn_still_speaks_the_clause_fixture
cargo test --manifest-path horbm/Cargo.toml -p horbm --lib int8_scorer_is_bit_exact_with_scalar_on_the_fixture
cargo test -p galion-cascade a_chat_turn_produces_the_fixture_clause
```

`cargo test --manifest-path horbm/Cargo.toml -p horbm` passed (21 lib tests, the retrieve integration tests, the card tests). `cargo test --manifest-path voice-stack/Cargo.toml -p samantha-local-server --lib shipped_card_answers` passed: the example card is still `The kettle is on the stove.` when `remember` is denied. `cargo clippy` on `horbm`, `horbm-brain`, and `horbm-keys` (with the SIMD features) was clean under `-D warnings`.

`cargo check --target x86_64-pc-windows-gnu -p galion-cascade --features cascade` still fails in `galion-asr-vosk` and `galion-ml-voice` (`AsRawFd`, `CommandExt::pre_exec`). Those workers are not ported here.
