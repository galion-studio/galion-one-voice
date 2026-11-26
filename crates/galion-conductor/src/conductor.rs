// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Floor machine. One `tick` is one virtual frame.
//!
//! The host calls `tick` on the turn worker, beside the 80 ms frame step.
//! This function is not that step. It may allocate. It does not sleep.
//!
//! Galion Research is asked only after the local brain has returned, and only
//! for a lookup. The ask itself returns on this frame. A later frame merges
//! the result when its virtual arrival is inside the cap. The same frames
//! post the side-band mark: a start when the request opens, an end when it
//! merges, drops, or is cancelled. A denied session posts neither.

use crate::backchannel::{Backchannel, BackchannelConfig, ListenCue, ListenMood, ListenPolicy};
use crate::brain::{ClauseBrain, PickedClause};
use crate::clock::{
    VirtualClock, COMMIT_SILENCE_FRAMES, DUCK_MS, FRAME_MS, HANG_FRAMES, MIXER_RESUME_QUIET_MS,
    PAUSE_MS, SIDE_BAND_CAP_MS, STABLE_TICKS,
};

/// Quiet frames after a barge before the floor listens again.
/// `MIXER_RESUME_QUIET_MS` is 240, one frame is 80, so this is 3.
const RESUME_QUIET_FRAMES: u32 = MIXER_RESUME_QUIET_MS / FRAME_MS;

const _: () = assert!(RESUME_QUIET_FRAMES == 3);
use crate::lexicon::is_listed_backchannel;
use crate::mne::read_back;
use crate::reaction::{Reaction, ReactionKind, REACTION_SAMPLES};
use crate::research::{
    cap_decision, classify, AskResult, CapDecision, OfflineClient, PollResult, ResearchClient,
    SideBandView, SideIntent,
};
use crate::stamps::Domain04Stamps;
use galion_mne_contract::Document;
use galion_sideband::{MarkKind, MarkLog, SidebandConsent, SidebandMark};

/// What the ear reported for this frame. Text is a partial, not audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ear<'a> {
    Quiet,
    Speech(&'a str),
}

/// End-of-turn message from the ear prosody scorer.
///
/// The same three variants live on `horbm_ear::TurnMsg`. This crate does not
/// link the ear. [`TurnMsg::PauseOnly`] is the path [`Conductor::tick`] has
/// always used, and it is what a denied prosody ear (`horbm_ear::Ear::deny`,
/// `GALION_EAR_SENSE` off) asks for. [`TurnMsg::Hold`] keeps the floor with
/// the user. [`TurnMsg::Yield`] releases the clause on this quiet frame.
///
/// Barge-in does not read this message. Overlap while the installed persona is speaking
/// still arms silence on the 200 ms lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnMsg {
    PauseOnly,
    Hold,
    Yield { question: bool },
}

/// Floor. Speculating holds the clause behind the gate. Speaking has released it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floor {
    Listening,
    Speculating,
    Speaking,
    Ducked,
    Paused,
}

/// What this frame decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickEvent {
    None,
    EagerEndOfTurn,
    TurnResumed,
    Committed,
    Backchannel,
    BargeIn,
    /// The installed persona nodded while the user kept the floor. Not a barge-in.
    ListenBackchannel,
}

struct Held {
    epoch: u64,
    picked: PickedClause,
}

struct PendingResearch {
    id: u64,
    asked_ms: u64,
    epoch: u64,
}

