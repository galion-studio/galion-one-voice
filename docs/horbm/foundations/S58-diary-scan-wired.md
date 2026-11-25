# S58 — Diary scan wired into the turn

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S58, wire the S54 diary scan into the turn crate |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` (PR #299, after #318) |
| Gap | S54 shipped `DiaryScan` in `horbm-brain` and named the one call site. `asked` still called `search_lexical`. |
| This change | `horbm/crates/horbm` and this note. `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, and the root workspace manifest were not edited. `SOUL.md` is not in this tree. |
| Not edited | `horbm/crates/horbm-brain`, `horbm/crates/horbm-soak`, `crates/galion-tensor`. |

## What it decides

A granted ask in `horbm/crates/horbm/src/retrieve.rs` `asked` is one scan:

```rust
let n = opened.scan.scan_selected_into(text, hit_buf);
```

`hit_buf` is `[horbm_brain::ScanHit; MEMORY_HIT_CAP]`. The retrieval worker allocates it once, on the stack, before it reads a command. A later ask reuses that buffer. `MEMORY_HIT_CAP` is 4, the same value as `horbm_brain::SCAN_CAP`.

`DiaryScan` is built when the consent grant opens the in-memory store, inside `open_store`. `apply_consent` drops it with `*store = None` when the grant falls. `remember` denied never calls `open_store`, so the diary is not built and not dotted. A missing `keys_path` is not opened on deny. The same missing path fails the grant, because that is when `StoredKeys::open` runs.

The grant passes `config.remember` into `DiaryScan::open`. Callers reach `open_store` only while the grant holds, so the flag is true and the diary leg is in the merge. Key rows keep their own allow bits.

Rows come from the store the grant already opened. Person-diary lines (`diary_list`) are embedded with `embed_diary_into` on the line text. Card documents are embedded with `embed_diary_into` on `Card::index_text`, the same text `horbm-memory`'s hashing embedder stored in `emb`. A marked diary line, and a card `blocked_answer_ids` rejects (`research_only` or `not_for` containing `answer`), is stored with `allow: false` and is not dotted. The scan does not read `GALION_HORBM_RESEARCH_DIR`. No research card was added to the repo.

`ScanPool::Diary` publishes as `HitPool::Memory`. `ScanPool::Keys` publishes as `HitPool::Knowledge`. `item_id` comes from `scan.item_id`. A version-1 key row has no string, so it is not published. The spoken clause is the catalog text captured at setup: the card gist, or the diary line. The covering rule is unchanged. Every content token of the ask must sit in that haystack. An empty token list does not match. Nothing left to publish is `CellState::Miss`, and the spoken line is Samantha's `I don't know.`

Building those `MemoryHit` strings still allocates the published cell. The scan call does not. `tests/retrieve_alloc.rs` arms a counting allocator after `DiaryScan::open` and runs 100 queries into a `[ScanHit; MEMORY_HIT_CAP]`, including an empty ask, `id-β`, `set_remember`, and an empty output buffer. The count stays zero.

## Keys

`RetrievalConfig::keys_path` and `GALION_HORBM_KEYS` name an HKB1 file. It is opened with `StoredKeys::open(path, KeyPrefault::Populate)` only while the grant holds. The file is not created.

When that path is unset, setup builds an in-memory version-2 pack instead of leaving the key leg empty. One row per diary row. The vector is `encode_query_into` of the row's rarest content token (lowest document frequency, then the longer token). Scale is `1` and the width is 32. A one-token ask for that token ranks the row on the key leg. The diary leg still embeds the full line. The 1,000-row fixture asks `m742`. The hashing cosine of that short ask against the full card is not in the top 4, because other rows collide on the same lanes. The key leg puts `sample-742` in the buffer. The covering rule drops the rows that do not contain the token. This pack is setup. It is not a second scoring call, and it is not written to disk.

`search_person_lexical` is unchanged. It still reads the person file with FTS5. This thread does not mix that file into the scan.

Do not call `RetrievalThread::ask`, `ask_wait`, `set_remember`, or `cell` from `Heartbeat::step`.

Consent in `LOCKS.toml` is unchanged: `remember = "deny"`, `learn = "deny"`, `learn_in_band = false`. Samantha stays the persona. Ear prosody is untouched.

## Tests

Fixtures are built in the test. Nothing under a card directory was added to the repo. `GALION_HORBM_RESEARCH_DIR` is not read.

Covered:

- `a_scanned_turn_speaks_the_fixture_and_a_miss_stays_i_dont_know` — a temp fact card and a temp HKB1 file, both written in the test. The turn speaks `The blue bowl is in the cupboard.` The cell has a memory hit and a knowledge hit for `bowl`. `zzznomatchtoken` is `CellState::Miss` and the reply is `I don't know.`
- `a_denied_turn_does_not_speak_the_scanned_diary` — the same files with `remember` false. The cell stays idle. The reply is `I don't know.`, not the bowl line.
- `deny_does_not_open_a_missing_key_file` — deny does not create the path and the cell stays empty.
- `a_missing_key_file_fails_the_grant` — the grant reports the open error. The message does not contain the ask.
- `a_query_does_not_allocate_after_setup` — 100 queries after `DiaryScan::open` allocate nothing.
- The phase-2 gates still hold: deny writes nothing, the 1,000-row fixture returns `sample-742` and withdraw clears the cell, stopwords alone are a miss, and a research-only card is not `Hits[0]`.

Nothing added here is Linux-only. `StoredKeys::open` goes through `MappedBytes`, which already shipped that split. The tests ran on Linux, rustc 1.85.

## Soak RSS

`horbm-soak` `samantha_duplex_long_call` reads `VmRSS` at warmup tick 250 and at the end. The gate is growth and peak growth of 128 KiB. The duplex brain is `Brain::with_cards`, which leaves `retrieval` as `None`. `retrieval_spoke` returns before `ask_wait`. The long call does not spawn the retrieval thread, does not open the store, does not build a `DiaryScan`, and does not map a key file.

The soak test binary does not contain `DiaryScan`, `scan_selected_into`, or `open_store`. `nm` on the debug `horbm-soak` test shows `RetrievalThread::ask_wait` only because `Brain::turn` mentions it. That call is not taken. Steady allocation counts stay put: idle 0, playback 4, hold 1, on every run below. A structure that grew after warmup would move those counts.

Eight runs on this branch after merging `main` `b8f33bb7`, and eight runs on that main, debug profile, five virtual minutes, rustc 1.85.0. Growth is `end - warm`, and peak equalled end.

| Tree | Growth, KiB |
|---|---|
| this branch | 24, 24, 24, 24, 24, 24, 24, 24 |
| `main` `b8f33bb7` | 24, 24, 24, 24, 24, 24, 24, 24 |

The laptop numbers for the same test sit on both sides of the gate with no change in the executing path: 156 KiB then 40 KiB on `#321` `ec71ada`, and 84 KiB on `main` `3c675114`. This host does not reproduce a value above 24 KiB, and the branch matches main. The 128 KiB gate was not raised.
