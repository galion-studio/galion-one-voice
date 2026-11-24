// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Floor machine. One `tick` is one virtual frame.
//!
//! The host calls `tick` on the turn worker, beside the 80 ms frame step.
//! This function is not that step. It may allocate. It does not sleep.

use crate::brain::{ClauseBrain, PickedClause};
use crate::clock::{
    VirtualClock, COMMIT_SILENCE_FRAMES, DUCK_MS, FRAME_MS, HANG_FRAMES, PAUSE_MS, STABLE_TICKS,
};
use crate::lexicon::is_listed_backchannel;
use crate::mne::read_back;
use crate::reaction::{Reaction, ReactionKind, REACTION_SAMPLES};
use crate::stamps::Domain04Stamps;
use galion_mne_contract::Document;

/// What the ear reported for this frame. Text is a partial, not audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ear<'a> {
    Quiet,
    Speech(&'a str),
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
}

struct Held {
    epoch: u64,
    picked: PickedClause,
}

/// Conductor over a [`ClauseBrain`]. The brain is the phase 4 seam.
pub struct Conductor<B: ClauseBrain> {
    brain: B,
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
    brain_calls: u32,
    rollbacks: u32,
    reaction_kind: ReactionKind,
    pending_reaction: Option<ReactionKind>,
    stamps: Domain04Stamps,
}

/// One frame of output. `t_ms` is the virtual frame start.
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
}

impl<B: ClauseBrain> Conductor<B> {
    pub fn new(brain: B) -> Self {
        Self::with_reaction(brain, ReactionKind::Silence)
    }

    pub fn with_reaction(brain: B, reaction_kind: ReactionKind) -> Self {
        Self {
            brain,
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
            brain_calls: 0,
            rollbacks: 0,
            reaction_kind,
            pending_reaction: None,
            stamps: Domain04Stamps::default(),
        }
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

    /// Advance one virtual frame. Does not read a wall clock.
    pub fn tick(&mut self, ear: Ear<'_>, intent: Option<&Document>) -> TickOut {
        let t_ms = self.clock.t_ms();
        let t_us = self.clock.t_us();
        let mut event = TickEvent::None;
        let me_silent = self.apply_armed_silence(&mut event);
        match ear {
            Ear::Quiet => self.on_quiet(t_us, &mut event),
            Ear::Speech(text) => self.on_speech(text, &mut event),
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

    fn on_quiet(&mut self, t_us: u64, event: &mut TickEvent) {
        if self.cancel || *event == TickEvent::BargeIn {
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
        if self.floor == Floor::Listening
            && !self.partial.is_empty()
            && self.stable_ticks >= STABLE_TICKS
            && self.silent_ticks >= HANG_FRAMES
        {
            self.eager(t_us, event);
            return;
        }
        if self.floor == Floor::Speculating && self.silent_ticks >= COMMIT_SILENCE_FRAMES {
            self.commit(t_us, event);
        }
    }

    fn on_speech(&mut self, text: &str, event: &mut TickEvent) {
        if self.cancel || *event == TickEvent::BargeIn {
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
        self.cell = None;
        self.pending_reaction = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.stamps.turn_id = self.epoch;
        self.stamps.clear_speculation();
        self.floor = Floor::Listening;
        self.rollbacks = self.rollbacks.saturating_add(1);
        *event = TickEvent::TurnResumed;
    }
}