/// Conductor over a [`ClauseBrain`]. The brain is the phase 4 seam.
/// `R` is the Galion Research client. The default is offline.
pub struct Conductor<B: ClauseBrain, R: ResearchClient = OfflineClient> {
    brain: B,
    research: R,
    clock: VirtualClock,
    floor: Floor,
    epoch: u64,
    partial: String,
    stable_ticks: u32,
    silent_ticks: u32,
    had_speech: bool,
    cell: Option<Held>,
    committed: Option<PickedClause>,
    overlap_ms: u32,
    ducked: bool,
    silence_next: bool,
    cancel: bool,
    /// Quiet frames seen while `cancel` is set. Speech resets it.
    resume_quiet: u32,
    brain_calls: u32,
    rollbacks: u32,
    reaction_kind: ReactionKind,
    pending_reaction: Option<ReactionKind>,
    stamps: Domain04Stamps,
    pending: Option<PendingResearch>,
    merged: Option<String>,
    side_note: SideBandView,
    frame_log: Option<String>,
    side_log: Vec<String>,
    side_cancels: u32,
    /// Posted start and end marks. Empty when the session denies them.
    marks: MarkLog,
    consent: SidebandConsent,
    /// Set for the frame that took a rising-pitch yield. Cleared each tick.
    frame_question: bool,
    /// Listening nods. Default on, for the installed persona. Does not own the floor.
    listen: ListenPolicy,
}

/// One frame of output. `t_ms` is the virtual frame start.
///
/// `clause` is the local brain line, once the turn commits. `research_clause`
/// is set only when a side-band result merged inside the cap. A drop leaves
/// it empty and the local clause in place.
pub struct TickOut {
    pub t_ms: u64,
    pub floor: Floor,
    pub event: TickEvent,
    pub cancel: bool,
    pub epoch: u64,
    pub me_silent: bool,
    pub ducked: bool,
    pub reaction: Option<Reaction>,
    pub clause: Option<PickedClause>,
    pub speculative: bool,
    pub rollbacks: u32,
    pub brain_calls: u32,
    pub readback: Option<Result<Document, String>>,
    pub stamps: Domain04Stamps,
    pub side_band: SideBandView,
    pub research_clause: Option<String>,
    pub side_log_line: Option<String>,
    /// True when this frame's yield was a rising terminal contour.
    pub end_question: bool,
    /// A listening nod on this frame. The floor stays with the user.
    pub backchannel: Option<Backchannel>,
}

impl<B: ClauseBrain> Conductor<B, OfflineClient> {
    /// Local brain, offline research. A lookup reports unavailable on the
    /// eager frame. The commit frame does not move.
    pub fn new(brain: B) -> Self {
        Self::assemble(brain, OfflineClient::new(), ReactionKind::Silence)
    }

    pub fn with_reaction(brain: B, reaction_kind: ReactionKind) -> Self {
        Self::assemble(brain, OfflineClient::new(), reaction_kind)
    }
}

impl<B: ClauseBrain, R: ResearchClient> Conductor<B, R> {
    pub fn with_research(brain: B, research: R) -> Self {
        Self::assemble(brain, research, ReactionKind::Silence)
    }

    fn assemble(brain: B, research: R, reaction_kind: ReactionKind) -> Self {
        Self {
            brain,
            research,
            clock: VirtualClock::new(),
            floor: Floor::Listening,
            epoch: 1,
            partial: String::new(),
            stable_ticks: 0,
            silent_ticks: 0,
            had_speech: false,
            cell: None,
            committed: None,
            overlap_ms: 0,
            ducked: false,
            silence_next: false,
            cancel: false,
            resume_quiet: 0,
            brain_calls: 0,
            rollbacks: 0,
            reaction_kind,
            pending_reaction: None,
            stamps: Domain04Stamps::default(),
            pending: None,
            merged: None,
            side_note: SideBandView::Idle,
            frame_log: None,
            side_log: Vec::new(),
            side_cancels: 0,
            marks: MarkLog::new(),
            consent: SidebandConsent::default(),
            frame_question: false,
            listen: ListenPolicy::new(),
        }
    }

    /// Listening nods. Default is on. `GALION_BACKCHANNEL=0|off` is
    /// [`BackchannelConfig::from_env`], applied here by the host.
    pub fn set_backchannel_config(&mut self, config: BackchannelConfig) {
        self.listen.enabled = config.enabled;
        self.listen.set_parked(config.parked);
    }

    pub fn backchannel_enabled(&self) -> bool {
        self.listen.enabled
    }

    /// Record the installed id. Nods do not read it. [`BackchannelConfig::parked`]
    /// silences every id.
    pub fn set_persona(&mut self, persona: impl Into<String>) {
        self.listen.set_persona(persona);
    }

