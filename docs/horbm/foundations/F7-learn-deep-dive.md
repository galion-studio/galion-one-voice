# F7 — Learn pass deep dive

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM foundations, phase 7 learn pass |
| Written | 2026-11-24. No benchmark, no weight download, no voice audio, no paid API. |
| Voice tree | Branch `cursor/horbm-learn-pass-ae07`, stacked on PR #253 `cursor/f2-retrieval-thread-3cd7` at `de26ba67db0d0eba83a0e66f1630b71da9e19d72`. `main` at the time of the read was `204fd8cd6e399d0e1cced23c595214be94de1f57` and does not contain `horbm::retrieve`. |
| Diary | `galion-studio/galion-knowledge` PR #36, branch `cursor/horbm-memory-git-face-3cd7`, head `974e594444448597741372fef578142c46d28566`. Open, not merged. The voice pin is that revision. |
| Open Mind | `galion-studio/obsidian-open-mind` `main` `0b0a898695b0525b179b5dd791bbfa8c281b7ffc`. Read only. Not cloned into this tree. Not written. |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §9 and §11 on closed draft PR #207, head `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`, branch `cursor/horbm-r4-cpu-realtime-2180`. Not merged. |
| This change | `horbm::learn` beside `horbm::retrieve`. `LOCKS.toml` `[consent]` gains `learn = "deny"` and `learn_in_band = false`. The four EmotionWire bits are unchanged. `SOUL.md` is not in this checkout and was not created. |

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A constant or a behaviour already in a named file. |
| **[R]** | Read from code or a schema on 2026-11-24. Not a timing. |
| **[T]** | A target in R4. The phase 7 gate below is the target this change implements. |

## What phase 7 is

R4 §11, line 444, is the whole assignment **[T]**:

| Build | Gate | Depends on |
|---|---|---|
| Learn pass: separate consent, source on every clause, pack swap, withdrawal deletes. | Fresh config writes nothing. Granted config writes one sourced row. Withdrawal leaves no row. | phase 2 |

Phase 2 is open PR #253. It is the preferred seam (`horbm::retrieve`, `RetrievalConfig`). PR #251 is a different crate and is not the base of this change. The name `cursor/horbm-memory-git-face-3cd7` is the galion-knowledge branch for PR #36, which #253 already pins. It is not the voice branch. This voice branch stacks on `cursor/f2-retrieval-thread-3cd7`.

Q1 inside the R4 file is still written as open (line 62 and line 486). The owner ruling this pass follows is the one named on 2026-11-18, option A: HORBM retrieval answers first. Galion Research, the Rust engine, handles hard questions off the tick with a 3 second cap. This note does not call that engine Hermes. F2 on the phase 2 branch records the same ruling as HORBM answering first (`docs/horbm/foundations/F2-retrieval-audit.md`, the paragraph under "What this phase ships"). The 3 second cap is already `RESEARCH_CAP` on the deep-miss path in `Brain::turn`. The learn pass does not call Galion Research and does not run on that cap. The loop is offline. The call never waits for it (R4 §9, line 389).

`SOUL.md` is not in `galion-one-voice` or at the root of `obsidian-open-mind`. It was not created. The owner ruling of 2026-11-24 19:15 allows a lock edit when the architecture needs it. This pass needs one key: learn consent is not one of the four in-band bits, and the lock file has to say so.

## 1. Consent, and why learn is not `remember`

`LOCKS.toml` `[consent]` on this tree:

```toml
bits = ["sense", "adapt", "remember", "share"]
default = "deny"
```

`withdraw_wipes_ram_history = true`. `adapt` is one of the two bits Rika requires, with `sense`. R4 §9 condition 2 (line 396) says a separate learn consent is required, that `remember` means recall and not training, and that `adapt` is not the learn bit. Q2 in that note asks what the bit should be called.

The lock file now names it. Under `[consent]`:

```toml
learn = "deny"
learn_in_band = false
```

`bits` stays `sense`, `adapt`, `remember`, `share`. EmotionWire byte 7 is still masked with `0x0f` in `galion-emotion` `encode_with_consent`, and bits 4..7 stay reserved. Learn does not travel in the frame. `scripts/check-locks.sh` requires `learn = "deny"`, `learn_in_band = false`, and the four-bit list. `LEDGER.md` records the owner line. The pass still does not parse `LOCKS.toml`. A fresh process matches the deny default. `GALION_HORBM_LEARN=grant` is the process grant and does not rewrite the file.

The write switch is `LearnConfig.granted`. `GALION_HORBM_LEARN=grant` is the only env value that sets it. `GALION_HORBM_REMEMBER` is not read. `RetrievalConfig.remember` is not copied into the learn pass.

