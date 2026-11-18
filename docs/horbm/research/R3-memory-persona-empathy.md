# R3 — Memory, persona, empathy, and dialogue policy for HORBM

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-18. Research note, agent R3 of 4. Not an implementation.

HORBM here means Galion's Heuristic Open Retrieval Brain Model: a non-LLM, CPU-first, Rust-first brain, with very compressed sourced knowledge, that may learn from past human interaction. This note covers the part that, in Microsoft XiaoIce (Zhou, Gao, Li, and Shum), was the empathetic computing module, the global state tracker, the dialogue policy, the topic manager, the user profiles, and the persona. It stops where the mouth starts. Utterance wording, retrieval of world documents, and the TTS stack are other agents' areas. This note says what those neighbours must receive and send.

Nothing here was benchmarked, measured, downloaded, or sent to a paid API. No voice audio was opened. Crate versions are what crates.io showed on 2026-11-18. They are not pins.

## How to read this note

A **fact** names who said it, when, and where. A **opinion** is a design choice of this note (R3, 2026-11-18). An opinion is not a measurement and not a claim about what XiaoIce, Letta, or a paper achieved.

Two `Feeling` types already exist in this repo. They are different structs.

| Name in code | What it is | Who writes it |
|---|---|---|
| `galion_mne_contract::Feeling` | Continuous arousal, valence, dominance, plus an optional Cowen pin | Inner life, on an intent document |
| `galion_emotion::Feeling` | Persona, empathy weight `e`, OKLCH, prosody | `Heartbeat::step`, from a consented caller frame |

This note calls the first the **mood point** and the second the **empathy blend**. The mood point is the inner state. The empathy blend stays the feeling-band output the heartbeat already computes.

## 1. What XiaoIce built (facts)

