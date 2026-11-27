// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Empathy, then the layer 5 route, then the mood the voice speaks with.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Instant;

use galion_emotion_core::{
    ConductorCue, EmotionCore, InnerLife, InnerTurn, IntentMsg, MomentState, VoiceBaseline,
};
use galion_sam_empathy::{Empathy, PronounClass, TickIn, FRAME_MS};
use horbm_persona::{BootConfig, MoveClaim, MoveVerdict, Persona, PersonaError};

use horbm_brain::{
    classify, pick, split_clause, Answer, Heuristic, Retrieved, Retriever, SidebandMark,
};
use horbm_inner::{
    DialogueLean, Heard, InnerHome, InnerMsg, Perceived, PersonaInner, Verb, CURIOSITY_HOME,
    EXPECT_HOME, HELP_HOME,
};

use galion_emotion_core::standins::memory::{PersonMemory, PersonState, TrackedName, WriteOutcome};

use crate::dialogue::{
    DecideIn, Decision, Dialogue, DialogueConsent, ForgetAsk, Move, ReplyPlan, TurnSnap,
};
use crate::learn::LearnedRow;
use crate::memory::{about_the_person, pin_command};
use crate::research_style::{ResearchHit, ResearchRoot, StylePersona};
use crate::retrieve::{CellState, MemoryHitCell, RetrievalThread, RetrieveError, RetrievedCells};
use crate::route::{
    accept_line, classify_ask, is_money_action, route_heard, AskOutcome, CardLookup, Gap,
    GapReason, ResearchAsk, Speakable,
};
use crate::{MemoryItem, Pad, TurnOut, RESEARCH_CAP, UNKNOWN_REPLY};

/// Mood and the prosody knobs that belong on the turn. Not a sentence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodMeta {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    pub e: f32,
    pub intent: &'static str,
    pub rate: f32,
    pub energy: f32,
    pub pitch_st: f32,
    pub pause_ms: f32,
    pub named: Option<&'static str>,
}

/// Open, met, and dropped wants after the turns so far. Counts only.
///
/// The list is the in-tree `WantList` (Galion Labs, MIT,
/// `voice-stack/empathy/src/wants.rs`). This copy does not hold the
/// caller's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WantCounts {
    pub open: u32,
    pub met: u32,
    pub dropped: u32,
}

impl MoodMeta {
    pub const RESTING: MoodMeta = MoodMeta {
        arousal: Pad::RESTING.arousal,
        valence: Pad::RESTING.valence,
        dominance: Pad::RESTING.dominance,
        e: 0.0,
        intent: "chat",
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
        named: None,
    };

    pub fn pad(self) -> Pad {
        Pad {
            arousal: self.arousal,
            valence: self.valence,
            dominance: self.dominance,
        }
    }
}

struct EmptyCards;

impl CardLookup for EmptyCards {
    fn lookup(&self, _query: &str) -> Option<crate::CardHit> {
        None
    }
}

/// Newest card misses kept on [`Brain`]. Older ones are dropped.
const GAP_CAP: usize = 32;

/// Ear prosody. Unset stays on. `0`, `off`, and `deny` turn it off.
const EAR_SENSE_ENV: &str = "GALION_EAR_SENSE";

/// Person diary. Only the word `grant` opens the file.
const REMEMBER_ENV: &str = "GALION_HORBM_REMEMBER";

/// The linked brain. `open` reads the knowledge directory when it already
/// exists and does not create it. The person-diary path is kept and is not
/// opened: `remember` is denied. The live server passes the same read-only
/// loader and may attach a [`RetrievalThread`]. That thread is not started here.
pub struct Brain {
    empathy: Empathy,
    cards: Box<dyn CardLookup + Send>,
    research: Option<Box<dyn ResearchAsk + Send>>,
    speakable: Speakable,
    gaps: Vec<Gap>,
    meta: MoodMeta,
    card_count: usize,
    emotion: EmotionCore,
    inner: InnerLife,
    persona: Persona,
    persona_inner: PersonaInner,
    ear_sense: bool,
    /// Unit tests only. The word path cannot hold a topic, so a test can
    /// force the flag [`Self::turn_with_ear`] passes to `decide_held`.
    #[cfg(test)]
    forced_hold: Option<bool>,
    retrieval: Option<RetrievalThread>,
    retrieval_epoch: u64,
    dialogue: Dialogue,
    style: ResearchRoot,
    memory: Option<Box<dyn PersonMemory>>,
    person_id: String,
    memory_tick: u64,
    /// In-flight follow-up. The port lives in the worker until this yields.
    follow_rx: Option<Receiver<FollowDone>>,
    pending_follow: Option<ResearchFollowUp>,
}

/// A Galion Research result that arrived after the local clause.
///
/// It does not replace [`TurnOut::reply`] or [`TurnOut::first_clause`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchFollowUp {
    pub heard: String,
    pub outcome: AskOutcome,
    pub spoken: Option<String>,
    pub sources: Vec<String>,
    pub evidence: Vec<String>,
    pub gap: Gap,
}

/// Last clone drops the sender and lets the worker call [`ResearchAsk::ask`].
#[derive(Clone)]
pub(crate) struct FollowHook {
    release: Option<mpsc::Sender<()>>,
}

impl FollowHook {
    fn idle() -> Self {
        Self { release: None }
    }
}

impl PartialEq for FollowHook {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for FollowHook {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(if self.release.is_some() {
            "armed"
        } else {
            "idle"
        })
    }
}

struct FollowDone {
    question: String,
    outcome: AskOutcome,
    port: Box<dyn ResearchAsk + Send>,
}

struct Spoke {
    reply: String,
    sources: Vec<String>,
    evidence: Vec<String>,
    lookup_us: u32,
    picked: Option<horbm_brain::Answer>,
}

struct Gathered {
    spoke: Spoke,
    gap: Option<Gap>,
    /// Deep miss with a port attached. The ask waits until the clause is dropped.
    research_later: bool,
}

fn effective_reply(spoke: &Spoke) -> &str {
    if let Some(answer) = &spoke.picked {
        if !answer.clause.trim().is_empty() {
            return answer.clause.as_str();
        }
    }
    spoke.reply.as_str()
}

fn apply_plan(spoke: Spoke, plan: ReplyPlan) -> Spoke {
    match plan {
        ReplyPlan::Keep => spoke,
        ReplyPlan::Replace {
            reply,
            sources,
            evidence,
        } => Spoke {
            reply,
            sources,
            evidence,
            lookup_us: spoke.lookup_us,
            picked: None,
        },
    }
}

impl Spoke {
    fn empty() -> Self {
        Self {
            reply: String::new(),
            sources: Vec::new(),
            evidence: Vec::new(),
            lookup_us: 0,
            picked: None,
        }
    }
}

struct CardRetriever<'a> {
    cards: &'a dyn CardLookup,
    speakable: Speakable,
}

/// Style mark for comfort, ask, and offer_topic. The example text stays off
/// the reply and off the sources.
fn style_mark(chosen: Option<Move>, hits: &[ResearchHit]) -> Option<String> {
    match chosen {
        Some(Move::Comfort | Move::Ask | Move::OfferTopic) => {
            let hit = hits.first()?;
            if hit.allows_answer() || hit.allows_cite() || !hit.tagged_not_for_answer_and_cite() {
                return None;
            }
            Some(hit.id.clone())
        }
        _ => None,
    }
}

fn miss_spoke(lookup_us: u32) -> Spoke {
    Spoke {
        reply: UNKNOWN_REPLY.to_string(),
        sources: vec!["honest:unknown".to_string()],
        evidence: Vec::new(),
        lookup_us,
        picked: None,
    }
}