R4 §9 condition 1 also says `remember` must still be granted before a clause is written. This pass does not enforce that coupling. A host that wants both grants withholds `GALION_HORBM_LEARN` until recall is granted. The pass itself will not treat a recall grant as permission to write, and it will not treat a learn grant as permission for the retrieval thread to open its diary. The phase 7 gate names one granted config. That config is the learn grant.

`crates/galion-consent` is the clone-time reference-voice gate (`consent.txt`, CC0, celebrity block). It is not this pass.

## 2. `horbm-memory` is the store

Crate `horbm-memory` in `galion-knowledge` `v2/memory`, MIT, `rusqlite` 0.32 with the bundled amalgamation, `rust-version` 1.83. **[R]** PR #36 adds a root manifest so Cargo can git-depend on the crate, `Store::search_lexical`, and a re-export of `index_tokens`. Phase 2 already pins `974e594444448597741372fef578142c46d28566`. This phase does not change the pin and does not vendor the crate.

There is no `learn()` method. The synchronous sourced write is `Store::diary_add(text, date, said_by, source)`. `require_fields` rejects an empty source, an empty speaker, an empty date, and an empty text. `diary_add` itself substitutes `"diary"` when source is empty and `"HORBM"` when speaker is empty, and only then calls `require_fields`. This pass rejects those empties before `Store::open`, so the substitute never runs.

`Store::remember` pins through the background worker. `Store::push_turn` enqueues extraction with source `session-ring` and no consent check. `Store::forget` copies the row into `archive` and deletes it from `items`. The crate comment says consent is given by use and there is no withdrawal command. That archive still holds the text, so `forget` does not meet "withdrawal leaves no row".

`Stack::open` creates `revisions.sqlite` and is how a knowledge directory swaps card heads. Phase 2 does not call it. This pass does not call it either. The card folder stays read-only. The learn pack is a separate directory the test creates under the temp dir.

`Store::open` always starts the worker thread. This pass calls `pause_worker` immediately and never calls `push_turn`, `remember`, `resume_worker`, or `drain`. One `diary_add` is the only insert.

## 3. The phase 2 seam this pass keeps

Public surface in `horbm/crates/horbm/src/retrieve.rs`, re-exported from `horbm`. This change does not edit that file.

| Item | Role |
|---|---|
| `RetrievalConfig` | `remember`, `knowledge_dir`, `fixture_rows`. Grant is the trimmed string `grant` only. |
| `RetrievalThread` | Std thread `horbm-retrieval`. `ask` / `ask_wait` / `set_remember`. |
| `MemoryHitCell` | `Idle` on deny or withdraw, `Miss` speaks `I don't know.`, `Hits` at most four. |
| `RetrievedCells` | `cell` / `try_cell`. Do not call `cell` from `Heartbeat::step`. |
| `Int8Scorer` | Seam for the mapped arena. Unused here. |

`set_remember(false)` drops the in-memory diary and publishes `Idle`. That is the recall withdraw. It is not the learn withdraw. `Brain::turn` is unchanged. Queries still use `Store::search_lexical` inside the retrieval thread, and only when `remember` is granted. A learn grant does not open that diary.

`LearnPass::open_beside` borrows a `RetrievalConfig` so a caller can hold both, and so `knowledge_dir` becomes a path the learn pass refuses to write. `remember` is not read.

## 4. PR #251 is a different seam

PR #251 (`cursor/horbm-phase2-retrieval-24dc`) adds a workspace crate `horbm-retrieval`, vendors `horbm-memory`, and exposes `horbm_retrieval::query`. Its cell type is not `MemoryHitCell`. Its env names are `GALION_KNOWLEDGE_HOME` and `HORBM_MEMORY_DB`. Phase 7 does not stack on that branch and does not call that crate.

## 5. `openmind-memory` is not the learn store

`obsidian-open-mind` `crates/openmind-memory` is a separate HTTP process (loopback 8420) with `vectors.db`. Schema `user_version` 4. Tables include `l0_conversations` and `l1_records`, plus FTS5. There is no card, clause, pack, or consent column in that SQL. **[R]** `crates/openmind-memory/src/store.rs`. The voice process does not open that database and does not POST to that server. Galion Research can keep using it. The learn row is an `items` diary row in a `horbm-memory` file this pass creates.

## 6. What the code does

Module `horbm/crates/horbm/src/learn.rs`.

