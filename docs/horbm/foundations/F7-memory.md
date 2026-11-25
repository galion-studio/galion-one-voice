# F7 — Person memory

Perpetual Open MIT License. (c) 2026 Galion Labs.

One page for the relationship diary the brain reads on a turn. CPU only. Local files only. No model call. Synthetic fixtures only.

## What it is

`horbm::EpisodicStore` implements `galion_emotion_core::standins::memory::PersonMemory`. That trait is the port for the dialogue manager (S14, layer 4, in progress) and the inner life. The stand-in that grants nothing is `DenyMemory` in `crates/galion-emotion-core/src/standins/memory.rs`.

The diary is `horbm-memory` (`galion-studio/galion-knowledge`, the revision already pinned by `horbm`). This crate does not copy that schema and does not ingest truth cards into the person file. `horbm::search_person_lexical` is FTS5 on the person file. `RetrievalThread` keeps the truth-card diary. A hit whose pool is not memory is dropped.

Rows are pins (`remember ...`), one extractive session summary, and name or mood lines from the state tracker. Each row has who (`person_id`), when (the caller's clock string, not the wall clock), and source (`pin`, `session-summary`, or `state-tracker`). The summary is `horbm_memory::summarise_session`. It does not call a model. `Brain::turn` buffers the line, pins an explicit remember command, and records the tracker. It does not close the session. `PersonMemory::end_session` writes the summary.

## Consent and forget

`LOCKS.toml` `[consent] default = "deny"` and `remember = "deny"` were already set. F7 does not change those values and does not add a bit. A comment under `[horbm]` says this diary uses that bit. `SOUL.md` is not in this tree and was not created.

Denied `remember` does not open the file, does not call `MemorySink`, and does not run the summary extractor. `Brain::turn` does not call the diary methods on that path. Withdraw drops the open connection and the RAM turn buffer. It does not delete the file. Granting again reads the same rows.

`forget` calls `Store::forget`. The live row moves to the `archive` table with reason `forget`. That row is the tombstone. Search does not read it, and `Brain::turn` does not either. There is no API that deletes the archive.

## What a turn may say

Memory is not a source for a world answer. `about_the_person` is the gate: “what is my …”, “how am I”, “what do you remember”, and the other markers in that function. “Where is the kettle” and “what is the capital of France” are not. The turn still reads the person diary when `remember` is granted, then drops those hits. A card, or `I don't know.`, answers the world question. A stored line is spoken only for a person question, with source `memory:<id>`. Layer 4 then picks the move. A person hit is not turned into a world answer.

A line that looks like a password is not pinned and is not buffered. The marker list matches `horbm-memory`'s private `looks_like_secret`. That function is not re-exported.

## Tests

`horbm/crates/horbm/tests/memory.rs` and the stand-in tests in `galion-emotion-core`:

- consent deny: zero diary writes, zero callbacks, zero extractions, no sqlite file
- pin, then the next turn recalls it
- forget: recall is empty, the tombstone text is the pin, the file remains
- a pinned world sentence is not the spoken answer; the kettle card still is
- two runs of one script return the same rows

`bash scripts/check-locks.sh` and `cargo test` for the root workspace and the `horbm` workspace are the gates.