fn fixture_spoke(query: &str) -> Option<Spoke> {
    let answer = Heuristic::committed().answer(query);
    if !answer.hit {
        return None;
    }
    let source = format!("fixture:{}", answer.id);
    Some(Spoke {
        reply: String::new(),
        sources: vec![source.clone()],
        evidence: vec![source],
        lookup_us: 0,
        picked: Some(answer),
    })
}

impl Retriever for CardRetriever<'_> {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        if limit == 0 {
            return Vec::new();
        }
        let Some(hit) = self.cards.lookup(query) else {
            return Vec::new();
        };
        let Some(text) = (self.speakable)(&hit.gist) else {
            return Vec::new();
        };
        vec![Retrieved {
            id: hit.id,
            text,
            atomic: false,
            score: 1,
        }]
    }
}

impl Brain {
    /// Cards and an already loaded persona. Does not read configuration.
    /// Ear prosody starts on.
    pub fn with_persona(
        cards: impl CardLookup + Send + 'static,
        research: Option<Box<dyn ResearchAsk + Send>>,
        persona: Persona,
    ) -> Self {
        let card_count = cards.len();
        let (emotion, inner, persona) = install(persona);
        let persona_inner = persona_inner_from(&persona);
        Self {
            empathy: Empathy::new(),
            cards: Box::new(cards),
            research,
            speakable: accept_line,
            gaps: Vec::new(),
            meta: resting_meta(),
            card_count,
            emotion,
            inner,
            persona,
            persona_inner,
            ear_sense: true,
            #[cfg(test)]
            forced_hold: None,
            retrieval: None,
            retrieval_epoch: 0,
            dialogue: Dialogue::new(),
            style: ResearchRoot::empty(),
            memory: None,
            person_id: "person".to_string(),
            memory_tick: 0,
            follow_rx: None,
            pending_follow: None,
        }
    }

    /// [`Self::with_persona`] after loading the configured development persona.
    ///
    /// A blank id, a missing directory, or a missing file is the error.
    /// This does not panic.
    pub fn try_with_cards(
        cards: impl CardLookup + Send + 'static,
        research: Option<Box<dyn ResearchAsk + Send>>,
    ) -> Result<Self, PersonaError> {
        let mut brain = Self::with_persona(cards, research, development_persona()?);
        brain.apply_configured_ear();
        Ok(brain)
    }

    /// Same cards as [`Self::try_with_cards`].
    ///
    /// When configuration is present, the persona is that one. When it is
    /// missing, the stand-in fixture is installed. Ear prosody follows
    /// `GALION_EAR_SENSE`. This does not panic.
    pub fn with_cards(
        cards: impl CardLookup + Send + 'static,
        research: Option<Box<dyn ResearchAsk + Send>>,
    ) -> Self {
        let persona = development_persona().unwrap_or_else(|_| Persona::standin());
        let mut brain = Self::with_persona(cards, research, persona);
        brain.apply_configured_ear();
        brain
    }

    /// Dialogue cards. Empty unless a research root was attached. They are
    /// not a fact source.
    pub fn with_style(mut self, style: ResearchRoot) -> Self {
        self.style = style;
        self
    }

    /// Person diary. Replaces a diary already attached, including the denied
    /// one from [`Brain::open`].
    pub fn with_memory(mut self, memory: impl PersonMemory + 'static) -> Self {
        self.memory = Some(Box::new(memory));
        self
    }

    /// Whose diary this turn reads and writes. Empty is ignored.
    pub fn set_person(&mut self, person_id: &str) {
        let person_id = person_id.trim();
        if person_id.is_empty() {
            return;
        }
        self.person_id = person_id.to_string();
    }

    pub fn person_id(&self) -> &str {
        &self.person_id
    }

    /// The port the dialogue manager and the inner life call.
    pub fn person_memory(&mut self) -> Option<&mut (dyn PersonMemory + 'static)> {
        self.memory.as_deref_mut()
    }

    /// Chat turns read this thread's cell. `Idle` keeps the card and fixture
    /// paths. `Hits` and `Miss` do not call research. Not for `Heartbeat::step`.
    pub fn with_retrieval(mut self, thread: RetrievalThread) -> Self {
        self.retrieval = Some(thread);
        self
    }

    /// Hand learned rows to the attached retrieval thread.
    ///
    /// Forwards to [`RetrievalThread::set_learned`]. With no thread this
    /// returns `Ok(())` and stores nothing. [`Self::turn`] does not call it.
    ///
    /// The caller decides consent. Pass `rows` only while learn and remember
    /// are both granted, and pass an empty list on withdraw. This method
    /// does not read those grants. The host caller is R9 S132.
    pub fn attach_learned(&mut self, rows: Vec<LearnedRow>) -> Result<(), RetrieveError> {
        match self.retrieval.as_ref() {
            Some(thread) => thread.set_learned(rows),
            None => Ok(()),
        }
    }

    /// Latest cell, if a thread is attached. Waits on the cell lock.
    pub fn retrieval_cell(&self) -> Option<std::sync::Arc<MemoryHitCell>> {
        self.retrieval.as_ref().map(|thread| thread.cell())
    }

    /// Clauses loaded at `open` or `with_cards`. The example kettle directory is one.
    pub fn cards(&self) -> usize {
        self.card_count
    }

    /// Read the persona file once, then open cards the way [`Self::open`] does.
    ///
    /// `None` loads the configured development persona. An explicit path
    /// replaces it. An unreadable file, an invalid file, or a missing
    /// configuration returns the error and does not build a brain.
    pub fn boot(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        persona_file: Option<&Path>,
    ) -> Result<Self, PersonaError> {
        let persona = match persona_file {
            Some(path) => Persona::load(path)?,
            None => development_persona()?,
        };
        let mut brain = Self::open_with_persona(memory_db, knowledge_dir, cases, persona);
        brain.apply_configured_ear();
        Ok(brain)
    }

    /// [`Self::open_with_persona`] after loading the configured persona.
    /// A missing configuration is the error. This does not panic.
    pub fn try_open(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
    ) -> Result<Self, PersonaError> {
        let persona = development_persona()?;
        let mut brain = Self::open_with_persona(memory_db, knowledge_dir, cases, persona);
        brain.apply_configured_ear();
        Ok(brain)
    }

