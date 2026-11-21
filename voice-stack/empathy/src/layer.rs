// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The layer the pipeline ticks and the layer 5 router reads.
//! Nothing here returns a sentence.

use galion_emotion::frame::EmotionFrame;
use galion_sam_prosody::EmpathyProsody;

use crate::intent::{detect_intent, Intent};
use crate::mood::{
    invitation, step_mood, FeelingName, InboundSignal, MoodPoint, MoodStep, FRAME_MS,
};
use crate::names::{NameBook, NameSource, PronounClass};
use crate::rewrite::rewrite_query;
use crate::wants::{Want, WantList, WantState};

/// HORBM layer number. Voice is the layer under this one. The router is layer 5.
pub const LAYER: u8 = 3;

/// One heartbeat of input. `inbound` is `None` when sense is denied: the
/// pipeline must not pass a frame the caller did not grant.
#[derive(Clone, Copy, Debug)]
pub struct TickIn<'a> {
    pub inbound: Option<InboundSignal>,
    /// Latest ASR text. Empty keeps the previous intent.
    pub asr: &'a str,
    /// Committed transcript. Wants are read only on a final, so a partial
    /// does not open "t" on the way to "tea".
    pub asr_final: bool,
    /// The caller asked to hear a feeling named. A matching phrase in `asr`
    /// counts as an invitation too.
    pub invite_name: bool,
    /// Elapsed heartbeat. The live clock is [`FRAME_MS`].
    pub frame_ms: u32,
}

/// What one tick decided. `named` is a single word, and only when invited.
/// There is no reply field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickOut {
    pub mood: MoodPoint,
    pub e: f32,
    /// [`galion_sam_prosody::apply_empathy`] of this tick. `e == 0` matches
    /// the unweighted map. The pipeline hands `speaking` and `control` to
    /// the mouth. There is no sentence on it.
    pub prosody: EmpathyProsody,
    pub intent: Intent,
    pub named: Option<FeelingName>,
    pub t_ms: u32,
}

/// What the layer 5 router reads before a lookup.
///
/// `query` has already had pronouns replaced from the state tracker's names.
/// `query_intent` is the lookup string. `utterance_intent` is the last
/// non-empty ASR tick. Mood and `e` are the last 80 ms step. `named` is set
/// only when that tick was invited. This value carries no sentence.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupPrep {
    pub query: String,
    pub query_intent: Intent,
    pub utterance_intent: Intent,
    pub mood: MoodPoint,
    pub e: f32,
    pub named: Option<FeelingName>,
    pub open: u32,
    pub met: u32,
    pub dropped: u32,
}

/// Snapshot for the layer 5 router. Counts, not the want text, so a router
/// that only needs the shape does not copy the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouterView {
    pub mood: MoodPoint,
    pub e: f32,
    pub intent: Intent,
    pub named: Option<FeelingName>,
    pub open: u32,
    pub met: u32,
    pub dropped: u32,
    pub t_ms: u32,
}

/// Called once per 80 ms from the pipeline, off the sample callback if the
/// pipeline already does that for the brain tick.
pub trait PipelineLayer: Send {
    fn tick(&mut self, input: TickIn<'_>) -> TickOut;
    fn mood(&self) -> MoodPoint;
}

/// Called by the layer 5 router. No audio and no model.
pub trait RouterLayer: Send {
    /// Before lookup. Uses the names already stored on this layer.
    fn prepare_lookup(&self, query: &str) -> LookupPrep;
    /// Before lookup, with the state tracker's names passed in for this call.
    fn prepare_lookup_with(&self, query: &str, names: &dyn NameSource) -> LookupPrep;
    fn view(&self) -> RouterView;
    fn rewrite_query(&self, query: &str) -> String;
    fn wants(&self) -> &[Want];
    fn mark_want(&mut self, id: u32, state: WantState) -> bool;
    fn open_want(&mut self, text: &str) -> Option<u32>;
    fn names_mut(&mut self) -> &mut NameBook;
}

/// Layer 3. In memory for the process. No socket, no file, no model.
#[derive(Clone, Debug)]
pub struct Empathy {
    mood: MoodPoint,
    e: f32,
    t_ms: u32,
    caller_valence: f32,
    caller_arousal: f32,
    caller_dominance: f32,
    caller_confidence: f32,
    cue: Option<FeelingName>,
    named: Option<FeelingName>,
    intent: Intent,
    names: NameBook,
    wants: WantList,
    last_final: String,
}

impl Default for Empathy {
    fn default() -> Self {
        Self::new()
    }
}

impl Empathy {
    pub fn new() -> Self {
        Self {
            mood: MoodPoint::RESTING,
            e: 0.0,
            t_ms: 0,
            caller_valence: 0.0,
            caller_arousal: MoodPoint::RESTING.arousal,
            caller_dominance: MoodPoint::RESTING.dominance,
            caller_confidence: 0.0,
            cue: None,
            named: None,
            intent: Intent::Chat,
            names: NameBook::new(),
            wants: WantList::new(),
            last_final: String::new(),
        }
    }

    pub fn mood(&self) -> MoodPoint {
        self.mood
    }

    pub fn empathy_weight(&self) -> f32 {
        self.e
    }

    pub fn intent(&self) -> Intent {
        self.intent
    }

    pub fn names(&self) -> &NameBook {
        &self.names
    }

