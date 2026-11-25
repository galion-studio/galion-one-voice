# S74 — One RAM slot, one write, off the turn

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S74, brain plan round 2, R2-D |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `fea54fb9af9eb098170c443cd7d67a556a113638`, then merged `26239c94e7c663e8f2baaf02870a390b79e23225` (PR #327, the brain plan) |
| Spec | `docs/horbm/PLAN-brain.md` section R2-D |
| Gap | `LearnPass::write_clause` can publish a row, and nothing holds one clause until a session ends. `Brain::turn` must not be the writer. |
| This change | `horbm/crates/horbm/src/learn.rs` and this note. No new dependency. |
| Not edited | `horbm/crates/horbm/src/lib.rs`, `horbm/crates/horbm/src/brain.rs`, `horbm/crates/horbm/tests/learn.rs`, `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm workspace manifests, `Cargo.lock`. `SOUL.md` is not in this tree. |

## What it decides

`SessionSeal` sits next to `LearnPass`. It holds one `NotedClause` in RAM. `note` stores that clause. A second `note` drops the first and keeps the second. `note` does not create a directory and does not open SQLite.

`seal` with a clause calls `LearnPass::write_clause` once and clears the slot, including when that call returns denied. `seal` with an empty slot returns `Ok(None)` and does not call `write_clause`. A denied pass returns `LearnError::is_denied` and creates no file. The slot is plain text. This type does not encrypt.

The grant is the `LearnPass` inside the seal. `GALION_HORBM_LEARN=grant` is still the only process grant, through `LearnConfig::from_flags`. Unset, empty, and any other word stay deny. `LOCKS.toml` `[consent] learn = "deny"` and `learn_in_band = false` are unchanged. This module still does not parse that file. Nothing here reads `GALION_HORBM_REMEMBER`.

`Brain::turn` does not call `SessionSeal`. `lib.rs` does not re-export it. That export is round 3. The retrieval path is unchanged: no new allocation, I/O, or lock on a query. The seal is not on that path.

No persona name is hard-coded. The seal does not load a persona file.

## Tests

`seal_writes_one_row_and_a_second_seal_writes_nothing` is a unit test in `src/learn.rs`. It is not in `tests/learn.rs`, because that file is a separate crate and `SessionSeal` is not public at the crate root yet. An empty seal creates no pack. Two notes leave only the second clause in RAM and still create no file. The first `seal` writes that second clause as one sourced row. A marker file is placed in `live/`. The second `seal` returns `Ok(None)`, the marker is still there, and the audit row is the same row.

`unset_learn_seal_creates_no_pack_file` builds the seal with `LearnConfig::from_flags(None, Some(pack))`, which is what an unset `GALION_HORBM_LEARN` becomes. `seal` is denied, the pack directory does not exist, and a second `seal` still creates nothing.

`tests/learn.rs` is unchanged. The phase 7 gate still passes: fresh config writes nothing, granted config writes one sourced row, withdrawal leaves no row.

Nothing added here is Linux-only. Paths come from the temp directory. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17). The image default `rustc` is 1.83.0, so the commands use `cargo +1.85.0`.

| Command | Result |
|---|---|
| `cargo +1.85.0 test --manifest-path horbm/Cargo.toml -p horbm --lib seal` | 2 passed |
| `cargo +1.85.0 test --manifest-path horbm/Cargo.toml -p horbm --test learn` | 5 passed, including the phase 7 gate |
| `cargo +1.85.0 test --manifest-path horbm/Cargo.toml --workspace` | 280 passed, 0 failed, 2 ignored |
| `cargo +1.85.0 clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo +1.85.0 test --workspace` | 1069 passed, 0 failed, 5 ignored |
| `cargo +1.85.0 clippy --workspace --all-targets -- -D warnings` | clean |

The two ignored horbm tests are `timed_scan_of_one_thousand_rows` and `real_card_folder_is_read_and_not_written`. `cargo test` on the horbm workspace also rewrote `voice-stack/Cargo.lock` while resolving path dependencies. That file is not part of this change and was restored. No `Cargo.toml` and no lockfile is committed.