    pub fn persona(&self) -> &str {
        self.listen.persona()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn floor(&self) -> Floor {
        self.floor
    }

    pub fn t_ms(&self) -> u64 {
        self.clock.t_ms()
    }

    /// How many pending asks barge-in or a resumed turn has cancelled.
    pub fn side_cancels(&self) -> u32 {
        self.side_cancels
    }

    /// Side-band trace. Counts and event names only. No transcript.
    pub fn side_log(&self) -> &[String] {
        &self.side_log
    }

    /// Start and end marks for Galion Research requests. Empty when
    /// [`SidebandConsent::Deny`] was set before the request opened.
    pub fn marks(&self) -> &[SidebandMark] {
        self.marks.marks()
    }

    pub fn sideband_consent(&self) -> SidebandConsent {
        self.consent
    }

    /// `Deny` posts no mark. Remember, learn, and in-band learning stay denied
    /// either way. The research ask is unchanged.
    pub fn set_sideband_consent(&mut self, consent: SidebandConsent) {
        self.consent = consent;
    }

    pub fn research(&self) -> &R {
        &self.research
    }

    /// The mouth finished the committed clause. Return to listening.
    ///
    /// A paused floor (barge-in) is left alone. Three quiet frames after
    /// that pause resume listening from the tick.
    pub fn mouth_idle(&mut self) {
        if self.cancel || self.silence_next {
            return;
        }
        if !matches!(self.floor, Floor::Speaking | Floor::Ducked) {
            return;
        }
        self.enter_listening();
    }

    fn enter_listening(&mut self) {
        self.floor = Floor::Listening;
        self.partial.clear();
        self.stable_ticks = 0;
        self.silent_ticks = 0;
        self.had_speech = false;
        self.cell = None;
        self.committed = None;
        self.overlap_ms = 0;
        self.ducked = false;
        self.silence_next = false;
        self.pending_reaction = None;
    }

    fn resume_listening(&mut self) {
        self.cancel = false;
        self.resume_quiet = 0;
        self.enter_listening();
    }

    /// Advance one virtual frame. Pause-only end of turn.
    /// Does not read a wall clock. Does not wait on Galion Research.
    pub fn tick(&mut self, ear: Ear<'_>, intent: Option<&Document>) -> TickOut {
        self.tick_turn(ear, intent, TurnMsg::PauseOnly)
    }

    /// [`Self::tick`] with an end-of-turn message from the prosody scorer.
    ///
    /// Speaking and ducked frames ignore `turn`, so barge-in stays the
    /// overlap rule. A yield on a quiet listening frame starts the brain and
    /// releases the clause on that same frame. A hold does neither.
    /// This path does not nod: the host passes a listening cue to
    /// [`Self::tick_listen`].
    pub fn tick_turn(&mut self, ear: Ear<'_>, intent: Option<&Document>, turn: TurnMsg) -> TickOut {
        self.tick_listen(ear, intent, turn, ListenCue::None, ListenMood::resting())
    }

    /// [`Self::tick_turn`] plus a listening cue and the installed persona's moment.
    ///
    /// A nod fires only while the user holds, on the quiet frame after a
    /// clause-final fall, and it does not take the floor. Yield, a question
    /// rise, a filler, and a denied ear (`PauseOnly`) do not nod. The nod is
    /// not a barge-in and it does not cancel the user's turn.
    pub fn tick_listen(
        &mut self,
        ear: Ear<'_>,
        intent: Option<&Document>,
        turn: TurnMsg,
        cue: ListenCue,
        mood: ListenMood,
    ) -> TickOut {
        let t_ms = self.clock.t_ms();
        let t_us = self.clock.t_us();
        self.side_note = SideBandView::Idle;
        self.frame_log = None;
        self.frame_question = false;
        let mut event = TickEvent::None;
        let me_silent = self.apply_armed_silence(&mut event);
        let user_has_floor = matches!(self.floor, Floor::Listening | Floor::Speculating);
        if user_has_floor {
            if let Ear::Speech(text) = ear {
                if !text.trim().is_empty() {
                    self.listen.note_voiced(t_ms);
                }
            }
        }
        let quiet = matches!(ear, Ear::Quiet);
        match ear {
            Ear::Quiet => self.on_quiet(t_us, &mut event, turn),
            Ear::Speech(text) => self.on_speech(text, &mut event),
        }
        let holding = quiet
            && matches!(turn, TurnMsg::Hold)
            && self.floor == Floor::Listening
            && event == TickEvent::None
            && !self.cancel;
        let backchannel = self.listen.consider(t_ms, quiet, holding, cue, mood);
        if backchannel.is_some() {
            event = TickEvent::ListenBackchannel;
        }
        if !self.cancel {
            self.poll_research(t_ms);
        }
        let reaction = self.pending_reaction.take().map(Reaction::from_kind);
        let readback = intent.map(|doc| read_back(doc, reaction.as_ref()));
        let speculative = self
            .cell
            .as_ref()
            .map(|held| held.epoch == self.epoch)
            .unwrap_or(false);
        let out = TickOut {
            t_ms,
            floor: self.floor,
            event,
            cancel: self.cancel,
            epoch: self.epoch,
            me_silent,
            ducked: self.ducked,
            reaction,
            clause: self.committed.clone(),
            speculative,
            rollbacks: self.rollbacks,
            brain_calls: self.brain_calls,
            readback,
            stamps: self.stamps.clone(),
            side_band: self.side_note,
            research_clause: self.merged.clone(),
            side_log_line: self.frame_log.clone(),
            end_question: self.frame_question,
            backchannel,
        };
        self.clock.advance();
        out
    }

    fn apply_armed_silence(&mut self, event: &mut TickEvent) -> bool {
        if !self.silence_next || self.cancel {
            return false;
        }
        self.silence_next = false;
        self.cancel = true;
        self.abandon_floor();
        self.epoch = self.epoch.wrapping_add(1);
        self.cell = None;
        self.committed = None;
        self.floor = Floor::Paused;
        self.ducked = false;
        self.overlap_ms = 0;
        self.stamps.turn_id = self.epoch;
        self.stamps.clear_speculation();
        *event = TickEvent::BargeIn;
        true
    }

    fn on_quiet(&mut self, t_us: u64, event: &mut TickEvent, turn: TurnMsg) {
        if self.cancel {
            // The frame that applies silence is the barge itself, not a
            // quiet frame of the pause that follows it.
            if *event != TickEvent::BargeIn {
                self.resume_quiet = self.resume_quiet.saturating_add(1);
                if self.resume_quiet >= RESUME_QUIET_FRAMES {
                    self.resume_listening();
                }
            }
            return;
        }
        if self.had_speech && self.silent_ticks == 0 {
            self.stamps.mark_silence(t_us);
        }
        self.silent_ticks = self.silent_ticks.saturating_add(1);
        if !self.partial.is_empty() {
            self.stable_ticks = self.stable_ticks.saturating_add(1);
        }
        if !self.silence_next {
            self.overlap_ms = 0;
            if self.ducked {
                self.ducked = false;
                if self.floor == Floor::Ducked {
                    self.floor = Floor::Speaking;
                }
            }
        }
        // The message applies while we are still deciding the user's turn.
        // Speaking and ducked stay on the barge-in path in `on_speech`.
        if matches!(self.floor, Floor::Speaking | Floor::Ducked | Floor::Paused) {
            return;
        }
        if self.floor == Floor::Speculating && matches!(turn, TurnMsg::Hold) {
            self.rollback(event);
            return;
        }
        let pause_only = matches!(turn, TurnMsg::PauseOnly)
            && !self.partial.is_empty()
            && self.stable_ticks >= STABLE_TICKS
            && self.silent_ticks >= HANG_FRAMES;
        let yielded = matches!(turn, TurnMsg::Yield { .. })
            && !self.partial.is_empty()
            && self.silent_ticks >= HANG_FRAMES;
        if self.floor == Floor::Listening && (pause_only || yielded) {
            if let TurnMsg::Yield { question } = turn {
                self.frame_question = question;
            }
            self.eager(t_us, event);
            // A prosodic yield already heard the ending contour, so the
            // confirm silence is this frame. Pause-only still waits.
            if yielded {
                self.commit(t_us, event);
            }
            return;
        }
        if self.floor == Floor::Speculating
            && !matches!(turn, TurnMsg::Hold)
            && self.silent_ticks >= COMMIT_SILENCE_FRAMES
        {
            self.commit(t_us, event);
        }
    }

    fn on_speech(&mut self, text: &str, event: &mut TickEvent) {
        if self.cancel || *event == TickEvent::BargeIn {
            if self.cancel {
                self.resume_quiet = 0;
            }
            return;
        }
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            self.had_speech = true;
        }
        self.silent_ticks = 0;

        if self.floor == Floor::Speculating && !trimmed.is_empty() {
            self.rollback(event);
        }

        if trimmed != self.partial {
            self.partial = trimmed.to_string();
            self.stable_ticks = if trimmed.is_empty() { 0 } else { 1 };
        } else if !trimmed.is_empty() {
            self.stable_ticks = self.stable_ticks.saturating_add(1);
        }
        self.stamps.heard_chars = trimmed.chars().count() as u32;

        if !matches!(self.floor, Floor::Speaking | Floor::Ducked) {
            return;
        }
        if is_listed_backchannel(trimmed) {
            if *event == TickEvent::None {
                *event = TickEvent::Backchannel;
            }
            return;
        }
        if trimmed.is_empty() {
            return;
        }
        self.overlap_ms = self.overlap_ms.saturating_add(FRAME_MS);
        if self.overlap_ms >= DUCK_MS {
            self.ducked = true;
            self.floor = Floor::Ducked;
        }
        // Same predicate as galion-heartbeat mixer::update. Silence is the next frame.
        if self.overlap_ms.saturating_add(FRAME_MS) > PAUSE_MS {
            self.silence_next = true;
        }
    }

