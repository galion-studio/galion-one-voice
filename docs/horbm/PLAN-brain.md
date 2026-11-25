# S66 — HORBM brain plan

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Base | `main` `b8f33bb7b097210c04156eb050e97618cbcc0531` |
| Kind | Research and build plan. This file does not change code, manifests, or locks. |
| Persona | Samantha is the only development persona. `personas/aria.toml` stays in the tree and stays inactive. |
| Ear | Prosody sensing starts on. `LOCKS.toml` `[consent.ear_prosody] sense = "allow"`. `GALION_EAR_SENSE=0\|off\|deny` is the off switch. |
| Research cards | Our own private research and development cards, on the laptop only, through `GALION_HORBM_RESEARCH_DIR`. None of those files belong in this repo. |
| Voice output | Whatever 7881 speaks today stays as it is. This plan does not retune Piper, the mouth, or the socket. |

`docs/horbm/` has no README and no index. This plan is the only new file.

Claim marks: **Found** means the file cited in this pass shows it. **Inferred** means a conclusion from those files. **Unknown** means this pass did not see it.

## The model

A turn is a lookup. `LOCKS.toml` `[horbm]` records owner decision 2026-11-24, Q1 option A: `hot_path = "retrieval"`, `student_on_tick = false`, `miss = "I don't know."`, `remember_grant = "GALION_HORBM_REMEMBER=grant"`. **Found.** There is no student decode on the chat turn. **Found** (`student_on_tick = false`).

`horbm/crates/horbm/src/lib.rs` fixes the order: empathy rewrites the lookup and does not speak the feeling; person memory is a second file and speaks only for a question about the person; a retrieval cell of `Hits` speaks the clause and `Miss` speaks `I don't know.` without research; a deep miss returns the local clause first and arms Galion Research only after that `TurnOut` is dropped; anything still unanswered is `I don't know.` Layer 4 then picks one legal move. `LearnPass` is not in that order. **Found.**

The build below glues those pieces. It adds no new engine, no new runtime, and no new environment variable. Reuse what is already linked: `horbm-memory` (git rev `974e594444448597741372fef578142c46d28566` in `horbm/crates/horbm/Cargo.toml`), `galion-tensor` for the int8 dot, `horbm-keys` / `HKB1`, `tantivy` 0.22 (Apache-2.0, style cards only), and `pitch-detection` inside `horbm-ear`. **Found** (those dependencies are already declared).

## Layer audit

### Retrieval and memory

**Must.** Hot path is retrieval, in process, not an HTTP diary. A miss is exactly `I don't know.` The covering rule keeps a card only when the ask's content tokens are in the haystack. Person diary and truth cards are different files. `remember` stays deny until the process is given `GALION_HORBM_REMEMBER=grant`. The emotion-wire `remember` bit is not that grant. `Brain::turn` is not called from `Heartbeat::step`. **Found** (`LOCKS.toml` `[horbm]`, `docs/horbm/foundations/F2-retrieval-audit.md`, `F7-memory.md`, `lib.rs`).

**On main.** `RetrievalThread` publishes `MemoryHitCell` (`Idle` / `Miss` / `Hits`) and `asked` calls `horbm_memory::Store::search_lexical`, then `covers`, then drops blocked ids. Cap `MEMORY_HIT_CAP` is 4. **Found** (`horbm/crates/horbm/src/retrieve.rs`, including the `asked` function). `KnowledgeCards::lookup` is a real covering index and drops cards with `blocks_answer`. **Found** (`cards.rs`). `EpisodicStore` is a real second `horbm-memory` file when `remember` is true, and it does not create the file when `remember` is false. **Found** (`memory.rs`). `DiaryScan` scores an `HKB1` arena and a person-diary arena with `galion_tensor::simd::dot_i8_for`, `SCAN_CAP` 4, `DIARY_DIM` 256, `SCAN_RRF_K` 60.0. Its module comment says the retrieval thread still calls `search_lexical` and does not call the scan. **Found** (`horbm/crates/horbm-brain/src/scan.rs`, `S54-diary-scan.md`). `horbm::Int8Scorer` is a single-id dot and the retrieval thread does not call it. **Found** (`retrieve.rs` trait docs, `lib.rs`). `Brain::open` attaches `EpisodicStore::open(memory_db, false)`. **Found** (`brain.rs`).

**Gap.** The thread never builds a `DiaryScan`. No `HKB1` path is read from the environment. **Found** (no `GALION_HORBM_` key path under `docs/horbm` or `horbm/`). Person recall at `Brain::open` stays denied even when the grant variable is set, because the flag passed in is `false`. **Found.** Whether the pinned `horbm-memory` rev exposes embedding rows to this crate is **Unknown** (the crate is a git dependency, not a file in this tree).