    /// Read `knowledge_dir` when it is already a directory. A missing path is
    /// an empty card set. Nothing is created. `memory_db` is the person diary.
    /// The file opens only when `GALION_HORBM_REMEMBER` is the word `grant`.
    /// `cases` is not opened.
    ///
    /// The persona is the configured one when that load succeeds, and the
    /// stand-in fixture when it does not. Ear prosody follows
    /// `GALION_EAR_SENSE`. This does not panic.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let persona = development_persona().unwrap_or_else(|_| Persona::standin());
        let mut brain = Self::open_with_persona(memory_db, knowledge_dir, cases, persona);
        brain.apply_configured_ear();
        brain
    }

    /// [`Self::open`] with an explicit remember flag.
    ///
    /// Does not read `GALION_HORBM_REMEMBER` or `GALION_EAR_SENSE`. Only the
    /// trimmed word `grant` opens the person file. Ear prosody stays on.
    pub fn open_with_remember_flag(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        remember: Option<&str>,
    ) -> Self {
        let persona = development_persona().unwrap_or_else(|_| Persona::standin());
        Self::open_loaded(memory_db, knowledge_dir, cases, persona, remember)
    }

    /// [`Self::open`] with an already loaded persona.
    ///
    /// The person diary follows `GALION_HORBM_REMEMBER`. Ear prosody is left
    /// as [`Self::with_persona`] set it.
    pub fn open_with_persona(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        persona: Persona,
    ) -> Self {
        let flag = Self::remember_flag_from_process();
        Self::open_loaded(memory_db, knowledge_dir, cases, persona, flag.as_deref())
    }

    /// Cards, persona, and a person diary from an explicit remember flag.
    /// Does not read `GALION_HORBM_REMEMBER`.
    fn open_loaded(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        persona: Persona,
        remember: Option<&str>,
    ) -> Self {
        let _ = cases;
        let mut brain = match crate::KnowledgeCards::load(knowledge_dir) {
            Ok(loaded) => {
                eprintln!("{}", loaded.report.log_line());
                Self::with_persona(loaded.cards, None, persona).with_style(ResearchRoot::from_env())
            }
            Err(crate::CardError::NotADirectory(_)) => {
                Self::with_persona(EmptyCards, None, persona).with_style(ResearchRoot::from_env())
            }
            Err(err) => {
                eprintln!("horbm cards: {err}");
                Self::with_persona(EmptyCards, None, persona).with_style(ResearchRoot::from_env())
            }
        };
        brain.memory = Some(Box::new(crate::EpisodicStore::open_from_flag(
            memory_db, remember,
        )));
        brain
    }

    pub fn set_speakable(&mut self, speakable: Speakable) {
        self.speakable = speakable;
    }

    pub fn note_name(&mut self, name: &str, class: PronounClass) {
        self.empathy.note_name(name, class);
        self.dialogue.note_name(name, class);
    }

    /// Layer 4 consent. Remember and learn start denied. Check-ins start off.
    /// This does not open the retrieval diary and does not write a file.
    pub fn set_dialogue_consent(&mut self, consent: DialogueConsent) {
        self.dialogue.set_consent(consent);
    }

    pub fn dialogue_consent(&self) -> DialogueConsent {
        self.dialogue.consent()
    }

    /// Store a callback. False when remember is denied. The text is not written.
    pub fn note_callback(&mut self, text: &str) -> bool {
        self.dialogue.note_callback(text)
    }

    pub fn callbacks(&self) -> &[String] {
        self.dialogue.callbacks()
    }

    pub fn check_ins(&self) -> bool {
        self.dialogue.check_ins()
    }

    /// The offline bandit. Off by default. Enabling it still does not count
    /// a move until learn consent is granted, and the count does not steer.
    pub fn set_bandit_enabled(&mut self, enabled: bool) {
        self.dialogue.set_bandit_enabled(enabled);
    }

    pub fn bandit_updates(&self) -> u32 {
        self.dialogue.bandit_updates()
    }

    pub fn bandit_counts(&self) -> [u32; 6] {
        self.dialogue.bandit_counts()
    }

    pub fn ring_len(&self) -> usize {
        self.dialogue.ring_len()
    }

    pub fn ring_get(&self, index: usize) -> Option<&TurnSnap> {
        self.dialogue.ring_get(index)
    }

    pub fn gaps(&self) -> &[Gap] {
        &self.gaps
    }

    pub fn mood_meta(&self) -> MoodMeta {
        self.meta
    }

    /// The speaker's moment. Empathy mood is [`Self::mood_meta`].
    pub fn emotion(&self) -> &EmotionCore {
        &self.emotion
    }

    /// Who is speaking. The moment starts on this baseline. Empathy, cards,
    /// and the inner life stay as they are. Later [`Self::step_moment`] calls
    /// still do not write the baseline.
    pub fn seat(&mut self, baseline: VoiceBaseline) {
        self.emotion = EmotionCore::from_baseline(baseline);
    }

    /// One 80 ms emotion frame. Does not change the empathy mood.
    pub fn step_moment(
        &mut self,
        intent: &IntentMsg,
    ) -> Result<MomentState, galion_emotion_core::CoreError> {
        self.emotion.step(intent)
    }

    /// The character loaded at boot. The moment step does not write it.
    pub fn persona(&self) -> &Persona {
        &self.persona
    }

    /// Legal-move port. The dialogue manager calls this with one candidate.
    pub fn admit_move(&self, claim: MoveClaim<'_>) -> MoveVerdict {
        self.persona.verdict(claim)
    }

    /// The token inner life. Memory is not stored here.
    pub fn inner(&self) -> &InnerLife {
        &self.inner
    }

    /// Slower goals for the installed persona. Session only. Not the baseline,
    /// not the moment.
    pub fn persona_inner(&self) -> &PersonaInner {
        &self.persona_inner
    }

    /// Ear switch for [`Self::turn`]. On unless `GALION_EAR_SENSE` closes it.
    /// Words still count when it is off.
    pub fn set_ear_sense(&mut self, on: bool) {
        self.ear_sense = on;
    }

    /// Whether ear prosody is applied on [`Self::turn_with_ear`].
    pub fn ear_sense(&self) -> bool {
        self.ear_sense
    }

    /// Same rule as the ear crate. Unset, empty, `1`, `on`, and `allow` are
    /// on. `0`, `off`, and `deny` are off. Trimmed, case-insensitive. This
    /// does not read the process environment.
    pub fn ear_sense_from_flag(flag: Option<&str>) -> bool {
        !matches!(
            flag.map(|value| value.trim().to_ascii_lowercase())
                .as_deref(),
            Some("0" | "off" | "deny")
        )
    }

    /// True when the installed persona is the one development boot names.
    pub fn is_configured_boot(&self) -> bool {
        BootConfig::from_process()
            .ok()
            .is_some_and(|cfg| self.persona.is_configured_boot(&cfg))
    }

    /// Pace and topic gate from the latest inner note.
    pub fn dialogue_lean(&self) -> DialogueLean {
        self.persona_inner.last().lean()
    }

    /// False while waiting or on a check-in. Do not open a new topic.
    pub fn may_open_topic(&self) -> bool {
        !self.dialogue_lean().hold_topic
    }

    /// One inner turn, then one moment frame, using the installed persona's note.
    ///
    /// Does not write the baseline. Does not keep `words`.
    pub fn persona_nudge(
        &mut self,
        words: &str,
        ear: Option<Heard>,
        sense: bool,
    ) -> Result<InnerMsg, galion_emotion_core::CoreError> {
        let seen = Perceived {
            t_ms: self.emotion.next_ms(),
            words,
            ear,
            sense,
        };
        let note = self.persona_inner.nudge(&mut self.emotion, seen)?;
        self.apply_prosody(note.lean());
        Ok(note)
    }

    /// One dialogue turn of cares, wants, and expects. Emits a GEMI intent
    /// and, when `inbound` is set, consumes one. Does not step the moment
    /// and does not write the baseline or the empathy mood.
    pub fn inner_turn(
        &mut self,
        t_ms: u32,
        dialogue: &str,
        inbound: Option<&IntentMsg>,
    ) -> InnerTurn {
        let point = self.emotion.moment().pos;
        let consent = self.emotion.consent();
        self.inner.turn(galion_emotion_core::TurnView {
            t_ms,
            dialogue: dialogue.as_bytes(),
            point,
            inbound,
            consent,
        })
    }

    /// [`Self::inner_turn`] on the next emotion frame, then
    /// [`Self::step_moment`] with that message. The existing port. The
    /// baseline and the empathy mood stay as they were.
    pub fn inner_nudge(
        &mut self,
        dialogue: &str,
        inbound: Option<&IntentMsg>,
    ) -> Result<InnerTurn, galion_emotion_core::CoreError> {
        self.inner
            .nudge(&mut self.emotion, dialogue.as_bytes(), inbound)
    }

    /// Snapshot for the conductor. The conductor does not own this state.
    pub fn conductor_cue(&self) -> ConductorCue {
        self.emotion.conductor_cue()
    }

    /// Open, met, and dropped counts from the caller's want list.
    ///
    /// Reads [`galion_sam_empathy::Empathy::view`]. Does not speak the counts
    /// and does not write the diary. A dropped want is closed and is not met.
    /// [`Self::inner_turn`] keeps a different store and is not included.
    pub fn want_counts(&self) -> WantCounts {
        let view = self.empathy.view();
        WantCounts {
            open: view.open,
            met: view.met,
            dropped: view.dropped,
        }
    }

    /// One 80 ms frame with no new words. Does not speak. Does not step the moment.
    pub fn tick_frame(&mut self) -> MoodMeta {
        let tick = self.empathy.tick(TickIn {
            inbound: None,
            asr: "",
            asr_final: false,
            invite_name: false,
            frame_ms: FRAME_MS,
        });
        self.meta = meta_of(&tick);
        let note = self.persona_inner.tick(FRAME_MS, true);
        self.apply_prosody(note.lean());
        self.meta
    }

    /// Empathy mood, then the person diary when `remember` is granted, then
    /// the retrieval cell when a thread is attached, then cards, then the
    /// fixture on an idle cell. Layer 4 then picks the move. The private
    /// finish step cuts an unpicked reply to the persona sentence budget,
    /// leaves the unknown line and an empty reply alone, and then splits
    /// the clause. A picked clause keeps its text. Galion Research is not
    /// asked here.
    /// A deep miss arms [`ResearchAsk`] so the ask starts when the returned
    /// value is dropped. [`Self::research_follow_up`] reads that result.
    ///
    /// A granted card hit speaks [`crate::MemoryHit::clause`]. A granted miss
    /// speaks [`crate::UNKNOWN_REPLY`] and does not call research. `Idle`
    /// means the truth-card `remember` bit is denied: that diary is not
    /// opened, and the committed fixture still answers when the cards miss.
    /// Person memory is a different file. It is spoken only for a question
    /// about the person, and never as the source of a world answer. This
    /// waits for the retrieval thread. Do not call it from `Heartbeat::step`.
    /// Barge-in is not this function. [`Self::turn`] passes no ear point.
    pub fn turn(&mut self, text: &str) -> TurnOut {
        self.turn_with_ear(text, None)
    }

    /// [`Self::turn`] with a listener point.
    ///
    /// `None` keeps the word path. The point counts only while
    /// [`Self::ear_sense`] is on. The spoken clause stays the lookup.
    /// After that clause is fixed, one [`Self::inner_turn`] runs and its
    /// result is not written into the reply, the first clause, the empathy
    /// mood, or the baseline.
    pub fn turn_with_ear(&mut self, text: &str, ear: Option<Heard>) -> TurnOut {
        self.reap_follow();
        let started = Instant::now();
        let heard = text.trim();
        let tick = self.empathy.tick(TickIn {
            inbound: None,
            asr: heard,
            asr_final: !heard.is_empty(),
            invite_name: false,
            frame_ms: FRAME_MS,
        });
        self.meta = meta_of(&tick);
        let note = self.persona_inner.turn(Perceived {
            t_ms: self.emotion.next_ms(),
            words: heard,
            ear,
            sense: self.ear_sense,
        });
        self.apply_prosody(note.lean());
        // A forget is not a fact. Archive before this turn is pushed, and
        // do not note the request into the diary.
        let forgotten = if self.memory_granted() && !heard.is_empty() {
            if let Some(ask) = crate::dialogue::forget_clause(heard) {
                self.archive_forgotten(ask)
            } else {
                let at = self.stamp();
                self.observe_memory(heard, &at);
                0
            }
        } else {
            0
        };
        let prep = self.empathy.prepare_lookup(heard);
        let query = if prep.query.trim().is_empty() {
            heard
        } else {
            prep.query.as_str()
        };
        if query.is_empty() {
            let out = self.finish(
                query,
                started,
                Spoke::empty(),
                &Decision::pass(),
                FollowHook::idle(),
            );
            return self.record_inner(heard, out);
        }
        let query_owned = query.to_string();
        let memory_hit = self.person_hit(heard);
        let gathered = match memory_hit {
            Some(spoke) => Gathered {
                spoke,
                gap: None,
                research_later: false,
            },
            None => self.gather(&query_owned),
        };
        let reply = effective_reply(&gathered.spoke).to_string();
        let sources = gathered.spoke.sources.clone();
        // An unset research root does not enter the index. The duplex soak
        // measures resident memory after warmup, and opening the search
        // library there looks like a leak.
        let style = if self.style.is_empty() {
            crate::research_style::StyleSearch::none()
        } else {
            self.style.search(StylePersona::Active, &query_owned)
        };
        let input = DecideIn {
            heard,
            intent: self.meta.intent,
            reply: &reply,
            sources: &sources,
            research: &style.hits,
            arousal: self.meta.arousal,
            valence: self.meta.valence,
            dominance: self.meta.dominance,
            e: self.meta.e,
        };
        // Held topics use `decide_held`. A clear topic stays on `decide`,
        // which is that call with the hold flag clear, so the entry stays live.
        let hold_topic = self.topic_hold(note.lean());
        let mut decision = if hold_topic {
            self.dialogue.decide_held(input, hold_topic)
        } else {
            self.dialogue.decide(input)
        };
        if let Some(chosen) = decision.chosen {
            let (spoken, spoken_sources) = match &decision.plan {
                ReplyPlan::Keep => (reply.as_str(), sources.as_slice()),
                ReplyPlan::Replace { reply, sources, .. } => (reply.as_str(), sources.as_slice()),
            };
            let claims_fact = matches!(chosen, Move::Answer | Move::Cite);
            let verdict = self.admit_move(MoveClaim {
                tag: chosen.as_str().as_bytes(),
                claims_fact,
                has_card: claims_fact && fact_backed(spoken, spoken_sources),
                sympathy_script: sympathy_script(spoken),
            });
            if verdict == MoveVerdict::Forbid {
                decision = Decision {
                    chosen: Some(Move::Ask),
                    plan: ReplyPlan::Replace {
                        reply: UNKNOWN_REPLY.to_string(),
                        sources: vec!["honest:unknown".to_string()],
                        evidence: Vec::new(),
                    },
                    propensity: 0.4,
                };
            }
        }
        if matches!(decision.plan(), ReplyPlan::Keep) {
            if let Some(gap) = gathered.gap {
                // A duplex prefetch asks on every partial. "where" and
                // "where is" miss before "where is the kettle" hits, and
                // nothing drains this queue. A long call kept every miss.
                // The newest misses are the ones a host can still act on.
                if self.gaps.len() == GAP_CAP {
                    self.gaps.remove(0);
                }
                self.gaps.push(gap);
            }
        }
        let research_later = gathered.research_later;
        let mut spoke = apply_plan(gathered.spoke, decision.plan().clone());
        if let Some(id) = style_mark(decision.chosen, &style.hits) {
            spoke.evidence.push(format!("style:{id}"));
        }
        if forgotten > 0 {
            self.speak_forgotten(&mut spoke, forgotten);
        }
        let mut out = self.finish(&query_owned, started, spoke, &decision, FollowHook::idle());
        if research_later {
            // The clause is already in `out`. The worker blocks until `out` is dropped.
            out.follow = self.arm_follow(&query_owned);
        }
        self.record_inner(heard, out)
    }

    /// The research result, if the worker has finished. Does not block.
    /// Does not change the clause already returned by [`Self::turn`].
    pub fn research_follow_up(&mut self) -> Option<ResearchFollowUp> {
        self.reap_follow();
        self.pending_follow.take()
    }

    /// Person diary hit, only when the line is about the person. A world
    /// question still reads the diary and then drops the hits.
    fn person_hit(&mut self, heard: &str) -> Option<Spoke> {
        if !self.memory_granted() {
            return None;
        }
        let query = crate::dialogue::recall_clause(heard);
        // Search the words after the cue. "do you remember my sister" would
        // otherwise require "you" and "remember" to be in the stored fact.
        let recalled = self.recall_person(crate::dialogue::recall_topic(query));
        if !about_the_person(query) {
            return None;
        }
        self.memory_spoke(query, &recalled)
    }

    /// Retrieval, cards, the idle fixture, then the gateway. No reply split.
    fn gather(&mut self, query: &str) -> Gathered {
        if let Some(spoke) = self.retrieval_spoke(query) {
            return Gathered {
                spoke,
                gap: None,
                research_later: false,
            };
        }
        let lookup_started = Instant::now();
        let picked = pick(
            query,
            &CardRetriever {
                cards: self.cards.as_ref(),
                speakable: self.speakable,
            },
            UNKNOWN_REPLY,
            "horbm-say-i-dont-know",
        );
        let lookup_us = elapsed_us(lookup_started.elapsed());
        if picked.hit {
            let source = format!("card:{}", picked.id);
            return Gathered {
                spoke: Spoke {
                    reply: String::new(),
                    sources: vec![source.clone()],
                    evidence: vec![source],
                    lookup_us,
                    picked: Some(picked),
                },
                gap: None,
                research_later: false,
            };
        }
        if self.retrieval.is_some() {
            if let Some(spoke) = fixture_spoke(query) {
                return Gathered {
                    spoke,
                    gap: None,
                    research_later: false,
                };
            }
        }
        // Comfort stays with them. The research ask does not open.
        // A payment is not a task and is not armed. The four phrases are
        // `is_money_action`, the same list as
        // `crates/galion-conductor/src/research.rs`. A question mark skips
        // that list and leaves `is_deep_question` in charge.
        // Every other miss stays local too. A deep miss is armed after the clause.
        let comfort = self.dialogue_lean().verb == Verb::Comfort;
        let routed = route_heard(query, self.cards.as_ref(), None, self.speakable);
        let research_later = self.research.is_some()
            && !comfort
            && !is_money_action(query)
            && routed
                .gap
                .as_ref()
                .is_some_and(|gap| gap.reason == GapReason::ResearchOff);
        Gathered {
            gap: routed.gap,
            spoke: Spoke {
                reply: routed.spoken,
                sources: routed.sources,
                evidence: routed.evidence,
                lookup_us: routed.lookup_us,
                picked: None,
            },
            research_later,
        }
    }

    fn memory_granted(&self) -> bool {
        self.memory
            .as_ref()
            .map(|memory| memory.remember_granted())
            .unwrap_or(false)
    }

    fn stamp(&mut self) -> String {
        self.memory_tick = self.memory_tick.saturating_add(1);
        format!("t{:06}", self.memory_tick)
    }

    // Drop what `ask` names. The count is how many rows were archived.
    // A taught `learned:<id>` snap is left for LearnPass::retract_clause.
    fn archive_forgotten(&mut self, ask: ForgetAsk<'_>) -> usize {
        match ask {
            ForgetAsk::About(topic) => {
                let topic = topic.trim();
                if topic.is_empty() {
                    return 0;
                }
                let person = self.person_id.clone();
                let Some(memory) = self.memory.as_mut() else {
                    return 0;
                };
                memory.forget_matching(&person, topic).len()
            }
            ForgetAsk::That => self.forget_newest_memory(),
        }
    }

    // The newest snap is the previous turn: this turn has not been pushed.
    // `memory:<id>` archives that one row. `learned:<id>` archives nothing.
    fn forget_newest_memory(&mut self) -> usize {
        let len = self.ring_len();
        if len == 0 {
            return 0;
        }
        let source = self.ring_get(len - 1).and_then(|snap| snap.source.clone());
        let Some(source) = source else {
            return 0;
        };
        if source.starts_with("learned:") {
            return 0;
        }
        let Some(id) = source
            .strip_prefix("memory:")
            .filter(|id| !id.is_empty() && *id != "callback")
        else {
            return 0;
        };
        let id = id.to_string();
        let person = self.person_id.clone();
        let Some(memory) = self.memory.as_mut() else {
            return 0;
        };
        match memory.forget(&person, &id) {
            WriteOutcome::Archived { .. } => 1,
            _ => 0,
        }
    }

    // The move stays the ask the policy already chose. The source names
    // the count only: not the text, and not an id.
    fn speak_forgotten(&self, spoke: &mut Spoke, archived: usize) {
        let line = self
            .persona
            .line(horbm_persona::PersonaLine::Forgot)
            .unwrap_or(UNKNOWN_REPLY);
        spoke.reply = line.to_string();
        spoke.sources = vec![format!("forget:{archived}")];
        spoke.evidence.clear();
        spoke.picked = None;
    }

    fn observe_memory(&mut self, heard: &str, at: &str) {
        if heard.is_empty() || !self.memory_granted() {
            return;
        }
        let person = self.person_id.clone();
        let state = person_state(&self.empathy, self.meta);
        let Some(memory) = self.memory.as_mut() else {
            return;
        };
        memory.note_turn(&person, &person, heard, at);
        if let Some(text) = pin_command(heard) {
            memory.pin(&person, text, at);
        }
        memory.note_state(&person, &state, at);
    }

    fn recall_person(&mut self, heard: &str) -> Vec<MemoryItem> {
        if !self.memory_granted() {
            return Vec::new();
        }
        let person = self.person_id.clone();
        let Some(memory) = self.memory.as_mut() else {
            return Vec::new();
        };
        memory.recall(&person, heard)
    }

    /// Spoken only when `query` is about the person. The caller drops `hits`
    /// for a world question, so they never become sources.
    fn memory_spoke(&self, query: &str, hits: &[MemoryItem]) -> Option<Spoke> {
        if !about_the_person(query) {
            return None;
        }
        let hit = hits.iter().find(|hit| !hit.text.trim().is_empty())?;
        let spoken = (self.speakable)(hit.text.trim())?;
        if spoken.trim().is_empty() {
            return None;
        }
        let (first, rest) = split_clause(&spoken, false);
        let source = format!("memory:{}", hit.id);
        Some(Spoke {
            reply: String::new(),
            sources: vec![source.clone()],
            evidence: vec![source],
            lookup_us: 0,
            picked: Some(Answer {
                id: hit.id.clone(),
                clause: spoken,
                first,
                rest,
                hit: true,
                mark: classify(query).map(|kind| SidebandMark {
                    kind,
                    posted: false,
                }),
            }),
        })
    }

    /// `Some` for `Hits` and `Miss`. `None` for `Idle`, which is deny.
    fn retrieval_spoke(&mut self, query: &str) -> Option<Spoke> {
        let thread = self.retrieval.as_ref()?;
        self.retrieval_epoch = self.retrieval_epoch.saturating_add(1);
        let epoch = self.retrieval_epoch;
        if thread.ask_wait(query, epoch, epoch).is_err() {
            return Some(miss_spoke(0));
        }
        let cell = thread.cell();
        match cell.state {
            CellState::Idle => None,
            CellState::Miss => Some(miss_spoke(cell.took_us)),
            CellState::Hits => Some(self.hit_spoke(query, cell.as_ref())),
        }
    }

    fn hit_spoke(&self, query: &str, cell: &MemoryHitCell) -> Spoke {
        let Some(hit) = cell.hits.first() else {
            return miss_spoke(cell.took_us);
        };
        let clause = hit.clause().trim();
        if clause.is_empty() {
            return miss_spoke(cell.took_us);
        }
        let Some(spoken) = (self.speakable)(clause) else {
            return miss_spoke(cell.took_us);
        };
        if spoken.trim().is_empty() {
            return miss_spoke(cell.took_us);
        }
        let (first, rest) = split_clause(&spoken, false);
        // A learned cell source is `learned;<label>`. The spoken source is
        // `learned:<diary id>`. Every other hit stays `retrieve:<item id>`.
        let learned = hit.source.starts_with("learned;");
        let source = if learned {
            format!("learned:{}", hit.item_id)
        } else {
            format!("retrieve:{}", hit.item_id)
        };
        let evidence = if learned {
            vec![source.clone(), hit.source.clone()]
        } else {
            vec![source.clone()]
        };
        Spoke {
            reply: String::new(),
            sources: vec![source],
            evidence,
            lookup_us: cell.took_us,
            picked: Some(Answer {
                id: hit.item_id.clone(),
                clause: spoken,
                first,
                rest,
                hit: true,
                mark: classify(query).map(|kind| SidebandMark {
                    kind,
                    posted: false,
                }),
            }),
        }
    }

    fn finish(
        &self,
        query: &str,
        started: Instant,
        spoke: Spoke,
        decision: &Decision,
        follow: FollowHook,
    ) -> TurnOut {
        let Spoke {
            reply,
            sources,
            evidence,
            lookup_us,
            picked,
        } = spoke;
        let (reply, first_clause, rest, sideband) = match picked {
            Some(answer) => (answer.clause, answer.first, answer.rest, answer.mark),
            None => {
                // Picked clauses are the other arm and keep their text.
                // The unknown line and an empty reply are not cut.
                let reply = if reply == UNKNOWN_REPLY || reply.is_empty() {
                    reply
                } else {
                    self.persona
                        .style()
                        .cap_sentences(&reply)
                        .trim_end()
                        .to_string()
                };
                let atomic = reply == UNKNOWN_REPLY || reply.is_empty();
                let (first, rest) = split_clause(&reply, atomic);
                let mark = if reply.is_empty() {
                    None
                } else {
                    classify(query).map(|kind| SidebandMark {
                        kind,
                        posted: false,
                    })
                };
                (reply, first, rest, mark)
            }
        };
        TurnOut {
            reply,
            intent: self.meta.intent.to_string(),
            arm: match decision.chosen {
                Some(move_) => move_.as_str().to_string(),
                None => "none".to_string(),
            },
            propensity: decision.propensity,
            evidence,
            sources,
            violations: 0,
            lookup_us,
            took_us: elapsed_us(started.elapsed()),
            mood: self.meta.pad(),
            mood_meta: self.meta,
            first_clause,
            rest,
            sideband,
            follow,
        }
    }

    /// Spawn the ask, blocked until the clause value is dropped.
    fn arm_follow(&mut self, question: &str) -> FollowHook {
        let Some(mut port) = self.research.take() else {
            return FollowHook::idle();
        };
        let question = question.trim().to_string();
        let (release_tx, release_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        thread::Builder::new()
            .name("horbm-research".into())
            .spawn(move || {
                let _ = release_rx.recv();
                let outcome = port.ask(&question, RESEARCH_CAP);
                let _ = done_tx.send(FollowDone {
                    question,
                    outcome,
                    port,
                });
            })
            .expect("horbm research follow-up");
        self.follow_rx = Some(done_rx);
        FollowHook {
            release: Some(release_tx),
        }
    }

    fn reap_follow(&mut self) {
        let received = {
            let Some(rx) = self.follow_rx.as_ref() else {
                return;
            };
            match rx.try_recv() {
                Ok(done) => Some(done),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => None,
            }
        };
        self.follow_rx = None;
        let Some(done) = received else {
            return;
        };
        let class = classify_ask(done.outcome.clone(), self.speakable);
        let gap = Gap {
            heard: done.question.clone(),
            reason: class.reason,
            advice: crate::GAP_ADVICE,
        };
        self.note_follow_gap(gap.clone());
        self.research = Some(done.port);
        self.pending_follow = Some(ResearchFollowUp {
            heard: done.question,
            outcome: done.outcome,
            spoken: class.spoken,
            sources: class.sources,
            evidence: class.evidence,
            gap,
        });
    }

    fn note_follow_gap(&mut self, gap: Gap) {
        if let Some(slot) = self
            .gaps
            .iter_mut()
            .rev()
            .find(|item| item.heard == gap.heard && item.reason == GapReason::ResearchOff)
        {
            *slot = gap;
            return;
        }
        if self.gaps.len() == GAP_CAP {
            self.gaps.remove(0);
        }
        self.gaps.push(gap);
    }

    /// The reply and the first clause are already fixed. One inner turn
    /// updates cares, wants, and expects. Its result is dropped.
    fn record_inner(&mut self, heard: &str, out: TurnOut) -> TurnOut {
        let _ignored = self.inner_turn(self.emotion.next_ms(), heard, None);
        out
    }

    fn topic_hold(&self, lean: DialogueLean) -> bool {
        #[cfg(test)]
        if let Some(forced) = self.forced_hold {
            return forced;
        }
        lean.hold_topic
    }

    fn apply_configured_ear(&mut self) {
        self.ear_sense = Self::ear_sense_from_process();
    }

    fn ear_sense_from_process() -> bool {
        match std::env::var(EAR_SENSE_ENV) {
            Ok(value) => Self::ear_sense_from_flag(Some(&value)),
            Err(_) => Self::ear_sense_from_flag(None),
        }
    }

    fn remember_flag_from_process() -> Option<String> {
        std::env::var(REMEMBER_ENV).ok()
    }
}

