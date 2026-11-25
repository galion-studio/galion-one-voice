# S67 — Diary scan opens at SCAN_CAP

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S67, brain plan round 1, R1-A |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `46d6bba0d7289ff6f7ff75a3bb45f01e58576de9` (PR #321) |
| Spec | `docs/horbm/PLAN-brain.md` section R1-A, on `cursor/s66-horbm-brain-plan-4194` (PR #327, not merged) |
| Gap | `DiaryScan::open` takes a caller-chosen `k`. The retrieval thread could pass a cap other than `SCAN_CAP`. |
| This change | `horbm/crates/horbm-brain` and this note. No new dependency. |
| Not edited | `horbm/crates/horbm`, `horbm/crates/horbm-inner`, `crates/horbm-ear`, `horbm/crates/horbm-persona`, `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm workspace manifests, `Cargo.lock`. `SOUL.md` is not in this tree. |

## What it decides

`DiaryScan::open_capped(keys, diary, remember)` calls the existing `DiaryScan::open` with `k` set to `SCAN_CAP` (4). That is the same number as `horbm::MEMORY_HIT_CAP`. The scratch built at setup is that length. A later query still writes into the caller's buffer through `scan_selected_into`, which is unchanged. After setup, a query does not allocate.

`remember` is forwarded. `false` still matches `LOCKS.toml` `[consent] remember = "deny"` and leaves the diary out of the merge. `learn = "deny"` and `learn_in_band = false` are unchanged. Samantha stays the persona. No research card was added.

The turn crate still calls `DiaryScan::open(..., SCAN_CAP)`. This swarm does not edit that call. Round 2 is the one that switches the thread onto `open_capped`.

## Tests

`open_capped_uses_scan_cap` builds a scan with `open_capped` and one with `open(..., SCAN_CAP)`. Both report `k() == SCAN_CAP`. The same query into a buffer longer than `SCAN_CAP` returns the same hits, and nothing past `SCAN_CAP` is written. `remember` false still stores deny and the same cap.

`tests/scan_alloc.rs` is unchanged. It still opens with `DiaryScan::open` and checks that a query does not allocate.

Nothing added here is Linux-only. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

| Command | Result |
|---|---|
| `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --lib open_capped_uses_scan_cap` | 1 passed |
| `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --test scan_alloc` | 1 passed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 273 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 1068 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

The two ignored horbm tests are `timed_scan_of_one_thousand_rows` and `real_card_folder_is_read_and_not_written`. `cargo test` also rewrote `voice-stack/Cargo.lock` (packages added while resolving horbm's path dependencies). That file is not part of this change and was restored. No `Cargo.toml` and no lockfile is committed.
