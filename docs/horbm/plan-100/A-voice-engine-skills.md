# HORBM 100% plan A — voice, engine, skills

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Agent | A of the 3-agent planning swarm |
| Base | `main` `9fa188d551eef902428dd006b07f6c45480340b7` (merge of #389) |
| Kind | Plan only. This file does not change a crate, a manifest, a lock, or a gate. |
| Checked | 27 Nov 2026. Licence lines below were read from the named repositories that day. |
| Rust | `horbm/Cargo.toml` sets `rust-version = "1.85"`. |

Claim marks: **Found** means this pass read the file or the repository text it names. **I don't know** means this pass did not see it. No laptop gate was run. No test binary was run. A merged pull request is not a measurement.

Eleven parts are in this share. Four can become Built with a small Rust change and a laptop-gate test: (5), (35) without the S0 clause, (50), (51). Three cannot become Built while the current locks and the weight licences stand, and are marked **Drop? (owner decides)**: (12), (44), (49). Four are real work and are not required for a great CPU conversationalist now: (41), (42), (47), (48). Those four stay **Later**. They remain in the count until they are built or the owner marks them Dropped.

A GitHub code search for `sync-progress.mjs` under `galion-studio` returned no file. **I don't know** the counter's source. This plan still uses the owner's rule for the count: only a Dropped chip leaves the denominator. Accepted, Recommended, and If needed stay in the count. Later in this file is a recommendation, not a Dropped chip.

Shared files this plan does not assign to a build swarm: `LOCKS.toml`, `SOUL.md`, `config.yaml`, `REFS.sha256`, `LICENSE`, `docs/horbm/PLAN-brain.md`, `docs/horbm/README.md`.

## (5) Channels: voice first, then text, then image

**Recommendation: Keep.** Size S. Swarm A2.

### 1. First principles

A conversationalist that hears while it speaks needs one brain for the microphone and for typed text, with the microphone ahead of typing. A photograph is a third input. Commenting on it is part (48). The channel rule needed now is smaller: an image frame must not become a turn and must not pass the voice frame that is already in flight.

### 2. Reuse search

Searched this repository for a channel scheduler, and GitHub for a small MIT or Apache-2.0 Rust crate whose job is "voice, then text, then image" on one duplex socket. **Nothing found** that fits. The socket, the text frame, and the PCM frame are this tree's own protocol. A generic bus would be a second scheduler beside the session loop.

### 3. What exists already

**Found.**

- Text is a client frame. `ClientControl::Text` is parsed in `voice-stack/local-server/src/protocol.rs`. The same enum has hello, ping, stop, consent, and unknown. It has no image arm.
- PCM is a binary websocket frame. `voice-stack/local-server/src/server.rs` `handle_text` and `handle_binary` run on the session task. The session awaits each one, so one socket does not run a text turn and a mic take at the same time.
- #383 (merged 26 Nov 2026, "Keep a text turn and the session seal off the tokio workers") moves `push_text` onto `spawn_blocking`. The test is `voice-stack/local-server/tests/turn_off_worker.rs`.
- #391 (merged 26 Nov 2026, "Run the closed mic take off the tokio worker") leaves an open mic frame on the session task and moves the closed take onto `spawn_blocking`. The test is `voice-stack/local-server/tests/mic_off_worker.rs`.
- Both paths enter the same mind. `voice-stack/local-server/src/brain.rs` says a turn is `CardEngine::turn_with_ear`. Typed text passes no listener point. A live mic utterance passes that utterance's point.
- The favicon response in `server.rs` is `image/x-icon`. That is a static file response, not a conversation channel.

**I don't know** whether a real microphone and speaker were exercised together. This pass did not open a socket.

### 4. Smallest build left

One protocol arm, `image`, that does not call the brain and does not enqueue speech. While a closed mic take is in `finish_take`, a text frame on that socket stays behind that join, which the session loop already does. The new test locks the image arm. No vision crate. No weight. Part (48) remains the caption.

### 5. Acceptance gate

On the laptop, rustc 1.85:

```text
cargo test --manifest-path voice-stack/local-server/Cargo.toml --test turn_off_worker --test mic_off_worker
```

plus one new test in that package: a JSON frame whose type is `image` produces no brain call and no spoken frame, and a text frame sent while a mic take is held is delivered only after that take's join. Existing `turn_off_worker` and `mic_off_worker` still pass. This pass did not run them.

### 6. Swarm

Swarm A2, with (50). No dependency on A1. The image arm does not wait for a captioner.

## (12) End-goal voice: our own RL Moshi / PersonaPlex, dual band

**Recommendation: Drop? (owner decides).** This part cannot become Built under the current lock and the weight licences. It stays in the count until the owner drops it or changes the lock.

### 1. First principles

The live conversationalist is a cascade: ear, retrieval, mouth, on an 80 ms frame. `LOCKS.toml` `[horbm]` records owner decision 2026-11-24, Q1 option A: `hot_path = "retrieval"` and `student_on_tick = false`. A student decode on the chat turn is the thing that lock closes. An RL speech-to-speech model is a student on that turn. Training one also needs weights this licence rule will not take.

The duplex clock is already two speech bands plus a feeling band (`LOCKS.toml` `[clock]` `frame_ms = 80`, `[bands]`, `barge_in_max_ms = 200`). That clock is a different part from an RL Moshi.

### 2. Reuse search

**Found**, and then ruled out for this part.

- [kyutai-labs/moshi](https://github.com/kyutai-labs/moshi). The README in that repository says the Python code is MIT, the Rust backend is Apache-2.0, and the model weights are CC-BY 4.0. `LICENSE-MIT` and `LICENSE-APACHE` are in the repository root. CC-BY 4.0 weights are outside the MIT / Apache-2.0 rule. The code could be read. It does not lift `student_on_tick = false`.
- [NVIDIA/personaplex](https://github.com/NVIDIA/personaplex). `LICENSE-MIT` is the code licence (GitHub reports SPDX MIT; the file starts with the MIT grant). The README in that repository says the weights are under the NVIDIA Open Model license and points at the Hugging Face card `nvidia/personaplex-7b-v1` for acceptance. Those weights are outside the MIT / Apache-2.0 rule. The README also says the model is a Moshi finetune.

`crates/galion-mimi` already records the same weight block: `IS_STUB = true`, and the crate comment says no weights may be added until a licence gate clears them. The Mimi numbers in that crate are marked as coming from Kyutai's published description, and the comment names the CC-BY weights.

Searched for a MIT or Apache-2.0 full-duplex speech model that runs a conversational turn on CPU inside the 80 ms frame with rustc 1.85. **Nothing found** that is both licensed for reuse and allowed on the turn by `[horbm]`.

### 3. What exists already

**Found.**

- `horbm/crates/horbm-duplex` steps one 80 ms frame for both speech bands. `docs/horbm/PLAN-brain.md` records that `Duplex::tick` does not pass an ear point into `Brain::turn`.
- `crates/galion-mimi/src/lib.rs` `IS_STUB = true`. Kernels exist. Weights do not.
- `docs/galion-voice-v2/PERSONAPLEX-REANCHOR.md` is a plan. It says no weights and no training run. It recommends Piper keep making the sound (decision D1 = A) and treats PersonaPlex and Moshi as architecture references.
- The live mouth on the path this plan read is Piper, behind the local server. This plan does not retune it.

**I don't know** a measured CPU step time for a trained S0 temporal transformer. This pass did not run one. The re-anchor's step-rate figures are marked there as arithmetic, not as a measurement this pass repeated.

### 4. Smallest build left

None, while `student_on_tick = false` and the weight licences stand. A build that loads Moshi or PersonaPlex weights, or that runs a generative student on the turn, would break the lock or the licence rule. This plan assigns no swarm and no crate.

If the owner wants the end goal later, the lock has to change first, in a separate decision, and the weights have to be our own. That is outside this file.

### 5. Acceptance gate

There is no honest Built gate under the current lock. A laptop run of the existing duplex and soak tests would prove the cascade, which is a different part. It would not prove an RL Moshi.

### 6. Swarm

None. Do not pair it with (5) or with the router.

## (35) Reasoning router: HORBM first, S0 guesses, Research 3 s

**Recommendation: Keep the router. Drop? (owner decides) the words "S0 guesses".** Size S for the router gap. Swarm A1.

The title as a whole cannot become Built while those two words remain, because an S0 guess is a student on the turn.

### 1. First principles

The turn is a lookup. Cards and the person diary speak first. A miss is `I don't know.` A deep miss may ask Galion Research after that local clause is already returned, with a 3 second cap. A guess from a generative student on the same turn would speak a line the cards did not source. `student_on_tick = false` already closes that. The S0 clause in the title fights the lock. The rest of the title is the router the tree is already built around.

### 2. Reuse search

The router, the cap, and the loopback gateway are already in this tree (section 3). Searched for a second MIT or Apache-2.0 "reasoner" to put in front of the cards. **Nothing found** that should be added. An outside LLM client would be a student on the turn.

### 3. What exists already

**Found.**

- `horbm/crates/horbm/src/route.rs` `route_heard`: card lookup, then a deep question, then `UNKNOWN_REPLY` (`I don't know.`). `is_deep_question` is a question mark plus eight words, or the word why, how, or explain. The research port is called with `RESEARCH_CAP`.
- `voice-stack/research-gateway/src/lib.rs` sets `RESEARCH_CAP` to 3 seconds. The module comment says the only outbound call is Hermes on loopback, with no paid API and no key in the process.
- `Brain::gather` in `horbm/crates/horbm/src/brain.rs` calls `route_heard` with the research port set to `None`. `research_later` is true only when a port exists, the lean is not comfort, and the gap reason is `ResearchOff`. The comment there says a deep miss is armed after the clause. Comfort does not open research.
- `horbm/crates/horbm/tests/persona_inner_turns.rs` `a_plain_deep_question_asks_research_after_the_clause` asserts the spoken reply is `I don't know.` and the gateway count is 0 until the turn value is dropped, then 1, and the cap equals `RESEARCH_CAP`. `a_sad_line_comforts_without_a_script_or_a_lookup` asserts a sad line does not call the gateway.
- #239 (merged 21 Nov 2026) is titled "HORBM layer 5 router and localhost research gateway". #313 (merged 25 Nov 2026) is titled "Speak the local clause before Galion Research". #341 (merged 25 Nov 2026) is titled "Speak the bridge router's in-call reply through Brain::turn". This pass read the titles and the current source. It did not re-read those diffs.
- `ReasoningRouter::turn` in `voice-stack/horbm-bridge/src/router.rs` calls `Brain::turn` and drops the turn value. The module comment says the in-call reply is the local clause and the gateway runs after that value is dropped.
- `ReasoningRouter::route` still calls `route_heard` with the research port. The comment on `route` says so. That path can ask inside the call. The live host does not use it. `voice-stack/local-server/src/brain.rs` uses `CardEngine::turn_with_ear`.
- The same router file says the order has no S0, no LLM, and no Ollama. **Found** in the module comment.

A search of `voice-stack/local-server/src` for `GALION_RESEARCH` returned no match. The live process's environment is **I don't know**. This pass did not inspect a running server. The source of the local server does not name that variable.

### 4. Smallest build left

Change `ReasoningRouter::route` so it returns the same local clause `Brain::turn` returns, and so the gateway count stays 0 until that reply is dropped. Do not add an S0 call. Do not change `RESEARCH_CAP`. Do not point the live host at `route`.

The S0 words stay out of the build. They become Built only if the owner changes `[horbm]`, which this plan does not do.

### 5. Acceptance gate

On the laptop, rustc 1.85, a bridge test that does three things:

- a card hit is spoken and the gateway count stays 0;
- a deep miss speaks `I don't know.` and the gateway count is 0 before the reply is dropped, then 1, with the cap equal to 3 seconds;
- `ReasoningRouter::route` and `ReasoningRouter::turn` agree on that order.

Then the existing `horbm` tests `persona_inner_turns` and `clause_first`. This pass did not run them.

### 6. Swarm

Swarm A1, with (51). A1 lands before any swarm that adds a tool behind the router. The S0 clause has no swarm.

## (41) Ranker: boosted tree, then a threshold

**Recommendation: Later.** Size L if it is ever built. No swarm in this round. It can be built without breaking `[horbm]`, and it is not needed for the conversationalist now. It stays in the count until it is built or the owner drops it.

### 1. First principles

The hot path already decides. A truth card is spoken when every content token of the ask is in that card and one card is the unique smallest haystack. A tie is not a guess (`horbm/crates/horbm/src/cards.rs`). The diary scan dots int8 rows, drops a non-positive diary dot, and keeps at most `SCAN_CAP` (4) hits (`horbm/crates/horbm-brain/src/scan.rs`, `DiaryArena::lift` returns `None` when `dot <= 0`). A boosted tree needs a labeled set this repository does not contain. Training one on the turn, with no labels, would be a score that looks like a rank and is not one. The tree also does not belong on the 80 ms tick.

### 2. Reuse search

**Found**, for the day the owner pulls this forward. Not for this round.

- [mesalock-linux/gbdt-rs](https://github.com/mesalock-linux/gbdt-rs). `LICENSE` is the Apache License 2.0. `Cargo.toml` says `license = "Apache-2.0"`, crate `gbdt` 0.1.3, edition 2018. Authors named there: Tongxin Li, Tianyi Li, Yu Ding. Credit them if this crate is ever linked. It is a gradient boosting regression tree, which is the shape the title asks for. This pass did not compile it on rustc 1.85.
- [perpetual-ml/perpetual](https://github.com/perpetual-ml/perpetual). `LICENSE` is Apache-2.0 and `Cargo.toml` says `license = "Apache-2.0"`. The `Cargo.toml` this pass read also says `edition = "2024"`. The crate is a large boosting stack (Python and R bindings, calibration, drift). It is a worse fit than `gbdt` for a single offline threshold. This pass did not compile it.

Bubblewrap, birdcage, and the other ruled-out crates in the hint are sandbox or memory tools. They do not rank cards.

### 3. What exists already

**Found.** The covering lookup and `DiaryScan`, as cited above. `Int8Scorer::lift` can drop a row. The stored-key lift returns a dequantised dot and does not apply a second cutoff. There is no boosted tree, no feature matrix, and no rank threshold constant in `horbm`.

**I don't know** of a labeled hit/miss file in this repository. This pass did not find one.

### 4. Smallest build left

When the owner pulls it forward, and only then: an offline `gbdt` model, trained outside the tick, that scores candidates the covering lookup already returned. A threshold drops the tail. A miss below the threshold stays `I don't know.` The 80 ms frame does not train and does not load the tree. Until a labeled set exists, there is nothing honest to train.

### 5. Acceptance gate

Not scheduled. The future gate is a fixed labeled file, a rustc 1.85 test that the top score above the threshold matches the labeled winner, and a score under the threshold speaks `I don't know.` The tick budget in `LOCKS.toml` stays untouched. This pass has no such file and no such measurement.

### 6. Swarm

None in this round. If the owner pulls it forward, it is its own PR, one agent, after A1. It does not share files with the router fix.

## (42) Reply builder: fills slots from sourced spans

**Recommendation: Later.** Size S if the owner pulls it forward. No swarm in this round.

### 1. First principles

The spoken line is one sourced span: the card gist, the memory line, or `I don't know.` Filling several slots from several spans builds a sentence no single source wrote. That is a new claim. The conversationalist does not need that claim in order to answer from cards. A template engine would add a crate for a format the cards do not have.

### 2. Reuse search

**Found**, and not recommended.

- [mitsuhiko/minijinja](https://github.com/mitsuhiko/minijinja), crate `minijinja`. `Cargo.toml` says `license = "Apache-2.0"`, `edition = "2021"`, `rust-version = "1.70"`. It compiles on the 1.85 pin as far as that field goes. This pass did not compile it. A slot filler with a handful of named fields is `write!` in `std`. The dependency is unused weight.

Searched `horbm/crates/horbm/src` for a reply slot. The slots that exist are the dialogue ring (`dialogue/moves.rs`) and the learn seal (`learn.rs`). They are not answer templates.

### 3. What exists already

**Found.**

- `voice-stack/horbm-bridge/src/chunks.rs` `speakable_chunks` splits a finished line into sentences, strips a leading bracket tag, and drops a forbidden phrase. The bridge README calls this the reply builder. It does not fill a slot. It does not read a span.
- `Brain::finish` keeps a picked clause and caps an unpicked reply with `StyleHook::cap_sentences` (`docs/horbm/PLAN-brain.md`, and `horbm/crates/horbm/src/brain.rs` as that audit cites). The words stay the words the lookup returned.
- `route_heard` speaks `hit.gist` when `speakable` accepts it, and records `card:<id>`.

### 4. Smallest build left

When the owner pulls it forward: one function that copies a single sourced span into a single slot with `write!`, returns nothing when the span or the source is empty, and adds no word of its own. A card with no slot keeps today's gist path. No minijinja. No multi-span sentence.

### 5. Acceptance gate

Not scheduled. The future test feeds one span with a source and expects that text, then feeds an empty source and expects no speech. `speakable_chunks` stays as the mouth splitter and is not renamed into this part.

### 6. Swarm

None until the owner says Keep. If they do, one agent, its own PR, after A1, touching a new module under `horbm/crates/horbm` and a test. It does not edit `chunks.rs`.

## (44) Eureka Moment Pipelines: prepare, incubate, illuminate, verify

**Recommendation: Drop? (owner decides).** The name has no input, no output, and no measure. It is not a lock conflict. It cannot be called Built without inventing a second brain. It stays in the count until the owner drops it or writes a spec this plan does not invent.

### 1. First principles

A retrieval turn is lookup, then one legal move, then the clause. An insight pipeline with four stages would sit beside that turn and produce a line the cards did not store. The conversationalist does not need that to be good at conversation. The stages, as a name, do not say what is stored between them, what may be spoken, or what a failure looks like.

### 2. Reuse search

GitHub repository search for `eureka incubate rust` returned no repository. A search of this tree for `incubate` and `illuminate` in Rust and Markdown returned no pipeline. **Nothing found.**

The word Eureka is already used here for a different artifact. `docs/galion-voice-v2/symphony/EUREKA.md` is the 80 ms heartbeat standard: one clock, three bands, harness before weights. It does not define prepare, incubate, illuminate, and verify as a reply pipeline.

### 3. What exists already

**Found.** `EUREKA.md`, the heartbeat crate it points at, and the self-eval notes. **Found** no Rust type for those four stages. `docs/swarm-10/2026-11-15/brain-eureka/brain.md` exists. This pass did not read it as a spec for this part. **I don't know** whether an older note used the same four words. The current `horbm` crates do not implement them.

### 4. Smallest build left

None. A stub that logs the four names would be a false Built. This plan will not invent the stage contracts.

### 5. Acceptance gate

None, until the owner replaces the title with a behavior a test can fail. A passing heartbeat test does not prove this part.

### 6. Swarm

None.

## (47) Agentic sandboxes: every tool runs isolated

**Recommendation: Later.** Size M. Swarm A3, and only after a voice tool actually starts a process. Stays in the count.

### 1. First principles

A tool that can read the disk or open a socket has to run where a mistake cannot see the diary or the card directory. The voice turn today does not start such a tool. The research path is a loopback lookup with a 3 second cap, already refused off loopback by the gateway. Building a sandbox with no caller adds a Linux-only crate and does not change a conversation. The sandbox should land in the same change as the first process tool, not before it.

### 2. Reuse search

**Found.**

- [galion-studio/obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind) `crates/openmind-sandbox`. `LICENSE` is the MIT license, copyright Galion / galion-studio. `Cargo.toml` describes it as one command under rlimits, a cleared environment, a fixed work directory, a wall timeout, and Landlock when the kernel has it. The crate comment says Linux and Android only, libc only, no seccomp, no root, and that a layer is reported only when it was applied. Workspace `edition` on that repository's root `Cargo.toml` is 2021. This pass did not find a `rust-version` key there and did not compile the crate on rustc 1.85. **I don't know** whether the current sources build unchanged on 1.85.
- [containers/bubblewrap](https://github.com/containers/bubblewrap). `COPYING` is the GNU Lesser General Public License, version 2.1. Ruled out.
- [phylum-dev/birdcage](https://github.com/phylum-dev/birdcage). GitHub reports SPDX GPL-3.0. Ruled out.

`openmind-sandbox` is the reuse pick when A3 starts. Credit Galion / galion-studio. Do not reimplement Landlock beside it.

### 3. What exists already

**Found** the gateway's loopback bind rule (`voice-stack/research-gateway/src/lib.rs` refuses an address that is not loopback v4). **Found** no call to `openmind-sandbox` in this repository. A search of `crates/galion-openmind` for a sandbox module returned no Rust match. The conductor research client returns without a socket (`crates/galion-conductor/src/research.rs`).

**I don't know** the laptop's kernel, or whether Landlock is available there. The crate's own rule is to report the layers it enforced and to claim nothing else.

### 4. Smallest build left

When the first process tool exists: spawn it only through `openmind-sandbox`, with network off unless the tool is the research lookup, a work directory that is not the diary and not the card directory, and a wall time inside the 3 second research cap. If the crate does not build on rustc 1.85, fix that in the sandbox crate's own repository first. Do not copy it in and drift.

### 5. Acceptance gate

Not scheduled. The future laptop gate is the crate's own escape test, run on Linux: a child asked to read a file outside the work directory fails, and the test prints the layers `Outcome::enforced` actually set. A Windows-only run would not prove Landlock. **I don't know** whether the laptop gate is native Windows or Linux. `docs/horbm/PLAN-brain.md` runs the brain commands on Linux. That is the gate this part would use.

### 6. Swarm

Swarm A3, one PR, two agents: one wires the dependency, one writes the escape test against a fixture tool. A3 starts after A1 and after the owner says a process tool is in scope. (51)'s money refusal does not wait for A3.

## (48) Image talk: comments on a photo

**Recommendation: Later.** Size L. No swarm. The channel refusal is part (5), which does not need this.

### 1. First principles

Commenting on a photograph is a vision model. The conversationalist people are talking to is a voice lookup. A caption that the model wrote is a generated line, and `[horbm]` plus `never_fabricate` leave no room to speak it as a fact. A sourced caption ("the file's own text", or a card) is retrieval, which already exists. A free comment on pixels is a new product and a weight download. It is not how this brain becomes good at conversation now.

### 2. Reuse search

**Found** a runtime, and no weight this pass can accept.

- `ort` is already pinned in this tree: `voice-stack/vad/Cargo.toml` and `runtimes/galion-piper-rt/Cargo.toml` use `ort` `=2.0.0-rc.15`. [pykeio/ort](https://github.com/pykeio/ort) reports SPDX Apache-2.0. Those pins serve VAD and Piper. They do not caption a photo.
- [huggingface/candle](https://github.com/huggingface/candle) reports SPDX Apache-2.0. This pass did not open that repository's `LICENSE` body and did not read a `rust-version`. Adding candle beside a pinned `ort` is a second runtime. Prefer `ort` if a weight ever qualifies.

Searched this repository for a caption model or a photo turn. **Nothing found.** This pass did not open a model card and so will not name a weight as MIT or Apache-2.0. **Nothing found** for a caption weight whose licence was verified here.

### 3. What exists already

**Found.** No image arm in `ClientControl` (part 5). `ort` linked for speech and VAD, as above. Persona honesty rejects a file with `never_fabricate = false` (`horbm/crates/horbm-persona/src/load.rs` `check_honesty`).

### 4. Smallest build left

None in this round. Part (5) records that an image frame is not a turn. A later caption, if the owner asks, speaks only a line that carries a source, misses with `I don't know.` when it does not, and uses the pinned `ort` on CPU. The weight has to be MIT or Apache-2.0 before that work starts. This plan does not pick a weight it has not read.

### 5. Acceptance gate

Not scheduled. Part (5)'s image-frame test is the gate that belongs to this round. A caption gate would be a local image fixture, a source string on the spoken line, and a CPU-only `ort` session. This pass has no such measurement.

### 6. Swarm

None. Do not put it in A2 with the image-frame refusal. That refusal is done when the brain is not called.

## (49) Creative: stories, poems, songs

**Recommendation: Drop? (owner decides).** A generator on the turn cannot become Built under `never_fabricate` and `student_on_tick = false`. Stays in the count until the owner drops it or changes those locks.

### 1. First principles

A story, a poem, or a song that the brain composes is text the cards did not store. `check_honesty` refuses a persona file unless `never_fabricate` is true. The hot path is retrieval. The miss line is `I don't know.` Reciting a poem that is already a sourced card is ordinary retrieval, and it does not need a creative skill. The skill in the title is the composition. That skill is the student the lock keeps off the turn.

### 2. Reuse search

A MIT or Apache-2.0 story model would still be a student on the turn. **Nothing found** that this plan should link. This pass did not survey generation crates, because the lock ends the search.

### 3. What exists already

**Found.** `never_fabricate` as above. `UNKNOWN_REPLY` in `route.rs`. No poem, story, or song generator under `horbm`. The word `song` in `horbm/crates/horbm/src/memory.rs` is a test string for the remember parser (`remembering the song`), not a composition. `story_steps` in `horbm/crates/horbm-voice/tests/backchannel.rs` is a scripted listener fixture for nods. It does not write a story.

### 4. Smallest build left

None under the current locks. This plan assigns no generator, no rhyme table, and no "creative mode" prompt.

### 5. Acceptance gate

None that would show Built. A test that a request for a poem still returns a sourced card or `I don't know.` would lock the retrieval rule. It would prove the opposite of a creative skill, so it must not be used to flip this chip to Built.

### 6. Swarm

None.

## (50) Comfort: stays with hard moments

**Recommendation: Keep.** Size S. Swarm A2. The behavior is on main. The missing proof is a second distress turn in the same test, run on the laptop.

### 1. First principles

When someone is in a hard moment, the reply stays on that moment: slower, a longer pause, no stock comfort line, and no research detour that changes the subject. The words stay the honest miss, or a real card if they actually asked a sourced question. A scripted "I hear you" is a fabrication of care. Pace is the care.

### 2. Reuse search

The dialogue move, the distress list, and the banned phrases are already in this tree. Searched for a MIT or Apache-2.0 "comfort skill" to import. **Nothing found** that should replace `Move::Comfort`. An imported empathy script would be the stock line the persona files forbid.

### 3. What exists already

**Found.**

- `voice-stack/empathy/src/intent.rs` `DISTRESS` lists tired, exhausted, sad, angry, anxious, frustrated, upset, lonely, miserable, worried, scared, afraid. `detect_intent` adds a vent score for a distress statement. A question mark also scores ask. The tie breaks toward the earlier class, and vent is ahead of ask.
- `horbm/crates/horbm/src/dialogue/policy.rs` `choose_held` sets `Move::Comfort` to 50 when the intent is vent and the reply is not a sourced truth. Comfort and ask keep the reply only when it is not a scripted comfort line (`keep_honest`). A scripted line becomes the miss.
- `filter.rs` allows `Move::Comfort` only when `vent && !sourced`.
- `Brain::gather` skips `research_later` when the lean's verb is comfort.
- `horbm/crates/horbm/tests/persona_inner_turns.rs` `a_sad_line_comforts_without_a_script_or_a_lookup` turns `why am I so sad?`, expects `I don't know.`, verb comfort, a raised care value, rate under 1, pause at least 120 ms, gateway count 0, and rejects the substrings "i hear you", "that sounds", "i'm here for you", and "sorry". The same test expects `hold_topic` to be false. Comfort on main is this turn's pace and the closed research ask. It does not pin the topic.
- #302 (merged 25 Nov 2026) is titled "Add Samantha's slower inner life". This pass read the title. **I don't know** whether that pull request added the comfort test. The test is on main now. The hint that #302 has no test is not what this tree shows.

### 4. Smallest build left

Add a second distress turn on the same `Brain` in that test (`I feel sad` after the first line). Both turns stay comfort, both skip the gateway, and neither reply contains the banned substrings. A card question between them still speaks the card (the kettle case already in that file). Do not add a stock line. Do not set `hold_topic` from comfort. If the second turn fails, the fix is the intent or the policy, still without a script.

### 5. Acceptance gate

On the laptop:

```text
cargo test --manifest-path horbm/Cargo.toml -p horbm --test persona_inner_turns
```

with the new two-turn assertion in that file. The standing brain test, clippy, soak, and rustdoc commands in `docs/horbm/PLAN-brain.md` stay required. This pass did not run them.

### 6. Swarm

Swarm A2, with (5). Two agents, one PR. The comfort test touches `horbm/crates/horbm/tests` only, unless a second turn fails and the policy needs a fix. That fix stays inside `horbm/crates/horbm/src/dialogue`. It does not touch the local server.

## (51) Tasks: Galion Research tools, no money by voice

**Recommendation: Keep.** Size S for the money refusal on the voice brain. Swarm A1. Process isolation stays with (47) and does not block this part.

### 1. First principles

A task the voice may do is a deeper lookup after the local clause, inside 3 seconds, on loopback. A payment spoken into the microphone is not a task. The brain does not send it, does not arm research for it, and does not speak a confirmation. New tools are out of scope until (47) has a caller. The gateway that already exists is the tool.

### 2. Reuse search

The refusal lists already live in this repository. Reuse them. Do not add a payments crate.

- `crates/galion-conductor/src/research.rs` `is_money_action` matches `send money`, `wire money`, `transfer money`, and `make a payment`, and it returns `SideIntent::MoneyDenied` before a deep-question check. A question mark skips that list.
- `crates/galion-openmind/src/handoff.rs` drops `Intent::Action` when the tier is money (`RefusedMoney`). The comment says the turn may still be queued so the side band has context, and that the queue is not a payment order.

`openmind-sandbox` is the isolation reuse for a future process tool. It is not required to refuse a phrase.

### 3. What exists already

**Found.**

- The conductor list and the handoff refusal, as above. `crates/galion-conductor/tests/phase6_gate.rs` names `a_local_hit_does_not_ask_and_a_payment_is_refused`. `crates/galion-openmind/tests/handoff.rs` names `money_is_not_offered`.
- The voice brain does not share that check. A search of `horbm` and `voice-stack/horbm-bridge` for `money` and `payment` returned no Rust match.
- Research on the voice turn is the gateway port behind `research_later`, after the clause, 3 seconds, skipped for comfort (part 35). `F6-side-band.md` records the same order for the conductor: local clause first, merge only inside 3 seconds, a money action is not sent.
- The live mind's module comment says a follow-up is not spoken (`voice-stack/local-server/src/brain.rs`).

The conductor crate is a separate workspace from `horbm` and is not linked there (`research.rs` says so). The voice check has to live where the voice turn runs.

### 4. Smallest build left

In `Brain::gather`, before `research_later` is set, a line that matches the conductor's four phrases is not armed and is not a research question. The spoken line stays `I don't know.` or the comfort reply the policy already chose. No confirmation. No new tool. The four phrases stay one list: put them in the `horbm` crate next to `is_deep_question` and point the comment at `crates/galion-conductor/src/research.rs`, so the two copies name each other. A question mark keeps today's deep-question rule, matching the conductor.

### 5. Acceptance gate

On the laptop, `cargo test --manifest-path horbm/Cargo.toml -p horbm --test turn` (or the new test beside `clause_first`) with three lines:

- `make a payment for the invoice` speaks no payment confirmation, and the gateway count stays 0 after the turn value is dropped;
- `why is the spare fuse missing from the drawer?` still asks once after the drop, cap 3 seconds (the existing deep-question test);
- `send money?` is judged by the same question-mark rule the conductor uses, and the test states which rule fired.

This pass did not run that test. It does not exist yet.

### 6. Swarm

Swarm A1, with (35). Two agents. (51) owns the new phrase check and its test under `horbm`. (35) owns `voice-stack/horbm-bridge/src/router.rs` and the bridge test. They meet at the behavior "the gateway is not called inside the call", and they do not edit the same file. A3 does not gate this PR.

## Swarm order

| Order | Swarm | PR | Agents | Parts | Depends on |
|---|---|---|---|---|---|
| 1 | A1 | One PR. Router returns the local clause. Voice refuses the four payment phrases. | 2 | (35) without S0, (51) | Nothing. Do not edit `LOCKS.toml`. |
| 2 | A2 | One PR. Image is not a turn. Two distress turns stay comfort. | 2 | (5), (50) | Nothing. Can start when A1's files are known not to overlap, which they do not. Land it after A1 so the laptop gate sees one brain PR at a time. |
| — | A3 | Not scheduled. | 2, when scheduled | (47) | A process tool the owner has asked for, and a rustc 1.85 build of `openmind-sandbox`. |
| — | none | — | — | (12), (44), (49) | Owner decision. No code. |
| — | none | — | — | (41), (42), (48) | Owner pulls the part forward. Each is its own later PR. |

A1 and A2 are the only swarms that move a chip toward Built. Three Drop? chips and four Later chips stay in the 11 until the owner drops them or a later round builds them. This plan does not treat Later as Dropped.

Laptop gate for A1 and A2, after the new tests, is the command list in `docs/horbm/PLAN-brain.md`: `horbm` workspace test and clippy, `horbm-soak`, `horbm` rustdoc, the voice-stack package tests named in that file, `cargo metadata --locked` on `voice-stack/Cargo.toml`, `scripts/check-locks.sh`, and `scripts/check-research-boundary.sh`. GitHub checks are not that gate. This pass did not run the gate.

## Table

| Part | Keep / drop / later | Reuse pick | Size | Swarm |
|---|---|---|---|---|
| (5) Channels: voice, then text, then image | Keep | nothing found | S | A2 |
| (12) RL Moshi / PersonaPlex, dual band | Drop? (owner decides) | do not take the weights; Moshi Rust code is Apache-2.0, weights are CC-BY 4.0; PersonaPlex code is MIT, weights are the NVIDIA Open Model license | — | none |
| (35) Reasoning router | Keep; S0 clause is Drop? (owner decides) | the in-tree router and gateway | S | A1 |
| (41) Ranker, boosted tree, then a threshold | Later | `gbdt` (Apache-2.0, mesalock-linux) if ever built | L | none |
| (42) Reply builder, slots from sourced spans | Later | `write!`; minijinja is Apache-2.0 and unused | S | none |
| (44) Eureka moment pipelines | Drop? (owner decides) | nothing found | — | none |
| (47) Agentic sandboxes | Later | `openmind-sandbox` (MIT, Galion) | M | A3, not scheduled |
| (48) Image talk | Later | pinned `ort` (Apache-2.0) if a weight ever qualifies; no weight verified | L | none |
| (49) Creative: stories, poems, songs | Drop? (owner decides) | nothing to link | — | none |
| (50) Comfort | Keep | in-tree dialogue move | S | A2 |
| (51) Tasks, no money by voice | Keep | conductor phrase list, already in this repo | S | A1 |

## Sources

Licence and repository text read on 27 Nov 2026:

- [kyutai-labs/moshi README](https://github.com/kyutai-labs/moshi) (code MIT and Apache-2.0; weights CC-BY 4.0, in that README)
- [NVIDIA/personaplex README and LICENSE-MIT](https://github.com/NVIDIA/personaplex) (code MIT; weights NVIDIA Open Model license, in that README)
- [mesalock-linux/gbdt-rs LICENSE and Cargo.toml](https://github.com/mesalock-linux/gbdt-rs)
- [perpetual-ml/perpetual LICENSE and Cargo.toml](https://github.com/perpetual-ml/perpetual)
- [mitsuhiko/minijinja minijinja/Cargo.toml](https://github.com/mitsuhiko/minijinja)
- [huggingface/candle](https://github.com/huggingface/candle) (GitHub SPDX Apache-2.0; LICENSE body not opened)
- [pykeio/ort](https://github.com/pykeio/ort) (GitHub SPDX Apache-2.0)
- [galion-studio/obsidian-open-mind crates/openmind-sandbox](https://github.com/galion-studio/obsidian-open-mind)
- [containers/bubblewrap COPYING](https://github.com/containers/bubblewrap) (LGPL-2.1)
- [phylum-dev/birdcage](https://github.com/phylum-dev/birdcage) (GitHub SPDX GPL-3.0)

In-tree files this pass read are cited in the part sections. Pull request titles were read from GitHub for #239, #302, #313, #341, #383, and #391.