**Evidence.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test retrieve --test memory --test cards`. `DiaryScan` tests live in `horbm-brain`, including `a_query_does_not_allocate_after_setup` in `tests/scan_alloc.rs`. **Found** (those files exist).

### Learning

**Must.** Learn is a separate consent, default deny, not an emotion-wire bit. `GALION_HORBM_LEARN=grant` is the only process grant. A fresh pass writes nothing. A granted pass writes one sourced row. Withdrawal leaves no row. `Brain::turn` does not call the pass. **Found** (`F7-learn-deep-dive.md`, `learn.rs`, `LOCKS.toml` `[consent] learn = "deny"`, `learn_in_band = false`).

**On main.** `LearnPass::write_clause` swaps `live` / `next` / `retiring` and refuses a denied config. **Found** (`learn.rs`). Nothing in `Brain::turn` names `LearnPass`. **Found** (`lib.rs` says the pass is outside the order; `brain.rs` has no learn call — **Inferred** from the turn body read in this pass, which steps empathy, Samantha, memory, gather, style, and dialogue).

**Gap.** There is no session edge that holds one clause in RAM and writes it once when the session ends. **Found** (`F7-learn-deep-dive.md` §9: wiring a session-end hook is later work). Who may grant learn, and encryption at rest, are still open. **Found** (same section, Q2, Q3, Q5).

**Evidence.** `phase7_gate_fresh_writes_nothing_granted_writes_one_sourced_row_withdrawal_leaves_no_row` in `horbm/crates/horbm/tests/learn.rs`. **Found.**

### Inner life

**Must.** Two layers, both Samantha. `InnerLife` is four slots each of care, want, and expect, tokens of at most 16 ASCII bytes, learning off while `learn` is deny. Seed tokens are `heard`, `answer`, `pause`. `SamanthaInner` is four floats (care, curiosity, help, expect) for this session only, and it does not keep user facts. A turn may bias pace. It does not replace the spoken clause. **Found** (`F-inner-life.md`, `F-persona.md` `[inner]`, `horbm-inner`, `lib.rs`).

**On main.** Both types are real and tested. `Brain::turn` calls `SamanthaInner::turn` with `ear: None` and `sense: self.ear_sense`. `with_cards` sets `ear_sense: false`. `set_ear_sense` has no other caller. `inner_turn` / `inner_nudge` / `samantha_nudge` are separate methods and are not called from `turn`. **Found** (`brain.rs`, repo search for `set_ear_sense`). `EAR_CONFIDENCE` is 0.35. A missing ear is ignored; words still count. **Found** (`horbm-inner/src/lib.rs`, `life.rs` `apply_turn`).

**Gap.** The chat turn never updates `InnerLife`, and it never receives an ear point. **Found.**

**Evidence.** `horbm-inner/tests/samantha.rs` (`ear_off_ignores_the_listener_and_still_reads_the_words`, `sources_do_not_name_another_persona`). `horbm/crates/horbm/tests/samantha_inner.rs`. **Found.**

### Persona

**Must.** Boot loads Samantha. `never_fabricate = true`. Unknown line is `I don't know.` Aria's file stays in the repo and boot does not load it. **Found** (`F-persona.md`, `personas/samantha.toml`).