    fn eager(&mut self, t_us: u64, event: &mut TickEvent) {
        let decided = t_us.saturating_add(u64::from(FRAME_MS).saturating_mul(1_000));
        self.stamps.mark_eou(decided);
        self.stamps.mark_asr(decided);
        self.stamps.mark_llm_req(decided);
        let query = self.partial.clone();
        let picked = self.brain.answer(&query);
        self.brain_calls = self.brain_calls.saturating_add(1);
        self.offer_research(self.clock.t_ms(), &query);
        self.stamps.mark_llm_token(decided);
        self.cell = Some(Held {
            epoch: self.epoch,
            picked,
        });
        self.floor = Floor::Speculating;
        self.pending_reaction = Some(self.reaction_kind);
        self.stamps.mark_media(decided);
        self.stamps.pcm_out_samples = REACTION_SAMPLES as u64;
        *event = TickEvent::EagerEndOfTurn;
    }

    fn commit(&mut self, t_us: u64, event: &mut TickEvent) {
        let flushed = t_us.saturating_add(u64::from(FRAME_MS).saturating_mul(1_000));
        if let Some(held) = self.cell.take() {
            if held.epoch == self.epoch {
                self.stamps.mark_clause(flushed);
                self.stamps.reply_chars = held.picked.clause.chars().count() as u32;
                self.committed = Some(held.picked);
            }
        }
        self.floor = Floor::Speaking;
        *event = TickEvent::Committed;
    }