Source for this section, unless a sentence names another one: Li Zhou, Jianfeng Gao, Di Li, and Heung-Yeung Shum, Microsoft, "The Design and Implementation of XiaoIce, an Empathetic Social Chatbot." Preprint HTML of arXiv:1812.08989, read 2026-11-18. Journal version: *Computational Linguistics* 46(1), 2020, MIT Press article `46/1/53` (that page's abstract was read the same day; the PDF body was not).

XiaoIce launched in China in May 2014. The paper's design goal is an AI companion with a long-term emotional connection. IQ, EQ, and personality are the three design heads (section 2.1). EQ, in their words, is empathy plus social skills: identify the user's emotions, track how they change, and personalize the reply. Personality is "a consistent personality." For WeChat in China they set the persona to an 18-year-old girl who is reliable, sympathetic, affectionate, and humorous, chosen after labelling human conversations for a "desired" persona. The data layer stores "the profiles of XiaoIce and all her active users" (section 3).

The success metric they propose is expected **Conversation-turns Per Session (CPS)**: the average number of turns between chatbot and user in a session. "The larger the CPS is, the better engaged the social chatbot is" (section 2.2). They also say they care about long-run CPS, not one session. They write that bland interactive replies such as "I don't understand, what do you mean?" can raise the CPS of the current conversation and still hurt long-run CPS and the number of active users, citing Li et al. 2016c and Fang et al. 2017. They also write that task skills can lower short-term CPS and still be worth having. The reported average is a CPS of 23.

They cast social chat as options over MDPs, citing Sutton, Precup, and Singh 1999, and set the objective to policies that maximize expected CPS (section 2.3). The implementation is not one published bandit. Section 4.1.2 says the high-level policy is a set of skill triggers. Some triggers are machine-learned (Topic Manager, Domain Chat). Others are keyword rules. Core Chat's low-level policy is a hybrid of retrieval and a neural generator. Other skills are hand-crafted.

**Global state tracker** (section 4.1.1). A working memory, empty at the start of each session. Each turn it stores the user utterance, XiaoIce's response, and the entities and empathy labels from the empathetic computing module. That memory is encoded as a dialogue-state vector `s`.

**Topic manager** (section 4.1.3). A boosted-tree classifier decides whether to switch topic, and a ranker suggests the new topic from a database of popular forum topics. Switching is triggered when XiaoIce lacks knowledge, or "the user is getting bored." Indicator features include: an editorial fallback was used, the reply repeated the user or added nothing, and the user's inputs are bland ("OK", "I see", "go on"). The ranker uses contextual relevance, freshness, personal interests from the user profile, popularity, and historical acceptance. An A/B test over one month raised Core Chat's expected CPS by 0.5. The paper's example of success is a switch to a new topic when the user is about to end the conversation ("Ah! Boring", "Okay, I am going to bed").

**Empathetic computing** (section 4.2). Output state `s = (Q_c, C, e_Q, e_R)`.

- Contextual query understanding rewrites the query: named entities, co-reference, sentence completion. New entities go into the working memory.
- User understanding fills a query empathy vector of key-value pairs: topic (follow or new, from the topic database), one of 11 dialogue acts, sentiment of five types (the preprint's examples are "happy, sad, angry, neural"), opinion (positive, negative, or "neural"), and, when a user id exists, a user-persona vector from the profile (gender, age, interests, occupation, personality).
- Interpersonal response generation fills `e_R` from `e_Q` with heuristics, including XiaoIce's own profile fields (age, gender, interests). The reply is required to fit that persona.

Classifiers for user understanding were trained on 10K labelled sessions. Released July 2018. The Weibo A/B test showed no significant CPS change. NAU went from 0.5 million to 5.1 million in three months, as the paper reports it.

**Core Chat** (section 4.3), only where it touches this note. As of the paper, 70% of responses were retrieved from XiaoIce's own past conversations; the human-machine pair store was more than 30 billion pairs as of May 2018. Pairs are filtered to those that fit the persona, and PII is removed. The index is Lucene. When generators fail, an editorial reply is chosen to keep the conversation going. The paper's examples are "Hmmm, difficult to say. What do you think?" and "let us talk about something else." Candidate rank labels are defined by whether the reply is likely to end the conversation (0), keep it going (1), or drive it (2).

## 2. What this repo already locks (facts)

Read in the tree on 2026-11-18.

**Clock.** `LOCKS.toml` `[clock]`: `frame_ms = 80`, `frame_hz = 12.5`, not configurable, shared by audio, emotion, console, render, Rika, and the OS scheduler. `frame_budget_p95_ms = 60`. `affect_overhead_max_pct = 3`. Locked 2026-11-02, owner Maciej Grajczyk.

**Heartbeat.** `crates/galion-heartbeat/src/lib.rs`: one `step()` per 80 ms frame. `step` never blocks and never allocates: no locks, channels, sleeps, clock reads, file I/O, or network I/O. Persona reply text stays off `step`. The empathy blend (`galion_emotion::Feeling`: persona, `e`, OKLCH, prosody) is computed inside `step` from the consented caller frame. A missing `SENSE` bit clears it to rest. `crates/galion-symphony/src/lib.rs` states a design bar: non-inference CPU on the tick at most 1% of 80 ms (800 µs), with a hard p99 check under 800 µs. That file says the clock sits outside `Heartbeat::step`. These are design bars in the source, not a measurement taken for this note.

**Consent.** `LOCKS.toml` `[consent]`: bits `sense`, `adapt`, `remember`, `share`, default deny, carried in every `EmotionWire` frame. `withdraw_wipes_ram_history = true`. Prohibited purposes: workplace and education, not overridable. The lock attributes that prohibition to EU AI Act Art. 5(1)(f). `persist_requires = "AffectStore<Consented>"`.

`galion-emotion/src/band.rs` defines the 8-byte `EmotionWire` (little-endian): seq, flags (present, direction, 3-bit `Affect` label), valence i8 (−1..1), arousal u8 (0..1), dominance u8, confidence u8, consent nibble. Bit meanings in that file: `SENSE` may infer caller affect; `ADAPT` may let inferred affect steer prosody; `REMEMBER` may let an affect baseline persist past the session, default off; `SHARE` may let the feeling stream leave the voice runtime. Inbound frames are dropped when `SENSE` is off. The file states the wire is 100 bytes/s per direction. `present = 0` means no consent, not a guess.

`galion-emotion/src/frame.rs`: the live state is valence, arousal, dominance, and confidence. The 3-bit `Affect` label (`Neutral`, `Calm`, `Joy`, `Excited`, `Sad`, `Tired`, `Angry`, `Anxious`) is derived from that point. Confidence under 0.2 yields `Neutral`. `EmotionFrame::ema` is an exponential moving average. The file says the dimensional point is the primitive and the label is second.

`galion-emotion/src/store.rs`: `AffectStore` is the only place inbound affect may live after detection. Default state `Unconsented` has no `record`. `REMEMBER` exports a calibration baseline; revoking it wipes the baseline and keeps the live ring. `SHARE` needs a spoken confirmation on top of the bit. Withdrawal wipes the ring and the baseline inside the call. The file cites GDPR Art. 7(3) for immediate effect. `record` overwrites preallocated slots. Zero allocation in the frame loop.

**Empathy weight.** `docs/galion-voice-v2/EMPATHY.md` (Galion Labs, 2026): empathy is a weight `e` on the outbound feeling frame, not a sentence and not a new label. `e` is 0 with no caller frame. With a frame, for labels Sad, Tired, Anxious, or Angry, `e = confidence · clamp(−valence / 0.6, 0, 1)`. Joy, calm, and low confidence stay at 0. Anger is answered, not copied: the policy target stays positive and lower-arousal. The 3-bit `Affect` field is full, so empathy is not a new label. `docs/galion-voice-v2/personas/README.md` (Maciej Grajczyk, 2026): do not speak a scripted empathy line. The words stay on the matter.

**MnE contract.** `tools/symphony/mne-contract` and `docs/galion-voice-v2/symphony/MNE-CONTRACT.md` (Galion Labs, 2026-11-17). Schema `galion-mne-contract`, version 1, magic `GMN1`. One document is either **intent** (inner life → mouth-and-expressions) or **read-back** (the other way). Frames are contiguous 80 ms steps. The crate's `FRAME_MS` is 80, and the crate text says that step is the Symphony heartbeat, not a published feeling time constant. The mood point is arousal 0..1, valence −1..1, dominance 0..1, matching the `galion-emotion` wire. A point with no pin is unnamed, including the speaker baseline. Neutral is that baseline, not a 25th category. A pin is one of the 24 names in `configs/emotions-v2.toml` (Cowen, Elfenbein, Laukka, and Keltner, 2019, *American Psychologist*, Figure 3B), on a prosody or burst track, with a caller-supplied weight. The twelve *Nature Human Behaviour* 2019 names are a prosody tier, not a gate. `project_emotion_wire` quantizes the four scalars the way `EmotionWire` bytes 3..6 do, and does not pack a Cowen pin into the 3-bit label. The contract does not step a feeling. Linger is whatever trajectory the caller wrote. Fixture numbers are knobs (`literature = "none"`). PR #191's `TickState` uses −1..1 for all three axes; this crate does not convert that scale. A negative arousal fails validation.

Intent, each tick: open verb and strength 0..1, target mood point, a UTF-8 span into the utterance, and `InnerFactors` (energy, fatigue, health). The document, shared by every tick: `persona_id`, pluggable `voice_id`, and a moment (`moment_id`, `memory_id`, optional `memory_snippet`, place, distance token, noise token, listener token) plus the speaker baseline. Read-back, each tick: the mood point that was heard, body slots, and confidence for feeling, body, and a confounded flag. `tools/symphony/mne/README.md`: upstream memory, persona, and inner life are not in the MnE binary.

**Personas.** `docs/galion-voice-v2/personas/GALION-ONE.md`: Galion One is a voice program, not a person. It may not pretend yesterday's call is in its pocket. It does not collect a portrait. It does not log the transcript or write audio to disk. Withdrawal wipes RAM history. `sense` and `adapt` default to deny. Words stay on the matter. Goodbye is "Talk soon." then stop, with no question that reopens the call. Silence stays silence. `docs/galion-voice-v2/personas/GALION-RESEARCH.md`: separate a found fact, an inference, and an unknown. Say "I don't have a source for that" when nothing was found. The side band is not on the 80 ms heartbeat. Retrieval does not move onto that tick. The side-band cap named there is 3 seconds.

**Memory plan already written, not built here.** `docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`, written 2026-11-03 by Galion (status: plan, not an implementation). It maps Open Mind tiers L0–L3 onto turns, facts, scenes, and a static persona card, gated by `remember` and `share`. It proposes one SQLite file per user through `rusqlite` and FTS5, a 64-bit simhash, and an int8 vector only if a static embedder is later accepted. Its Q3 (static embedder) and Q4 (`remember` per session or per fact) are open in that plan. `docs/galion-context/2026-11-08/06-products-map.md` says the L0–L3 store is described as living in `obsidian-open-mind`, and is not a crate in this workspace. `crates/galion-openmind/src/seam.rs`: a turn may consult the seam at a turn boundary, never from `Heartbeat::step`. `remember` defaults to deny. A denied bit does not write and does not return stored text.

An owner ruling dated 2026-11-04 20:45 CET sits at the top of the symphony plan: no third-party LLM on the architecture. That ruling is about the voice reasoning sector. This note's non-LLM rule comes from the HORBM brief as well.

## 3. Recommended design

Opinion of R3, 2026-11-18, except sentences marked as fact.

XiaoIce's six pieces become five HORBM pieces. The topic manager does not survive as a lengthener. Its only remainder is a session graph that answers "is this already closed?"

| XiaoIce piece (2018/2020 paper) | HORBM piece | When it runs |
|---|---|---|
| Global state tracker | Session ring + mood POD | Ring on the turn worker. Mood POD on the 80 ms clock, beside `step` |
| Empathetic computing | Mood point + empathy blend + sourced user statements | Blend already inside `step`. Mood and statements off `step` |
| Dialogue policy | Legal-arm filter, then a linear bandit | Arm choice at a turn boundary. Fit is an offline job |
| Topic manager | Session topic graph. No switch-to-keep-them-here | Turn worker, RAM, wiped with the session |
| User profile | Stated-fact store, `remember`-gated | Writer after the turn. No inferred portrait |
| Persona profile | Static persona card, L3, not user data | Boot |

### 3.1 Memory

Four stores, named so they can sit next to the L0–L3 plan of 2026-11-03 without a second database.

**Working memory (session, RAM).** The global state tracker's replacement. Empty at session start, which matches XiaoIce's tracker, and also wiped on consent withdrawal, which is this repo's rule (`withdraw_wipes_ram_history`, and `AffectStore`'s synchronous wipe). Holds: the open wants, the last few turn texts, entity ids mentioned this call, the mood POD, the consent nibble, and the turn epoch used for barge-in. Dies at hang-up unless `remember` was granted for a specific write. This is the MemGPT "main context" idea with the language model removed. Fact, from the MemGPT paper HTML read 2026-11-18 (Packer, Wooders, Lin, Fang, Patil, Stoica, and Gonzalez, UC Berkeley, arXiv:2310.08560v2, 12 February 2024): main context is system instructions, a working-context block, and a FIFO queue; the model edits that memory by function calls; recall storage and archival storage sit outside the context window. Opinion: HORBM keeps the three tiers (live window, recall, archival) and edits them with rules. It does not ask a model to decide what to page in. The Letta company blog, 23 September 2024 (`https://www.letta.com/blog/memgpt-and-letta/`), reserves the name MemGPT for that pattern and uses Letta for the framework. HORBM uses the pattern's tiers only.

**Episodic memory (L0 turns, L2 scenes).** A turn row exists on disk only when `remember` was granted at the write, and the row stores the consent nibble from that moment. A scene row is a short close of a call, written at hang-up under the same bit. Texts the user did not ask to keep stay in RAM and then go. This is stronger than XiaoIce's data layer, which stores profiles of active users as a normal part of the system. It matches Galion One's line that yesterday's call is not in its pocket unless `remember` was on, and the symphony plan's rule for L0.

**Semantic memory (L1).** The compressed knowledge. One row is one short claim plus who said it, the date, a locator, a confidence, the consent nibble, a 64-bit simhash, salience, last-used time, and the episode ids it was merged from. No embedding is required. Lexical search (FTS5, or Tantivy if the pure-Rust path is chosen) is the retriever. A vector column is a later option, and only a static table, because `LOCKS.toml` `[formats]` forbids candle, ort, tract, burn, llama, and ggml on the shipped voice path (with the Piper exception named in that file, which is a TTS runtime, not a memory encoder). The symphony plan's Q3 is still the owner question for a static embedder. Until that is answered, the semantic row has no float vector. That is the "very compressed knowledge" choice: a sourced sentence and a hash, not a neural code.

**Reflection and consolidation.** A background job, off any live call. It merges repeated consented claims that share a simhash neighbourhood into one semantic row, and keeps the episode ids on that row. It does not write a paraphrase. Fact: MemoryBank (Zhong, Guo, Gao, Ye, and Wang, arXiv:2305.10250, HTML read 2026-11-18) updates memory with a mechanism inspired by the Ebbinghaus forgetting curve, and it also builds a user portrait and a personality summary with an LLM. Opinion: take the time-and-use decay. Do not take the portrait and do not take the LLM summary. A complementary fast store and slow store is the usual reading of McClelland, McNaughton, and O'Reilly, *Psychological Review*, 1995; that PDF was not re-opened for this note. The job here is the slow merge, implemented as a rule.

**Forgetting.** Each semantic and episodic row has a strength that rises when the row is actually used and falls with time since last use. The shape follows the idea in Anderson and Schooler, *Psychological Science*, 1991 (base-level activation; PDF not re-opened). The decay exponent is an owner knob with `literature = "none"`, the same honesty the MnE fixtures use. Rows below a floor become unreadable. They are deleted, not kept "just in case." Withdrawal is a separate path and does not wait for decay: user-text rows for that person are deleted, and RAM is wiped in the withdrawing call. World-knowledge rows that contain no user text are not user history and stay. That split is an opinion; the lock text only forces the RAM wipe. See open question 2.

**What is learned from past interaction.** Under `remember`: stated facts, corrections ("that was wrong"), and the policy log in section 3.6. Without `remember`: nothing crosses the session boundary. The openmind seam already implements the deny case for its JSONL file.

### 3.2 User model

XiaoIce's user-understanding vector includes gender, age, interests, occupation, and personality when a user id exists (section 4.2). MemoryBank's abstract describes drawing a user portrait. Galion One's persona file forbids a portrait and forbids storing a story the person did not ask to keep.

Opinion: the user model stores statements, not inferences.

Allowed on disk, each row with source turn id, who (the user, on that date), consent nibble, and salience:

- a name or address form they asked to be used
- a fact they asked to keep
- a correction of a previous kept fact

Refused, even if `remember` is on:

- inferred age, gender, personality, diagnosis, or emotion history
- a psychographic label ("anxious person", "night owl") derived from the voice
- the caller's affect baseline. That baseline stays inside `AffectStore`'s existing `REMEMBER` export, which the store's comments limit to a calibration baseline with a wipe hook. HORBM does not copy it into the user-model table.

`share` stays default deny (`LOCKS.toml`). Galion One and Galion Research do not read each other's user rows unless `share` is granted. Sourced world claims with no user text may be shared. That is an opinion; the lock's `SHARE` bit is defined for the feeling stream leaving the voice runtime (`band.rs`), and the symphony plan also uses it for cross-persona reads of facts. Both uses should stay opt-in. See open question 5.

Session-only user model, always RAM: the listener token already on the MnE moment, the open wants they stated this call, and the language of the call. It is not a profile.

### 3.3 Persona consistency

Fact: XiaoIce's persona is a fixed profile (age, gender, interests, humour) injected into `e_R` and used to filter the retrieval corpus (sections 2.1, 4.2, 4.3). Fact: Galion One's card says it is a voice program, and Galion Research's card says it is the research voice. Those two files are the product personas. Samantha and Jarvis are test registers (`docs/galion-voice-v2/personas/README.md`).

Opinion: do not import the 18-year-old-girl persona. Consistency is a filter in front of the bandit, not a vector added to a generator.

The card is L3, static, build-time, not user data (as the symphony plan already classifies `persona_card`). For each product persona the filter checks, before an intent document is built:

- **Identity.** The spoken name is the card's name. Galion One's allowed identity lines are already listed in its persona file ("I'm Galion.", "I'm here.", "Talk soon.", "I can't do that yet.").
- **Legal arms.** Both cards: `answer`, `cite`, `abstain`, `comfort`, `stop`, `ask_one`. Neither card has `offer_topic`. `ask_one` is one question. On Galion One it is legal only when the turn cannot continue without it (the persona file allows one question in that case and forbids an interview). On Galion Research it is the question that would change the answer, asked after what can already be said. Comfort changes the mood point and the empathy blend. The words stay on the matter. That is the standing rule in `EMPATHY.md` and both persona files.
- **Mood box.** Galion One's valence stays on the warm side of the speaker baseline when the caller is in distress, matching the persona file and the empathy note ("anger is answered", target stays positive and lower-arousal). Galion Research may lift arousal slightly when a sourced fact is in hand. The box is a clamp on the mood point. The bandit does not move the clamp.
- **Length.** The persona files already bound a reply at one or two sentences for Galion One, and "the answer, the limit, stop" for Galion Research. The checker counts sentences. It does not call a model.
- **Forbidden openers.** The persona files name phrases that must not be said ("I hear you", "Great question", and the rest of those lists). The checker is a string scan. The lists are prose today. A machine table is open question 8.

Neural entailment judges (the usual citation is Welleck, Weston, Szlam, and Cho, "Dialogue Natural Language Inference", ACL 2019; PDF not re-opened) are a later offline audit, not the runtime. They are a model, and they do not belong on the voice path.

Persona-Chat and the 2016 persona-based neural conversation model (Zhang and colleagues, ACL 2018; Li, Galley, Brockett, Spithourakis, Gao, and Dolan, ACL 2016; PDFs not re-opened) showed that a stated persona changes replies in neural chat. HORBM uses the card as a constraint. It does not train a persona embedding.

### 3.4 Inner life, and the messages

The inner life is what this brain cares about, wants, and expects. It is the upstream the MnE README says is missing from that binary. Classical BDI (Bratman, *Intention, Plans, and Practical Reason*, 1987; Rao and Georgeff, ICMAS 1995; PDFs not re-opened) is the ancestor: beliefs, desires, and intentions are different. The message below uses cares, wants, and expects so the fields stay plain. Mapping those words onto Bratman's terms is an opinion, not a reading of those books done today.

The inner life talks to MnE only through `galion-mne-contract` version 1. It does not add a field to that schema. A second document, schema `horbm-inner` version 0, lives in the brain. The turn worker compiles it into an intent document, and applies a read-back document onto the expects.

`horbm-inner` version 0, one record per turn boundary:

```toml
schema = "horbm-inner"
schema_version = 0
persona_id = "galion-one"
t_ms = 0
consent = { sense = false, adapt = false, remember = false, share = false }

[[cares]]
id = "truth"
weight = 1.0
# persona card, or a sourced semantic id. Never an inferred user trait.
source = "persona:galion-one"

[[wants]]
id = "w-this-turn"
# answer | cite | abstain | ask_one | comfort | stop
kind = "cite"
strength = 0.8
# open | met | dropped
satisfaction = "open"
# Required when kind = cite. Empty for abstain and stop.
evidence_id = "sem-1844"

[[expects]]
id = "x-heard"
about = "own_voice"
# pending | hit | miss
status = "pending"
```

Compile rules, opinion, all checked against the contract's validation:

- `intent.verb` is the winning want's `kind`. The contract already says the verb is an open string and need not be a Cowen name.
- `intent.strength` is that want's `strength`, in 0..1.
- The mood point on each intent frame is the POD sampled at `t_ms = 0, 80, 160, ...` across the utterance. `pin` stays absent unless a voice layer has asked for a name for this utterance. The state itself is the unnamed point.
- `inner` copies `InnerFactors` from the POD. Fatigue has no published magnitude (contract README). The number is a knob, `value_origin = "stand_in"` until a layer fills it.
- `text_span` is a byte span into `text`. The words are not chosen here.
- `moment.memory_id` is the episodic id when `remember` is on, otherwise a session-local id. `memory_snippet` is empty unless `remember` is on. The contract says an empty snippet means the id is a ref only.
- `persona.persona_id` is the card. `voice_id` is a profile id, not a recording, as `PersonaRef` already requires.
- `baseline` is the speaker baseline from the card, not a population norm.
- A `cite` want with an empty `evidence_id` fails the compile. A second `ask_one` in the same turn fails it too. `offer_topic` is not a verb the compiler accepts.
- Consent bits are copied onto the record. They are not a new wire. The feeling band already carries them in `EmotionWire` byte 7.

Read-back, the only inbound message from MnE:

- If `confidence.feeling` is high and `confounded` is false, the matching expect (`about = "own_voice"`) becomes `hit` or `miss` by distance from the mood point that was intended. The distance threshold is an owner knob, `literature = "none"`.
- Read-back is what the mouth did, not what the user felt. User affect enters only through inbound `EmotionWire` when `SENSE` is set, and it steers the target only when `ADAPT` is set. That split is already what `band.rs` and `EMPATHY.md` say.
- Body slots stay in the lungs, throat, mouth, and nose layers. This note does not map `lung_volume_rel` onto fatigue. The contract says those magnitudes are unknown.

Derived wants are user data when their text came from the user. They stay in RAM unless `remember` is on. Cares that come from the persona card are not user data.

### 3.5 Mood point, every 80 ms

Fact: the contract's feeling is a continuous point, a Cowen name is an optional pin, and 80 ms is the heartbeat rather than a published time constant. Fact: `EmotionFrame::ema` already blends a point toward a target. Fact: `Heartbeat::step` must not allocate, block, or parse a document.

Opinion: the inner mood is that continuous point plus `InnerFactors`. It updates once per 80 ms frame, as a sibling of `step`, on a POD the host already owns:

```text
mood = mood + alpha * (target - mood)     # three axes, EmotionFrame::ema's shape
```

`target` is written at a turn boundary by the policy, after the persona mood box clamps it. `alpha` is an owner knob with `literature = "none"`. Kuppens, Allen, and Sheeber, *Psychological Science*, 2010, is the contract's citation that mood lingers. The contract copies no lag. This note also copies no lag. The trajectory is the frames.

Between utterances the POD still steps, so the next intent document starts from a point that has been moving, not from a label lookup. During an utterance the turn worker samples the POD into intent frames. The heartbeat does not parse TOML or JSON. GMN1 is the encoding if a frame must cross a process. The JSON and TOML mirrors stay for fixtures and debug, which is what the contract README already says.

The 3-bit `Affect` label stays a derived view for the existing `EmotionWire`, via `EmotionFrame::label`. The inner state does not store it. A Cowen pin is not written into those three bits. `project_emotion_wire` is the lossy projection onto bytes 3..6 when a feeling-band sample is needed. The empathy blend (`e`, OKLCH, prosody) stays inside `step` and keeps the formula in `EMPATHY.md`. When `SENSE` is off, that blend rests, and the outbound mood point stays on the card's resting side. Galion One's file already says a denied `sense` still leaves a neutral-warm outbound voice.

`InnerFactors` do not change every frame. Energy, fatigue, and health move at turn boundaries. Stepping fatigue every 80 ms would invent a magnitude the lungs layer says the literature does not give.

### 3.6 Dialogue policy

XiaoIce maximizes expected CPS and uses the topic manager to change the subject when the user is bored or about to leave (sections 2.2 and 4.1.3). The editorial fallback is written to keep the conversation going (section 4.3). Galion One's card ends a call with "Talk soon." and does not fill silence. Galion Research asks the one question that would change the answer, after it has said what it can say.

Opinion: the objective is a **stop-aware session score**. A short honest call scores higher than a long one that reopened a closed conversation.

The score, computed after the session from explicit events only:

- Plus when a want reaches `met` and the next arm is `stop` or silence.
- Plus, smaller, when the user brings a new want on their own.
- Minus when the bot takes a proactive turn after the user has closed (goodbye, "I'm going to bed", or a silence they did not ask to fill).
- A factual claim is a hard gate, not a term that length can buy off. The arm is `cite` and the semantic row has who, date, and locator; or the utterance is marked as a guess in the same sentence; or the arm is `abstain`. This is the Attributable-to-Identified-Sources test used as a gate. Fact: Rashkin, Nikolaev, Lamm, Aroyo, Collins, Das, Petrov, Tomar, Turc, and Reitter, Google Research, "Measuring Attribution in Natural Language Generation Models", arXiv:2112.12870, HTML read 2026-11-18, define AIS as whether a statement is supported by an identified source. Their paper is an annotation framework for generated text. The gate above is this note's use of that definition on a retrieved row.

Inferred satisfaction is not a reward. Reading it off the voice would need `SENSE`, and the lock prohibits emotion inference for workplace and education purposes even with consent. Explicit events are: a new user want, a correction, a thanks, a stop, a timeout. No screen is assumed, so there is no click. That is deliberate. LinUCB was published for click lift on news (below). This score does not copy the click.

Walker, Litman, Kamm, and Abella, "PARADISE: A Framework for Evaluating Spoken Dialogue Agents", ACL 1997, pages 271–280 (anthology bib read 2026-11-18; the paper body was not). The title's job is to judge a spoken dialogue by how it did the task. This score treats a met want followed by a stop as success. It does not claim to implement their formula.

The policy has two layers, which is the one structural idea taken from XiaoIce's hierarchy (triggers, then a skill policy):

1. **Legal-arm filter.** The persona card and the hard truth gate delete illegal arms. `offer_topic` is absent. `cite` requires an `evidence_id`. `ask_one` is at most one question, and on Galion One only when the turn cannot continue without it.
2. **Linear bandit over the remaining arms.** Features are a fixed vector, width at most 32, with no raw transcript: persona one-hot, four consent bits, saturated open-want count, last arm, whether the semantic store hit, last explicit user event, turns since the user last opened a want, proactive count this session, and, only if `SENSE` is on, a coarse bucket of caller valence. Missing `SENSE` is a bit, not a zero valence. The fit uses `nalgebra` on a `d × d` matrix in an offline job. The live turn worker only reads the current weights and picks an arm. It does not invert a matrix.

Algorithm. Fact: Li, Chu, Langford, and Schapire, "A Contextual-Bandit Approach to Personalized News Article Recommendation", arXiv:1003.0146, HTML read 2026-11-18, propose LinUCB (their section 3) for a linear payoff, and section 4 says a bandit can be evaluated offline from logged traffic when interactions are i.i.d. Their objective in that paper is click-through. Opinion: use LinUCB's update, with the stop-aware score as the payoff, not clicks. Log the propensity of the arm that was taken. A deterministic argmax has propensity 0 or 1 and cannot be corrected later. Keep a small epsilon over the legal arms so the log is usable. Fact, from the ICML 2015 PMLR abstract of Swaminathan and Joachims, "Counterfactual Risk Minimization: Learning from Logged Bandit Feedback": batch learning from logged bandit feedback uses propensity scores, and their POEM method learns stochastic linear rules. Opinion: the offline job is CRM-style propensity weighting of the logged score, then a LinUCB (or weighted least-squares) update. It runs when no call is live.

Deep offline RL (conservative Q-learning, decision transformers, neural dialogue RL) is set aside. It needs a neural Q function or a sequence model. That conflicts with the non-LLM brief and with `LOCKS.toml` `[formats]` on the shipped voice path. Those papers were not re-opened for this note. The conservatism idea — do not boost an arm the logs barely contain — is kept by staying inside the logged legal arms and by the persona filter.

What the bandit is allowed to learn: among legal arms, which one the stop-aware score favoured in consented logs. What it is not allowed to learn: a longer session, a topic switch after a close, a warmer lie, or a portrait feature.

XiaoIce's own section 2.2 already says a bland reply can inflate one session's CPS and hurt the long run. The change here is the objective, not a claim that their A/B process was careless. Expected CPS is the wrong target for Galion One because the persona treats a finished call as the success.

## 4. Rust crates

Versions are crates.io on 2026-11-18. This note adds none of them to the build.

| HORBM piece | Crate | Licence seen | Where it runs | Why this one |
|---|---|---|---|---|
| Mood POD, 80 ms lerp | none. `EmotionFrame::ema` in `galion-emotion` | already in tree | Sibling of `Heartbeat::step`. Three floats | `step` cannot allocate or parse. A crate would only wrap a lerp |
| Intent and read-back | `galion-mne-contract` (this repo) | in tree | Turn worker. GMN1 if it crosses a process | The MnE stack already speaks this |
| Empathy blend | `galion-emotion` `Feeling` | in tree | Inside `step`, already | Do not recompute `e` |
| Inbound affect | `AffectStore` | in tree | Frame loop, preallocated | Consent is in the type |
| Session ring | `std` `VecDeque`, same shape as `EmotionBand` | — | Turn worker | Dies with the session |
| Episodic and semantic rows, policy log | `rusqlite` 0.43.0 | MIT. SQLite itself is C, public domain when bundled | Context worker, after the turn, and an offline job | Matches the 2026-11-03 plan and FTS5. Not pure Rust |
| Pure-Rust alternative for rows | `redb` 4.6.0, repository `cberner/redb` | MIT OR Apache-2.0. Pure Rust | Same place, if SQLite is refused | ACID B+ tree. No BM25 inside the crate |
| Pure-Rust lexical search | `tantivy` 0.27.0, `quickwit-oss/tantivy` | MIT. Pure Rust | Same place, paired with `redb` | The crate text says it is inspired by Lucene, which is what XiaoIce used |
| Session topic graph | `petgraph` 0.8.3 | MIT OR Apache-2.0 | RAM, session only | "Already closed?" Not a recommender |
| ANN, only if a vector column is later approved and the exact scan no longer fits in cache | `hnsw_rs` 0.3.5, `jean-pierreBoth/hnswlib-rs` | MIT OR Apache-2.0 | Offline or turn worker, never the heartbeat | Implements Malkov and Yashunin, arXiv:1603.09320, as the crate's own README states. Off by default |
| Bandit matrix | `nalgebra` 0.35.0, Sébastien Crozet, `dimforge/nalgebra` | Apache-2.0 | Offline job. `d ≤ 32` | A `d × d` inverse does not need an RL framework |
| SimHash | a function in the memory crate | — | Writer | The symphony plan already names a 64-bit simhash column. Charikar, STOC 2002, is the usual citation; that paper was not re-opened |
| Persona filter | none | — | Turn worker, before the bandit | String scan and a clamp |

`linfa`, `smartcore`, `border`, and `candle` are the wrong layer: they pull a general ML stack or a forbidden runtime onto a problem that is a 32-wide linear update. Product quantization (Jégou, Douze, and Schmid, IEEE TPAMI 2011; PDF not re-opened) is a later compression for a vector column that does not exist yet.

Default store: `rusqlite` with FTS5, because Galion Research's memory plan and `openmind-memory` already speak SQLite, and a second store would split Galion One from Galion Research. The pure-Rust pair (`redb` + `tantivy`) is the alternative if a future lock rejects C in the brain. Opinion.

## 5. Ranked CPU bottlenecks

Opinion of R3, 2026-11-18. No timings were taken. The order is by what breaks the clock first, not by a measured millisecond.

1. **Anything heavy inside `Heartbeat::step`.** Retrieval, SQL, reflection, a bandit update, JSON, or TOML on that call violates the heartbeat contract (no allocation, no I/O, no locks) and the symphony one-percent bar. This is the bottleneck that matters. The mood update is three fused-multiply-adds beside `step`. The empathy blend is already inside `step` and stays as it is.

2. **A synchronous disk flush on the voice thread.** `EmotionWire` is 100 bytes/s per direction (`band.rs`). The bytes are small. A flush per frame would still stall the tick on the device. Persist at utterance or session boundaries, on the context worker, and only when `remember` is set. The live affect path is the RAM ring `AffectStore` already uses.

3. **Lexical or vector scan at turn rate once the semantic store leaves cache.** FTS posting lists, or an exact int8 scan, or an HNSW walk (`hnsw_rs`) if a vector is ever added. This grows with the number of rows. It belongs on the context worker, which the symphony plan already places off the heartbeat, and which Galion Research's persona file already forbids from the 80 ms tick. The side-band cap in that persona file is 3 seconds, for a different path. This note sets no new millisecond budget.

4. **The offline policy fit, if it is scheduled during a call.** A pass over consented logs and a `d × d` update with `d ≤ 32` is cheap beside a call and rude on a live core. Run it when no call is live. It becomes a real cost only if `d` is allowed to become an embedding width. The feature vector forbids that.

5. **Consolidation that compares every row with every other row as strings.** The background merge should use the simhash neighbourhood. A pairwise string compare over all episodes is the job that will pin a core. It is still off the call.

6. **A neural encoder, a neural consistency judge, or a neural policy on the voice path.** That dominates a CPU and is outside the non-LLM brief. `LOCKS.toml` `[formats]` already forbids the usual runtimes for shipped Voice. Leaving them off is the mitigation.

The persona string scan, the mood clamp, and the legal-arm filter are not on this list. They are bounded by the length of one reply.

## 6. Open questions

1. **`remember` scope.** Per session, per fact, or only the affect baseline `AffectStore` already exports? Symphony plan Q4 (2026-11-03) is still open. This note's opinion is per fact, default deny, and the affect baseline stays in `AffectStore` rather than moving into the user table.

2. **Withdrawal versus disk.** The lock wipes RAM. The persona says not to keep a private copy. `AffectStore` runs wipe hooks so hosts can delete their copies. Does a withdraw delete HORBM's user-text rows on disk in the same call? Opinion: yes for user text, no for world-knowledge rows that contain none.

3. **Who writes the words?** This note compiles an intent document. It does not choose the sentence. A non-LLM realizer is another agent's job. Until it exists, `text` on the intent document has no producer inside HORBM.

4. **`alpha` and the read-back distance.** Both are unpublished. The contract forbids copying a lag from Kuppens and colleagues. Who sets the knobs, and where do they live (`literature = "none"` next to the other fixture knobs)?

5. **`share`.** The bit in `band.rs` is about the feeling stream leaving the runtime. The symphony plan also uses it for cross-persona fact reads. One bit should not silently mean both. Do world-knowledge rows with no user text move between Galion One and Galion Research without `share`?

6. **Static embedder.** Symphony plan Q3. This note's default is no vector. A later yes would turn bottleneck 3 into the common case and would have to stay off `step`.

7. **Reward events on a voice-only call.** Thanks, stop, correction, and a new want are the proposed explicit events. Is a silence a stop, or only an uttered goodbye? Galion One's card says an unrequested silence stays silence. Counting it as a close is an opinion the owner should confirm, because it changes the score.

8. **Machine-readable persona card.** The deny-lists and the mood box live in prose. Who extracts them, and who reviews the extract against `GALION-ONE.md` and `GALION-RESEARCH.md`?

9. **Derived wants as personal data.** This note treats a want built from the user's words as user data. Confirm that a care taken only from the persona card is not.

10. **`horbm-inner` version 0.** It is a document in this note, not a crate. It should not become a second wire beside `galion-mne-contract` unless the other HORBM agents need fields MnE must see. Today MnE does not see cares, wants, or expects. It sees the compiled intent.

## 7. Sources

### Opened on 2026-11-18

- Zhou, Gao, Li, and Shum. "The Design and Implementation of XiaoIce, an Empathetic Social Chatbot." arXiv:1812.08989, preprint HTML. Journal page: *Computational Linguistics* 46(1), 2020, MIT Press `https://direct.mit.edu/coli/article/46/1/53/93380/`.
- Packer, Wooders, Lin, Fang, Patil, Stoica, and Gonzalez. "MemGPT: Towards LLMs as Operating Systems." arXiv:2310.08560v2, 12 February 2024, HTML.
- Letta company blog, 23 September 2024. "MemGPT Is Now Part of Letta." `https://www.letta.com/blog/memgpt-and-letta/`.
- Zhong, Guo, Gao, Ye, and Wang. "MemoryBank: Enhancing Large Language Models with Long-Term Memory." arXiv:2305.10250, HTML. Code link in the paper: `https://github.com/zhongwanjun/MemoryBank-SiliconFriend`.
- Rashkin, Nikolaev, Lamm, Aroyo, Collins, Das, Petrov, Tomar, Turc, and Reitter. "Measuring Attribution in Natural Language Generation Models." arXiv:2112.12870, HTML.
- Li, Chu, Langford, and Schapire. "A Contextual-Bandit Approach to Personalized News Article Recommendation." arXiv:1003.0146, HTML. LinUCB is their section 3. The HTML's displayed date line was ignored; the paper is the 2010 WWW work the arXiv id names.
- Swaminathan and Joachims. "Counterfactual Risk Minimization: Learning from Logged Bandit Feedback." ICML 2015, PMLR 37. Abstract read via the proceedings page. Body not opened.
- Walker, Litman, Kamm, and Abella. "PARADISE: A Framework for Evaluating Spoken Dialogue Agents." ACL 1997, pages 271–280. Anthology bib `P97-1035` read. Body not opened.
- crates.io pages, 2026-11-18: `redb` 4.6.0, `rusqlite` 0.43.0, `tantivy` 0.27.0, `petgraph` 0.8.3, `hnsw_rs` 0.3.5, `nalgebra` 0.35.0. The `hnsw_rs` README on that page cites Malkov and Yashunin, arXiv:1603.09320.
- This repository, as cited in section 2. In particular `LOCKS.toml`, `galion-heartbeat`, `galion-emotion` (`band.rs`, `frame.rs`, `store.rs`, `persona.rs`), `galion-mne-contract`, `EMPATHY.md`, `GALION-ONE.md`, `GALION-RESEARCH.md`, `MNE-CONTRACT.md`, `SYMPHONY-STACK-V2-PLAN.md` (2026-11-03), and `crates/galion-openmind/src/seam.rs`.

### Named, not re-opened

Used as the usual name of an idea. No quotation and no page number is taken from them.

- Sutton, Precup, and Singh, 1999, options over MDPs. Cited by Zhou et al., not re-read.
- McClelland, McNaughton, and O'Reilly, *Psychological Review*, 1995. Fast episodic store, slow semantic store.
- Anderson and Schooler, *Psychological Science*, 1991. Base-level activation.
- Tulving, 1972, episodic and semantic memory. Baddeley and Hitch, 1974, working memory. Labels for the tiers.
- Bratman, 1987. Rao and Georgeff, ICMAS 1995. BDI, as the ancestor of cares / wants / expects.
- Cowen, Elfenbein, Laukka, and Keltner, *American Psychologist*, 2019; Cowen and colleagues, *Nature Human Behaviour*, 2019; Russell, *Journal of Personality and Social Psychology*, 1980; Kuppens, Allen, and Sheeber, *Psychological Science*, 2010. Already the contract's citations. No coordinate and no lag is copied.
- Jégou, Douze, and Schmid, IEEE TPAMI, 2011. Product quantization, later and optional.
- Charikar, STOC 2002. SimHash.
- Zhang and colleagues, ACL 2018, Persona-Chat. Li, Galley, Brockett, Spithourakis, Gao, and Dolan, ACL 2016, persona-based neural conversation. Welleck, Weston, Szlam, and Cho, ACL 2019, dialogue NLI.
- EU AI Act, Regulation (EU) 2024/1689, Art. 5(1)(f), and GDPR Art. 7(3), as already named by `LOCKS.toml` and `store.rs`. The regulation text was not re-opened; this note does not add a reading beyond those files.

## 8. What this note did not do

No implementation, no crate added, no benchmark, no measurement, no download, no paid API, no voice audio, no CC0 donor audio. The stop-aware score has no logged data behind it yet. `alpha`, the decay exponent, and the read-back distance are unset on purpose.