**On main.** `Persona::samantha()` and `Brain::boot(None)` load Samantha. `Persona::load` accepts `aria.toml` (the schema tests require that). `Brain::boot(Some(path))` installs whatever valid file the path names, including an id other than `samantha`. **Found** (`horbm-persona`, `brain.rs` `boot`, `lib.rs`: an explicit path replaces Samantha). `admit_move` checks boundary tokens. `StyleHook` (`brevity` 0.55, `warmth` 0.8, `max_sentences` 2 in Samantha's file; `rate = 1 - 0.15 * brevity`) is not read by `apply_prosody`. **Found** (`style.rs` `BREVITY_RATE`, `samantha.toml`, `brain.rs` `apply_prosody` multiplies `lean.rate` only).

**Gap.** An explicit path can boot Aria. Style numbers do not enter the turn. Changing `MoodMeta.rate` would change what 7881 already sends the mouth (`HorbmTurn` copies `mood_meta.rate`). **Found** (`horbm-voice/src/brain.rs`). That rate change is out of this plan.

**Evidence.** `development_default_is_samantha`, `boot_without_a_file_loads_samantha`, `aria_keeps_warmth_and_adds_precision`. **Found.**

### Emotion core

**Must.** One moment: valence, arousal, dominance, approach, on an 80 ms frame. Inbound pulls mix only when sense and adapt are both granted. Session `[consent]` stays deny. The moment step does not rewrite the spoken clause or the baseline. **Found** (`F-emotion-core.md`, `lib.rs`, `wire.rs` `admit_pull`).

**On main.** `EmotionCore::step` and `admit_pull` are real. `Brain::step_moment` is a separate method. `turn` does not call it. `LinkOut` / `horbm_ear::link::push_frame` can step a core the ear crate owns; that is not `Brain`'s core. **Found** (`crates/horbm-ear/src/link.rs`, `brain.rs`).

**Gap.** The brain's moment does not move on a chat turn. That split is what `lib.rs` specifies, and this plan keeps it. **Found.**

**Evidence.** `crates/galion-emotion-core/tests/moment.rs` (`own_intent_moves_without_caller_consent_and_inbound_does_not`), `tests/zero_alloc.rs`. **Found.**

### Intent and dialogue

**Must.** After the evidence is in, one move: answer, cite, ask, comfort, offer topic, or stop. Answer and cite need a truth-card source. A research hit is never an answer or a citation. The heuristic is deterministic. The offline bandit counts a move only when learn is granted, and the count does not change the move. **Found** (`F4b-dialogue-manager.md`, `dialogue/mod.rs`, `policy.rs`).

**On main.** `Dialogue::decide` is that heuristic. `Brain::turn` calls it, then `admit_move`, and a forbid becomes ask plus `I don't know.` **Found** (`brain.rs`). `choose_steered` exists and `decide` does not call it. **Found** (`policy.rs` module docs). `hold_topic` is on `DialogueLean` and is not an input to `decide`. **Found** (`DecideIn` has no hold field). `Move::OfferTopic` can speak `Back to {topic}?`. **Found** (`policy.rs` `plan_for`).

**Gap.** A hold from Samantha does not block a new topic. The bandit stays an observer. This plan leaves the bandit as an observer (fewest moving parts). **Found** for the gap; the bandit decision is this plan.

**Evidence.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test dialogue`. **Found.**

### Ear into the brain

**Must.** The ear starts on. One frame is 80 ms, 16 kHz, 1280 samples. The point is valence, arousal, dominance, approach, plus a strength. It is not a category. Denied frames are not read. The core mixes an inbound pull only when sense and adapt are both on. Session adapt stays deny. **Found** (`S27-ear-prosody.md`, `LOCKS.toml` `[consent.ear_prosody]`, `crates/horbm-ear/src/lib.rs`: `EarConfig::default` has `sense: true`, `FRAME_SAMPLES` 1280).

**On main.** `ListenerOffer` already exposes `valence`, `arousal`, and `strength`, and `inbound_intent` builds a `hear` pull without stepping the core. **Found** (`port.rs`). `Brain::turn` passes `ear: None`. The voice loop's ear is energy plus an endpointer; the transcript that reaches `Brain::turn` is a `&str`. **Found** (`horbm-voice/src/lib.rs`, `horbm-voice/src/brain.rs`). Duplex `ProsodyEar` reports `f0_hz` and a voiced bit. It is a different estimator from `horbm-ear`, and `BrainHandle` calls `brain.turn(query)`. **Found** (`horbm-duplex/src/prosody.rs`, `horbm-duplex/src/brain.rs`). `set_ear_sense` defaults off inside the brain while the ear crate defaults on. **Found.**

**Gap.** Prosody is computed and then dropped before `Brain::turn`. **Found.** Mapping duplex `f0_hz` onto valence would be a second coupling. This plan does not add that map. **Inferred** (no existing function does that map; this pass did not find one).

**Evidence.** `crates/horbm-ear/tests/frames.rs` `default_is_on_and_only_the_off_switch_closes_it`. `horbm-duplex/tests/adapters.rs` `prosody_hears_a_synthetic_sine_and_the_off_switch_is_explicit`. **Found.**

### How `Brain::turn` runs

**Must.** The order in `lib.rs`, summarized above. Research inside the turn is the style root only (`GALION_HORBM_RESEARCH_DIR`, hits tagged `not_for` answer and cite). The gateway ask waits until `TurnOut` is dropped. `RESEARCH_CAP` is 3 seconds. **Found** (`lib.rs`, `voice-stack/research-gateway/src/lib.rs` `Duration::from_secs(3)`).

**On main.** `turn` (`brain.rs`): reap a previous follow-up; empathy tick; `SamanthaInner::turn` with `ear: None`; optional person observe; `person_hit` or `gather`; style search; `decide`; `admit_move`; `finish`; `arm_follow` when `research_later`. `gather` calls `route_heard(..., None)`. `research_later` is true only when a port exists, the lean is not comfort, and the gap reason is `ResearchOff`. **Found** (`brain.rs` around the `turn` and `gather` functions). `route_heard` itself will call `ResearchAsk::ask` when the port is `Some`. `voice-stack/horbm-bridge/src/router.rs` `ReasoningRouter::route` passes `Some`. `CardEngine::turn`, which 7881 uses, calls `Brain::turn`. **Found** (`voice-stack/horbm-bridge/src/cards.rs`, `voice-stack/local-server/src/brain.rs`).

**Gap.** `ReasoningRouter` is a second brain that can speak a gateway result inside the call. `CardEngine` does not attach `EpisodicStore`. **Found** (`cards.rs` `brain_from_dir` attaches retrieval and style, and does not call `with_memory`). `HorbmEngine::open` discards the memory path and calls `brain_from_dir`. **Found** (`linked.rs`).

**Evidence.** `horbm/crates/horbm/tests/turn.rs`, `tests/clause_first.rs`. **Found.**

### Research cards

**Must.** Unset, empty, or not a directory means no read and no created directory. Cards are not answers. No dataset text in the repo. Gate: `scripts/check-research-boundary.sh`. **Found** (`docs/horbm/RESEARCH-DATA.md`, `research_style.rs` `from_env`, `accept_card` drops a card whose `not_for` does not bar answer and cite).

**Gap.** None that this plan needs a new loader for. The router gap above is the part that can still speak a gateway body inside a call. **Found.**

## Shared files

Every swarm leaves these untouched. They are not assigned to a swarm in any round.

| Path | Why it stays |
|---|---|
| `Cargo.toml`, `Cargo.lock` | Root workspace. `horbm` is excluded. `horbm-ear` and `horbm-keys` are already members. |
| `horbm/Cargo.toml` | Horbm workspace membership. Every crate the swarms edit is already a member. |
| `LOCKS.toml`, `scripts/check-locks.sh` | The ear, learn, remember, and research defaults are already recorded. |
| `SOUL.md` | Not in this tree. Do not create it. |
| `docs/horbm/PLAN-brain.md` | This plan. |
| `docs/horbm/RESEARCH-DATA.md`, `docs/horbm/PORTABILITY.md`, `docs/horbm/foundations/**` | Notes, not the build. |
| `scripts/check-research-boundary.sh` | The corpus gate. |
| `voice-stack/Cargo.toml`, `voice-stack/Cargo.lock` | Voice workspace. |
| `voice-stack/horbm-bridge/Cargo.toml`, `voice-stack/horbm-bridge/Cargo.lock` | Bridge manifest. No new dependency. |
| `voice-stack/local-server/**` | 7881 process, including `PiperMouth`. Speech stays as it is. |
| `horbm/Cargo.lock` | Assigned in round 4 only, to swarm R4-A. Rounds 1–3 leave it untouched. |

A swarm edits a crate manifest only when the table below names that manifest. No swarm adds a dependency except R4-A, and that dependency is the existing `horbm-ear` crate by path.

Round N starts after every swarm in rounds before it has merged. Inside a round the four branches start from that same base. Paths in one round do not overlap, so the four branches merge without editing the same file.

## Round 1 — pure contracts

No swarm edits `horbm/crates/horbm`. No swarm edits a workspace manifest.

### R1-A — Scan opens at the cap the thread will use

**Goal.** Add `DiaryScan::open_capped(keys, diary, remember)` that calls the existing `open` with `SCAN_CAP`, so the thread cannot pick a second cap.

**Owns.** `horbm/crates/horbm-brain/**` (that crate's `Cargo.toml` included; do not add a dependency).

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --lib open_capped_uses_scan_cap` and the existing `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --test scan_alloc`. `k()` equals `SCAN_CAP`. `scan_selected_into` is unchanged.

**Serves the brain.** The hot path's scorer already exists. The thread needs one setup call that uses the same cap as `MEMORY_HIT_CAP` (both are 4). **Found.**

**Depends on.** Nothing.

### R1-B — One constructor for an ear point

**Goal.** Add `Heard::from_axes(valence, arousal, confidence) -> Option<Heard>`. Return `None` when any value is non-finite or when `confidence < EAR_CONFIDENCE`. Do not change `apply_turn`.

**Owns.** `horbm/crates/horbm-inner/**`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-inner --test samantha a_low_confidence_ear_is_no_claim`. The existing `ear_off_ignores_the_listener_and_still_reads_the_words` still passes.

**Serves the brain.** `SamanthaInner` already knows how to read a `Heard`. The missing piece is a constructor the ear's three floats can fill.

**Depends on.** Nothing. It does not import `horbm-ear` (different workspace).

### R1-C — The offer exposes those three floats

**Goal.** Add `ListenerOffer::heard_axes(self) -> Option<(f32, f32, f32)>` as `(valence, arousal, strength)`, or `None` when any of the three is non-finite. Sensing stays on by default. The new function does not call `EmotionCore::step`.

**Owns.** `crates/horbm-ear/**` (that crate's `Cargo.toml` included; do not add a dependency). The root `Cargo.toml` stays untouched.

**Acceptance.** `cargo test -p horbm-ear --test frames a_heard_frame_exposes_axes_and_a_denied_frame_does_not`. `default_is_on_and_only_the_off_switch_closes_it` still passes.

**Serves the brain.** The ear and the inner layer meet as three floats. A denied frame still has no point.

**Depends on.** Nothing.

### R1-D — Boot id is Samantha

**Goal.** Add `Persona::allows_development_boot(&self) -> bool`, true only when the id is `samantha`. `Persona::load` of `aria.toml` still succeeds. Do not delete `personas/aria.toml`.

**Owns.** `horbm/crates/horbm-persona/**`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-persona --test persona aria_file_loads_and_is_not_a_development_boot`. `development_default_is_samantha` still passes.

**Serves the brain.** Samantha is the only persona the turn may install. Aria remains a schema fixture.

**Depends on.** Nothing.

### Round 1 clash table

| Path | R1-A | R1-B | R1-C | R1-D |
|---|---|---|---|---|
| `horbm/crates/horbm-brain/**` | yes | | | |
| `horbm/crates/horbm-inner/**` | | yes | | |
| `crates/horbm-ear/**` | | | yes | |
| `horbm/crates/horbm-persona/**` | | | | yes |
| Root `Cargo.toml`, root `Cargo.lock`, `horbm/Cargo.toml`, `horbm/Cargo.lock`, `LOCKS.toml` | | | | |

The four owned trees are different directories. Their intersection is empty.

## Round 2 — crate `horbm` modules, beside `Brain::turn`

No swarm edits `brain.rs` or `lib.rs`. Adding a struct field is done with `Default` so existing struct literals in `brain.rs` still compile.

### R2-A — The retrieval thread calls the scan when a key file is passed

**Goal.** Add `RetrievalConfig.keys_path: Option<PathBuf>`, default `None`. Do not read a new environment variable. When `keys_path` is `None`, `asked` stays `search_lexical` plus `covers` plus the blocked-id filter, which is the path 7881 already runs. When it is `Some` and `remember` is granted, build `DiaryScan::open_capped` at setup (not on the query), call `scan_selected_into` into a buffer of length `MEMORY_HIT_CAP`, and publish `MemoryHit` rows only for ids the open store can already resolve. Drop an id the store cannot resolve. Do not invent clause text. If the pinned `horbm-memory` API already returns diary embedding rows, pass them into `DiaryArena`. If it does not, pass an empty diary and scan keys only. Do not change the git rev.

**Owns.** `horbm/crates/horbm/src/retrieve.rs`, `horbm/crates/horbm/tests/retrieve.rs`, and the module comment at the top of `horbm/crates/horbm-brain/src/scan.rs` (the sentence that says the thread does not call the scan). Do not change `open_capped` or `scan_selected_into`.

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test retrieve`. New test `unset_keys_path_stays_lexical`. A second test writes a temp file with the existing `HKB1` `write` (`horbm-keys` `KeyPack::write` or `horbm-brain` `hkb.rs` `write`), sets `keys_path`, and asserts a granted ask publishes a cell whose hit count is at most 4. Denied `remember` still publishes an empty cell and does not open the file.

**Serves the brain.** S54 names this call as the one the thread is missing. The lexical path stays for every host that has no key file, including 7881.

**Depends on.** R1-A.

### R2-B — The person file opens only on the grant word

**Goal.** Add `EpisodicStore::open_from_flag(path, flag: Option<&str>)` that calls `open(path, true)` only when the trimmed flag equals `grant`. Any other value, including `None`, calls `open(path, false)`.

**Owns.** `horbm/crates/horbm/src/memory.rs`, `horbm/crates/horbm/tests/memory.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test memory only_the_grant_word_opens_the_person_file`. A denied open creates no file. `"yes"` and `"true"` stay denied, matching the lock comment.

**Serves the brain.** `Brain::open` currently passes `false` and cannot see the grant. The store learns the rule here; the turn calls it in round 3.

**Depends on.** Nothing in round 1. It sits in round 2 so round 3 can call it.

### R2-C — A hold does not open a new topic

**Goal.** Add `Dialogue::decide_held(&mut self, input, hold_topic: bool)`. `hold_topic == false` returns the same `Decision` as `decide`. `hold_topic == true` with `Move::OfferTopic` selected becomes `Move::Ask` and does not build `Back to {topic}?`. `decide` itself stays. The bandit still does not change the move. A research hit still cannot be an answer or a citation.

**Owns.** `horbm/crates/horbm/src/dialogue/**`, `horbm/crates/horbm/tests/dialogue.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test dialogue`. New test `hold_false_matches_decide_and_hold_true_blocks_offer_topic`.

**Serves the brain.** Samantha already computes `hold_topic`. The move picker ignores it, so a check-in can still offer a new topic.

**Depends on.** Nothing. The flag is a `bool`, so this swarm does not edit `horbm-inner`.

### R2-D — One RAM slot, one write, off the turn

**Goal.** Add `pub struct SessionSeal` next to `LearnPass`. `note` keeps one clause in RAM and replaces it on a second note. `seal` calls `write_clause` once and clears the slot. A second `seal` with an empty slot writes nothing. A denied pass writes nothing and creates no file. Unit-test it inside `learn.rs`. Do not call it from `turn`.

**Owns.** `horbm/crates/horbm/src/learn.rs`, `horbm/crates/horbm/tests/learn.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test learn`. The phase 7 gate still passes. New unit test `seal_writes_one_row_and_a_second_seal_writes_nothing`.

**Serves the brain.** Learning stays off the lookup and gains the session edge F7 left open.

**Depends on.** Nothing.

### Round 2 clash table

| Path | R2-A | R2-B | R2-C | R2-D |
|---|---|---|---|---|
| `horbm/crates/horbm/src/retrieve.rs` | yes | | | |
| `horbm/crates/horbm/tests/retrieve.rs` | yes | | | |
| `horbm/crates/horbm-brain/src/scan.rs` (module comment only) | yes | | | |
| `horbm/crates/horbm/src/memory.rs` | | yes | | |
| `horbm/crates/horbm/tests/memory.rs` | | yes | | |
| `horbm/crates/horbm/src/dialogue/**` | | | yes | |
| `horbm/crates/horbm/tests/dialogue.rs` | | | yes | |
| `horbm/crates/horbm/src/learn.rs` | | | | yes |
| `horbm/crates/horbm/tests/learn.rs` | | | | yes |
| `brain.rs`, `lib.rs`, `horbm/Cargo.toml`, `horbm/Cargo.lock`, root manifests, `LOCKS.toml` | | | | |

No path is marked in two columns.

## Round 3 — the turn, the second router, the moment lock, the learn export

### R3-A — `Brain::turn` takes an ear and keeps the clause

**Goal.**

- `turn(&str)` calls `turn_with_ear(text, None)`.
- `turn_with_ear` passes the `Heard` into `Perceived`. `None` keeps today's word path.
- Add `ear_sense_from_flag(Option<&str>) -> bool` with the same rule as `Ear::from_env`: unset, empty, `1`, `on`, and `allow` are on; `0`, `off`, and `deny` are off; trim and ignore case. `with_cards` uses that function on `GALION_EAR_SENSE` instead of the literal `false`. Add `ear_sense(&self) -> bool`. The test calls the pure function and does not set the process environment.
- `Brain::open` uses `EpisodicStore::open_from_flag`.
- `boot` / `install_persona` refuses a persona for which `allows_development_boot` is false. `with_cards` still installs Samantha.
- Call `decide_held` with `note.lean().hold_topic` in place of `decide`.
- After `reply` and `first_clause` are fixed, call `inner_turn` and do not write its result into those strings, the empathy mood, or the baseline.
- Do not call `step_moment`, `LearnPass`, or `SessionSeal`. Do not change `apply_prosody`. Do not cut a clause to `max_sentences`.
- A card hit stays that card's clause when a `Heard` is also passed, including a sad ear (`valence` at or below `SAD_EAR_VALENCE`, which is −0.20, and confidence at or above `EAR_CONFIDENCE`).

**Owns.** `horbm/crates/horbm/src/brain.rs`, `horbm/crates/horbm/tests/turn.rs`, `horbm/crates/horbm/tests/samantha_inner.rs`, `horbm/crates/horbm/tests/moment.rs`, `horbm/crates/horbm/tests/persona.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test turn --test samantha_inner --test moment --test persona`. New tests: `text_only_kettle_clause_is_unchanged`, `a_sad_ear_does_not_replace_a_card_clause`, `ear_sense_from_flag_defaults_on`, `boot_refuses_a_persona_whose_id_is_not_samantha`. The kettle line stays `The kettle is on the hob.` with first clause `The kettle`, which is what `horbm-brain` already asserts for that fixture. **Found** (`horbm-brain/src/lib.rs` test `a_turn_produces_the_fixture_clause`).

**Serves the brain.** This is the one function a host already calls. After this round it can see an ear, it installs only Samantha, it opens the person file on the real grant, and the spoken clause stays the lookup.

**Depends on.** R1-B, R1-D, R2-B, R2-C. It does not edit `retrieve.rs`. An unset `keys_path` keeps 7881 on lexical search.

### R3-B — The bridge router uses the same turn

**Goal.** `ReasoningRouter::turn` builds the `Brain` it can already build and calls `Brain::turn`. It copies the local reply out and drops the `TurnOut` so a deep-miss follow-up still starts after the local clause exists. It does not call `route_heard` with a research port. `route_heard` stays, because changing it would change every current caller in the same edit. Clause cutting for the socket (`chunks.rs`) stays.

**Owns.** `voice-stack/horbm-bridge/src/router.rs`, `voice-stack/horbm-bridge/src/research.rs`, `voice-stack/horbm-bridge/tests/router_gateway.rs`, `voice-stack/horbm-bridge/tests/bridge.rs`

**Acceptance.** `cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-horbm`. A deep question returns `I don't know.` on this call's reply text when the local cards miss. The test does not require a network listener.

**Serves the brain.** 7881's `CardEngine` already uses `Brain::turn`. `ReasoningRouter` is the remaining entry that can speak a gateway body inside the call.

**Depends on.** The `Brain::turn` that is already on main. It does not need R3-A's new method. Developed in parallel with R3-A, it keeps calling `turn(&str)`.

### R3-C — A text turn leaves the moment where it was

**Goal.** Add one integration test. After `Brain::turn` on a text line, the moment position and the baseline identity match the values from before the call. `samantha_nudge` / `step_moment` remain the methods that move the moment.

**Owns.** `horbm/crates/horbm/tests/moment_split.rs` (new file only).

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test moment_split`.

**Serves the brain.** `lib.rs` keeps the moment beside the turn so a feeling cannot replace the clause. The test locks that split against the edit in R3-A.

**Depends on.** Nothing. The test passes on today's `turn` and on R3-A's `turn` as long as R3-A does not call `step_moment`.

### R3-D — Export the session seal

**Goal.** `pub use` `SessionSeal` from `horbm/crates/horbm/src/lib.rs`. Add `tests/session.rs`: construct a brain, call `turn`, and assert no learn-pack sqlite file appears when learn is unset. Calling `SessionSeal::seal` stays the only write.

**Owns.** `horbm/crates/horbm/src/lib.rs`, `horbm/crates/horbm/tests/session.rs` (new).

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test session --test learn`.

**Serves the brain.** Hosts in round 4 can name the seal. `turn` still cannot write the pack.

**Depends on.** R2-D.

### Round 3 clash table

| Path | R3-A | R3-B | R3-C | R3-D |
|---|---|---|---|---|
| `horbm/crates/horbm/src/brain.rs` | yes | | | |
| `horbm/crates/horbm/tests/turn.rs` | yes | | | |
| `horbm/crates/horbm/tests/samantha_inner.rs` | yes | | | |
| `horbm/crates/horbm/tests/moment.rs` | yes | | | |
| `horbm/crates/horbm/tests/persona.rs` | yes | | | |
| `voice-stack/horbm-bridge/src/router.rs` | | yes | | |
| `voice-stack/horbm-bridge/src/research.rs` | | yes | | |
| `voice-stack/horbm-bridge/tests/router_gateway.rs` | | yes | | |
| `voice-stack/horbm-bridge/tests/bridge.rs` | | yes | | |
| `horbm/crates/horbm/tests/moment_split.rs` | | | yes | |
| `horbm/crates/horbm/src/lib.rs` | | | | yes |
| `horbm/crates/horbm/tests/session.rs` | | | | yes |
| `cards.rs` in the bridge, `chunks.rs`, `local-server/**`, `horbm/Cargo.lock`, root manifests, `LOCKS.toml` | | | | |

`tests/moment.rs` and `tests/moment_split.rs` are different files. `brain.rs` and `lib.rs` are different files.

## Round 4 — hosts pass the point; 7881's mouth stays

No swarm edits `voice-stack/local-server/**`. No swarm edits `mouth.rs`, `piper_mouth.rs`, or `moment_voice.rs`.

### R4-A — Duplex stores an ear frame on the brain it already shares

**Goal.** On a PCM slice whose length is `FRAME_SAMPLES` (1280), call the existing `horbm_ear::Ear::push`. When the frame is `Heard` and `heard_axes` is `Some`, keep `Heard::from_axes(...)` and pass it to `turn_with_ear` on the next prefetch. Any other length, a denied frame, or `from_axes` returning `None` passes `None`. Do not map `f0_hz` from `ProsodyEar` onto valence. Do not change when the mouth starts. Add the path dependency on `crates/horbm-ear` in the duplex crate manifest. Feature `piper` stays off.

**Owns.** `horbm/crates/horbm-duplex/Cargo.toml`, `horbm/crates/horbm-duplex/src/lib.rs`, `horbm/crates/horbm-duplex/src/brain.rs`, `horbm/crates/horbm-duplex/src/duplex.rs`, `horbm/crates/horbm-duplex/src/prosody.rs`, `horbm/crates/horbm-duplex/tests/adapters.rs`, `horbm/crates/horbm-duplex/tests/duplex.rs`, `horbm/Cargo.lock`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex --test adapters --test duplex`. New test `a_full_ear_frame_is_stored_and_the_card_clause_stays`. `tests/piper_mouth.rs` is left untouched and is not the gate.

**Serves the brain.** Duplex is the loop that already has PCM and already calls `Brain::turn`. The ear crate is the prosody model; the duplex pitch estimate stays a voiced bit for the conductor.

**Depends on.** R1-B, R1-C, R3-A.

### R4-B — The voice trait can carry a point, and the energy ear does not invent one

**Goal.** Add `TurnBrain::turn_heard(&mut self, heard: &str, ear: Option<Heard>)` with a default body that calls `turn(heard)`. `HorbmTurn` implements it with `turn_with_ear`. `VoiceLoop` keeps calling `turn(&str)`. The energy ear does not build a `Heard`. Add `HorbmTurn::end_session`, which calls `SessionSeal::seal` only when the host has already supplied a `ClauseSource`. With no source, it writes nothing. Do not implement this on `Drop`. `voice-stack/local-server` `MindBrain` keeps compiling because the new trait method has a default body. **Found** (that impl is `voice-stack/local-server/src/native.rs`; this swarm does not edit it).

**Owns.** `horbm/crates/horbm-voice/src/brain.rs`, `horbm/crates/horbm-voice/src/ear.rs`, `horbm/crates/horbm-voice/src/turn.rs`, `horbm/crates/horbm-voice/src/energy.rs`, `horbm/crates/horbm-voice/src/lib.rs`, `horbm/crates/horbm-voice/tests/ear.rs`, `horbm/crates/horbm-voice/tests/end_of_turn.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-voice --test ear --test end_of_turn`. New tests `energy_ear_does_not_invent_a_heard_point` and `end_session_without_a_source_writes_nothing`.

**Serves the brain.** 7881's text turn stays a `&str`. A later host can pass a real point through the same trait, and learn still waits for an explicit session end.

**Depends on.** R3-A, R3-D, R1-B.

### R4-C — The eval gate locks the brain, not the mouth

**Goal.** Add `tests/brain_model.rs`. Assert the miss text is `I don't know.`, the development persona id is `samantha`, `ear_sense()` is true when the flag function is given `None`, and a text turn's reply matches the card clause for the kettle fixture. Do not edit `thresholds.toml`, `real.rs`, `speech.rs`, or `prosody.rs`. Do not change a WER number or a mouth score.

**Owns.** `horbm/crates/horbm-eval/src/gate.rs`, `horbm/crates/horbm-eval/src/score.rs`, `horbm/crates/horbm-eval/src/turn.rs`, `horbm/crates/horbm-eval/tests/gate.rs`, `horbm/crates/horbm-eval/tests/turn_gate.rs`, `horbm/crates/horbm-eval/tests/brain_model.rs` (new). Edit `gate.rs` / `score.rs` / `turn.rs` only if the new test cannot call the public `horbm` API without them. Prefer the new test file alone.

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-eval --test brain_model --test gate --test turn_gate`

**Serves the brain.** The eval crate already scores `Brain::turn`. This lock is the model (lookup, miss line, Samantha, ear default), separate from the mouth round-trip.

**Depends on.** R3-A (`ear_sense()` and `turn_with_ear` exist; the test may call `turn(&str)` only).

### R4-D — 7881's card engine opens the person file on the same grant

**Goal.** In `brain_from_dir` / `CardEngine::open`, attach `EpisodicStore::open_from_flag` for the memory path the function already receives. Unset grant: no file, and `turn(&str)` stays the call. Do not pass a `Heard`. Do not set `keys_path`. `HorbmEngine::open` calls `brain_from_dir`, so it picks up the same attach without an edit to `linked.rs`.

**Owns.** `voice-stack/horbm-bridge/src/cards.rs`, `voice-stack/horbm-bridge/tests/diary.rs` (new).

**Acceptance.** `cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-horbm --test diary --test bridge`. With the grant unset, the kettle reply is unchanged and no person-diary file is created.

**Serves the brain.** `HorbmMind` in the 7881 server calls `CardEngine::open`. **Found** (`voice-stack/local-server/src/brain.rs`). The person diary becomes reachable there on the existing grant, and the mouth is not in this diff.

**Depends on.** R2-B. It does not depend on R4-A, R4-B, or R4-C.

### Round 4 clash table

| Path | R4-A | R4-B | R4-C | R4-D |
|---|---|---|---|---|
| `horbm/crates/horbm-duplex/Cargo.toml` | yes | | | |
| `horbm/crates/horbm-duplex/src/lib.rs` | yes | | | |
| `horbm/crates/horbm-duplex/src/brain.rs` | yes | | | |
| `horbm/crates/horbm-duplex/src/duplex.rs` | yes | | | |
| `horbm/crates/horbm-duplex/src/prosody.rs` | yes | | | |
| `horbm/crates/horbm-duplex/tests/adapters.rs` | yes | | | |
| `horbm/crates/horbm-duplex/tests/duplex.rs` | yes | | | |
| `horbm/Cargo.lock` | yes | | | |
| `horbm/crates/horbm-voice/src/brain.rs` | | yes | | |
| `horbm/crates/horbm-voice/src/ear.rs` | | yes | | |
| `horbm/crates/horbm-voice/src/turn.rs` | | yes | | |
| `horbm/crates/horbm-voice/src/energy.rs` | | yes | | |
| `horbm/crates/horbm-voice/src/lib.rs` | | yes | | |
| `horbm/crates/horbm-voice/tests/ear.rs` | | yes | | |
| `horbm/crates/horbm-voice/tests/end_of_turn.rs` | | yes | | |
| `horbm/crates/horbm-eval/src/gate.rs` | | | yes | |
| `horbm/crates/horbm-eval/src/score.rs` | | | yes | |
| `horbm/crates/horbm-eval/src/turn.rs` | | | yes | |
| `horbm/crates/horbm-eval/tests/gate.rs` | | | yes | |
| `horbm/crates/horbm-eval/tests/turn_gate.rs` | | | yes | |
| `horbm/crates/horbm-eval/tests/brain_model.rs` | | | yes | |
| `voice-stack/horbm-bridge/src/cards.rs` | | | | yes |
| `voice-stack/horbm-bridge/tests/diary.rs` | | | | yes |
| `piper_mouth.rs`, `mouth.rs`, `local-server/**`, root `Cargo.lock`, `voice-stack/Cargo.lock`, bridge `Cargo.lock`, `LOCKS.toml` | | | | |

`horbm-duplex/src/brain.rs` and `horbm-voice/src/brain.rs` are different files. `horbm/Cargo.lock` appears in one column.

Left untouched inside duplex on purpose: `src/mouth.rs`, `src/piper_mouth.rs`, `src/breath.rs`, `src/echo.rs`, `src/live.rs`, `src/recog.rs`, `src/report.rs`, `src/ports.rs`, `src/emotion.rs`, `src/bin/horbm-duplex-live.rs`, `tests/piper_mouth.rs`, `tests/echo.rs`.

## Dependency order

```text
R1-A scan cap ─────────────── R2-A thread calls scan
R1-B Heard::from_axes ──┬──── R3-A Brain::turn_with_ear ──┬── R4-A duplex frame
R1-C offer axes ────────┤                                 ├── R4-B voice trait + session end
R1-D boot id ───────────┘                                 └── R4-C eval lock
R2-B open_from_flag ──┬── R3-A
                      └── R4-D 7881 person file
R2-C decide_held ──────── R3-A
R2-D SessionSeal ──────── R3-D export ── R4-B
R3-B router → Brain::turn     (parallel with R3-A; uses turn(&str))
R3-C moment lock              (parallel with R3-A)
```

Round 2 waits for round 1. Round 3 waits for round 2. Round 4 waits for round 3. No swarm waits on a swarm in the same round.

## What this plan leaves in place

These are recommendations. This PR does not move or delete them.

**Already archived.** `archive/2026-11-25/crates/galion-live-demo/`, `archive/2026-11-24/crates/galion-v2-live/`, `archive/2026-11-24/voice-stack/horbm-bridge/seam/horbm/`. Each has an `ARCHIVED.md`. **Found.**

**Keep, because 7881 still speaks through them.** `voice-stack/local-server` `PiperMouth` (`mouth.rs` documents the Piper path). `runtimes/galion-piper-rt`. The research-gateway library (`RESEARCH_CAP`). `galion-conductor`, which duplex uses to call `BrainHandle`. `galion-heartbeat` stays; `FP-first-principles-audit.md` step 8 says so, and this plan does not retune its constants. **Found.**

**Keep, because the brain still uses them.** `horbm-brain` `MemoryRetriever` and the clause fixture: `gather` still calls `pick` on the idle path. **Found** (`brain.rs` `gather`). `personas/aria.toml`: inactive schema fixture. **Found** (`F-persona.md`).

**Not the brain, and not free to delete.** `crates/galion-cascade` `PhoneLane` is the console `phone-turn` feature (`crates/galion-console/src/turn_worker.rs`). `horbm/` has no reference to `galion_cascade`. Cascade's `horbm_chat.rs` uses `horbm_brain::Heuristic`, not `horbm::Brain::turn`. **Found.** `.github/workflows/gates.yml` still runs `cargo test -p galion-cascade --features cascade --lib`. **Found.** Archive that lane only after that gate is retargeted. `voice-stack/pipeline-orchestrator` (`galion-sam-orch`) returns `"horbm is not linked; Brain::open stays in crate horbm"` and does not link `horbm`. **Found** (`samantha_traits.rs`). `voice-stack/local-server/tests/stream.rs` still names `galion_sam_orch::samantha_traits::WaitGate`, so the crate is not unused. **Found.** No brain swarm should edit it.

**Chatterbox copies.** This pass found no `cbx-v2` or `chatterbox` string under `voice-stack/eval-golden`. A full caller count across `tools/` was not made. **Unknown** how many references remain. `FP-first-principles-audit.md` already says to archive those copies after the voice-v2 and eval paths stop pointing at them. That is a later non-brain pass.

## Questions only Mate can answer

1. Who may set `GALION_HORBM_LEARN=grant`, and is the learn pack encrypted at rest? The bit name and the default deny are already in `LOCKS.toml`. Q2, Q3, and Q5 in the R4 note are still open. **Found** (`F7-learn-deep-dive.md` §9).
2. Is there an `HKB1` file on the laptop the retrieval thread should open, and under which existing variable? This plan adds `RetrievalConfig.keys_path` with no default and no new variable. 7881 leaves it unset and stays on `search_lexical`.
3. Should a host call `HorbmTurn::end_session` at process shutdown? The method is the hook. This plan does not edit the 7881 server, so nothing calls it until you say so.
