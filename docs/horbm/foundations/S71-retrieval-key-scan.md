# S71 — Retrieval calls the scan when a key file is passed

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S71, brain plan round 2, R2-A |
| Written | 2026-11-25. No weight download, no voice audio, no paid API. The PR is not merged. |
| Base | `main` `fea54fb9af9eb098170c443cd7d67a556a113638`, then merged `26239c94e7c663e8f2baaf02870a390b79e23225` (PR #327, plan doc only) |
| Spec | `docs/horbm/PLAN-brain.md` section R2-A |
| Gap | A granted ask always scored with `DiaryScan`, including when no HKB1 file was set. That path built a synthetic key pack at setup. R2-A keeps lexical search until a key file is passed, and then calls `open_capped`. |
| This change | `horbm/crates/horbm/src/retrieve.rs`, `horbm/crates/horbm/tests/retrieve.rs`, the module comment at the top of `horbm/crates/horbm-brain/src/scan.rs`, and this note. |
| Not edited | `open_capped`, `scan_selected_into`, `brain.rs`, `lib.rs`, `horbm-memory` git rev `974e594444448597741372fef578142c46d28566`, `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm manifests, `Cargo.lock`. `SOUL.md` is not in this tree. |

## What it decides

`RetrievalConfig.keys_path` stays `Option<PathBuf>`. `Default` is `None`. No new environment variable is read. `GALION_HORBM_KEYS` was already on main and is unchanged.

`keys_path == None` does not open a key file and does not build a `DiaryScan`. `asked` is `search_lexical`, then `covers`, then the blocked-id filter. That is the ask 7881 runs when no key file is configured. The synthetic in-memory key pack is gone.

`keys_path == Some` and `remember` granted builds the scan once, inside `open_store`, which also runs from `set_remember(true)`. The call is `DiaryScan::open_capped`. A query calls `scan_selected_into` into the `[ScanHit; MEMORY_HIT_CAP]` the worker allocates once. That call does not allocate. `remember` denied does not call `open_store`, so the file is not opened.

A scan hit is published only when `scan.item_id` is present and that id was already resolved from the open store at setup. A version-1 key row has no string, so it is dropped. An id the store cannot resolve is dropped. The clause is the stored card gist or diary line. Nothing is invented. The blocked-id filter still applies. `covers` still applies, so a key row whose stored text does not contain the ask is not spoken. `ScanPool::Diary` is `HitPool::Memory`. `ScanPool::Keys` is `HitPool::Knowledge`.

The pinned `horbm-memory` API does not return stored embedding rows. `DiaryEntry` is text, and the int8 arena is private. The git rev is not changed. When the store has rows, setup rebuilds those hashing vectors with `embed_diary_into` on text the store already holds, which is the path main already used for a configured key file. `Brain::turn`'s scanned-fixture test, which this swarm does not edit, requires that diary leg. A store with no rows is an empty `DiaryArena`, and the scan ranks keys only.

Publishing the cell still builds `MemoryHit` strings. `search_person_lexical` is unchanged. Consent stays `remember = "deny"`, `learn = "deny"`, `learn_in_band = false`. No persona name is hard-coded. No research card was added.

## Tests

`unset_keys_path_stays_lexical` uses `RetrievalConfig::default()` (`keys_path` is `None`) and one temp card. The granted ask publishes one knowledge hit whose clause is the stored gist.

`granted_key_file_publishes_at_most_four_hits` writes a temp card and a temp HKB1 file with `horbm_brain::StoredPack::write`. The file has the card id and an id the store does not hold. A granted ask publishes at most four hits, every clause is the stored gist, and the unknown id is absent. The file is then replaced with bytes that are not HKB1. Denied `remember` still spawns, publishes an empty cell, and leaves those bytes untouched. A later grant on the same bytes fails, which is how the test shows deny did not open them.

`retrieve_alloc` is unchanged. A query into the scan buffer still allocates nothing after setup.

Nothing added here is Linux-only. `StoredKeys::open` goes through `MappedBytes`, which already shipped that split. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

| Command | Result |
|---|---|
| `cargo test --locked --manifest-path horbm/Cargo.toml -p horbm --test retrieve` | 7 passed, 0 failed, 1 ignored |
| `cargo test --locked --manifest-path horbm/Cargo.toml -p horbm --test retrieve_alloc` | 1 passed |
| `cargo test --locked --manifest-path horbm/Cargo.toml -p horbm --lib` | 41 passed |
| `cargo clippy --locked --manifest-path horbm/Cargo.toml -p horbm -p horbm-brain --all-targets -- -D warnings` | clean |
| `cargo test --locked --manifest-path horbm/Cargo.toml --workspace` | 280 passed, 0 failed, 2 ignored |
| `cargo clippy --locked --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --locked --workspace` | 1069 passed, 0 failed, 5 ignored |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | clean |

`cargo test` also rewrote `voice-stack/Cargo.lock` while resolving path dependencies. That file is not part of this change and was restored. No `Cargo.toml` and no lockfile is committed.