| Step | Behaviour |
|---|---|
| Fresh | `LearnConfig::fresh` or any flag other than `grant`. `write_clause` returns denied before `Store::open`. The pack directory is not created. |
| Granted | Validate session, time, and consent record. Validate speaker and text. Refuse an empty pack path, `G:\galion\horbm\knowledge`, a directory that already holds `manifest.json` or `revisions.sqlite`, and any `knowledge_dir` passed through `open_beside`. |
| Source | Label `session=…;turn=…;time=…;consent=…` stored in the diary `source` column. The pass reads the row back and refuses the swap if the stored source, text, speaker, or date differs, or if the diary count is not 1, or if `archive` is not empty. |
| Pack swap | Insert into `next/learn-pack.sqlite`. Close the store. Rename `live/` to `retiring/` when it exists, rename `next/` to `live/`, delete `retiring/`. The live directory is the only one left. A second write builds a new `next/` and swaps again, so the live pack still has one row and the previous file is gone. |
| Withdrawal | Delete `live/`, `next/`, and `retiring/` under the pack directory. Do not call `Store::forget`. A missing pack is a no-op. A card folder is refused and left as it was. |
| Audit | If `live/learn-pack.sqlite` is absent, return zero rows and do not create the file. |

The retrieval thread is not the writer. `Heartbeat::step` is not the caller. No PCM, no voiceprint, no weight file, no `tokio`, no `target-cpu=native`.

Heuristic ranker updates from R4 §9 ("that was wrong" demotes a clause) are not in this gate. The gate is one inserted row and a withdrawal that removes it. No gradient and no student update.

## 7. Phase gate

The gate is `phase7_gate_fresh_writes_nothing_granted_writes_one_sourced_row_withdrawal_leaves_no_row` in `horbm/crates/horbm/tests/learn.rs`. On Linux, rustc 1.85.0, that test passed in this session, along with the other learn tests and `cargo test --manifest-path horbm/Cargo.toml -p horbm`. Clippy on the same crate with `-D warnings` passed. Windows was not booted here.

```text
cargo test --manifest-path horbm/Cargo.toml -p horbm --test learn -- --test-threads=1
```

Fresh config: the pack path does not exist and no sqlite file appears under the scratch directory. Granted config: `audit` returns one row, `archived` is 0, and `source` is the label above. Withdrawal: `audit` returns no row and the scratch directory contains no sqlite file. The test also checks that the current directory and the temp directory did not gain a sqlite file as a direct child, and that `revisions.sqlite` was not created.

Supporting tests in the same file: a recall grant with learn denied writes nothing; a learn grant with recall denied still writes one row and leaves the retrieval cell `Idle`; an empty source writes nothing; a second write swaps the pack down to the new row; a directory with `manifest.json` is not written.

`cargo test --manifest-path horbm/Cargo.toml -p horbm` is the crate run, including the phase 2 retrieve tests. Those tests must stay green. They use temp directories and a RAM fixture. They do not open a real knowledge folder.

## 8. Laptop test

From `G:\galion\work\galion-one-voice`, after this branch is checked out. Rust 1.85 or newer. The MSVC C++ tools are required once, because bundled SQLite is compiled. No model download. No audio. Do not point the learn pack at `G:\galion\horbm\knowledge`. The tests create their own temp directories. Do not merge.

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo test --manifest-path horbm/Cargo.toml -p horbm --test learn -- --test-threads=1
cargo test --manifest-path horbm/Cargo.toml -p horbm --test retrieve -- --test-threads=1
```

A pass of the first command is the phase 7 gate: fresh writes nothing, granted writes one sourced row, withdrawal leaves no row. The second command is the phase 2 gate on the same branch and must still pass. Leave `GALION_HORBM_LEARN` unset unless a later host process is meant to learn. Unset, the learn pass writes nothing.

## 9. Gaps left

| Gap | Where it stands |
|---|---|
| `Brain::turn` does not call `LearnPass` | The pass is offline. Wiring it into a session-end hook is later work. |
| Ranker demotion | R4 §9. Not this gate. |
| `Store::forget` still archives | The voice pass deletes its own pack files instead. The diary crate is unchanged. |
| Emotion-wire bits | `remember` is still not read from `EmotionWire` (F2 gap 3). Learn is not an emotion-wire bit. |
| Int8 pack swap | Phase 3 maps keys. This swap is the learn-pack directory, not a `.gq` file. |
| Q2, Q3, Q5 in R4 | The bit is named `learn` in `LOCKS.toml`, default deny, and it is not on the emotion wire. Who may grant it, and encryption at rest, are still open. The process grant is `GALION_HORBM_LEARN=grant`. |
| PR #36 and PR #253 | Both open. This branch does not merge them. |
