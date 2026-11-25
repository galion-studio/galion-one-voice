# Phase 4 deep dive — the HORBM brain answers

Swarm 4 of the HORBM foundations. Read against `galion-one-voice` `main` at the start of this branch, plus the R4 note on PR #207 (`docs/horbm/research/R4-cpu-realtime-integration.md`, branch `cursor/horbm-r4-cpu-realtime-2180`). Owner Q1, settled 2026-11-18, option A: retrieval answers the hot path; a small local S0 model fills gaps later and only as a labelled guess; Galion Research (the Rust engine in `obsidian-open-mind`, crates `hermes-rs` and `hermes-gateway`) handles hard questions off the tick with a 3 second cap.

`LOCKS.toml` and `SOUL.md` were not edited. No new dependency names a forbidden runtime. The new crate `horbm-brain` has an empty `[dependencies]` table.

## What this phase is

R4 section 11, phase 4:

> Heuristic pick wired where `CannedLlm` now answers a chat turn. First clause ≤ 2 words, from the fixture. Piper path unchanged and still warm-or-absent.

Gate: a test turn produces the fixture clause. No network. No audio file of a person.

This swarm also had to define a `Retriever` so it is not blocked on Swarm 2 (`crates/horbm-retrieval`, FTS5) or Swarm 3 (`crates/horbm-keys`, int8 keys), and to make the 7881 voice server load `G:\galion\horbm\knowledge` (`manifest.json` plus the 1,506-card core pack) instead of stopping at the kettle note.

## What already existed

| Piece | Path | State on this branch's base |
|---|---|---|
| 80 ms clock, consent default deny, Piper exception | `LOCKS.toml` | Locked. Not touched. |
| R4 phased build | PR #207, not merged into `main` | Research only. Q1 in the task is option A. |
| Crate `horbm` | `horbm/crates/horbm` | Landed. One turn is empathy, then a covering card lookup, then an optional localhost research ask, then `I don't know.` Separate workspace (`Cargo.toml` `exclude = ["horbm"]`). |
| Card loader | `horbm/crates/horbm/src/cards.rs` | Works. Reads nested JSON, checks `galion-shard-manifest/1` / `card-bundle/1` (sha256, bytes, `card_count`). `WINDOWS_KNOWLEDGE_DIR` is `G:\galion\horbm\knowledge`. Does not create the directory and does not open `horbm-memory`. |
| Bridge | `voice-stack/horbm-bridge` (`galion-sam-horbm`) | Calls `horbm::Brain::turn` through `CardEngine`. The old "brain not landed" string is not this process. |
| Orchestrator seam | `voice-stack/pipeline-orchestrator/src/samantha_traits.rs` `BrainOpen::open_unavailable` | Still returns `horbm is not linked; Brain::open stays in crate horbm`. Tested. This swarm left that seam alone. The 7881 server does not use it. |
| 7881 mind | `voice-stack/local-server` `HorbmMind`, health field `mind` | Name is `horbm`. Directory order: `GALION_HORBM_KNOWLEDGE` if it is already a directory, else `G:\galion\horbm\knowledge` if that folder exists, else `var/knowledge` if it holds a card, else the tracked `knowledge/kettle.md`. |
| Pack sync | `scripts/windows/Sync-HorbmKnowledge.ps1` | Clones `galion-knowledge` `v2/data/cards` plus `v2/shards/manifest.json` into `G:\galion\horbm\knowledge`. Cards are not committed. Publication 5 of that manifest is `card_count` 1506, sha256 `5dfa2a00ed3e64e079accae8f5a8ffb05cd2ffd37f7141bd6d7979fb3a60b44f`, 3,060,127 bytes. |
| Canned chat table | `crates/galion-cascade/src/canned.rs` `CannedLlm` | Works. Intent table, emotion tag, honest miss `Mm. Say a bit more?`. Phone tests and presence eval require those exact lines when `GALION_LLM=canned`. |
| Phone slot | `crates/galion-cascade/src/phone.rs` | `GALION_LLM` unset or `hermes` is the loopback gateway. `canned` builds `CannedLlm`. Piper is pre-warmed when the A21 pack is present and stays absent when it is not. |
| Side band types | `crates/galion-sideband` | `Intent::Lookup` and `Intent::Action`, `ToHermes`, 3 s `RETRIEVAL_CAP`. Types only. No transport. |
| Research gateway | `voice-stack/research-gateway` | Localhost, already on the layer-5 miss path. Phase 6 is the side-band post. This phase does not add a dial. |
| int8 kernel | `crates/galion-tensor/src/gemv_i8.rs` | Scalar oracle, SIMD behind the tensor features. Not called from the brain. Swarm 3 should call it. |
| Persona miss | `docs/galion-voice-v2/personas/GALION-RESEARCH.md` and galion-knowledge cards `horbm-say-i-dont-know`, `horbm.honesty.i-dont-know` | The spoken line already used by the router is `I don't know.` The cards say to say that first and not decorate it. |
| Swarm 2 / Swarm 3 crates | `crates/horbm-retrieval`, `crates/horbm-keys`, and the same names under `horbm/crates/` | Not on `main` and not on any open PR or branch of this repo at the time of this dive. |