fn person_state(empathy: &Empathy, meta: MoodMeta) -> PersonState {
    PersonState {
        names: empathy
            .names()
            .people()
            .map(|(name, class)| TrackedName {
                name: name.to_string(),
                class_label: class.as_str().to_string(),
            })
            .collect(),
        arousal: meta.arousal,
        valence: meta.valence,
        dominance: meta.dominance,
        feeling: meta.named.map(|word| word.to_string()),
    }
}

fn apply_prosody(meta: &mut MoodMeta, lean: DialogueLean) {
    meta.rate = (meta.rate * lean.rate).clamp(0.80, 1.30);
    meta.pause_ms = (meta.pause_ms + lean.pause_ms).clamp(0.0, 400.0);
}

impl Brain {
    fn apply_prosody(&mut self, lean: DialogueLean) {
        apply_prosody(&mut self.meta, lean);
    }
}

/// Configured development persona.
///
/// `GALION_HORBM_DEV_PERSONA` names the id when it is non-empty. Otherwise
/// `development.toml` does. The file is `<persona dir>/<id>.toml`. The
/// directory is `GALION_HORBM_PERSONA_DIR`, otherwise `persona_dir` in the
/// boot file. A blank id, a missing directory, or a missing file is an error.
fn development_persona() -> Result<Persona, PersonaError> {
    Persona::load_development()
}