    fn rollback(&mut self, event: &mut TickEvent) {
        self.abandon_floor();
        self.cell = None;
        self.pending_reaction = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.stamps.turn_id = self.epoch;
        self.stamps.clear_speculation();
        self.floor = Floor::Listening;
        self.rollbacks = self.rollbacks.saturating_add(1);
        *event = TickEvent::TurnResumed;
    }

    fn offer_research(&mut self, now_ms: u64, query: &str) {
        match classify(query) {
            SideIntent::None => {}
            SideIntent::MoneyDenied => {
                self.side_note = SideBandView::Refused;
                self.note_log("side_band event=refused elapsed_ms=0");
            }
            SideIntent::Lookup => match self.research.ask(query, now_ms) {
                AskResult::Unavailable => {
                    self.side_note = SideBandView::Unavailable;
                    self.note_log("side_band event=unavailable elapsed_ms=0");
                    let turn = self.epoch;
                    self.marks.post_start(self.consent, turn, query, now_ms);
                    self.marks.post_end(turn, MarkKind::Unavailable, now_ms);
                }
                AskResult::Pending { id } => {
                    let turn = self.epoch;
                    self.pending = Some(PendingResearch {
                        id,
                        asked_ms: now_ms,
                        epoch: turn,
                    });
                    self.marks.post_start(self.consent, turn, query, now_ms);
                    self.side_note = SideBandView::Pending { elapsed_ms: 0 };
                }
            },
        }
    }

