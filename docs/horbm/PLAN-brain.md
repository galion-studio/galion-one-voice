# S66 — HORBM brain plan

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Base | `main` `86f9f0a703a01c73b29e017662ba143af04ef958` (includes #382, #384, #385, #387, #391, #392, and #393). #386 is open and is not this commit. The layer audit below is this commit. The rounds further down are the S66 assignment from `b8f33bb7` and are not a second spec. |
| Kind | Research and build plan. This file does not change code, manifests, or locks. |
| Persona | Boot loads the configured development persona (`GALION_HORBM_DEV_PERSONA`, else `horbm/crates/horbm-persona/development.toml`, through `Persona::load_development` / `is_configured_boot`). There is no `allows_development_boot`. `horbm/crates/horbm-persona/personas/aria.toml` stays in the tree and is not selected unless that config names it. |
| Ear | Prosody sensing starts on. `LOCKS.toml` `[consent.ear_prosody] sense = "allow"`. `GALION_EAR_SENSE=0\|off\|deny` is the off switch. |
| Research cards | Our own private research and development cards, on the laptop only, through `GALION_HORBM_RESEARCH_DIR`. None of those files belong in this repo. |
| Voice output | S82 (`voice-stack/local-server`) is the live host. This plan does not retune Piper, the mouth, or the socket, and it does not restart the process on 7881. |

The index of this directory is `docs/horbm/README.md`. Research data is `docs/horbm/RESEARCH-DATA.md`. Portability is `docs/horbm/PORTABILITY.md`.

Claim marks: **Found** means the file and symbol cited here show it on this base. **Inferred** means a conclusion from those files. **Unknown** means this pass did not see it.

## Gates

Every brain PR runs these on rustc 1.85, Linux, next to the test, clippy, and soak commands. The 128 KiB soak gate is live heap bytes, end minus the warmup sample. It is never raised. `VmRSS` is printed and is not gated.

- Test: `cargo test --manifest-path horbm/Cargo.toml --workspace`
- Clippy: `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings`
- Soak: `cargo test --manifest-path horbm/Cargo.toml -p horbm-soak`
- Rustdoc, required for every brain PR: `RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path horbm/Cargo.toml -p horbm --no-deps`
- Voice-stack lock: `cargo metadata --locked --manifest-path voice-stack/Cargo.toml`

`scripts/check-locks.sh` and `scripts/check-research-boundary.sh` stay required. The root workspace test and clippy, and the voice-stack package test and clippy in the laptop gate, stay required too. `.github/workflows/voice-stack-horbm-router.yml` runs that rustdoc command as the `cargo doc` step. `Brain::turn` describes the private finish step in prose and does not link `Self::finish`. **Found** (`horbm/crates/horbm/src/brain.rs` `turn`, the workflow file). `horbm-soak` `HEAP_GROWTH_KIB` is 128. **Found** (`horbm/crates/horbm-soak/src/lib.rs`).

## The model

A turn is a lookup. `LOCKS.toml` `[horbm]` records owner decision 2026-11-24, Q1 option A: `hot_path = "retrieval"`, `student_on_tick = false`, `miss = "I don't know."`, `remember_grant = "GALION_HORBM_REMEMBER=grant"`. **Found.** There is no student decode on the chat turn. **Found** (`student_on_tick = false`).

`horbm/crates/horbm/src/lib.rs` fixes the order: empathy rewrites the lookup and does not speak the feeling; person memory is a second file and speaks only for a question about the person; a retrieval cell of `Hits` speaks the clause and `Miss` speaks `UNKNOWN_REPLY` without research; a deep miss returns the local clause first and arms Galion Research only after that `TurnOut` is dropped; anything still unanswered is `UNKNOWN_REPLY`. Layer 4 then picks one legal move. `LearnPass` is not in that order, and `Brain::turn` does not note or seal. **Found** (`lib.rs`). A host may seal when the session finishes. That call is `HostSession::finish`, not `Brain::finish`. **Found** (`voice-stack/local-server/src/host.rs` `finish`, `horbm/crates/horbm/src/brain.rs` `finish`).

Already linked, and not new engines: `horbm-memory` at git rev `974e594444448597741372fef578142c46d28566` in `horbm/crates/horbm/Cargo.toml`, `galion-tensor` in `horbm/crates/horbm-brain/Cargo.toml`, `horbm-keys` by path from that crate and from `horbm`, `tantivy` 0.22 in the `horbm` manifest, and `pitch-detection` in `crates/horbm-ear/Cargo.toml`. **Found.**

## Layer audit

### Cards

**Must.** Truth cards are a directory, kept apart from the person diary. The covering rule keeps a card only when every content token of the ask is in the haystack. A card with `research_only` or `not_for` containing `answer` is not indexed. The loader does not write a card and does not open `horbm-memory`. **Found** (`horbm/crates/horbm/src/cards.rs` `KnowledgeCards::load`, `blocks_answer`, `CardLookup::lookup`).

**On main.** `KnowledgeCards::load` sorts by id, drops blocked cards, and checks a card-bundle manifest (schema card-bundle, version 1) when one is present. `lookup` intersects posting lists and does not guess a tie. **Found** (`cards.rs`). `Brain::open_loaded` calls `KnowledgeCards::load`. When a retrieval thread is attached, `Brain::retrieval_spoke` returns `Miss` as `UNKNOWN_REPLY` and `Brain::gather` does not then pick a card. **Found** (`brain.rs` `gather`, `retrieval_spoke`). `CardEngine::turn_with_ear` speaks a loaded card when that reply is `UNKNOWN_REPLY`, without a second read of the retrieval cell. **Found** (`voice-stack/horbm-bridge/src/cards.rs` `turn_with_ear`, `card_after_unknown`). `RetrievalThread::reload` re-reads the card directory on the worker. Callers of `reload` in this tree are `horbm/crates/horbm/tests/reload.rs`. **Found.**

`RetrievalConfig::from_flags` uses `WINDOWS_KNOWLEDGE_DIR` only on Windows, and only when that directory already exists. Other targets return no fallback. **Found** (`horbm/crates/horbm/src/retrieve.rs` `windows_fallback`).

**Evidence.** `horbm/crates/horbm/tests/cards.rs`, `horbm/crates/horbm/tests/reload.rs`. **Found** (those files exist).

### Retrieval

**Must.** The hot path is retrieval, in process. A miss is `UNKNOWN_REPLY` (`I don't know.`). `remember` stays deny until the process is given `GALION_HORBM_REMEMBER=grant`. The emotion-wire `remember` bit is not that grant. `Brain::turn` is not called from `Heartbeat::step`. **Found** (`LOCKS.toml` `[horbm]`, `horbm/crates/horbm/src/lib.rs`, `brain.rs` `turn`). `crates/galion-heartbeat` `step` functions do not name `Brain::turn`. **Found.**

**On main.** `MEMORY_HIT_CAP` is 4, the same number as `horbm_brain::SCAN_CAP`. **Found** (`retrieve.rs`, `horbm/crates/horbm-brain/src/scan.rs` `SCAN_CAP`). `RetrievalConfig.keys_path` defaults to `None`. `RetrievalConfig::from_env` sets it from `GALION_HORBM_KEYS` when that value is non-empty, and does not open the file. **Found** (`retrieve.rs` `from_env`, `keys_path_from_flag`). `asked` calls `asked_scan` when a scan is built, and `asked_lexical` otherwise: `search_lexical`, then `covers`, then the blocked-id filter, capped at `MEMORY_HIT_CAP`. **Found** (`asked`, `asked_lexical`). A granted rebuild with `keys_path` set calls `DiaryScan::open_capped` inside `prepare_scan`, not on the query. `scan_selected_into` writes into a `[ScanHit; MEMORY_HIT_CAP]` the worker owns. **Found** (`rebuild`, `prepare_scan`, `asked_scan`). `DiaryScan` also stores `DIARY_DIM` 256 and `SCAN_RRF_K` 60.0, and the dot is `galion_tensor::simd::dot_i8_for`. **Found** (`scan.rs`). The module comment on `scan.rs` says the thread calls the scan when a key file is passed and `search_lexical` when it is not. **Found.**

The pinned `horbm-memory` rev does not return stored embedding rows. `prepare_scan` rebuilds them with `embed_diary_into` from text the store already holds. An empty store scans keys only. **Found** (`prepare_scan`). `Int8Scorer` is implemented for the mapped key arena and the retrieval thread does not call it. **Found** (`retrieve.rs` `Int8Scorer`, `lib.rs`).

Learned rows are a third input on that same thread. `LEARNED_ROW_CAP` is 256, the same number as `LEARN_PACK_CAP`. `RetrievalThread::set_learned` keeps the newest dates. `Brain::attach_learned` forwards that call and is not used by `Brain::turn`. A hit whose source starts with `learned;` is spoken as `learned:<item_id>`. **Found** (`retrieve.rs` `LEARNED_ROW_CAP`, `insert_learned`, `brain.rs` `attach_learned`, `hit_spoke`).

`CardEngine::open` is the host read of `GALION_HORBM_REMEMBER` and `GALION_HORBM_KEYS`. An empty key path leaves `keys_path` unset, which stays lexical. **Found** (`voice-stack/horbm-bridge/src/cards.rs` `open`, `retrieval_config`).

**Evidence.** `horbm/crates/horbm/tests/retrieve.rs`, `horbm/crates/horbm/tests/retrieve_alloc.rs`, `horbm/crates/horbm/tests/learned_recall.rs`. **Found** (those files exist).

### Person memory

**Must.** The person diary is a second file. A stored line is spoken only when the utterance is about the person. Denied `remember` does not create the file. Forget archives a tombstone. The text stays. There is no hard delete. **Found** (`LOCKS.toml` `[horbm]`, `horbm/crates/horbm/src/memory.rs`, `about_the_person`).

**On main.** `EpisodicStore::open_from_flag` calls `open(path, true)` only when the trimmed flag is `grant`. `Brain::open` passes `remember_flag_from_process` through `open_with_persona`. **Found** (`memory.rs` `open_from_flag`, `brain.rs` `open`, `remember_flag_from_process`). `CardEngine` attaches `EpisodicStore::open` with the same grant bit the retrieval config already computed. **Found** (`voice-stack/horbm-bridge/src/cards.rs` `open_from_config`).

Pins. `pin_command` returns the text after `remember` or `please remember`. `turn_with_ear` calls `observe_memory` only when the line is not a forget. `observe_memory` calls `PersonMemory::pin` when `pin_command` returns text. A pin whose normalised text is already live is not written again. **Found** (`memory.rs` `pin_command`, `pin`; `brain.rs` `turn_with_ear`, `observe_memory`).

Forget. `forget_clause` yields `ForgetAsk::That` or `ForgetAsk::About`. `archive_forgotten` calls `forget_matching` for a topic and `forget` of the newest `memory:<id>` snap for "that". A `learned:<id>` snap is not archived. `forget_matching` is the trait default: `recall`, then `forget` each hit. `PersonMemory::forget` copies the row into the archive. **Found** (`horbm/crates/horbm/src/dialogue/intent.rs` `forget_clause`, `brain.rs` `archive_forgotten`, `forget_newest_memory`, `crates/galion-emotion-core/src/standins/memory.rs` `forget_matching`). When the archived count is greater than zero, `speak_forgotten` replaces the reply with `Persona::line(PersonaLine::Forgot)`, or `UNKNOWN_REPLY` when that line is unset, and the source is `forget:<count>`. **Found** (`brain.rs` `speak_forgotten`).

List. `EpisodicStore::list_person` returns live rows for one person, newest `source_date` first, at most `limit.min(MEMORY_HIT_CAP * 4)`. A forgotten row and the session-ring twin stay out. `Brain::turn` does not call it. **Found** (`memory.rs` `list_person`). `horbm-memory-audit list` reads `EpisodicStore::read_items` and `read_archive`, so tombstones are in that print, and it does not call `list_person`. **Found** (`horbm/crates/horbm/src/bin/horbm-memory-audit.rs` `list`).

The RAM session buffer keeps `SESSION_TURN_CAP` lines, defined as `horbm_memory::RING_CAP`. `note_turn` drops the oldest. `end_session` writes that tail once. **Found** (`memory.rs` `SESSION_TURN_CAP`, `note_turn`, `end_session`). Recall on the turn searches `recall_topic(recall_clause(heard))` and speaks a hit only when `about_the_person` is true. **Found** (`brain.rs` `person_hit`, `horbm/crates/horbm/src/dialogue/intent.rs` `recall_topic`).

**Evidence.** `horbm/crates/horbm/tests/memory.rs`, `horbm/crates/horbm/tests/memory_forget.rs`, `horbm/crates/horbm/tests/memory_list.rs`, `horbm/crates/horbm/tests/forget_turn.rs`, `horbm/crates/horbm/tests/memory_audit.rs`. **Found** (those files exist).

### Learn pack

**Must.** Learn is a separate consent, default deny, not an emotion-wire bit. `GALION_HORBM_LEARN=grant` is the only process grant. A fresh pass writes nothing. A granted pass writes one sourced row. Withdrawal leaves no row. `Brain::turn` does not call the pass. **Found** (`docs/horbm/foundations/F7-learn-deep-dive.md`, `horbm/crates/horbm/src/learn.rs`, `LOCKS.toml` `[consent] learn = "deny"`, `learn_in_band = false`).

**On main.** `LEARN_PACK_CAP` is 256. `LearnPass::write_clause` copies live rows into `LEARN_PACK_NEXT` and swaps that directory onto `LEARN_PACK_LIVE`. The same trimmed speaker and text is not inserted twice. At the cap a further distinct clause is refused. `LearnPass::retract_clause` removes one speaker-and-text row and copies the rest in order. `LearnedRow::item_id` is not stable across that swap. The pack is not encrypted. **Found** (`learn.rs` `write_clause`, `retract_clause`, `LEARN_PACK_CAP`). `Brain::turn` does not call `retract_clause`. Callers in this tree are `horbm/crates/horbm/tests/learn_retract.rs`. **Found.**

`SessionSeal` keeps one clause in RAM. `note` replaces it. `note_taught` keeps `heard` only when the previous turn was a non-empty miss, `heard` is not a question, and `covers` is true against that miss. `seal` calls `write_clause` once and clears the slot. A second `seal` with an empty slot writes nothing. A denied pass writes nothing. **Found** (`learn.rs` `SessionSeal::note`, `note_taught`, `seal`). `Brain::turn` does not call them. **Found** (`learn.rs` module comment).

Seal at finish is the host. `HostSession::on_turn` calls `note_taught` when learn is granted and a previous miss is held, and otherwise `note`, until a taught line is kept. `HostSession::finish` calls the diary `end_session`, then `SessionSeal::seal`. It is not called from `Drop`. `Brain::finish` does not seal: it caps an unpicked reply and splits the clause. **Found** (`voice-stack/local-server/src/host.rs` `on_turn`, `finish`; `brain.rs` `finish`).

**Evidence.** `horbm/crates/horbm/tests/learn.rs` `phase7_gate_fresh_writes_nothing_granted_writes_one_sourced_row_withdrawal_leaves_no_row`, `horbm/crates/horbm/tests/learn_retract.rs`, `horbm/crates/horbm/tests/taught.rs`, `horbm/crates/horbm/tests/session.rs`. **Found** (those files exist).

### Dialogue intents

**Must.** After the evidence is in, one move: answer, cite, ask, comfort, offer topic, or stop. Answer and cite need a truth-card source. A research hit is never an answer or a citation. The heuristic is deterministic. The offline bandit counts a move only when learn is granted, and the count does not change the move. **Found** (`docs/horbm/foundations/F4b-dialogue-manager.md`, `horbm/crates/horbm/src/dialogue/mod.rs`, `policy.rs` `choose_held`).

**On main.** `TurnIntent` is `Chat`, `Ask`, `Task`, `Vent`, `Goodbye`, `Recall`, `Correct`, `Forget`. `TurnIntent::read` prefers a correction phrase, then a forget phrase, then a recall phrase, then the empathy label. The tables are `CORRECT`, `FORGET`, and `RECALL` in `horbm/crates/horbm/src/dialogue/intent.rs`. A negation against a forget phrase is not forget. **Found** (`intent.rs` `TurnIntent::read`). `Dialogue::decide` calls `decide_held(input, false)`. `Brain::turn_with_ear` calls `decide_held` when `topic_hold` is set (`DialogueLean.hold_topic`) and `decide` otherwise. A held `Move::OfferTopic` becomes `Move::Ask` and the offer line is not built. **Found** (`horbm/crates/horbm/src/dialogue/mod.rs` `decide`, `decide_held`; `brain.rs` `turn_with_ear`, `topic_hold`).

Recall keeps the gathered clause when a source is `memory:<id>` (not `memory:callback`) or `learned:<id>`. Any other recall, and every `Correct` or `Forget`, is `Move::Ask` with `UNKNOWN_REPLY` and `honest:unknown`. A correction or a forget is not bandit-counted. **Found** (`policy.rs` `recall_without_memory`, `choose_held`, `honest_unknown`). A forget that archived at least one row then replaces that miss with the forgot line, in `speak_forgotten`, after `decide`. A correction does not call `LearnPass::retract_clause`. **Found** (`brain.rs`).

`TurnOut.intent` and `DecideIn.intent` are empathy's label (`MoodMeta.intent`). `Dialogue::last_intent_label` stores the typed label. `brain.rs` does not call it. **Found** (`brain.rs` `finish`, `horbm/crates/horbm/src/dialogue/mod.rs` `last_intent_label`). `OfflineBandit::choose_steered` exists and `decide_held` does not call it. **Found** (`policy.rs` `choose_steered`).

**Evidence.** `horbm/crates/horbm/tests/intent.rs`, `horbm/crates/horbm/tests/dialogue.rs`, `horbm/crates/horbm/tests/turn.rs`. **Found** (those files exist).

### Persona lines and caps

**Must.** Boot loads the configured development persona. There is no `allows_development_boot`, and boot does not load one named persona. `never_fabricate = true`. Unknown line is `I don't know.` `horbm/crates/horbm-persona/personas/aria.toml` stays in the repo and boot does not load it unless the config names that id. **Found** (`docs/horbm/foundations/F-persona.md` honesty rules, `horbm/crates/horbm-persona/src/load.rs` `never_fabricate`, `horbm/crates/horbm/src/lib.rs` boot paragraph, `horbm/crates/horbm-persona/development.toml`).

**On main.** `Persona::load_development` and `Brain::boot` with no path load the configured development persona. `Brain::boot` with a path installs that file and does not check the configured id. `Brain::with_cards` and `Brain::open` install the configured persona when configuration is present, and `Persona::standin` when it is not. **Found** (`horbm/crates/horbm/src/lib.rs`, `brain.rs` `boot`, `with_cards`, `open`).

`PersonaLine` is `Forgot` and `NothingRemembered`. `LINE_BYTES` is 80. `Persona::line` returns `None` when the file leaves the slot unset. `speak_forgotten` reads `PersonaLine::Forgot`. No function under `horbm/crates/horbm/src` reads `PersonaLine::NothingRemembered`. The loader packs both slots. **Found** (`horbm/crates/horbm-persona/src/lib.rs` `PersonaLine`, `LINE_BYTES`, `Persona::line`; `horbm/crates/horbm-persona/src/load.rs` `pack_lines`). The persona file named by `development.toml` keys `development_persona` and `persona_dir` sets `[style] brevity` 0.55, `warmth` 0.8, `max_sentences` 2, and `[lines]` `forgot` plus `nothing_remembered`. `load.rs` accepts `max_sentences` from 1 to 4. **Found.**

`StyleHook::cap_sentences` borrows the reply down to `max_sentences`. `Brain::finish` calls it on an unpicked reply that is not `UNKNOWN_REPLY` and not empty. A picked clause keeps its text. **Found** (`horbm/crates/horbm-persona/src/style.rs` `cap_sentences`, `brain.rs` `finish`). `StyleHook.rate` is `1 - BREVITY_RATE * brevity` with `BREVITY_RATE` 0.15. `apply_prosody` multiplies `MoodMeta.rate` by `DialogueLean.rate` and does not read `StyleHook.rate`. **Found** (`style.rs` `BREVITY_RATE`, `brain.rs` `apply_prosody`). Duplex copies `hook.rate` onto the configured persona port. **Found** (`horbm/crates/horbm-duplex/src/persona.rs`).

**Evidence.** `horbm/crates/horbm/tests/persona.rs`, `horbm/crates/horbm-persona/tests/lines.rs`, `horbm/crates/horbm-persona/tests/style.rs`. **Found** (those files exist).

### Ear

**Must.** The ear starts on. One frame is 80 ms, 16 kHz, 1280 samples. The point is valence, arousal, dominance, approach, plus a strength. It is not a category. Denied frames are not read. The core mixes an inbound pull only when sense and adapt are both on. Session adapt stays deny. **Found** (`docs/horbm/foundations/S27-ear-prosody.md`, `LOCKS.toml` `[consent.ear_prosody]`, `crates/horbm-ear/src/lib.rs` `Ear::from_env`, `FRAME_SAMPLES`).

**On main.** `ListenerOffer::heard_axes` is valence, arousal, and strength, or nothing when one is non-finite. `Heard::from_axes` returns `None` when a value is non-finite or `confidence < EAR_CONFIDENCE` (0.35). `SAD_EAR_VALENCE` is −0.20. **Found** (`crates/horbm-ear/src/lib.rs`, `horbm/crates/horbm-inner/src/lib.rs` `EAR_CONFIDENCE`, `SAD_EAR_VALENCE`, `life.rs` `Heard::from_axes`). `Brain::ear_sense_from_flag` uses the ear's rule: unset, empty, `1`, `on`, and `allow` are on; `0`, `off`, and `deny` are off. `with_cards`, `open`, and `try_open` call `apply_configured_ear`. `turn` passes no point. `turn_with_ear` passes the `Heard` into `PersonaInner::turn` and counts it only while `ear_sense` is on. **Found** (`brain.rs` `turn`, `turn_with_ear`, `ear_sense_from_flag`, `apply_configured_ear`).

The live host pushes PCM until `FRAME_SAMPLES`, then `heard_point` calls `heard_axes` and `Heard::from_axes`. `ProsodyBrain::turn` takes that point and `HorbmMind::turn_heard` calls `CardEngine::turn_with_ear`. **Found** (`voice-stack/local-server/src/live.rs` `flush_prosody_frame`, `heard_point`, `ProsodyBrain::turn`; `voice-stack/local-server/src/brain.rs` `turn_heard`). `HorbmTurn::turn` calls `Brain::turn` and does not pass a point. **Found** (`horbm/crates/horbm-voice/src/brain.rs` `turn`).

**Evidence.** `crates/horbm-ear/tests/frames.rs` `default_is_on_and_only_the_off_switch_closes_it`. **Found.**

### Duplex

**On main.** `Duplex::tick` is one 80 ms frame at 16 kHz, 1280 samples. Prosody sensing is on unless `Duplex::set_prosody` turns it off. `ProsodyEar` reports a voiced bit and `f0_hz`. It does not build a `Heard`. **Found** (`horbm/crates/horbm-duplex/src/lib.rs`, `horbm/crates/horbm-duplex/src/prosody.rs` `ProsodyEar`). `BrainHandle::fill` calls `Brain::turn`, which passes no ear point. This crate does not name `horbm_ear`. Nothing maps `f0_hz` onto valence. **Found** (`horbm/crates/horbm-duplex/src/brain.rs` `fill`). The configured development persona is `ConfiguredPersona` via `open_persona`. **Found** (`horbm/crates/horbm-duplex/src/persona.rs`).

**Gap.** The live host passes a listener point into `turn_with_ear`. Duplex prefetch does not. **Found.**

**Evidence.** `horbm/crates/horbm-duplex/tests/adapters.rs` `prosody_hears_a_synthetic_sine_and_the_off_switch_is_explicit`. **Found.**

### Inner life

**Must.** Two layers. `InnerLife` is four slots each of care, want, and expect, tokens of at most 16 ASCII bytes, learning off while `learn` is deny. Seed tokens are `heard`, `answer`, `pause`. `PersonaInner` is four floats (care, curiosity, help, expect) for this session only, and it does not keep user facts. A turn may bias pace. It does not replace the spoken clause. **Found** (`docs/horbm/foundations/F-inner-life.md`, `docs/horbm/foundations/F-persona.md`, `horbm/crates/horbm-inner/src/lib.rs`, `horbm/crates/horbm/src/lib.rs`).

**On main.** `turn_with_ear` calls `PersonaInner::turn` with the ear option and `sense: self.ear_sense`, then `apply_prosody` on that lean. After the reply strings are fixed, `record_inner` calls `inner_turn` and drops the result. It does not write the reply, the empathy mood, or the baseline, and it does not call `step_moment`. **Found** (`brain.rs` `turn_with_ear`, `record_inner`). `inner_nudge` and `persona_nudge` remain separate methods. **Found** (`brain.rs` `persona_nudge`). A missing ear is ignored when `sense` is off; words still count. **Found** (`horbm/crates/horbm-inner/src/life.rs` `apply_turn`).

**Evidence.** `horbm/crates/horbm-inner/tests/persona_inner.rs`, `horbm/crates/horbm-inner/tests/dev_persona.rs`, `horbm/crates/horbm/tests/persona_inner_turns.rs`. **Found** (those files exist).

### Emotion core

**Must.** One moment: valence, arousal, dominance, approach, on an 80 ms frame. Inbound pulls mix only when sense and adapt are both granted. Session `[consent]` stays deny. The moment step does not rewrite the spoken clause or the baseline. **Found** (`docs/horbm/foundations/F-emotion-core.md`, `horbm/crates/horbm/src/lib.rs`).

**On main.** `EmotionCore::step` is reached from `Brain::step_moment`, which `turn_with_ear` does not call. **Found** (`brain.rs` `step_moment`). `horbm_ear::link::push_frame` can step a core the ear crate owns. That is not `Brain`'s core. **Found** (`crates/horbm-ear/src/lib.rs`).

**Gap.** The brain's moment does not move on a chat turn. That split is what `lib.rs` specifies. **Found.**

**Evidence.** `horbm/crates/horbm/tests/moment_split.rs`, `crates/galion-emotion-core/tests/moment.rs`. **Found** (those files exist).

### How `Brain::turn` runs

**Must.** The order in `lib.rs`. Research inside the turn is the style root only (`GALION_HORBM_RESEARCH_DIR`). The gateway ask waits until `TurnOut` is dropped. `RESEARCH_CAP` is 3 seconds. **Found** (`horbm/crates/horbm/src/lib.rs`, `voice-stack/research-gateway/src/lib.rs`).

**On main.** `turn` calls `turn_with_ear(text, None)`. That function reaps a previous follow-up, ticks empathy, steps `PersonaInner`, archives a forget or notes person memory, takes `person_hit` or `gather`, searches style when a research root is set, calls `decide` or `decide_held`, runs `admit_move`, applies `speak_forgotten` when rows were archived, then `finish`, then `record_inner`. `research_later` is true only when a port exists, the lean is not comfort, and the gap reason is `ResearchOff`. `gather` calls `route_heard` with the research port `None`. **Found** (`brain.rs` `turn_with_ear`, `gather`). `GAP_CAP` is 32. **Found** (`brain.rs`).

`ReasoningRouter::turn` calls `Brain::turn` and drops the `TurnOut`. `ReasoningRouter::route` still calls `route_heard` with a research port. The in-call `ReplyEngine::turn` does not speak that route. **Found** (`voice-stack/horbm-bridge/src/router.rs` `turn`, `route`). `CardEngine::turn_with_ear` is what the live host calls. **Found** (`voice-stack/local-server/src/brain.rs` `turn_heard`). A closed mic take runs that call off the tokio worker, in `finish_take_off_worker`. The 80 ms frame step stays on the task. **Found** (`voice-stack/local-server/src/server.rs` `handle_binary`, `finish_take_off_worker`).

**Evidence.** `horbm/crates/horbm/tests/turn.rs`, `horbm/crates/horbm/tests/clause_first.rs`. **Found** (those files exist).

### Research cards

**Must.** Unset, empty, or not a directory means no read and no created directory. Cards are not answers. No dataset text in the repo. Gate: `scripts/check-research-boundary.sh`. **Found** (`docs/horbm/RESEARCH-DATA.md`, `horbm/crates/horbm/src/research_style.rs` `from_env`, `accept_card`).

**On main.** `ResearchRoot::from_env` reads `GALION_HORBM_RESEARCH_DIR` only. `accept_card` drops a card whose `not_for` does not bar answer and cite. `Brain::turn_with_ear` skips `StyleSearch` when the root is empty, so an unset root does not open the search library on the soak. **Found** (`research_style.rs`, `brain.rs` `turn_with_ear`).

**Gap.** `ReasoningRouter::route` can still call the gateway inside that function. `ReasoningRouter::turn` does not. **Found** (`voice-stack/horbm-bridge/src/router.rs`).

**Evidence.** `horbm/crates/horbm/tests/research_style.rs`. **Found** (that file exists).

## S66 rounds

The sections below are the S66 assignment, written against `b8f33bb7`. They are not a second spec. The layer audit above is `main` `86f9f0a703a01c73b29e017662ba143af04ef958`. A symbol the audit names is on main.


## Shared files

Every swarm leaves these untouched. They are not assigned to a swarm in any round.

| Path | Why it stays |
|---|---|
| `Cargo.toml`, `Cargo.lock` | Root workspace. `horbm` is excluded. `horbm-ear` and `horbm-keys` are already members. |
| `horbm/Cargo.toml` | Horbm workspace membership. Every crate the swarms edit is already a member. |
| `LOCKS.toml`, `scripts/check-locks.sh` | The ear, learn, remember, and research defaults are already recorded. |
| (no soul file) | Not in this tree. Do not create one. |
| `docs/horbm/PLAN-brain.md` | This plan. |
| `docs/horbm/README.md`, `docs/horbm/RESEARCH-DATA.md`, `docs/horbm/PORTABILITY.md`, `docs/horbm/foundations` | Index and notes, not the build. |
| `scripts/check-research-boundary.sh` | The corpus gate. |
| `voice-stack/Cargo.toml`, `voice-stack/Cargo.lock` | Voice workspace. |
| `voice-stack/horbm-bridge/Cargo.toml`, `voice-stack/horbm-bridge/Cargo.lock` | Bridge manifest. No new dependency. |
| `voice-stack/local-server` | S82, the live host, including `PiperMouth`. Other swarms leave it. Speech stays as it is. Do not restart the process on 7881. |
| `horbm/Cargo.lock` | Assigned in round 4 only, to swarm R4-A. Rounds 1–3 leave it untouched. |

A swarm edits a crate manifest only when the table below names that manifest. No swarm adds a dependency except R4-A, and that dependency is the existing `horbm-ear` crate by path.

Round N starts after every swarm in rounds before it has merged. Inside a round the four branches start from that same base. Paths in one round do not overlap, so the four branches merge without editing the same file.

## Round 1 — pure contracts

No swarm edits `horbm/crates/horbm`. No swarm edits a workspace manifest.

### R1-A — Scan opens at the cap the thread will use

**Goal.** Add `DiaryScan::open_capped(keys, diary, remember)` that calls the existing `open` with `SCAN_CAP`, so the thread cannot pick a second cap.

**Owns.** `horbm/crates/horbm-brain` (that crate's `Cargo.toml` included; do not add a dependency).

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --lib open_capped_uses_scan_cap` and the existing `cargo test --manifest-path horbm/Cargo.toml -p horbm-brain --test scan_alloc`. `k()` equals `SCAN_CAP`. `scan_selected_into` is unchanged.

**Serves the brain.** The hot path's scorer already exists. The thread needs one setup call that uses the same cap as `MEMORY_HIT_CAP` (both are 4). **Found.**

**Depends on.** Nothing.

### R1-B — One constructor for an ear point

**Goal.** Add `Heard::from_axes(valence, arousal, confidence) -> Option<Heard>`. Return `None` when any value is non-finite or when `confidence < EAR_CONFIDENCE`. Do not change `apply_turn`.

**Owns.** `horbm/crates/horbm-inner`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-inner --test dev_persona --test persona_inner`. The low-confidence ear is no claim. Words still count when the ear is off.

**Serves the brain.** `PersonaInner` already knows how to read a `Heard`. The missing piece is a constructor the ear's three floats can fill.

**Depends on.** Nothing. It does not import `horbm-ear` (different workspace).

### R1-C — The offer exposes those three floats

**Goal.** Add `ListenerOffer::heard_axes(self) -> Option<(f32, f32, f32)>` as `(valence, arousal, strength)`, or `None` when any of the three is non-finite. Sensing stays on by default. The new function does not call `EmotionCore::step`.

**Owns.** `crates/horbm-ear` (that crate's `Cargo.toml` included; do not add a dependency). The root `Cargo.toml` stays untouched.

**Acceptance.** `cargo test -p horbm-ear --test frames a_heard_frame_exposes_axes_and_a_denied_frame_does_not`. `default_is_on_and_only_the_off_switch_closes_it` still passes.

**Serves the brain.** The ear and the inner layer meet as three floats. A denied frame still has no point.

**Depends on.** Nothing.

### R1-D — Boot id is the configured development persona

**Goal.** Superseded. Do not add `Persona::allows_development_boot`. `Persona::load` of `horbm/crates/horbm-persona/personas/aria.toml` still succeeds. Do not delete that file. Development boot loads the configured development persona (`Persona::load_development`, `Persona::is_configured_boot`).

**Owns.** `horbm/crates/horbm-persona`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-persona --test persona`. `horbm/crates/horbm-persona/personas/aria.toml` still loads. It is a development boot only when the config names its id.

**Serves the brain.** The turn installs the configured development persona. `horbm/crates/horbm-persona/personas/aria.toml` remains a schema fixture.

**Depends on.** Nothing.

### Round 1 clash table

| Path | R1-A | R1-B | R1-C | R1-D |
|---|---|---|---|---|
| `horbm/crates/horbm-brain` | yes | | | |
| `horbm/crates/horbm-inner` | | yes | | |
| `crates/horbm-ear` | | | yes | |
| `horbm/crates/horbm-persona` | | | | yes |
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

**Owns.** `horbm/crates/horbm/src/dialogue`, `horbm/crates/horbm/tests/dialogue.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test dialogue`. New test `hold_false_matches_decide_and_hold_true_blocks_offer_topic`.

**Serves the brain.** `PersonaInner` already computes `hold_topic`. The move picker ignores it, so a check-in can still offer a new topic.

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
| `horbm/crates/horbm/src/dialogue` | | | yes | |
| `horbm/crates/horbm/tests/dialogue.rs` | | | yes | |
| `horbm/crates/horbm/src/learn.rs` | | | | yes |
| `horbm/crates/horbm/tests/learn.rs` | | | | yes |
| `brain.rs`, `lib.rs`, `horbm/Cargo.toml`, `horbm/Cargo.lock`, root manifests, `LOCKS.toml` | | | | |

No path is marked in two columns.

## Round 3 — the turn, the second router, the moment lock, the learn export

Boot does not use `allows_development_boot` and does not load one named persona. `Brain::boot` with no path, `try_open`, and `try_with_cards` load the configured development persona. An explicit path installs that file and does not check the configured id. `with_cards` and `open` install the configured persona when configuration is present, and the stand-in fixture when it is not. In development, S82 (`voice-stack/local-server`), the live host, defaults `GALION_HORBM_LEARN` and `GALION_HORBM_REMEMBER` to grant in its dev profile. Outside development those stay off unless granted. The horbm crates and `LOCKS.toml` stay deny.

### R3-A — `Brain::turn` takes an ear and keeps the clause

**Goal.**

- `turn(&str)` calls `turn_with_ear(text, None)`.
- `turn_with_ear` passes the `Heard` into `Perceived`. `None` keeps today's word path.
- Add `ear_sense_from_flag(Option<&str>) -> bool` with the same rule as `Ear::from_env`: unset, empty, `1`, `on`, and `allow` are on; `0`, `off`, and `deny` are off; trim and ignore case. `with_cards` uses that function on `GALION_EAR_SENSE` instead of the literal `false`. Add `ear_sense(&self) -> bool`. The test calls the pure function and does not set the process environment.
- `Brain::open` uses `EpisodicStore::open_from_flag`.
- `boot` with no path loads the configured development persona (`Persona::load_development` / `is_configured_boot`). Do not call `allows_development_boot`. An explicit path installs that file. `with_cards` installs the configured persona when configuration is present, and the stand-in fixture when it is not.
- Call `decide_held` with `note.lean().hold_topic` in place of `decide`.
- After `reply` and `first_clause` are fixed, call `inner_turn` and do not write its result into those strings, the empathy mood, or the baseline.
- Do not call `step_moment`, `LearnPass`, or `SessionSeal`. Do not change `apply_prosody`. Do not cut a clause to `max_sentences`.
- A card hit stays that card's clause when a `Heard` is also passed, including a sad ear (`valence` at or below `SAD_EAR_VALENCE`, which is −0.20, and confidence at or above `EAR_CONFIDENCE`).

**Owns.** `horbm/crates/horbm/src/brain.rs`, `horbm/crates/horbm/tests/turn.rs`, `horbm/crates/horbm/tests/persona_inner_turns.rs`, `horbm/crates/horbm/tests/moment.rs`, `horbm/crates/horbm/tests/persona.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test turn --test persona_inner_turns --test moment --test persona`. New tests: `text_only_kettle_clause_is_unchanged`, `a_sad_ear_does_not_replace_a_card_clause`, `ear_sense_from_flag_defaults_on`, `boot_installs_the_persona_the_env_names`, `boot_refuses_when_nothing_is_configured`. The kettle line stays `The kettle is on the hob.` with first clause `The kettle`, which is what `horbm-brain` already asserts for that fixture. **Found** (`horbm/crates/horbm-brain/src/lib.rs` test `a_turn_produces_the_fixture_clause`).

**Serves the brain.** This is the one function a host already calls. After this round it can see an ear, it installs the configured development persona, it opens the person file on the real grant, and the spoken clause stays the lookup.

**Depends on.** R1-B, R1-D, R2-B, R2-C. It does not edit `retrieve.rs`. An unset `keys_path` keeps the live host on lexical search.

### R3-B — The bridge router uses the same turn

**Goal.** `ReasoningRouter::turn` builds the `Brain` it can already build and calls `Brain::turn`. It copies the local reply out and drops the `TurnOut` so a deep-miss follow-up still starts after the local clause exists. It does not call `route_heard` with a research port. `route_heard` stays, because changing it would change every current caller in the same edit. Clause cutting for the socket (`chunks.rs`) stays.

**Owns.** `voice-stack/horbm-bridge/src/router.rs`, `voice-stack/horbm-bridge/src/research.rs`, `voice-stack/horbm-bridge/tests/router_gateway.rs`, `voice-stack/horbm-bridge/tests/bridge.rs`

**Acceptance.** `cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-horbm`. A deep question returns `I don't know.` on this call's reply text when the local cards miss. The test does not require a network listener.

**Serves the brain.** The live host's `CardEngine` already uses `Brain::turn`. `ReasoningRouter` is the remaining entry that can speak a gateway body inside the call.

**Depends on.** The `Brain::turn` that is already on main. It does not need R3-A's new method. Developed in parallel with R3-A, it keeps calling `turn(&str)`.

### R3-C — A text turn leaves the moment where it was

**Goal.** Add one integration test. After `Brain::turn` on a text line, the moment position and the baseline identity match the values from before the call. `persona_nudge` / `step_moment` remain the methods that move the moment.

**Owns.** `horbm/crates/horbm/tests/moment_split.rs`.

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test moment_split`.

**Serves the brain.** `lib.rs` keeps the moment beside the turn so a feeling cannot replace the clause. The test locks that split against the edit in R3-A.

**Depends on.** Nothing. The test passes on today's `turn` and on R3-A's `turn` as long as R3-A does not call `step_moment`.

### R3-D — Export the session seal

**Goal.** `pub use` `SessionSeal` from `horbm/crates/horbm/src/lib.rs`. Add `horbm/crates/horbm/tests/session.rs`: construct a brain, call `turn`, and assert no learn-pack sqlite file appears when learn is unset. Calling `SessionSeal::seal` stays the only write.

**Owns.** `horbm/crates/horbm/src/lib.rs`, `horbm/crates/horbm/tests/session.rs`.

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm --test session --test learn`.

**Serves the brain.** Hosts in round 4 can name the seal. `turn` still cannot write the pack.

**Depends on.** R2-D.

### Round 3 clash table

| Path | R3-A | R3-B | R3-C | R3-D |
|---|---|---|---|---|
| `horbm/crates/horbm/src/brain.rs` | yes | | | |
| `horbm/crates/horbm/tests/turn.rs` | yes | | | |
| `horbm/crates/horbm/tests/persona_inner_turns.rs` | yes | | | |
| `horbm/crates/horbm/tests/moment.rs` | yes | | | |
| `horbm/crates/horbm/tests/persona.rs` | yes | | | |
| `voice-stack/horbm-bridge/src/router.rs` | | yes | | |
| `voice-stack/horbm-bridge/src/research.rs` | | yes | | |
| `voice-stack/horbm-bridge/tests/router_gateway.rs` | | yes | | |
| `voice-stack/horbm-bridge/tests/bridge.rs` | | yes | | |
| `horbm/crates/horbm/tests/moment_split.rs` | | | yes | |
| `horbm/crates/horbm/src/lib.rs` | | | | yes |
| `horbm/crates/horbm/tests/session.rs` | | | | yes |
| `cards.rs` in the bridge, `chunks.rs`, `voice-stack/local-server`, `horbm/Cargo.lock`, root manifests, `LOCKS.toml` | | | | |

`horbm/crates/horbm/tests/moment.rs` and `horbm/crates/horbm/tests/moment_split.rs` are different files. `horbm/crates/horbm/src/brain.rs` and `horbm/crates/horbm/src/lib.rs` are different files.

## Round 4 — hosts pass the point; the mouth stays

S82 (`voice-stack/local-server`) is the live host. Swarms R4-A through R4-D do not edit that tree, `mouth.rs`, `piper_mouth.rs`, or `moment_voice.rs`, and they do not restart the process on 7881. In development, S82's dev profile defaults `GALION_HORBM_LEARN` and `GALION_HORBM_REMEMBER` to grant. Outside development nothing turns them on unless granted. The horbm crates and `LOCKS.toml` stay deny (`remember = "deny"`, `learn = "deny"`, `learn_in_band = false`). No encryption. `RetrievalConfig.keys_path` stays unset by default.

### R4-A — Duplex stores an ear frame on the brain it already shares

**Goal.** On a PCM slice whose length is `FRAME_SAMPLES` (1280), call the existing `horbm_ear::Ear::push`. When the frame is `Heard` and `heard_axes` is `Some`, keep `Heard::from_axes(...)` and pass it to `turn_with_ear` on the next prefetch. Any other length, a denied frame, or `from_axes` returning `None` passes `None`. Do not map `f0_hz` from `ProsodyEar` onto valence. Do not change when the mouth starts. Add the path dependency on `crates/horbm-ear` in the duplex crate manifest. Feature `piper` stays off.

**Owns.** `horbm/crates/horbm-duplex/Cargo.toml`, `horbm/crates/horbm-duplex/src/lib.rs`, `horbm/crates/horbm-duplex/src/brain.rs`, `horbm/crates/horbm-duplex/src/duplex.rs`, `horbm/crates/horbm-duplex/src/prosody.rs`, `horbm/crates/horbm-duplex/tests/adapters.rs`, `horbm/crates/horbm-duplex/tests/duplex.rs`, `horbm/Cargo.lock`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex --test adapters --test duplex`. New test `a_full_ear_frame_is_stored_and_the_card_clause_stays`. `horbm/crates/horbm-duplex/tests/piper_mouth.rs` is left untouched and is not the gate.

**Serves the brain.** Duplex is the loop that already has PCM and already calls `Brain::turn`. The ear crate is the prosody model; the duplex pitch estimate stays a voiced bit for the conductor.

**Depends on.** R1-B, R1-C, R3-A.

### R4-B — The voice trait can carry a point, and the energy ear does not invent one

**Goal.** Add `TurnBrain::turn_heard(&mut self, heard: &str, ear: Option<Heard>)` with a default body that calls `turn(heard)`. `HorbmTurn` implements it with `turn_with_ear`. `VoiceLoop` keeps calling `turn(&str)`. The energy ear does not build a `Heard`. Add `HorbmTurn::end_session`, which calls `SessionSeal::seal` only when the host has already supplied a `ClauseSource`. With no source, it writes nothing. Do not implement this on `Drop`. `voice-stack/local-server` `MindBrain` keeps compiling because the new trait method has a default body. **Found** (that impl is `voice-stack/local-server/src/native.rs`; this swarm does not edit it).

**Owns.** `horbm/crates/horbm-voice/src/brain.rs`, `horbm/crates/horbm-voice/src/ear.rs`, `horbm/crates/horbm-voice/src/turn.rs`, `horbm/crates/horbm-voice/src/energy.rs`, `horbm/crates/horbm-voice/src/lib.rs`, `horbm/crates/horbm-voice/tests/ear.rs`, `horbm/crates/horbm-voice/tests/end_of_turn.rs`

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-voice --test ear --test end_of_turn`. New tests `energy_ear_does_not_invent_a_heard_point` and `end_session_without_a_source_writes_nothing`.

**Serves the brain.** The live host's text turn stays a `&str`. A later host can pass a real point through the same trait, and learn still waits for an explicit session end. S82 owns that host.

**Depends on.** R3-A, R3-D, R1-B.

### R4-C — The eval gate locks the brain, not the mouth

**Goal.** Add the brain-model lock. It landed as `horbm/crates/horbm-eval/tests/brain_lock.rs`. Assert the miss text is `I don't know.`, the development persona is the configured development persona (not a hard-coded id), `ear_sense()` is true when the flag function is given `None`, and a text turn's reply matches the card clause for the kettle fixture. Do not edit `horbm/crates/horbm-eval` threshold, real, speech, or prosody sources. Do not change a WER number or a mouth score.

**Owns.** `horbm/crates/horbm-eval/src/gate.rs`, `horbm/crates/horbm-eval/src/score.rs`, `horbm/crates/horbm-eval/src/turn.rs`, `horbm/crates/horbm-eval/tests/gate.rs`, `horbm/crates/horbm-eval/tests/turn_gate.rs`, `horbm/crates/horbm-eval/tests/brain_lock.rs`. Edit `horbm/crates/horbm-eval/src/gate.rs`, `horbm/crates/horbm-eval/src/score.rs`, or `horbm/crates/horbm-eval/src/turn.rs` only if the test cannot call the public `horbm` API without them. Prefer the test file alone.

**Acceptance.** `cargo test --manifest-path horbm/Cargo.toml -p horbm-eval --test brain_lock --test gate --test turn_gate`

**Serves the brain.** The eval crate already scores `Brain::turn`. This lock is the model (lookup, miss line, the configured development persona, ear default), separate from the mouth round-trip.

**Depends on.** R3-A (`ear_sense()` and `turn_with_ear` exist; the test may call `turn(&str)` only).

### R4-D — The live host's card engine opens the person file on the same grant

**Goal.** In `brain_from_dir` / `CardEngine::open`, attach `EpisodicStore::open_from_flag` for the memory path the function already receives. Unset, empty, or any value other than `grant` stays deny in the library: no file, and `turn(&str)` stays the call. The development grant is S82's dev profile, not a default inside `horbm`. Do not pass a `Heard`. Do not set `keys_path`. `HorbmEngine::open` calls `brain_from_dir`, so it picks up the same attach without an edit to `linked.rs`.

**Owns.** `voice-stack/horbm-bridge/src/cards.rs`, `voice-stack/horbm-bridge/tests/diary.rs` (new).

**Acceptance.** `cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-horbm --test diary --test bridge`. With the grant unset, the kettle reply is unchanged and no person-diary file is created.

**Serves the brain.** `HorbmMind` in the live host (S82, `voice-stack/local-server`) calls `CardEngine::open`. **Found** (`voice-stack/local-server/src/brain.rs`). The person diary becomes reachable there when remember is granted. In development the dev profile grants it. The mouth is not in this diff.

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
| `horbm/crates/horbm-eval/tests/brain_lock.rs` | | | yes | |
| `voice-stack/horbm-bridge/src/cards.rs` | | | | yes |
| `voice-stack/horbm-bridge/tests/diary.rs` | | | | yes |
| `piper_mouth.rs`, `mouth.rs`, `voice-stack/local-server`, root `Cargo.lock`, `voice-stack/Cargo.lock`, bridge `Cargo.lock`, `LOCKS.toml` | | | | |

`horbm/crates/horbm-duplex/src/brain.rs` and `horbm/crates/horbm-voice/src/brain.rs` are different files. `horbm/Cargo.lock` appears in one column.

Left untouched inside duplex on purpose: `horbm/crates/horbm-duplex/src/mouth.rs`, `horbm/crates/horbm-duplex/src/piper_mouth.rs`, `horbm/crates/horbm-duplex/src/breath.rs`, `horbm/crates/horbm-duplex/src/echo.rs`, `horbm/crates/horbm-duplex/src/live.rs`, `horbm/crates/horbm-duplex/src/recog.rs`, `horbm/crates/horbm-duplex/src/report.rs`, `horbm/crates/horbm-duplex/src/ports.rs`, `horbm/crates/horbm-duplex/src/emotion.rs`, `horbm/crates/horbm-duplex/src/bin/horbm-duplex-live.rs`, `horbm/crates/horbm-duplex/tests/piper_mouth.rs`, `horbm/crates/horbm-duplex/tests/echo.rs`.

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

**Already archived.** `archive/2026-11-25/crates/galion-live-demo/ARCHIVED.md`, `archive/2026-11-24/crates/galion-v2-live/ARCHIVED.md`, `archive/2026-11-24/voice-stack/horbm-bridge/seam/horbm/ARCHIVED.md`. **Found.**

**Keep, because the live host still speaks through them.** S82 is `voice-stack/local-server`. `PiperMouth` (`voice-stack/local-server/src/mouth.rs` documents the Piper path). `runtimes/galion-piper-rt`. The research-gateway library (`voice-stack/research-gateway/src/lib.rs` `RESEARCH_CAP`). `galion-conductor`, which duplex uses to call `BrainHandle` (`horbm/crates/horbm-duplex/src/lib.rs`). `galion-heartbeat` stays; `docs/horbm/foundations/FP-first-principles-audit.md` step 8 says so, and this plan does not retune its constants. **Found.**

**Keep, because the brain still uses them.** `horbm-brain` `MemoryRetriever` and the clause fixture: `gather` still calls `pick` on the idle path. **Found** (`horbm/crates/horbm/src/brain.rs` `gather`). `horbm/crates/horbm-persona/personas/aria.toml`: inactive schema fixture. **Found** (`docs/horbm/foundations/F-persona.md`).

**Not the brain, and not free to delete.** `crates/galion-cascade` `PhoneLane` is the console `phone-turn` feature (`crates/galion-console/src/turn_worker.rs`). `horbm/` has no reference to `galion_cascade`. `crates/galion-cascade/src/horbm_chat.rs` uses `horbm_brain::Heuristic`, not `horbm::Brain::turn`. **Found.** `.github/workflows/gates.yml` still runs `cargo test -p galion-cascade --features cascade --lib`. **Found.** Archive that lane only after that gate is retargeted. `voice-stack/pipeline-orchestrator` (`galion-sam-orch`) returns "horbm is not linked; Brain::open stays in crate horbm" and does not link `horbm`. **Found** (`voice-stack/pipeline-orchestrator/src/voice_traits.rs`). The traits alias named for one persona is gone. `voice-stack/local-server/tests/stream.rs` names `galion_sam_orch::voice_traits::WaitGate`, so the crate is not unused. **Found.** No brain swarm should edit it.

**Chatterbox copies.** This pass found no `cbx-v2` or `chatterbox` string under `voice-stack/eval-golden`. Copies remain under `tools/cloning-v2/pipeline/cbx-v2` and `tools/symphony/sound-out/src/chatterbox.rs`. **Found.** A line count across `tools/` was not made. **Unknown** how many references remain. `docs/horbm/foundations/FP-first-principles-audit.md` already says to archive those copies after the voice-v2 and eval paths stop pointing at them. That is a later non-brain pass.

## Questions only Mate can answer

1. Who may set `GALION_HORBM_LEARN=grant`, and is the learn pack encrypted at rest? In development, S82's dev profile defaults learn and remember to grant. Outside development nothing turns them on unless granted. No encryption for now. The horbm crates still default deny and do not read the dev profile. **Found** for the deny default (`docs/horbm/foundations/F7-learn-deep-dive.md`, `LOCKS.toml` `[consent]`). **Found** for no encryption (`horbm/crates/horbm/src/learn.rs` `SessionSeal`).
2. Is there an HKB1 file on the laptop the retrieval thread should open? `RetrievalConfig.keys_path` is on main and defaults to `None`. `RetrievalConfig::from_env` and `CardEngine::open` read `GALION_HORBM_KEYS`. Empty or unset stays `asked_lexical`. **Found** (`horbm/crates/horbm/src/retrieve.rs` `from_env`, `voice-stack/horbm-bridge/src/cards.rs` `open`). Whether that file is on the laptop is still only Mate's.
3. `HorbmTurn` has no `end_session`. `HostSession::finish` calls `SessionSeal::seal` after the diary `end_session`, and it is not called from `Drop`. **Found** (`voice-stack/local-server/src/host.rs` `finish`). `horbm/crates/horbm-voice` does not name `SessionSeal`. **Found.**