fn install(persona: Persona) -> (EmotionCore, InnerLife, Persona) {
    let emotion = EmotionCore::from_baseline(persona.baseline());
    let inner = persona.inner_life();
    (emotion, inner, persona)
}

/// Inner home from this persona's baseline valence. Curiosity, help, and
/// expect stay the standalone levels. An id the slot refuses, or a baseline
/// `from_baseline` refuses, keeps those numbers when it can and an empty id.
/// It never installs a different persona.
fn persona_inner_from(persona: &Persona) -> PersonaInner {
    let baseline = persona.baseline();
    let Some(home) =
        InnerHome::from_baseline(baseline.valence(), CURIOSITY_HOME, HELP_HOME, EXPECT_HOME)
    else {
        return PersonaInner::new();
    };
    PersonaInner::with_home_for(persona.id(), home).unwrap_or_else(|| PersonaInner::with_home(home))
}

fn resting_meta() -> MoodMeta {
    let tick = Empathy::new().tick(TickIn {
        inbound: None,
        asr: "",
        asr_final: false,
        invite_name: false,
        frame_ms: FRAME_MS,
    });
    meta_of(&tick)
}

fn meta_of(tick: &galion_sam_empathy::TickOut) -> MoodMeta {
    MoodMeta {
        arousal: tick.mood.arousal,
        valence: tick.mood.valence,
        dominance: tick.mood.dominance,
        e: tick.e,
        intent: tick.intent.as_str(),
        rate: tick.prosody.speaking.rate,
        energy: tick.prosody.speaking.energy,
        pitch_st: tick.prosody.speaking.pitch_shift_st,
        pause_ms: tick.prosody.speaking.pre_pause_ms,
        named: tick.named.map(|name| name.word()),
    }
}

