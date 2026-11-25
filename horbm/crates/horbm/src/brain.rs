// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Empathy, then the layer 5 route, then the mood the voice speaks with.

use std::path::Path;
use std::time::Instant;

use galion_emotion_core::{
    ConductorCue, EmotionCore, InnerLife, InnerTurn, IntentMsg, MomentState,
};
use galion_sam_empathy::{Empathy, PronounClass, TickIn, FRAME_MS};
use horbm_persona::{MoveClaim, MoveVerdict, Persona, PersonaError};

use horbm_brain::{
    classify, pick, split_clause, Answer, Heuristic, Retrieved, Retriever, SidebandMark,
};
use horbm_inner::{DialogueLean, Heard, InnerMsg, Perceived, SamanthaInner, Verb};

use crate::cards::ResearchRoot;
use crate::dialogue::{DecideIn, Decision, Dialogue, DialogueConsent, Move, ReplyPlan, TurnSnap};
use crate::retrieve::{CellState, MemoryHitCell, RetrievalThread, RetrievedCells};
use crate::route::{accept_line, route_heard, CardLookup, Gap, ResearchAsk, Speakable};
use crate::{Pad, TurnOut, UNKNOWN_REPLY};

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

/// The linked brain. `open` reads the knowledge directory when it already
/// exists and does not create it. Memory and case paths are not opened.
/// The live server passes the same read-only loader and may attach a
/// [`RetrievalThread`]. That thread is not started here.
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
    samantha: SamanthaInner,
    ear_sense: bool,
    retrieval: Option<RetrievalThread>,
    retrieval_epoch: u64,
    dialogue: Dialogue,
    style: ResearchRoot,
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
    pub fn with_cards(
        cards: impl CardLookup + Send + 'static,
        research: Option<Box<dyn ResearchAsk + Send>>,
    ) -> Self {
        let card_count = cards.len();
        let (emotion, inner, persona) = install(development_persona());
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
            samantha: SamanthaInner::new(),
            ear_sense: false,
            retrieval: None,
            retrieval_epoch: 0,
            dialogue: Dialogue::new(),
            style: ResearchRoot::empty(),
        }
    }

    /// Dialogue cards. Empty unless a research root was attached. They are
    /// not a fact source.
    pub fn with_style(mut self, style: ResearchRoot) -> Self {
        self.style = style;
        self
    }

    /// Chat turns read this thread's cell. `Idle` keeps the card and fixture
    /// paths. `Hits` and `Miss` do not call research. Not for `Heartbeat::step`.
    pub fn with_retrieval(mut self, thread: RetrievalThread) -> Self {
        self.retrieval = Some(thread);
        self
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
    /// `None` loads Samantha. An explicit path replaces her. An unreadable or
    /// invalid file returns the error and does not build a brain.
    pub fn boot(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        persona_file: Option<&Path>,
    ) -> Result<Self, PersonaError> {
        let persona = match persona_file {
            Some(path) => Persona::load(path)?,
            None => development_persona(),
        };
        let mut brain = Self::open(memory_db, knowledge_dir, cases);
        brain.install_persona(persona);
        Ok(brain)
    }

    /// Read `knowledge_dir` when it is already a directory. A missing path is
    /// an empty card set. Nothing is created. Memory and case paths are not
    /// opened.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let _ = (memory_db, cases);
        match crate::KnowledgeCards::load(knowledge_dir) {
            Ok(loaded) => {
                eprintln!("{}", loaded.report.log_line());
                Self::with_cards(loaded.cards, None).with_style(ResearchRoot::from_env())
            }
            Err(crate::CardError::NotADirectory(_)) => {
                Self::with_cards(EmptyCards, None).with_style(ResearchRoot::from_env())
            }
            Err(err) => {
                eprintln!("horbm cards: {err}");
                Self::with_cards(EmptyCards, None).with_style(ResearchRoot::from_env())
            }
        }
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

    /// Samantha's slower goals. Session only. Not the baseline, not the moment.
    pub fn samantha(&self) -> &SamanthaInner {
        &self.samantha
    }

    /// Ear switch for [`Self::turn`]. Default off. Words still count.
    pub fn set_ear_sense(&mut self, on: bool) {
        self.ear_sense = on;
    }

    /// Pace and topic gate from the latest inner note.
    pub fn dialogue_lean(&self) -> DialogueLean {
        self.samantha.last().lean()
    }

    /// False while she is waiting or on a check-in. Do not open a new topic.
    pub fn may_open_topic(&self) -> bool {
        !self.dialogue_lean().hold_topic
    }

    /// One inner turn, then one moment frame, using her GEMI note.
    ///
    /// Does not write the baseline. Does not keep `words`.
    pub fn samantha_nudge(
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
        let note = self.samantha.nudge(&mut self.emotion, seen)?;
        self.apply_prosody(note.lean());
        Ok(note)
    }

    fn install_persona(&mut self, persona: Persona) {
        let (emotion, inner, persona) = install(persona);
        self.emotion = emotion;
        self.inner = inner;
        self.persona = persona;
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
        let note = self.samantha.tick(FRAME_MS, true);
        self.apply_prosody(note.lean());
        self.meta
    }

    /// Empathy mood, then the retrieval cell when a thread is attached,
    /// then cards, then the fixture on an idle cell, then the gateway.
    /// Layer 4 then picks the move. [`Self::finish`] splits the clause.
    ///
    /// A granted hit speaks [`crate::MemoryHit::clause`]. A granted miss
    /// speaks [`crate::UNKNOWN_REPLY`] and does not call research. `Idle`
    /// means `remember` is denied: the diary is not opened, and the committed
    /// fixture still answers when the cards miss. This waits for the retrieval
    /// thread. Do not call it from `Heartbeat::step`. Barge-in is not this
    /// function.
    pub fn turn(&mut self, text: &str) -> TurnOut {
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
        let note = self.samantha.turn(Perceived {
            t_ms: self.emotion.next_ms(),
            words: heard,
            ear: None,
            sense: self.ear_sense,
        });
        self.apply_prosody(note.lean());
        let prep = self.empathy.prepare_lookup(heard);
        let query = if prep.query.trim().is_empty() {
            heard
        } else {
            prep.query.as_str()
        };
        if query.is_empty() {
            return self.finish(query, started, Spoke::empty(), &Decision::pass());
        }
        let query_owned = query.to_string();
        let gathered = self.gather(&query_owned);
        let reply = effective_reply(&gathered.spoke).to_string();
        let sources = gathered.spoke.sources.clone();
        let mut decision = self.dialogue.decide(DecideIn {
            heard,
            intent: self.meta.intent,
            reply: &reply,
            sources: &sources,
            arousal: self.meta.arousal,
            valence: self.meta.valence,
            dominance: self.meta.dominance,
            e: self.meta.e,
        });
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
                self.gaps.push(gap);
            }
        }
        let mut spoke = apply_plan(gathered.spoke, decision.plan().clone());
        if let Some(id) = self.style_id(decision.chosen, &query_owned) {
            spoke.evidence.push(format!("style:{id}"));
        }
        self.finish(&query_owned, started, spoke, &decision)
    }

    /// Style mark for comfort, ask, and offer_topic. Not an answer source.
    fn style_id(&self, chosen: Option<Move>, query: &str) -> Option<String> {
        match chosen {
            Some(Move::Comfort | Move::Ask | Move::OfferTopic) => {
                self.style.lookup(query).map(|hint| hint.id)
            }
            _ => None,
        }
    }

    /// Retrieval, cards, the idle fixture, then the gateway. No reply split.
    fn gather(&mut self, query: &str) -> Gathered {
        if let Some(spoke) = self.retrieval_spoke(query) {
            return Gathered { spoke, gap: None };
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
            };
        }
        if self.retrieval.is_some() {
            if let Some(spoke) = fixture_spoke(query) {
                return Gathered { spoke, gap: None };
            }
        }
        // Comfort stays with them. The research ask does not open.
        let comfort = self.dialogue_lean().verb == Verb::Comfort;
        let routed = if comfort {
            route_heard(query, self.cards.as_ref(), None, self.speakable)
        } else {
            match self.research.as_mut() {
                Some(port) => route_heard(
                    query,
                    self.cards.as_ref(),
                    Some(port.as_mut()),
                    self.speakable,
                ),
                None => route_heard(query, self.cards.as_ref(), None, self.speakable),
            }
        };
        Gathered {
            gap: routed.gap,
            spoke: Spoke {
                reply: routed.spoken,
                sources: routed.sources,
                evidence: routed.evidence,
                lookup_us: routed.lookup_us,
                picked: None,
            },
        }
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
        let source = format!("retrieve:{}", hit.item_id);
        Spoke {
            reply: String::new(),
            sources: vec![source.clone()],
            evidence: vec![source],
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

    fn finish(&self, query: &str, started: Instant, spoke: Spoke, decision: &Decision) -> TurnOut {
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
        }
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

fn development_persona() -> Persona {
    Persona::samantha().expect("samantha.toml is the development persona")
}

fn install(persona: Persona) -> (EmotionCore, InnerLife, Persona) {
    let emotion = EmotionCore::from_baseline(persona.baseline());
    let inner = persona.inner_life();
    (emotion, inner, persona)
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
        ["card:", "retrieve:", "fixture:", "research:"]
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
}