    fn poll_research(&mut self, now_ms: u64) {
        let Some((id, asked_ms, pending_epoch)) = self
            .pending
            .as_ref()
            .map(|pending| (pending.id, pending.asked_ms, pending.epoch))
        else {
            return;
        };
        if pending_epoch != self.epoch {
            self.abandon_floor();
            return;
        }
        let waited = now_ms.saturating_sub(asked_ms);
        match self.research.poll(id, now_ms) {
            PollResult::Ready { elapsed_ms, text } => {
                self.pending = None;
                if cap_decision(Some(elapsed_ms), waited, SIDE_BAND_CAP_MS) == CapDecision::Merge {
                    let chars = text.chars().count();
                    self.marks.post_end(
                        pending_epoch,
                        MarkKind::Merged { text: text.clone() },
                        now_ms,
                    );
                    self.merged = Some(text);
                    self.side_note = SideBandView::Merged { elapsed_ms };
                    self.note_log_owned(format!(
                        "side_band event=merged elapsed_ms={elapsed_ms} cap_ms={SIDE_BAND_CAP_MS} chars={chars}"
                    ));
                } else {
                    self.research.cancel(id);
                    self.marks
                        .post_end(pending_epoch, MarkKind::Dropped, now_ms);
                    self.side_note = SideBandView::Dropped { elapsed_ms };
                    self.note_log_owned(format!(
                        "side_band event=dropped elapsed_ms={elapsed_ms} cap_ms={SIDE_BAND_CAP_MS}"
                    ));
                }
            }
            PollResult::Pending => {
                if cap_decision(None, waited, SIDE_BAND_CAP_MS) == CapDecision::Drop {
                    self.abandon_cap(waited);
                } else {
                    self.side_note = SideBandView::Pending { elapsed_ms: waited };
                }
            }
            PollResult::Gone => {
                self.pending = None;
                self.marks
                    .post_end(pending_epoch, MarkKind::Unavailable, now_ms);
            }
        }
    }

    fn abandon_floor(&mut self) {
        self.merged = None;
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.research.cancel(pending.id);
        self.marks
            .post_end(pending.epoch, MarkKind::Cancelled, self.clock.t_ms());
        self.side_cancels = self.side_cancels.saturating_add(1);
        self.side_note = SideBandView::Cancelled;
    }

    fn abandon_cap(&mut self, elapsed_ms: u64) {
        if let Some(pending) = self.pending.take() {
            self.research.cancel(pending.id);
            self.marks
                .post_end(pending.epoch, MarkKind::Dropped, self.clock.t_ms());
        }
        self.side_note = SideBandView::Dropped { elapsed_ms };
        self.note_log_owned(format!(
            "side_band event=dropped elapsed_ms={elapsed_ms} cap_ms={SIDE_BAND_CAP_MS}"
        ));
    }

    fn note_log(&mut self, line: &str) {
        self.note_log_owned(line.to_string());
    }

    fn note_log_owned(&mut self, line: String) {
        self.frame_log = Some(line.clone());
        self.side_log.push(line);
    }
}