## What was a stub

- `CannedLlm` is the chat-turn stand-in. It does not read a stored clause.
- The orchestrator `Brain::open` error is a stub seam. The live mind is not that stub.
- Galion Research on the tick is out of scope. The existing layer-5 gateway still runs only when the caller passes a port and the question is deep. The new side-band mark is recorded and `posted` stays `false`.
- FTS5 and the int8 key scan are not in this crate. `RetrievalAdapter` and `KeyAdapter` are function-pointer plugs. Features `retrieval` and `keys` compile a status note and do not add a path dependency.

## Gaps this phase closes

1. No heuristic pick of one stored clause, and no two-word lead.
2. The cascade chat slot had no `horbm` mode. Replacing `CannedLlm` in place would break the canned phone tests, so the pick is a sibling mode on the same `Llm::reply` slot.
3. `Retriever` did not exist, so a later FTS or int8 crate had nothing to implement.
4. Health did not report how many cards the mind loaded, so a laptop run could not tell the kettle note from the 1,506-card pack.

## Risks

- A partial token overlap against 1,506 cards would guess. The fixture retriever requires every content token to be in the clause. The live card path still uses the covering index in `KnowledgeCards`, which has the same rule and refuses an equal-length tie.
- The persona miss in the voice is the short clause `I don't know.`, not the long summary of card `horbm-say-i-dont-know`. Speaking the summary would change every miss test. The fixture stores the short clause under that card's id.
- `speakable_chunks` still cuts on the existing four-word clause rule. Piper's splitter was not retimed. The two-word lead is on the brain's `Answer` / `TurnOut` (`first_clause`, `rest`). The mouth still receives the whole clause.
- Enabling a path dependency on `horbm-retrieval` or `horbm-keys` before those crates exist breaks resolution even with the feature off. The plugs stay function pointers until the crates land.
- `galion-cascade` now path-depends on `horbm-brain` only. It does not depend on crate `horbm`, so the root workspace does not build the empathy stack for a cascade test.

## What this branch builds

- `horbm/crates/horbm-brain` — `Retriever`, `MemoryRetriever` over `fixtures/clauses.json`, `Heuristic::answer`, `split_clause`, `SidebandMark` (`posted: false`). Adapters: `RetrievalAdapter`, `KeyAdapter`.
- `horbm::Brain::turn` calls that pick on the card lookup. A hit speaks the card gist. `first_clause` is the two-word lead when the gist is longer. A miss stays `I don't know.` and the existing deep-question research port is unchanged.
- `galion-cascade::HorbmChat` implements `Llm`. `GALION_LLM=horbm` selects it on the phone lane (`LlmMode::Horbm`). `GALION_LLM=canned` is still `CannedLlm`. Default phone mode is still Hermes.
- 7881 health JSON adds `knowledge_cards` and `knowledge_dir`. The directory choice was already the real store when `G:\galion\horbm\knowledge` exists. The count is what the same loader indexed (JSON cards plus markdown notes).

Piper is not opened by the gate tests. `PhoneLane` still calls `speech.prewarm()` when a lane is built, which is warm if the A21 pack is on disk and absent otherwise.

## Phase gate

From the repo root. No network and no audio file of a person. The turn reply is the fixture clause `The kettle is on the hob.` The first piece is `The kettle` (2 words) and the rest is `is on the hob.` A miss is `I don't know.`

```text
cargo test -p galion-cascade a_chat_turn_produces_the_fixture_clause -- --nocapture
cargo test --manifest-path horbm/Cargo.toml -p horbm-brain a_turn_produces_the_fixture_clause -- --nocapture
cargo test --manifest-path horbm/Cargo.toml -p horbm a_card_hit_is_the_stored_clause_with_a_two_word_lead -- --nocapture
```

The phone slot, with Piper left on its existing warm-or-absent path. This command is Unix-only. `--features cascade` compiles `galion-cores` and the phone ASR parent, which use `std::os::unix`.

```text
cargo test -p galion-cascade --features cascade horbm_chat_turn_is_the_fixture_clause_and_does_not_speak -- --nocapture
```

On Windows the same assertion is the first command above, without `--features cascade`:

```text
cargo test -p galion-cascade a_chat_turn_produces_the_fixture_clause -- --nocapture
```

Result on this branch, Linux, rustc 1.85.0: the four commands that do not need a Windows host passed. The chat turn reply was `The kettle is on the hob.` The lead was `The kettle`. The miss was `I don't know.` No audio file was read.

Laptop commands from `G:\galion\work\galion-one-voice` omit `--features cascade`.
