// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Samantha's session goals. One turn updates them. One tick decays them.
//!
//! The block is four numbers: care, curiosity, help, expect. It does not
//! store the words, the listener point, or a user id. Learning a new care
//! stays in the older token layer. Remember stays deny: nothing here is a
//! fact about the person.

use galion_emotion::text_affect;
use galion_emotion_core::{CoreError, EmotionCore, IntentMsg};

use crate::msg::{InnerMsg, MsgDir, Verb};
use crate::{
    CARE_HOME, CARE_SAD, CHECKIN_GAP_MS, CHECKIN_MS, CURIOSITY_ASK, CURIOSITY_HOME, DECAY_KEEP_DEN,
    DECAY_KEEP_NUM, EAR_CONFIDENCE, EXPECT_HOME, EXPECT_QUESTION, HELP_HOME, HELP_QUESTION,
    HELP_SAD, SAD_EAR_VALENCE, SAD_WORD_VALENCE, TICK_MS, TURN_STEP, WAIT_MS,
};

/// How many 80 ms decay steps one tick will run. A longer `dt` is clipped.
const DECAY_FRAME_CAP: u32 = 4_000;

/// What the ear claims about the listener. Dropped when the turn returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    /// Pleasure, −1..=1.
    pub valence: f32,
    /// Activation, 0..=1.
    pub arousal: f32,
    /// How sure the reading is, 0..=1. Below [`EAR_CONFIDENCE`] is no claim.
    pub confidence: f32,
}

impl Heard {
    /// Listener point from pleasure, activation, and how sure the reading is.
    ///
    /// `None` when any value is non-finite, or when `confidence` is below
    /// [`EAR_CONFIDENCE`]. That `None` is no claim. Finite values are stored
    /// as given.
    pub const fn from_axes(valence: f32, arousal: f32, confidence: f32) -> Option<Self> {
        if !valence.is_finite() || !arousal.is_finite() || !confidence.is_finite() {
            return None;
        }
        if confidence < EAR_CONFIDENCE {
            return None;
        }
        Some(Self {
            valence,
            arousal,
            confidence,
        })
    }
}

/// One turn's evidence. The words are borrowed and not copied.
#[derive(Clone, Copy, Debug)]
pub struct Perceived<'a> {
    /// Stamped onto the outbound note. The emotion core wants its own clock.
    pub t_ms: u32,
    pub words: &'a str,
    pub ear: Option<Heard>,
    /// When false, `ear` is ignored. Words still count.
    pub sense: bool,
}

/// What the dialogue manager reads off the latest note. Not a sentence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogueLean {
    pub verb: Verb,
    /// Multiply the speaking rate. Comfort and check-in sit under 1.
    pub rate: f32,
    /// Added pre-pause, milliseconds.
    pub pause_ms: f32,
    /// Wait and check-in. Do not open a new topic.
    pub hold_topic: bool,
    pub care: f32,
    pub curiosity: f32,
    pub help: f32,
    pub expect: f32,
}

impl DialogueLean {
    pub const fn rest() -> Self {
        Self {
            verb: Verb::Hold,
            rate: 1.0,
            pause_ms: 0.0,
            hold_topic: false,
            care: CARE_HOME,
            curiosity: CURIOSITY_HOME,
            help: HELP_HOME,
            expect: EXPECT_HOME,
        }
    }
}

impl InnerMsg {
    pub fn lean(self) -> DialogueLean {
        let (rate, pause_ms, hold_topic) = match self.verb {
            Verb::Comfort => (0.88, 120.0, false),
            Verb::CheckIn => (0.92, 160.0, true),
            Verb::Wait => (1.0, 0.0, true),
            // Rate stays 1. A greeting and a question are both neutral lines.
            // The closed eval requires those lines to keep one voice.
            Verb::With => (1.0, 40.0, false),
            Verb::Ask => (1.0, 40.0, false),
            Verb::Answer | Verb::Hold | Verb::Heard => (1.0, 0.0, false),
        };
        DialogueLean {
            verb: self.verb,
            rate,
            pause_ms,
            hold_topic,
            care: self.care,
            curiosity: self.curiosity,
            help: self.help,
            expect: self.expect,
        }
    }
}

/// Four slower goals and the silence clock. Copy. Session only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamanthaInner {
    care: f32,
    curiosity: f32,
    help: f32,
    expect: f32,
    /// Silence clock. Ticks advance it. Turns do not.
    clock_ms: u32,
    quiet_ms: u32,
    last_checkin_ms: Option<u32>,
    checkin_used: bool,
    ear_sense: bool,
    turns: u32,
    ticks: u32,
    last: InnerMsg,
}

impl Default for SamanthaInner {
    fn default() -> Self {
        Self::new()
    }
}