    pub fn names_mut(&mut self) -> &mut NameBook {
        &mut self.names
    }

    pub fn note_name(&mut self, name: &str, class: PronounClass) {
        self.names.note(name, class);
    }

    /// Mood half of a tick. This is the call the 80 ms budget measures.
    /// It does not allocate, does not rewrite a query, and does not touch
    /// the wants list.
    pub fn update_mood(
        &mut self,
        inbound: Option<InboundSignal>,
        asr: &str,
        frame_ms: u32,
    ) -> MoodPoint {
        let step = step_mood(self.mood, inbound, asr, frame_ms);
        self.apply_step(step);
        self.mood
    }

    pub fn tick(&mut self, input: TickIn<'_>) -> TickOut {
        let step = step_mood(self.mood, input.inbound, input.asr, input.frame_ms);
        self.apply_step(step);
        self.t_ms = self.t_ms.saturating_add(input.frame_ms);
        if !input.asr.is_empty() {
            self.intent = detect_intent(input.asr);
        }
        if input.asr_final && !input.asr.is_empty() && input.asr != self.last_final {
            self.wants.observe(input.asr);
            self.last_final.clear();
            self.last_final.push_str(input.asr);
        }
        self.named = self.gated_name(input.invite_name, input.asr);
        self.output()
    }

    pub fn view(&self) -> RouterView {
        let (open, met, dropped) = self.wants.counts();
        RouterView {
            mood: self.mood,
            e: self.e,
            intent: self.intent,
            named: self.named,
            open,
            met,
            dropped,
            t_ms: self.t_ms,
        }
    }

    pub fn rewrite_query(&self, query: &str) -> String {
        rewrite_query(query, &self.names)
    }

    /// Layer 5 calls this before lookup. Pass [`LookupPrep::query`] to
    /// `CardLoader::lookup` (and use it as the heard line for
    /// `ReasoningRouter::route`) so a pronoun is the state tracker's name.
    /// The clock does not move and this does not speak.
    pub fn prepare_lookup(&self, query: &str) -> LookupPrep {
        self.prepare_lookup_with(query, &self.names)
    }

    /// Same as [`Self::prepare_lookup`], with names from the state tracker
    /// for this call. Stored names are left as they are.
    pub fn prepare_lookup_with(&self, query: &str, names: &dyn NameSource) -> LookupPrep {
        let (open, met, dropped) = self.wants.counts();
        LookupPrep {
            query: rewrite_query(query, names),
            query_intent: detect_intent(query),
            utterance_intent: self.intent,
            mood: self.mood,
            e: self.e,
            named: self.named,
            open,
            met,
            dropped,
        }
    }

    pub fn wants(&self) -> &[Want] {
        self.wants.as_slice()
    }

    pub fn mark_want(&mut self, id: u32, state: WantState) -> bool {
        self.wants.mark(id, state)
    }

    pub fn open_want(&mut self, text: &str) -> Option<u32> {
        self.wants.open(text)
    }

    fn apply_step(&mut self, step: MoodStep) {
        self.mood = step.mood;
        self.e = step.e;
        self.caller_valence = step.valence;
        self.caller_arousal = step.arousal;
        self.caller_dominance = step.dominance;
        self.caller_confidence = step.confidence;
        self.cue = step.cue;
    }

    fn gated_name(&self, invite: bool, asr: &str) -> Option<FeelingName> {
        if !invite && !invitation(asr) {
            return None;
        }
        if let Some(cue) = self.cue {
            return Some(cue);
        }
        if self.caller_confidence < 0.2 {
            return None;
        }
        FeelingName::from_affect(
            EmotionFrame {
                seq: 0,
                t_ms: self.t_ms,
                dur_ms: FRAME_MS as u16,
                valence: self.caller_valence,
                arousal: self.caller_arousal,
                dominance: self.caller_dominance,
                confidence: self.caller_confidence,
            }
            .label(),
        )
    }

    fn output(&self) -> TickOut {
        let prosody = galion_sam_prosody::apply_empathy(self.mood.to_prosody_pad(), self.e);
        TickOut {
            mood: self.mood,
            e: self.e,
            prosody,
            intent: self.intent,
            named: self.named,
            t_ms: self.t_ms,
        }
    }
}

impl PipelineLayer for Empathy {
    fn tick(&mut self, input: TickIn<'_>) -> TickOut {
        Empathy::tick(self, input)
    }

    fn mood(&self) -> MoodPoint {
        self.mood
    }
}

impl RouterLayer for Empathy {
    fn prepare_lookup(&self, query: &str) -> LookupPrep {
        Empathy::prepare_lookup(self, query)
    }

    fn prepare_lookup_with(&self, query: &str, names: &dyn NameSource) -> LookupPrep {
        Empathy::prepare_lookup_with(self, query, names)
    }

    fn view(&self) -> RouterView {
        Empathy::view(self)
    }

    fn rewrite_query(&self, query: &str) -> String {
        Empathy::rewrite_query(self, query)
    }

    fn wants(&self) -> &[Want] {
        Empathy::wants(self)
    }

    fn mark_want(&mut self, id: u32, state: WantState) -> bool {
        Empathy::mark_want(self, id, state)
    }

    fn open_want(&mut self, text: &str) -> Option<u32> {
        Empathy::open_want(self, text)
    }

    fn names_mut(&mut self) -> &mut NameBook {
        Empathy::names_mut(self)
    }
}
