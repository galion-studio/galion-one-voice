# HORBM 100% plan B — memory, knowledge, safety

Perpetual Open MIT License. (c) 2026 Galion Labs.

Planning only. This file adds no product code. Agent B of the three-agent plan swarm. Base is `galion-one-voice` `main` `9fa188d551eef902428dd006b07f6c45480340b7` (merge of #389, 27 Nov 2026). I did not run the laptop gate, and I did not see a `Laptop gate PASS` comment.

The part names are the chips on the plate in [galion-knowledge `v2/src/horbm/interactive/horbm.json`](https://github.com/galion-studio/galion-knowledge/blob/master/v2/src/horbm/interactive/horbm.json), version 1.2.0, `updated` `2026-11-19T18:54:00+01:00`. That committed file still marks (56), (57), (61), (64), (71), (72), and (75) as Accepted. I did not read a copy stamped 27 Nov 2026 15:35. The layer notes in the assignment are the owner's later status. Where those notes and the committed JSON disagree, both are cited and the code I opened decides what is built.

`v2/src/horbm/interactive/progress.js` reads `layer.build.percent`. It does not recompute a percent from chip status. The committed plate's statuses are Built, Accepted, Recommended, and If needed. I listed the master tree of `galion-knowledge` and found no `sync-progress.mjs`. I don't know, from this repo, the rule that only a Dropped chip leaves the count.

A part below is **Drop? (owner decides)** only when current `LOCKS.toml` makes "Built" impossible without editing that file. This plan does not edit `LOCKS.toml`.

Open work this plan leaves alone: #386 (learn pack at boot, still open) and #390 (learned retraction queue, still open). Neither is one of these seven parts.

## (56) Relationship timeline — call summaries and milestones

**Keep.** Size M.

### 1. First principles

A conversationalist who remembers a person needs the calls, in order, and a few moments that mattered. The current call is already a ring. What is missing is a spoken list of past calls and of moments the person marked. That is needed now. A scored relationship graph, trust numbers, or a second database is not.

### 2. Reuse search

Checked on the repo, 27 Nov 2026:

| Repo | Licence on the repo | Fit |
|---|---|---|
| [yonk-labs/lede](https://github.com/yonk-labs/lede) | Apache-2.0. Root `LICENSE` is the Apache 2.0 text. `rust/Cargo.toml` says `license = "Apache-2.0"`, `rust-version = "1.85"`. | Extractive summarizer. Call summaries already exist (below). The companion crate `lede-enrich` is `Apache-2.0 AND CC-BY-SA-4.0` because its CRF was trained on Wikipedia text ([crates.io lede-enrich 0.2.1](https://crates.io/crates/lede-enrich)). Do not link that crate. |
| [alpaim/ashley](https://github.com/alpaim/ashley) | MIT. `LICENSE` copyright 2026 alpaim. | A whole experimental companion. It rewrites `MILESTONES.md` by itself. That fights "never rewrites the past". Do not vendor it. |
| [mpuchstein/hortus](https://github.com/mpuchstein/hortus) | MIT. `LICENSE` copyright 2026 Matthias Puchstein. | A file garden with a weekly diary and a compost directory. An application, not a diary library. |
| [Mathspy/diary-generator](https://github.com/Mathspy/diary-generator) | MIT OR Apache-2.0. Both `LICENSE-MIT` and `LICENSE-APACHE` are in the repo. | Builds a site from Notion. Not local. |
| [tomatyss/knotter](https://github.com/tomatyss/knotter) | Apache-2.0. Root `LICENSE` is the Apache 2.0 text. | A personal CRM binary. Too large for one list of calls. |
| [etherfunlab/eros-engine](https://github.com/etherfunlab/eros-engine) | AGPL-3.0-only, stated in the README. | Out. Wrong licence, and it is a companion engine. |

Nothing found that is a small Rust library for "list past call summaries and marked moments, and do not rewrite them." Search words: the hint `yonk-labs/lede`, plus "rust relationship timeline diary milestones" MIT / Apache-2.0.

### 3. What exists

Found on this tree:

- `horbm/crates/horbm/src/memory.rs` `EpisodicStore::end_session` calls `horbm_memory::summarise_session` and stores one row with source `session-summary`. A later summary hides the previous summary id in the RAM map (`hide`). The text of the old row is a diary question I did not re-read in `horbm-memory` for this path. I don't know, from this pass, whether the previous summary text stays in the archive.
- `summarise_session` in [galion-knowledge `v2/memory/src/extract.rs`](https://github.com/galion-studio/galion-knowledge/blob/master/v2/memory/src/extract.rs) builds one sentence: session id, dates, extracted facts, topic tokens, aside count. No model call. No milestone.
- `voice-stack/local-server/src/host.rs` `HostSession::finish` calls `end_diary_session`. That is the live host writing the summary when remember is granted.
- Sources in `memory.rs` are `pin`, `session-summary`, `state-tracker`, and `session-ring`. There is no milestone source.
- `list_person` returns live rows, newest `source_date` first, capped at `limit.min(MEMORY_HIT_CAP * 4)`. `docs/horbm/PLAN-brain.md` records that `Brain::turn` does not call `list_person`. I did not find a turn phrase that speaks a timeline.
- #387 (`horbm/crates/horbm-eval/tests/memory_lock.rs`, merged) locks forget, `list_person`, retract, card reload, and `learned:`. It does not lock a timeline.

`horbm-memory` is pinned at git rev `974e594444448597741372fef578142c46d28566` in `horbm/crates/horbm/Cargo.toml`. I read `extract.rs` and `session.rs` and `rank.rs` from `galion-knowledge` master, which may be ahead of that pin. The summary sentence above is master. I did not diff the pin.

### 4. Smallest build

Stay on the existing diary. No new crate.

- A milestone is a pin the person asked for (`milestone ...` / `please remember this moment ...`), source `milestone`, same grant as `remember`. The text is stored once. There is no edit command.
- One recall phrase ("what have we been through", "our timeline") speaks `session-summary` and `milestone` rows for that person, oldest date first, capped at the existing list cap. Other diary rows stay on ordinary recall.
- `Brain::turn` is the speaker. `list_person` stays the reader it already is; the new read is a filter of that list, not a second index.

### 5. Acceptance gate

New test, name it `relationship_timeline_lists_summaries_and_milestones_in_date_order`, under `horbm/crates/horbm-eval/tests/` or `horbm/crates/horbm/tests/`. Fixture person, two granted sessions with different dates, one milestone pin between them. The timeline reply contains the two summary sentences and the milestone text in date order. A world question does not speak them. Denied remember writes no row. A second call with the same milestone text does not insert another row.

That test is inside the standing laptop gate from `docs/horbm/PLAN-brain.md`: `cargo test --manifest-path horbm/Cargo.toml --workspace`, clippy `-D warnings`, `horbm-soak` at 128 KiB, rustdoc `-D warnings`, voice-stack `cargo metadata --locked`, `scripts/check-locks.sh`, `scripts/check-research-boundary.sh`. Built means a `Laptop gate PASS <sha>` comment on the build PR. I have not seen that comment.

### 6. Swarm

With (57). See the grouping at the end.

## (57) Fading — old rows leave live use, stay archived

**Keep.** Size S.

### 1. First principles

A long diary will drown the person who is here today. Old unpinned chatter has to leave search and stay on disk. Pins and milestones stay live. This is needed once (56) starts keeping years of summaries. It is a few dozen lines on the archive path that already exists.

### 2. Reuse search

The hint ruled out `fsrs-rs` (BSD-3-Clause). I did not open that repo again.

[mpuchstein/hortus](https://github.com/mpuchstein/hortus) (MIT, verified above) keeps composted notes in the garden. That is the same idea as this diary's archive. The binary is not a library to link.

Nothing else found that archives a SQLite row by age under MIT or Apache-2.0 and fits this schema. Search words: the hortus compost note, plus the existing `horbm-memory` archive. The archive function is already in the pinned crate (`Store::forget` copies into `archive`). Use that.

### 3. What exists

Found:

- Forget already archives. `memory.rs` module comment: the archive is the tombstone, and this module has no delete of that table. `forget` requires `reason == "forget"` or it returns `Failed`.
- [galion-knowledge `v2/docs/MEMORY_STACK.md`](https://github.com/galion-studio/galion-knowledge/blob/master/v2/docs/MEMORY_STACK.md) says, owner decision 18 Nov 2026: "Decay over time is not implemented." The same page documents a recency weight.
- [galion-knowledge `v2/memory/src/rank.rs`](https://github.com/galion-studio/galion-knowledge/blob/master/v2/memory/src/rank.rs): after fusion, memory hits are rescored `rrf × recency × importance × pin_boost`. Recency is `exp(-ln(2) / 30 × age_days)`. `RECENCY_HALF_LIFE_DAYS` is 30. Knowledge hits are not rescored. `query` does not write `last_access`. A row with a missing date keeps recency 1.0, so it never ages.
- #369 and #382 are the forget-archive work the layer note names. I confirmed the archive path in `memory.rs` and the tests `memory_forget.rs` and `memory_audit.rs`. I did not re-read those PR bodies in this pass.

Recency changes a score. The row stays in live search. That is not fading as this chip defines it ("old rows leave live use, stay archived").

### 4. Smallest build

No new crate. No spaced-repetition scheduler.

On `end_session`, after the summary is written, archive live rows for that person whose `source_date` is older than 180 days, whose source is `state-tracker` or `session-ring`, and which are not pinned. Reason `fade`. The text stays in `archive`. Recall and `list_person` already skip the archive. Pins, `session-summary`, and `milestone` are not aged out. A missing date is not aged out (the ranker already treats a missing date as fresh; fading it would hide undated pins of fact).

`PersonMemory::forget` stays reason `forget`. The fade path is a separate function so a fade tombstone is not rejected by the `reason == "forget"` check.

180 days is six half-lives of the existing 30-day curve (weight about 1/64). The constant lives next to the call, named, not learned.

### 5. Acceptance gate

Test `an_old_unpinned_chatter_row_leaves_live_recall_and_stays_in_the_archive`:

- A state-tracker row dated 200 days before the session clock: `recall` misses it; the archive reader returns the same text with reason `fade`.
- A pin and a milestone with that same old date: both still come back from `recall` / the timeline read.
- A state-tracker row dated yesterday: still live.
- Denied remember: no archive write.

Same standing laptop gate as (56). Built waits on `Laptop gate PASS <sha>`.

### 6. Swarm

Same PR as (56). The fade function has to know the `milestone` source, so (56)'s source constant lands first in that PR.

## (61) Core pack — about 1,500 now, target 50k to 200k

**Later.** Size L if it is ever staffed as content. No code swarm now.

### 1. First principles

The conversationalist already answers from the cards it has and says `I don't know.` on a miss (`horbm/crates/horbm/src/lib.rs`, and the honesty lock in #363). Fifty thousand authored cards do not make the turn more honest. They are a publishing programme. `horbm/crates/horbm/src/cards.rs` line 5 says card authoring is paused and this module only reads. #389 is the brain-plan rewrite that records that pause. Writing the pack now fights that pause.

Removing it from the count is the owner's Dropped mark, if that mark exists on the live page. I did not find that mark in the committed JSON. I am not recommending Drop. The target stays on the plate. It is not this swarm's build.

### 2. Reuse search

The index that can hold the target is already the in-house crate. [MEMORY_STACK.md](https://github.com/galion-studio/galion-knowledge/blob/master/v2/docs/MEMORY_STACK.md) says the stack is built for a core pack of about 50,000 to 200,000 cards, and it reports a release test on 18 Nov 2026: 200,000 generated cards, `Stack::query` p95 at most 10 ms, on an Intel Xeon, rustc 1.83. That paragraph is their measurement. I did not run `cargo test --release --test whole`. I don't know if that binary still meets the figure on the laptop.

No external corpus under MIT or Apache-2.0 was searched as a dump to import. A dump would still have to pass the truth gate, and card authoring is paused. Search words: the memory-stack page and the card tree. Nothing else to vendor.

### 3. What exists

- `KnowledgeCards::load` reads a directory, sorts by id, drops blocked cards, and checks a card-bundle manifest when one is present (`cards.rs`, as audited in `docs/horbm/PLAN-brain.md`). Test `load_reads_real_cards_and_does_not_write` is in the same file. I did not execute it.
- GitHub tree of `galion-knowledge` master, read 27 Nov 2026: 1,893 JSON files under `v2/data/cards/` (1,786 in `world/`, 107 in `horbm/`). I counted paths. I did not open each file, so I don't know how many are cards rather than an index file.
- The assignment says the live voice host health reports 1,523 cards. I did not see that health report.
- Truth gate PR [galion-knowledge #11](https://github.com/galion-studio/galion-knowledge/pull/11), merged 18 Nov 2026, counted 436 cards in its body. The tree is larger now. I did not re-run the gate.
- Gap export #378, card reload #379, and the Windows-only drive-letter root #373 are named by the layer note and by `PLAN-brain.md` (`RetrievalThread::reload`, `RetrievalConfig::from_flags`). I confirmed `tests/reload.rs` exists. I did not re-open those three PR diffs.

The retrieval machine for the target size is described as measured. The authored pack is still around two thousand files, under the 50,000 floor.

### 4. Smallest build

None in this repo while authoring is paused. The later build is cards in `galion-knowledge`, each one passing the truth gate, imported by the existing `horbm-memory import`. It is not a new index and not a model.

### 5. Acceptance gate

When the pause is lifted, and not before: the truth gate exits 0, and its report's card count is at least 50,000 and at most 200,000. Command already documented in the knowledge repo README:

`cargo run --manifest-path v2/tools/truth-gate/Cargo.toml -- --cards v2/data/cards --schema v2/data/schema.json --catalog v2/data/catalog.json --config v2/tools/truth-gate/truth-gate.toml`

I did not run it. A green GitHub check is not the laptop gate. The import of that pack on the laptop, then one `Stack::query` under 10 ms at the 99th percentile of a fixed query list, is the machine half. Their 18 Nov table is not that laptop run.

### 6. Swarm

No swarm. Do not staff agents to draft cards.

## (64) Source rules — no Wikipedia; one government, one media

**Later.** Size S in `galion-knowledge`, after the authoring pause, and only if a trial run shows the pack still passes. The Wikipedia half is already a hard failure. Do not rebuild it.

### 1. First principles

A spoken fact needs a source that is not an encyclopedia mirror, and a topic must not be one outlet's voice. That rule belongs on the card as it is published. The turn already refuses a card that is blocked and already says `I don't know.` Checking Wikipedia again inside `Brain::turn` would duplicate the publication gate. Part (63) on the plate is the truth gate and is already Built. Part (64) is the rule the gate encodes.

"One government, one media" as "every fact must cite both a government site and a news site" would throw out a paper that cites the paper. The gate that was merged does not say that. It warns when one registrable domain, or one government or IGO voice, dominates a topic. That reading matches the code. The other reading is a different rule, and I am not recommending it.

### 2. Reuse search

The check is already Galion's, MIT, in [galion-knowledge `v2/tools/truth-gate`](https://github.com/galion-studio/galion-knowledge/tree/master/v2/tools/truth-gate). `Cargo.toml` of that tool is in-repo; the knowledge repo licence is MIT (GitHub licence API on the #11 payload: SPDX MIT). I did not look for a second Wikipedia-host blocklist. Nothing to add.

### 3. What exists

Found in `v2/tools/truth-gate/src/check.rs` and `truth-gate.toml` on master:

- A source host on `banned_suffixes` or `banned_host_substrings` is `Severity::Hard`, code `banned_source`. The message says Wikipedia and its mirrors are not sources. The suffix list includes `wikipedia.org`, `wikimedia.org`, `wikidata.org`, and several mirrors. `wikisource.org` is classified `primary_text`, not banned. The toml comment says a DOI, a statute, a dataset, and a transcription are not Wikipedia.
- Mentioning the word Wikipedia in the card text is `Severity::Warn`, code `mentions_wikipedia`.
- `check_diversity`: one registrable domain at or above `outlet_share` 0.60, with at least `outlet_min_urls` 6, is `Severity::Warn`, code `outlet_dominates`. One voice tagged `government:` or `igo:` at or above `voice_share` 0.75, with at least `voice_min_urls` 8, is `Severity::Warn`, code `voice_dominates`. News hosts (`reuters.com`, `apnews.com`, `bbc.com`, and others) are class `news` with evidence ceiling `weak`. They are not a required second source.
- PR #11 body: hard failures exit 1; warnings stay in the report. Merged 18 Nov 2026. Its "after" column was 0 hard failures and 214 warnings on 436 cards. I did not re-run it on the 1,893 files.
- `horbm` `cards.rs` does not look at source hosts. A card that reached the directory is readable. The layer note is right that cards stay read-only on this side.

The hint that (64) may already be Built is half right. The Wikipedia ban is a hard failure in the gate. The dominance rule is a warning, and the plate still has (64) as Accepted while (63) is Built. I am not marking (64) Built.

### 4. Smallest build

In `galion-knowledge`, not in this repo, and not while changing a card is paused:

1. Run the truth gate. Read the report. I don't know the current hard-failure count.
2. If `outlet_dominates` and `voice_dominates` are the owner's "one government, one media", promote those two codes from `Warn` to `Hard`, with the same thresholds. If the report then exits 1, the failing topics wait. Do not rewrite card prose to force a green run, and do not add a news URL that the card did not already have.
3. Leave the Wikipedia hard failure as it is. Add a fixture test so a `wikipedia.org` URL cannot regress to a warning.
4. This repo stays a reader. No host check on the turn.

If step 2 fails the pack, (64) stays Later until the pause lifts and the topics are actually fixed. Promoting the warning on a red pack would be a false Built.

### 5. Acceptance gate

On the knowledge repo, rustc 1.85:

- `cargo test --manifest-path v2/tools/truth-gate/Cargo.toml` includes a fixture where the only source host is `wikipedia.org` and the finding is `Hard`.
- The same suite includes a fixture topic where one news domain is 6 of 6 URLs, and one where one `government:` voice is 8 of 8, and both findings are `Hard`.
- `cargo run` of the gate on `v2/data/cards` exits 0.

I have not seen those results. The one-voice laptop gate does not compile this tool. A Built mark for (64) also needs the owner's laptop (or the machine they use for the knowledge repo) to record the pass. I don't know their gate phrase for that repo.

### 6. Swarm

Not in the one-voice swarms below. One agent, in `galion-knowledge`, after the owner says card fixes may land. Two agents would both edit `check.rs`.

## (71) Harm limits — no harm methods, no romance under 21

**Keep.** Size S.

### 1. First principles

The turn speaks a stored clause. It does not invent a procedure. A stored clause can still be a method, and a romance line can still name someone under 21. The reply has to drop those. A profanity star-out is a different product: it censors ordinary swearing and it does not know a dose from a diary. Needed now, as a refusal in front of the clause, with the text of the refusal fixed in the persona file.

### 2. Reuse search

[finnbear/rustrict](https://github.com/finnbear/rustrict) is the hint. Verified: `LICENSE-MIT` is the MIT text, copyright 2021 Finn Bear. `Cargo.toml` on the README and the crates.io page is `MIT OR Apache-2.0` ([crates.io rustrict 0.7.39](https://crates.io/crates/rustrict)). It classifies profane, offensive, sexual, mean, and spam. The README says it does not understand context, and it is mostly English. A sexual tag would fire on adult swearing and would miss "how do I make a bomb" and "she is 16, talk to her like a girlfriend." Do not add the dependency.

Nothing found that is a small MIT or Apache-2.0 Rust library for "refuse a how-to of harm, and refuse romance when the line is about someone under 21." Search words: `rustrict`, and the persona boundary files already in this tree. The rule text is already written by the owner. The turn should use a short local table, credited as a list we write, not a vendored word list from a closed product.

### 3. What exists

Found:

- The boundary is prose in `docs/galion-voice-v2/personas/GALION-ONE.md`, `GALION-RESEARCH.md`, `JARVIS.md`, and `SAMANTHA.md`: no method, dose, target, or procedure for hurting someone; no sexual or romantic roleplay involving anyone under 21. Those files are voice-product copy. `Brain::turn` does not read them.
- `horbm/crates/horbm/src/dialogue/filter.rs` rejects a scripted comfort line and an unsourced fact. `Brain::admit_move` calls `Persona::verdict`. Boundaries on the development persona (`horbm/crates/horbm-persona/personas/samantha.toml`) are `fake-sympathy`, `invented-past`, and `unsourced-fact`.
- #363 locks the miss line, the card source, person-only memory, denied remember, and forget. It does not lock harm or age.
- I searched `docs/` for "alignment charter" and found no file. I don't know of a charter beyond the persona pages above.

The layer note is right: no merged test for harm limits showed up in `horbm-eval` or `horbm/tests` under those words.

### 4. Smallest build

No new crate. In `filter.rs`, before a clause is spoken:

- If the clause is a how-to (markers such as "how to", "steps to", "the dose", "mix the") and the object is harm (markers such as "kill", "poison", "explosive", "overdose"), replace the reply with one fixed refusal line and source `safety:harm`. The markers are a table in the file, fixture-tested, not a model.
- If the same clause is romantic or sexual (a short marker table) and the utterance or the clause contains an age under 21 ("16", "seventeen", "under 21", "minor", "child"), same treatment, source `safety:age`. Adult warmth with no under-21 marker is outside this part.
- The refusal line is a new `PersonaLine`, capped by the existing 80-byte line slot. Empty slot falls back to `UNKNOWN_REPLY`, the same pattern as `Forgot`.
- A card that is itself the method does not get spoken. The filter runs on the chosen clause, after retrieval.

This does not store the person's age. There is no age column today, and adding a profile of the user is a larger product.

### 5. Acceptance gate

Test `a_harm_method_and_an_under21_romance_line_are_not_spoken`:

- A fixture card whose clause is a short how-to of harm: the reply is the refusal line, source `safety:harm`, and the method text is absent.
- A fixture card that is a romance line, asked in a sentence that says the person is 16: source `safety:age`, romance text absent.
- The kettle card, asked as "where is the kettle", is still the kettle clause.
- "I love this song" with no age marker is not the age refusal.

Standing laptop gate as in (56). Built waits on `Laptop gate PASS <sha>`.

### 6. Swarm

With (72), after the memory PR, because both edit the reply path in `brain.rs`.

## (72) Says it is an AI — never pretends to be human

**Keep.** Size S.

### 1. First principles

Asked what it is, it has to say it is an AI. A miss, or a card about a person, would be a dodge or a costume. Needed on the first turn it is asked. A biography card in the knowledge pack is the wrong home: that pack is paused, and a self-description is not a world fact.

### 2. Reuse search

Nothing found. Search words: "rust ai identity disclosure" beside the persona loader we already have. One fixed line does not need a library. [alpaim/ashley](https://github.com/alpaim/ashley) keeps an identity file that the agent rewrites. That is the opposite of a fixed line.

### 3. What exists

Found:

- `horbm/crates/horbm-persona/personas/samantha.toml` comment: "Not a recording. Not a measured person. No clip and no donor." That comment is not a spoken line. `[lines]` has `forgot` and `nothing_remembered` only.
- `development.toml` sets `development_persona = "samantha"`. Boot loads that id (`docs/horbm/PLAN-brain.md`). The id is a name. I found no turn that answers "are you human" with a fixed sentence. A covering card, if one existed in the loaded pack, could be spoken. I did not search the 1,893 JSON files for an identity card.
- `never_fabricate = true` and `unknown = "I don't know."` stop invented facts. They do not answer an identity question.
- #363 locks boot persona id and the unknown line. It does not ask "are you a person".

### 4. Smallest build

Before retrieval, if the utterance is an identity question ("are you human", "are you a person", "are you a real girl", "what are you", "are you an ai"), speak one persona line: it is an AI, and it is not a human. Source `persona:identity`. Do not speak a card on that turn. Do not rename the persona id in this part. The name can stay; the sentence is the claim.

The line has to fit `LINE_BYTES` (80), already enforced by the persona loader.

### 5. Acceptance gate

Test `an_identity_question_says_it_is_an_ai`:

- `turn("are you human")` reply contains "AI" and does not contain a claim of being human. Source `persona:identity`.
- The same turn with a fixture card whose tokens would cover "are you human": the card clause is not the reply.
- `turn("where is the kettle")` is still the kettle clause.

Standing laptop gate. Built waits on `Laptop gate PASS <sha>`.

### 6. Swarm

Same PR as (71). Identity runs first, so a card cannot answer the question. The harm filter runs on every other clause.

## (75) No voice clones — no real third-party voice

**Drop? (owner decides).** Size S if the owner changes the lock. Under the lock as it stands, a total ban cannot become Built.

### 1. First principles

The CPU conversationalist has to speak with a voice this project is allowed to ship. A clone of a real person is a different product, and the owner has already pinned that product in `LOCKS.toml`. I cannot make (75) Built as "no code path may speak a real third party" without editing that pin. This plan does not edit it.

### 2. Reuse search

Nothing to vendor. The choice is already in `voice-stack/local-server/src/mouth.rs`. Search words: that file and `[formats.exceptions.v2s14_chatterbox]` in `LOCKS.toml`. No external clone-blocker under MIT or Apache-2.0 would override an owner pin, so I stopped there.

### 3. What exists

Found:

- `LOCKS.toml` `[formats.exceptions.a21_piper]` pins `weights/tts/en_US-ljspeech-medium.onnx`, CPU, separate process. Decision `t123u`, widened `t613s2992`, owner Maciej Grajczyk. Sunset `2027-02-01`.
- `LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` is "V2-S14 clone TTS only". `reference_clips = true`. `ref_clip_scope` names two LibriVox donors, Amy Koenig (samantha) and Alan Davis Drake (jarvis), CC0 / public domain, stored outside the tree. "No celebrity voice, no film audio, no Armstrong." Decision `t230u`, same owner, same sunset.
- `voice-stack/local-server/src/mouth.rs`: CPU does not select the s711 clone. The log line says it keeps Piper `en_US-ljspeech-medium`. s711 is selected when `GALION_VOICE_DEVICE=cuda`, a local reference, and a local checkpoint are all present. Tests in that file: `cpu must not select s711`, and `cuda_with_local_files_selects_s711_without_a_realtime_claim`.
- `voice-stack/local-server/src/lib.rs` says the s711 clone is selected only with the local reference, the local checkpoint, and an explicit cuda opt-in.
- #385 (`scripts/check-licences.sh`, merged) checks third-party crate SPDX expressions. It does not look at voice donors.
- The brain plan says other swarms leave `voice-stack/local-server` alone, and it does not retune Piper.

I did not run those mouth tests. The CPU refusal of s711 is what the test is written to assert. The cuda path is written to select s711. Both donors are real people. The Piper pack is a published single-speaker model pinned by name of the file. I don't know whether the owner counts that Piper pack as a "real third-party voice." The chip's words are "no voice clones" and "no real third-party voice." The lock allows two named donor clones until the sunset.

### 4. Smallest build

No code while the pin stands. The CPU mouth already stays on the pinned Piper pack.

If the owner withdraws `t230u` and says HORBM never selects s711 on any device, the build is: `decide` in `mouth.rs` returns the Piper choice even when cuda and the files are present, the cuda test is rewritten to expect Piper, and the lock change is a separate owner commit. That lock change is not this plan's PR. I will not draft it.

If the owner instead says the chip means "CPU HORBM does not select s711, and the clone lane stays for the voice product," the part can be marked Built from the existing test, with no new code. That is an owner status mark. I have not marked it.

### 5. Acceptance gate

There is no new test that can pass under the current lock and also forbid every real third-party voice.

The existing test name to re-run, if the owner keeps the lock and only wants the CPU fact confirmed: the test in `voice-stack/local-server/src/mouth.rs` whose assertion is that CPU does not select s711. I did not run it. A pass there does not make (75) Built on the stronger reading.

### 6. Swarm

No swarm until the owner answers the Drop? line.

## Swarm grouping

Two build PRs, in this order. Two agents each. They share `brain.rs`, so the second PR starts after the first has merged. Neither PR edits `LOCKS.toml`, `SOUL.md`, `config.yaml`, `REFS.sha256`, `LICENSE`, or `voice-stack/local-server`. Neither PR implements #386 or #390.

| Order | PR | Agents | Parts | Dependency |
|---|---|---|---|---|
| 1 | Diary timeline and fade | 2 | (56), then (57) in the same PR | (57) uses the `milestone` source from (56). Agent A: milestone source, timeline read, the timeline test. Agent B: fade archive, the fade test. Agent B waits on the source constant. |
| 2 | Spoken safety lines | 2 | (72), then (71) | Identity return happens before retrieval. Harm filter runs on the clause that remains. Agent A: persona line and the identity test. Agent B: the marker table, the refusal line, the harm test. |
| — | Knowledge, later | 1, other repo | (64) | After the owner lifts the card pause, and after a truth-gate run shows whether a hard diversity check is green. |
| — | No PR | — | (61) Later, (75) Drop? | Owner status only. |

## Table

| Part | Keep / drop / later | Reuse pick | Size | Swarm |
|---|---|---|---|---|
| (56) Relationship timeline | Keep | None. `lede` is Apache-2.0 and rustc 1.85, and it only summarizes; summaries already exist. `lede-enrich` is out (CC-BY-SA-4.0 Wikipedia model). | M | PR 1, agent A |
| (57) Fading | Keep | None. Use `Store::forget`'s archive. `fsrs-rs` stays out (BSD-3). | S | PR 1, agent B, after the milestone source |
| (61) Core pack 50k–200k | Later | None. The in-house index is the one the memory stack already measured at 200k generated cards. I did not re-run that test. | L as content | No swarm while authoring is paused |
| (64) Source rules | Later | In-house truth gate, MIT, already merged as galion-knowledge #11. Wikipedia is already a hard failure. | S | One agent in `galion-knowledge`, after the pause |
| (71) Harm limits | Keep | None. `rustrict` is MIT OR Apache-2.0 and filters profanity, which is a different rule. | S | PR 2, agent B |
| (72) Says it is an AI | Keep | None. One persona line. | S | PR 2, agent A, before the harm filter |
| (75) No voice clones | Drop? (owner decides) | None. `LOCKS.toml` `t230u` allows two named LibriVox clones until 2027-02-01, and `t123u` pins the Piper pack. | S only if the lock changes | No swarm |

## Sources

- [galion-knowledge horbm.json](https://github.com/galion-studio/galion-knowledge/blob/master/v2/src/horbm/interactive/horbm.json) (file `updated` 19 Nov 2026)
- [galion-knowledge progress.js](https://github.com/galion-studio/galion-knowledge/blob/master/v2/src/horbm/interactive/progress.js)
- [galion-knowledge MEMORY_STACK.md](https://github.com/galion-studio/galion-knowledge/blob/master/v2/docs/MEMORY_STACK.md) (measurement dated 18 Nov 2026 in that file)
- [galion-knowledge truth-gate check.rs](https://github.com/galion-studio/galion-knowledge/blob/master/v2/tools/truth-gate/src/check.rs) and [truth-gate.toml](https://github.com/galion-studio/galion-knowledge/blob/master/v2/tools/truth-gate/truth-gate.toml)
- [galion-knowledge PR 11](https://github.com/galion-studio/galion-knowledge/pull/11) (merged 18 Nov 2026)
- [galion-knowledge extract.rs](https://github.com/galion-studio/galion-knowledge/blob/master/v2/memory/src/extract.rs), [rank.rs](https://github.com/galion-studio/galion-knowledge/blob/master/v2/memory/src/rank.rs)
- [yonk-labs/lede](https://github.com/yonk-labs/lede) `LICENSE` and `rust/Cargo.toml` (Apache-2.0, rust-version 1.85)
- [lede-enrich 0.2.1](https://crates.io/crates/lede-enrich) (Apache-2.0 AND CC-BY-SA-4.0)
- [alpaim/ashley LICENSE](https://github.com/alpaim/ashley/blob/main/LICENSE) (MIT)
- [mpuchstein/hortus LICENSE](https://github.com/mpuchstein/hortus/blob/main/LICENSE) (MIT)
- [Mathspy/diary-generator](https://github.com/Mathspy/diary-generator) `LICENSE-MIT` and `LICENSE-APACHE`
- [tomatyss/knotter LICENSE](https://github.com/tomatyss/knotter/blob/main/LICENSE) (Apache-2.0)
- [etherfunlab/eros-engine](https://github.com/etherfunlab/eros-engine) (AGPL-3.0-only, from the README)
- [finnbear/rustrict LICENSE-MIT](https://github.com/finnbear/rustrict/blob/main/LICENSE-MIT) and [crates.io rustrict](https://crates.io/crates/rustrict)
- This tree: `docs/horbm/PLAN-brain.md`, `horbm/crates/horbm/src/memory.rs`, `cards.rs`, `dialogue/filter.rs`, `brain.rs`, `horbm/crates/horbm-persona/personas/samantha.toml`, `development.toml`, `voice-stack/local-server/src/mouth.rs`, `host.rs`, `LOCKS.toml`, `horbm/crates/horbm-eval/tests/memory_lock.rs`
- Pull requests on this repo, read from GitHub: [#363](https://github.com/galion-studio/galion-one-voice/pull/363) merged, [#385](https://github.com/galion-studio/galion-one-voice/pull/385) merged, [#387](https://github.com/galion-studio/galion-one-voice/pull/387) merged, [#389](https://github.com/galion-studio/galion-one-voice/pull/389) merged, [#386](https://github.com/galion-studio/galion-one-voice/pull/386) open, [#390](https://github.com/galion-studio/galion-one-voice/pull/390) open