impl SamanthaInner {
    pub const fn new() -> Self {
        Self {
            care: CARE_HOME,
            curiosity: CURIOSITY_HOME,
            help: HELP_HOME,
            expect: EXPECT_HOME,
            clock_ms: 0,
            quiet_ms: 0,
            last_checkin_ms: None,
            checkin_used: false,
            ear_sense: false,
            turns: 0,
            ticks: 0,
            last: InnerMsg::rest(CARE_HOME, CURIOSITY_HOME, HELP_HOME, EXPECT_HOME),
        }
    }

    pub const fn care(self) -> f32 {
        self.care
    }
    pub const fn curiosity(self) -> f32 {
        self.curiosity
    }
    pub const fn help(self) -> f32 {
        self.help
    }
    pub const fn expect(self) -> f32 {
        self.expect
    }
    pub const fn quiet_ms(self) -> u32 {
        self.quiet_ms
    }
    pub const fn clock_ms(self) -> u32 {
        self.clock_ms
    }
    pub const fn turns(self) -> u32 {
        self.turns
    }
    pub const fn ticks(self) -> u32 {
        self.ticks
    }
    pub const fn last(self) -> InnerMsg {
        self.last
    }
    pub const fn ear_sense(self) -> bool {
        self.ear_sense
    }

    /// This layer keeps no fact about the person. The lock is deny.
    pub const fn keeps_user_facts(self) -> bool {
        false
    }

    /// The next state and the note, without committing either.
    pub fn preview(self, seen: Perceived<'_>) -> (Self, InnerMsg) {
        let mut next = self;
        let msg = next.apply_turn(seen);
        (next, msg)
    }

    /// Commit one turn. Does not step a moment core and does not decay.
    #[must_use]
    pub fn turn(&mut self, seen: Perceived<'_>) -> InnerMsg {
        self.apply_turn(seen)
    }

    /// [`Self::turn`] then [`EmotionCore::step`] with that GEMI note.
    ///
    /// `seen.t_ms` is replaced with the core's next frame. A refused step
    /// leaves this life as it was. The baseline is not an argument.
    pub fn nudge(
        &mut self,
        core: &mut EmotionCore,
        mut seen: Perceived<'_>,
    ) -> Result<InnerMsg, CoreError> {
        seen.t_ms = core.next_ms();
        let (next, msg) = self.preview(seen);
        let gemi = msg.to_gemi_at(core.next_ms())?;
        core.step(&gemi)?;
        *self = next;
        Ok(msg)
    }

    /// Inbound GEMI plus the words already in hand.
    ///
    /// A `heard` verb carries the listener point. Any other inbound verb is
    /// not a fact about them. Sense off drops the point and keeps the words.
    pub fn hear(&mut self, inbound: &IntentMsg, words: &str) -> InnerMsg {
        let parsed = InnerMsg::from_gemi(inbound);
        let ear = if parsed.verb == Verb::Heard {
            Some(Heard {
                valence: parsed.valence,
                arousal: parsed.arousal.clamp(0.0, 1.0),
                confidence: parsed.strength,
            })
        } else {
            None
        };
        self.turn(Perceived {
            t_ms: inbound.t_ms,
            words,
            ear,
            sense: parsed.sense,
        })
    }

    /// Decay toward home, and advance silence when `quiet` is set.
    ///
    /// `dt_ms` is the virtual gap. The usual call passes [`TICK_MS`].
    #[must_use]
    pub fn tick(&mut self, dt_ms: u32, quiet: bool) -> InnerMsg {
        self.ticks = self.ticks.saturating_add(1);
        self.clock_ms = self.clock_ms.saturating_add(dt_ms);
        let frames = (dt_ms / TICK_MS).min(DECAY_FRAME_CAP);
        self.decay(frames);
        if quiet {
            self.quiet_ms = self.quiet_ms.saturating_add(dt_ms);
            let verb = self.take_silence();
            self.emit(self.clock_ms, verb)
        } else {
            self.quiet_ms = 0;
            self.checkin_used = false;
            self.emit(self.clock_ms, Verb::Hold)
        }
    }

