# S72 — The person file opens only on the grant word

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S72, brain plan round 2, R2-B |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `fea54fb9af9eb098170c443cd7d67a556a113638`, then merged `26239c94e7c663e8f2baaf02870a390b79e23225` (PR #327, the brain plan) |
| Spec | `docs/horbm/PLAN-brain.md` section R2-B |
| Gap | `EpisodicStore::open` takes a bool. `Brain::open` passes `false`, so a process grant cannot open the person file. |
| This change | `horbm/crates/horbm/src/memory.rs`, `horbm/crates/horbm/tests/memory.rs`, and this note. No new dependency. |
| Not edited | `brain.rs`, `lib.rs`, `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm workspace manifests, `Cargo.lock`. `SOUL.md` is not in this tree. |

## What it decides

`EpisodicStore::open_from_flag(path, flag)` calls `open(path, true)` only when `flag` trimmed is the word `grant`. Any other value, including `None`, calls `open(path, false)`. The comparison is the same exact word `RetrievalConfig::from_flags` and `LearnConfig::from_flags` already use. `" grant "` grants. `"yes"`, `"true"`, `"granted"`, `"GRANT"`, and `None` deny. This matches `LOCKS.toml` `[horbm] remember_grant = "GALION_HORBM_REMEMBER=grant"`. The function does not read the process environment and does not parse `LOCKS.toml`.

`open` still does not create the file. A denied store refuses a pin and leaves no sqlite file. A granted store creates the file on the first stored pin. `[consent] remember = "deny"`, `learn = "deny"`, and `learn_in_band = false` are unchanged. No persona name is hard-coded.

`Brain::open` still calls `EpisodicStore::open(memory_db, false)`. Round 3 is the caller. This round does not put the flag on the retrieval hot path.

## Tests

`only_the_grant_word_opens_the_person_file` in `horbm/crates/horbm/tests/memory.rs`.

Nothing added here is Linux-only. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17). `cargo test` once rewrote `voice-stack/Cargo.lock` while resolving path dependencies. That file is not part of this change and was restored. No `Cargo.toml` and no lockfile is committed.

| Command | Result |
|---|---|
| `cargo test --manifest-path horbm/Cargo.toml -p horbm --test memory only_the_grant_word_opens_the_person_file` | 1 passed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 279 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 1069 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

The two ignored horbm tests are the ones already ignored on main (`timed_scan_of_one_thousand_rows` and `real_card_folder_is_read_and_not_written`).
