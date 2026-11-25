// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Session goals for one persona. One turn updates them. One tick decays them.
//!
//! The block is four numbers: care, curiosity, help, expect. The persona id
//! is a fixed ASCII slot the host filled in. This module does not store the
//! words, the listener point, or a user id. Learning a new care stays in the
//! older token layer. Remember stays deny: nothing here is a fact about the
//! person.

use galion_emotion::text_affect;
use galion_emotion_core::{CoreError, EmotionCore, IntentMsg};

use crate::msg::{InnerMsg, MsgDir, Verb};
use crate::{
    CARE_ABOVE_REST, CARE_HOME, CARE_SAD, CHECKIN_GAP_MS, CHECKIN_MS, CURIOSITY_ASK,
    CURIOSITY_HOME, DECAY_KEEP_DEN, DECAY_KEEP_NUM, EAR_CONFIDENCE, EXPECT_HOME, EXPECT_QUESTION,
    HELP_HOME, HELP_QUESTION, HELP_SAD, PERSONA_ID_BYTES, SAD_EAR_VALENCE, SAD_WORD_VALENCE,
    TICK_MS, TURN_STEP, WAIT_MS,
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
    /// Rest lean at the standalone home.
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

/// Four home levels. No persona id. The host fills this from baseline numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnerHome {
    pub care: f32,
    pub curiosity: f32,
    pub help: f32,
    pub expect: f32,
}

impl InnerHome {
    /// Standalone home. Same bits as the historical `*_HOME` constants.
    pub const STANDALONE: Self = Self {
        care: CARE_HOME,
        curiosity: CURIOSITY_HOME,
        help: HELP_HOME,
        expect: EXPECT_HOME,
    };

    /// Home from a baseline valence and three goal levels.
    ///
    /// Care is [`CARE_ABOVE_REST`] plus `valence`, then clamped to `0..=1`.
    /// `curiosity`, `help`, and `expect` are kept as given when each one is
    /// finite and inside `0..=1`. A non-finite input, or a goal outside
    /// `0..=1`, is refused. The value has no persona id.
    #[must_use]
    pub const fn from_baseline(
        valence: f32,
        curiosity: f32,
        help: f32,
        expect: f32,
    ) -> Option<Self> {
        if !valence.is_finite()
            || !curiosity.is_finite()
            || !help.is_finite()
            || !expect.is_finite()
        {
            return None;
        }
        if !in_unit(curiosity) || !in_unit(help) || !in_unit(expect) {
            return None;
        }
        let care = CARE_ABOVE_REST + valence;
        if !care.is_finite() {
            return None;
        }
        let care = if care < 0.0 {
            0.0
        } else if care > 1.0 {
            1.0
        } else {
            care
        };
        Some(Self {
            care,
            curiosity,
            help,
            expect,
        })
    }
}

/// Four slower goals, the home they decay toward, and the silence clock.
///
/// Copy. Session only. The persona id is a [`PERSONA_ID_BYTES`] ASCII slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersonaInner {
    id: [u8; PERSONA_ID_BYTES],
    home: InnerHome,
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

impl Default for PersonaInner {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonaInner {
    /// Standalone home, with an empty persona id.
    ///
    /// Deprecated: use PersonaInner / InnerHome.
    /// The four levels match the historical home constants bit for bit.
    /// A host passes the configured id to [`Self::with_home_for`].
    pub const fn new() -> Self {
        Self::from_clean([0; PERSONA_ID_BYTES], InnerHome::STANDALONE)
    }

    /// Session at `home` with an empty persona id.
    ///
    /// Finite levels are clamped to `0..=1`. A non-finite level becomes 0.
    /// In-range finite levels are stored bit for bit.
    #[must_use]
    pub fn with_home(home: InnerHome) -> Self {
        Self::from_clean([0; PERSONA_ID_BYTES], cleaned(home))
    }

    /// Session for `id` at `home`.
    ///
    /// `id` is copied into a [`PERSONA_ID_BYTES`] ASCII slot. Empty is
    /// allowed. Any other id must fit the slot and use the persona token
    /// shape: a lowercase letter, then lowercase letters, digits, or `-`.
    /// Anything else is refused. Tick and turn do not allocate.
    #[must_use]
    pub fn with_home_for(id: &str, home: InnerHome) -> Option<Self> {
        Some(Self::from_clean(pack_id(id)?, cleaned(home)))
    }

    const fn from_clean(id: [u8; PERSONA_ID_BYTES], home: InnerHome) -> Self {
        Self {
            id,
            home,
            care: home.care,
            curiosity: home.curiosity,
            help: home.help,
            expect: home.expect,
            clock_ms: 0,
            quiet_ms: 0,
            last_checkin_ms: None,
            checkin_used: false,
            ear_sense: false,
            turns: 0,
            ticks: 0,
            last: InnerMsg::rest(home.care, home.curiosity, home.help, home.expect),
        }
    }

    /// Id the host passed to [`Self::with_home_for`]. Empty for [`Self::new`].
    pub fn persona_id(&self) -> &str {
        let end = self
            .id
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(self.id.len());
        core::str::from_utf8(&self.id[..end]).unwrap_or_default()
    }

    /// Home the four goals decay toward.
    pub const fn home(self) -> InnerHome {
        self.home
    }

    /// Put the four goals back on this home. The persona id stays.
    #[must_use]
    pub fn reset(&mut self) -> InnerMsg {
        self.care = self.home.care;
        self.curiosity = self.home.curiosity;
        self.help = self.home.help;
        self.expect = self.home.expect;
        self.emit(self.clock_ms, Verb::Hold)
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
            self.care = keep(self.care, self.home.care);
            self.curiosity = keep(self.curiosity, self.home.curiosity);
            self.help = keep(self.help, self.home.help);
            self.expect = keep(self.expect, self.home.expect);
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

const fn in_unit(value: f32) -> bool {
    value >= 0.0 && value <= 1.0
}

fn cleaned(home: InnerHome) -> InnerHome {
    InnerHome {
        care: unit(home.care),
        curiosity: unit(home.curiosity),
        help: unit(home.help),
        expect: unit(home.expect),
    }
}

fn pack_id(id: &str) -> Option<[u8; PERSONA_ID_BYTES]> {
    let bytes = id.as_bytes();
    if bytes.len() > PERSONA_ID_BYTES || !id_shape(bytes) {
        return None;
    }
    let mut slot = [0u8; PERSONA_ID_BYTES];
    slot[..bytes.len()].copy_from_slice(bytes);
    Some(slot)
}

fn id_shape(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
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