    fn apply_turn(&mut self, seen: Perceived<'_>) -> InnerMsg {
        let words = seen.words.trim();
        self.ear_sense = seen.sense;
        let ear = if seen.sense { seen.ear } else { None };
        let affect = text_affect(words);
        let word_sad = affect.valence.is_some_and(|v| v < SAD_WORD_VALENCE);
        let ear_sad = ear.is_some_and(|heard| {
            heard.confidence >= EAR_CONFIDENCE && heard.valence <= SAD_EAR_VALENCE
        });
        let sad = word_sad || ear_sad;
        let question = is_question(words);
        let curious = !sad && !question && has_curiosity(words);
        let present =
            !words.is_empty() || ear.is_some_and(|heard| heard.confidence >= EAR_CONFIDENCE);

        if !present && !sad {
            let verb = if self.quiet_ms >= WAIT_MS {
                Verb::Wait
            } else {
                Verb::Hold
            };
            return self.emit(seen.t_ms, verb);
        }

        self.quiet_ms = 0;
        self.checkin_used = false;
        self.turns = self.turns.saturating_add(1);

        if sad {
            self.care = step_toward(self.care, CARE_SAD);
            self.help = step_toward(self.help, HELP_SAD);
        }
        if question && !sad {
            self.expect = step_toward(self.expect, EXPECT_QUESTION);
            self.help = step_toward(self.help, HELP_QUESTION);
        }
        if curious {
            self.curiosity = step_toward(self.curiosity, CURIOSITY_ASK);
        }

        let verb = if sad {
            Verb::Comfort
        } else if question {
            Verb::Answer
        } else if curious {
            Verb::Ask
        } else {
            Verb::With
        };
        self.emit(seen.t_ms, verb)
    }

    fn take_silence(&mut self) -> Verb {
        if self.quiet_ms >= CHECKIN_MS && self.checkin_allowed() {
            self.last_checkin_ms = Some(self.clock_ms);
            self.checkin_used = true;
            return Verb::CheckIn;
        }
        if self.quiet_ms >= WAIT_MS {
            Verb::Wait
        } else {
            Verb::Hold
        }
    }

    fn checkin_allowed(self) -> bool {
        if self.checkin_used {
            return false;
        }
        match self.last_checkin_ms {
            None => true,
            Some(at) => self.clock_ms.saturating_sub(at) >= CHECKIN_GAP_MS,
        }
    }

    fn decay(&mut self, frames: u32) {
        for _ in 0..frames {
            self.care = keep(self.care, CARE_HOME);
            self.curiosity = keep(self.curiosity, CURIOSITY_HOME);
            self.help = keep(self.help, HELP_HOME);
            self.expect = keep(self.expect, EXPECT_HOME);
        }
    }

    fn emit(&mut self, t_ms: u32, verb: Verb) -> InnerMsg {
        let (valence, arousal, dominance, approach) = pulls(verb);
        let strength = strength_of(verb, self.care, self.curiosity, self.expect);
        let msg = InnerMsg {
            dir: MsgDir::Out,
            verb,
            strength,
            t_ms,
            care: self.care,
            curiosity: self.curiosity,
            help: self.help,
            expect: self.expect,
            valence,
            arousal,
            dominance,
            approach,
            sense: self.ear_sense,
        };
        self.last = msg;
        msg
    }
}

fn step_toward(level: f32, target: f32) -> f32 {
    unit(level + TURN_STEP * (target - level))
}

fn keep(level: f32, home: f32) -> f32 {
    unit(home + (level - home) * (DECAY_KEEP_NUM as f32) / (DECAY_KEEP_DEN as f32))
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn pulls(verb: Verb) -> (f32, f32, f32, f32) {
    match verb {
        // Warmer, quieter, less sharp, still turned toward them.
        Verb::Comfort => (0.28, -0.22, -0.10, 0.18),
        Verb::Answer => (0.06, 0.12, 0.04, 0.10),
        Verb::Ask => (0.08, 0.10, 0.0, 0.16),
        Verb::Wait => (0.04, -0.12, -0.06, 0.08),
        // Gentler than comfort. One soft reach, then wait.
        Verb::CheckIn => (0.10, -0.08, -0.06, 0.12),
        Verb::With => (0.08, 0.0, 0.0, 0.10),
        Verb::Hold | Verb::Heard => (0.0, 0.0, 0.0, 0.0),
    }
}

fn strength_of(verb: Verb, care: f32, curiosity: f32, expect: f32) -> f32 {
    match verb {
        Verb::Comfort => care,
        Verb::Answer => expect,
        Verb::Ask => curiosity,
        Verb::Wait => 0.40,
        Verb::CheckIn => 0.32,
        Verb::With => 0.34,
        Verb::Hold | Verb::Heard => 0.0,
    }
}

fn is_question(text: &str) -> bool {
    if text.as_bytes().contains(&b'?') {
        return true;
    }
    let word = first_word(text);
    const WORDS: &[&[u8]] = &[b"who", b"what", b"where", b"when", b"why", b"how", b"which"];
    WORDS.iter().any(|candidate| eq_ignore(word, candidate))
}

fn has_curiosity(text: &str) -> bool {
    contains_word(text, b"wonder")
        || contains_word(text, b"curious")
        || contains_word(text, b"interesting")
}

fn first_word(text: &str) -> &[u8] {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    &bytes[start..i]
}

fn contains_word(text: &str, word: &[u8]) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        if eq_ignore(&bytes[start..i], word) {
            return true;
        }
    }
    false
}

fn eq_ignore(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
}