fn fact_backed(reply: &str, sources: &[String]) -> bool {
    let reply = reply.trim();
    if reply.is_empty() || reply == UNKNOWN_REPLY {
        return false;
    }
    sources.iter().any(|source| {
        ["card:", "retrieve:", "fixture:", "research:", "learned:"]
            .iter()
            .any(|prefix| source.starts_with(prefix) && source.len() > prefix.len())
    })
}

fn sympathy_script(reply: &str) -> bool {
    let trimmed = reply.trim().trim_end_matches(['.', '!', '?']).trim();
    matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "i hear you"
            | "that sounds so hard"
            | "i'm here for you no matter what"
            | "i feel that in my soul"
            | "that sounds hard"
            | "i'm so sorry you're going through this"
            | "let's slow down"
            | "i know how you feel"
            | "that must be so difficult"
    )
}

fn elapsed_us(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AskOutcome, CardHit, CardLookup, GapReason, MarkKind, ResearchAsk, SidebandMark,
        UNKNOWN_REPLY,
    };
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct Cards {
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl CardLookup for Cards {
        fn lookup(&self, query: &str) -> Option<CardHit> {
            self.seen.lock().expect("seen").push(query.to_string());
            if query.to_ascii_lowercase().contains("kettle") {
                Some(CardHit {
                    id: "kettle".into(),
                    gist: "The kettle is on the hob.".into(),
                })
            } else {
                None
            }
        }
    }

    fn cards() -> Cards {
        Cards {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    struct Rec {
        calls: Arc<AtomicU32>,
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl ResearchAsk for Rec {
        fn ask(&mut self, question: &str, cap: Duration) -> AskOutcome {
            assert_eq!(cap, crate::RESEARCH_CAP);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().expect("seen").push(question.to_string());
            AskOutcome::Timeout
        }
    }

    #[test]
    fn open_does_not_create_a_directory() {
        let missing = std::env::temp_dir().join(format!(
            "horbm-open-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        assert!(!missing.exists());
        let mut brain = Brain::open(&missing, &missing.join("knowledge"), &missing.join("cases"));
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, UNKNOWN_REPLY);
        assert!(!missing.exists());
    }

    #[test]
    fn tired_miss_keeps_the_exact_line_and_passes_the_mood() {
        let mut brain = Brain::with_cards(cards(), None);
        let out = brain.turn("I am so tired");
        assert_eq!(out.reply, "I don't know.");
        assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
        assert!(out.mood_meta.e > 0.0);
        assert!(out.mood.valence > 0.0);
        assert_eq!(out.mood.arousal, out.mood_meta.arousal);
        assert!(out.mood_meta.named.is_none());
        assert!(!out.reply.to_ascii_lowercase().contains("tired"));
        assert!(!out.reply.to_ascii_lowercase().contains("sorry"));
        assert_eq!(brain.gaps()[0].reason, GapReason::NoCard);
    }

    #[test]
    fn the_gap_queue_keeps_the_newest_misses() {
        let mut brain = Brain::with_cards(cards(), None);
        for n in 0..40 {
            brain.turn(&format!("missing number {n}"));
        }
        assert_eq!(brain.gaps().len(), GAP_CAP);
        assert_eq!(brain.gaps()[0].heard, "missing number 8");
        assert_eq!(
            brain.gaps().last().expect("newest").heard,
            "missing number 39"
        );
    }

    #[test]
    fn a_card_hit_keeps_the_mood_and_does_not_ask() {
        let calls = Arc::new(AtomicU32::new(0));
        let mut brain = Brain::with_cards(
            cards(),
            Some(Box::new(Rec {
                calls: Arc::clone(&calls),
                seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            })),
        );
        let out = brain.turn("I am so tired where is the kettle");
        assert_eq!(out.reply, "The kettle is on the hob.");
        assert_eq!(out.sources, vec!["card:kettle".to_string()]);
        assert!(out.mood_meta.e > 0.0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(brain.gaps().is_empty());
    }

    #[test]
    fn a_name_is_rewritten_before_lookup() {
        let looked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut brain = Brain::with_cards(
            Cards {
                seen: Arc::clone(&looked),
            },
            None,
        );
        brain.note_name("Maciej", PronounClass::He);
        let out = brain.turn("why did he move the kettle?");
        assert_eq!(out.reply, "The kettle is on the hob.");
        let queries = looked.lock().unwrap().clone();
        assert!(queries.iter().any(|q| q.contains("Maciej")), "{queries:?}");
        assert!(queries.iter().all(|q| !q.contains(" he ")));
    }

    #[test]
    fn a_card_hit_is_the_stored_clause_with_a_two_word_lead() {
        let mut brain = Brain::with_cards(cards(), None);
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, "The kettle is on the hob.");
        assert_eq!(out.first_clause, "The kettle");
        assert!(out.first_clause.split_whitespace().count() <= 2);
        assert_eq!(out.rest, "is on the hob.");
        assert_eq!(out.sources, vec!["card:kettle".to_string()]);
        match out.sideband {
            Some(SidebandMark {
                kind: MarkKind::Lookup { query },
                posted: false,
            }) => assert!(query.to_ascii_lowercase().contains("kettle"), "{query}"),
            other => panic!("lookup was not marked: {other:?}"),
        }
        assert_eq!(
            horbm_brain::Heuristic::committed().miss_text(),
            UNKNOWN_REPLY
        );
    }

    /// The word path does not hold a topic, so the flag is forced. A hold
    /// turns the offer into ask and does not speak the topic line.
    #[test]
    fn a_hold_blocks_offer_topic_in_the_turn() {
        let mut open = Brain::with_cards(cards(), None);
        let primed = open.turn("where is the kettle");
        assert_eq!(primed.reply, "The kettle is on the hob.");
        let offered = open.turn("something else");
        assert_eq!(offered.reply, "Back to kettle?");
        assert_eq!(offered.arm, "offer_topic");

        let mut held = Brain::with_cards(cards(), None);
        let primed = held.turn("where is the kettle");
        assert_eq!(primed.reply, "The kettle is on the hob.");
        held.forced_hold = Some(true);
        let blocked = held.turn("something else");
        assert_eq!(blocked.arm, "ask");
        assert_eq!(blocked.reply, UNKNOWN_REPLY);
        assert!(!blocked.reply.contains("Back to"));
        assert!(!blocked.first_clause.contains("Back to"));
    }

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horbm-turn-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn no_diary_files(dir: &std::path::Path) {
        let entries = std::fs::read_dir(dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains("horbm-mem"), "{name}");
            assert!(
                !name.ends_with(".sqlite")
                    && !name.ends_with(".sqlite-wal")
                    && !name.ends_with(".sqlite-shm")
                    && !name.ends_with(".db"),
                "{name}"
            );
        }
        assert!(!dir.join("revisions.sqlite").exists());
    }

    /// Gate. A granted RAM fixture answers through the retrieval cell.
    #[test]
    fn a_granted_turn_returns_the_fixture_clause_from_the_cell() {
        let scratch = scratch_dir("grant");
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: true,
            knowledge_dir: Some(scratch.clone()),
            fixture_rows: 1_000,
            keys_path: None,
        })
        .unwrap();
        let mut brain = Brain::with_cards(EmptyCards, None).with_retrieval(thread);
        let out = brain.turn("m742");
        let cell = brain.retrieval_cell().unwrap();
        assert_eq!(cell.state, crate::CellState::Hits);
        assert!(cell.answer.is_empty(), "{}", cell.answer);
        let hit = cell
            .hits
            .iter()
            .find(|hit| hit.item_id == "sample-742")
            .expect("sample-742");
        assert_eq!(out.reply, hit.clause());
        assert!(out.reply.contains("marker m742"), "{}", out.reply);
        assert!(out.first_clause.split_whitespace().count() <= 2);
        assert_eq!(out.sources, vec!["retrieve:sample-742".to_string()]);
        no_diary_files(&scratch);
        let _ = std::fs::remove_dir_all(scratch);
    }

    /// A granted miss is the persona line, not a card that would otherwise hit.
    #[test]
    fn a_granted_miss_does_not_fall_through_to_cards_or_research() {
        let calls = Arc::new(AtomicU32::new(0));
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: true,
            knowledge_dir: None,
            fixture_rows: 1_000,
            keys_path: None,
        })
        .unwrap();
        let mut brain = Brain::with_cards(
            cards(),
            Some(Box::new(Rec {
                calls: Arc::clone(&calls),
                seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            })),
        )
        .with_retrieval(thread);
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, "I don't know.");
        assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
        assert_eq!(
            brain.retrieval_cell().unwrap().state,
            crate::CellState::Miss
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(brain.gaps().is_empty());
    }

    /// Gate. `remember` denied keeps the fixture path and does not open the diary.
    #[test]
    fn a_denied_turn_still_speaks_the_clause_fixture() {
        let scratch = scratch_dir("deny");
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: false,
            knowledge_dir: Some(scratch.clone()),
            fixture_rows: 1_000,
            keys_path: None,
        })
        .unwrap();
        let mut brain = Brain::with_cards(EmptyCards, None).with_retrieval(thread);
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, "The kettle is on the hob.");
        assert_eq!(out.first_clause, "The kettle");
        assert_eq!(out.rest, "is on the hob.");
        assert_eq!(out.sources, vec!["fixture:kettle".to_string()]);
        let cell = brain.retrieval_cell().unwrap();
        assert_eq!(cell.state, crate::CellState::Idle);
        assert!(cell.is_empty());
        no_diary_files(&scratch);
        let _ = std::fs::remove_dir_all(scratch);
    }

    /// Idle still prefers a loaded card over the fixture.
    #[test]
    fn a_denied_turn_with_cards_keeps_the_card_clause() {
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: false,
            knowledge_dir: None,
            fixture_rows: 0,
            keys_path: None,
        })
        .unwrap();
        struct Stove;
        impl CardLookup for Stove {
            fn lookup(&self, query: &str) -> Option<CardHit> {
                if query.to_ascii_lowercase().contains("kettle") {
                    Some(CardHit {
                        id: "kettle".into(),
                        gist: "The kettle is on the stove.".into(),
                    })
                } else {
                    None
                }
            }
        }
        let mut brain = Brain::with_cards(Stove, None).with_retrieval(thread);
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, "The kettle is on the stove.");
        assert_eq!(out.sources, vec!["card:kettle".to_string()]);
        assert_eq!(
            brain.retrieval_cell().unwrap().state,
            crate::CellState::Idle
        );
    }

    fn bowl_card() -> String {
        r#"{
  "id": "bowl",
  "title": "The blue bowl is in the cupboard.",
  "section": "horbm",
  "category": "fixture",
  "summary": "The blue bowl is in the cupboard.",
  "body": "The blue bowl is in the cupboard.",
  "claim_type": "fact",
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "horbm",
  "as_of_date": "2026-11-25",
  "status": "draft",
  "contested_views": [],
  "related": []
}
"#
        .to_string()
    }

    fn bowl_dirs() -> (std::path::PathBuf, std::path::PathBuf) {
        let root = scratch_dir("bowl");
        let cards = root.join("cards");
        std::fs::create_dir(&cards).unwrap();
        std::fs::write(cards.join("bowl.json"), bowl_card()).unwrap();
        let mut lane = [0i8; 8];
        horbm_brain::encode_query_into("where is the blue bowl", &mut lane);
        let pack = horbm_brain::StoredPack {
            k: 8,
            keys: lane.to_vec(),
            clause_id: vec![1],
            allow: vec![true],
            source: vec![1],
            item_id: vec!["bowl".to_string()],
            scale: vec![1],
        };
        let keys = root.join("keys.hkb");
        pack.write(&keys).unwrap();
        (cards, keys)
    }

    /// The scan is the turn's retrieval. A fixture built in the test scores.
    /// A query that covers nothing stays the persona miss.
    #[test]
    fn a_scanned_turn_speaks_the_fixture_and_a_miss_stays_i_dont_know() {
        let (cards, keys) = bowl_dirs();
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: true,
            knowledge_dir: Some(cards.clone()),
            fixture_rows: 0,
            keys_path: Some(keys.clone()),
        })
        .unwrap();
        let mut brain = Brain::with_cards(EmptyCards, None).with_retrieval(thread);
        let out = brain.turn("where is the blue bowl");
        let cell = brain.retrieval_cell().unwrap();
        assert_eq!(cell.state, crate::CellState::Hits);
        assert!(cell.answer.is_empty(), "{}", cell.answer);
        assert!(
            cell.hits
                .iter()
                .any(|hit| hit.item_id == "bowl" && hit.pool == crate::HitPool::Memory),
            "{:?}",
            cell.hits
        );
        assert!(
            cell.hits
                .iter()
                .any(|hit| hit.item_id == "bowl" && hit.pool == crate::HitPool::Knowledge),
            "{:?}",
            cell.hits
        );
        assert_eq!(out.reply, "The blue bowl is in the cupboard.");
        assert_eq!(out.sources, vec!["retrieve:bowl".to_string()]);
        assert!(out.first_clause.split_whitespace().count() <= 2);

        let missed = brain.turn("zzznomatchtoken");
        assert_eq!(missed.reply, "I don't know.");
        assert_eq!(missed.sources, vec!["honest:unknown".to_string()]);
        let miss = brain.retrieval_cell().unwrap();
        assert_eq!(miss.state, crate::CellState::Miss);
        assert!(miss.hits.is_empty());
        assert_eq!(miss.answer, "I don't know.");
        let _ = std::fs::remove_dir_all(cards.parent().unwrap());
    }

    /// `remember` denied does not open the diary or the key file.
    #[test]
    fn a_denied_turn_does_not_speak_the_scanned_diary() {
        let (cards, keys) = bowl_dirs();
        let thread = crate::RetrievalThread::spawn(crate::RetrievalConfig {
            remember: false,
            knowledge_dir: Some(cards.clone()),
            fixture_rows: 0,
            keys_path: Some(keys),
        })
        .unwrap();
        let mut brain = Brain::with_cards(EmptyCards, None).with_retrieval(thread);
        let out = brain.turn("where is the blue bowl");
        assert_ne!(out.reply, "The blue bowl is in the cupboard.");
        assert_eq!(out.reply, "I don't know.");
        let cell = brain.retrieval_cell().unwrap();
        assert_eq!(cell.state, crate::CellState::Idle);
        assert!(cell.is_empty());
        assert!(cell.hits.iter().all(|hit| hit.item_id != "bowl"));
        let _ = std::fs::remove_dir_all(cards.parent().unwrap());
    }

    #[test]
    fn a_learned_source_is_fact_backed_when_the_id_is_present() {
        let taught = "You remember the spare oar is in the locker.";
        let learned = vec!["learned:diary-1".to_string()];
        assert!(fact_backed(taught, &learned));
        assert!(!fact_backed(taught, &["learned:".to_string()]));
        assert!(!fact_backed(UNKNOWN_REPLY, &learned));
        assert!(!fact_backed("", &learned));
        assert!(fact_backed(
            "The kettle is on the hob.",
            &["retrieve:kettle".to_string()]
        ));
        assert!(fact_backed(
            "The kettle is on the hob.",
            &["card:kettle".to_string()]
        ));
    }
}
