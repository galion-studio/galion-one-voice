// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! What the speaker cares about, wants, and expects.
//!
//! The state is a fixed block of open tokens. It is not a list of emotions
//! and not a Cowen pin. One [`InnerLife::turn`] reads the dialogue as ASCII
//! bytes and the current emotion point, then emits one [`crate::IntentMsg`].
//! That message is the same 72-byte `GEMI` record the MnE side already
//! speaks. [`InnerLife::nudge`] hands it to [`crate::EmotionCore::step`].
//! The step is the port. This module has no [`crate::VoiceBaseline`] field
//! and never assigns one.
//!
//! [`LEARN_LOCK`] mirrors `LOCKS.toml` `[consent] learn`. The default is
//! deny. A denied turn still updates the live care, want, and expectation.
//! It does not store a new motive.

use crate::axes::{clamp01, APPROACH, AROUSAL, AXES, DOMINANCE, VALENCE};
use crate::intent::{validate, IntentMsg, IntentSource, INTENT_BYTES, PULL_MASK_ALL, VERB_BYTES};
use crate::state::EmotionCore;
use crate::CoreError;

/// `LOCKS.toml` `[consent] learn`. Not an EmotionWire bit. Those four bits
/// stay sense, adapt, remember, and share. This constant must stay equal to
/// the lock. `scripts/check-locks.sh` checks the file. The test in this
/// module checks the constant against the file.
pub const LEARN_LOCK: &str = "deny";

/// True only when [`LEARN_LOCK`] is `allow`.
pub fn learning_allowed() -> bool {
    LEARN_LOCK == "allow"
}

/// Slot capacity for cares. Not an emotion count.
pub const CARE_SLOTS: usize = 4;
/// Slot capacity for wants. Not an emotion count.
pub const WANT_SLOTS: usize = 4;
/// Slot capacity for expectations. Not an emotion count.
pub const EXPECT_SLOTS: usize = 4;
/// Slot capacity for motives that learning would store. Not an emotion count.
pub const LEARNED_SLOTS: usize = 4;

/// Heap allocations one [`InnerLife::turn`] or [`InnerLife::nudge`] may make.
pub const TURN_ALLOC_BUDGET: usize = 0;

/// Seeded care level, and the home the valence step aims at.
pub const CARE_LEVEL: f32 = 0.80;
/// How far a negative valence lifts the care target. A knob, not a published loading.
pub const CARE_VALENCE_GAIN: f32 = 0.15;
/// Seeded want level.
pub const WANT_LEVEL: f32 = 0.70;
/// Want target when the dialogue has a question mark.
pub const WANT_QUESTION: f32 = 0.90;
/// Extra want target from positive approach. Negative approach adds nothing.
pub const WANT_APPROACH_GAIN: f32 = 0.10;
/// Seeded expectation belief. The first mismatch reports this as surprise.
pub const EXPECT_LEVEL: f32 = 1.0;
/// Belief written after a mismatch, once surprise has been sampled.
pub const BELIEF_AFTER_MISS: f32 = 0.35;
/// Added to belief when the event matches. Clamped to 1.
pub const BELIEF_ON_HIT: f32 = 0.05;
/// Fraction of the gap closed in one turn. No exponential.
pub const LEVEL_STEP: f32 = 0.25;
/// Level a learned motive would start at, if the lock allowed the write.
pub const LEARNED_LEVEL: f32 = 0.50;

/// Pull at surprise 1, valence axis. `step` multiplies by strength.
pub const SURPRISE_VALENCE: f32 = -0.20;
/// Pull at surprise 1, arousal axis.
pub const SURPRISE_AROUSAL: f32 = 0.45;
/// Pull at surprise 1, dominance axis.
pub const SURPRISE_DOMINANCE: f32 = -0.08;
/// Pull at surprise 1, approach axis.
pub const SURPRISE_APPROACH: f32 = 0.10;

const EMPTY: Motive = Motive::empty();

/// Dialogue acts the scanner can see. Not emotion names. `wait` is read as
/// `pause`. `thank` and `thanks` are read as `thanks`. Anything else in the
/// dialogue is not an event. A novel word hits the learning gate instead.
const CUES: &[&[u8]] = &[
    b"heard",
    b"answer",
    b"pause",
    b"question",
    b"thanks",
    b"thank",
    b"wait",
    b"hello",
    b"yes",
    b"no",
    b"stop",
    b"again",
];

const CARE_PREFER: &[&[u8]] = &[b"heard"];
const WANT_PREFER: &[&[u8]] = &[b"answer", b"question"];
const EXPECT_PREFER: &[&[u8]] = &[b"pause", b"wait"];

/// One care, want, or expectation. The token is an open ASCII word, at most
/// 16 bytes, the same width as a GEMI verb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motive {
    token: [u8; VERB_BYTES],
    level: f32,
}

impl Motive {
    pub const fn empty() -> Self {
        Self {
            token: [0; VERB_BYTES],
            level: 0.0,
        }
    }

    const fn known(word: &[u8], level: f32) -> Self {
        Self {
            token: pack_token(word),
            level,
        }
    }

    pub fn token(&self) -> &[u8] {
        let n = token_len(&self.token);
        &self.token[..n]
    }

    pub fn token_str(&self) -> &str {
        core::str::from_utf8(self.token()).unwrap_or("")
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn is_empty(&self) -> bool {
        self.token == [0; VERB_BYTES]
    }
}

/// What one turn saw. Borrowed. The turn does not keep the bytes.
#[derive(Clone, Copy, Debug)]
pub struct TurnView<'a> {
    /// Frame time copied onto the outbound GEMI message.
    pub t_ms: u32,
    /// Dialogue, scanned as ASCII. Not stored.
    pub dialogue: &'a [u8],
    /// Emotion point. The moment's position, read and not written.
    pub point: [f32; AXES],
    /// Intent coming back from the MnE side, if this turn has one.
    pub inbound: Option<&'a IntentMsg>,
    /// Consent nibble copied onto her own message so a later step keeps it.
    pub consent: u8,
}

/// Her outbound intent, and the surprise that went into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnerTurn {
    pub outbound: IntentMsg,
    /// 0 when the event matched the expectation, or when no event arrived.
    pub surprise: f32,
    /// True only when this turn wrote a learned motive. False while the lock is deny.
    pub learned: bool,
    /// How many novel words the lock has refused, including this turn.
    pub learn_refused: u32,
}

/// Cares, wants, and one live expectation. Copy. No heap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnerLife {
    cares: [Motive; CARE_SLOTS],
    wants: [Motive; WANT_SLOTS],
    expects: [Motive; EXPECT_SLOTS],
    learned: [Motive; LEARNED_SLOTS],
    learn_refused: u32,
    turns: u32,
    last_surprise: f32,
    last_outbound: IntentMsg,
}

impl Default for InnerLife {
    fn default() -> Self {
        Self::seeded()
    }
}

impl InnerLife {
    /// The stand-in seed: care `heard`, want `answer`, expect `pause`.
    pub const fn seeded() -> Self {
        Self {
            cares: [Motive::known(b"heard", CARE_LEVEL), EMPTY, EMPTY, EMPTY],
            wants: [Motive::known(b"answer", WANT_LEVEL), EMPTY, EMPTY, EMPTY],
            expects: [Motive::known(b"pause", EXPECT_LEVEL), EMPTY, EMPTY, EMPTY],
            learned: [EMPTY; LEARNED_SLOTS],
            learn_refused: 0,
            turns: 0,
            last_surprise: 0.0,
            last_outbound: IntentMsg::hold(0),
        }
    }

    /// Build the seed from the three prose lines in `inner-life.txt`.
    /// Loading text is the caller's job. This scan does not allocate.
    pub fn from_prose(care: &str, want: &str, expect: &str) -> Self {
        let mut life = Self::seeded();
        life.cares[0].token = find_prefer(care.as_bytes(), CARE_PREFER);
        life.wants[0].token = find_prefer(want.as_bytes(), WANT_PREFER);
        life.expects[0].token = find_prefer(expect.as_bytes(), EXPECT_PREFER);
        life
    }

    pub fn care(&self, index: usize) -> Option<Motive> {
        occupied(&self.cares, index)
    }

    pub fn want(&self, index: usize) -> Option<Motive> {
        occupied(&self.wants, index)
    }

    pub fn expect(&self, index: usize) -> Option<Motive> {
        occupied(&self.expects, index)
    }

    pub fn learned(&self, index: usize) -> Option<Motive> {
        occupied(&self.learned, index)
    }

    pub fn learned_count(&self) -> u32 {
        self.learned.iter().filter(|m| !m.is_empty()).count() as u32
    }

    pub fn learn_refused(&self) -> u32 {
        self.learn_refused
    }

    pub fn turns(&self) -> u32 {
        self.turns
    }

    pub fn surprise(&self) -> f32 {
        self.last_surprise
    }

    pub fn outbound(&self) -> IntentMsg {
        self.last_outbound
    }

    /// The next state and the message, without committing either.
    pub fn preview(&self, view: TurnView<'_>) -> (InnerLife, InnerTurn) {
        let mut next = *self;
        let turn = next.apply(view);
        (next, turn)
    }

    /// Commit one turn. Does not touch a moment core.
    #[must_use]
    pub fn turn(&mut self, view: TurnView<'_>) -> InnerTurn {
        self.apply(view)
    }

    /// Encode the last outbound message. Same bytes [`IntentMsg::encode`] returns.
    pub fn emit_gemi(&self) -> Result<[u8; INTENT_BYTES], CoreError> {
        self.last_outbound.encode()
    }

    /// Decode one GEMI message from the MnE side and commit a turn on it.
    /// A bad buffer leaves this life unchanged.
    pub fn consume_gemi(
        &mut self,
        bytes: &[u8],
        t_ms: u32,
        dialogue: &[u8],
        point: [f32; AXES],
        consent: u8,
    ) -> Result<InnerTurn, CoreError> {
        let msg = IntentMsg::decode(bytes)?;
        Ok(self.turn(TurnView {
            t_ms,
            dialogue,
            point,
            inbound: Some(&msg),
            consent,
        }))
    }

    /// Build the outbound GEMI message, then step the core with it.
    ///
    /// `t_ms` is [`EmotionCore::next_ms`]. The emotion point is the moment
    /// before the step. If the step is refused, this life is left as it was.
    pub fn nudge(
        &mut self,
        core: &mut EmotionCore,
        dialogue: &[u8],
        inbound: Option<&IntentMsg>,
    ) -> Result<InnerTurn, CoreError> {
        let view = TurnView {
            t_ms: core.next_ms(),
            dialogue,
            point: core.moment().pos,
            inbound,
            consent: core.consent(),
        };
        let (next, turn) = self.preview(view);
        core.step(&turn.outbound)?;
        *self = next;
        Ok(turn)
    }

    fn apply(&mut self, view: TurnView<'_>) -> InnerTurn {
        let observed = match view.inbound {
            Some(msg) => observe_verb(&msg.verb).or_else(|| observe_dialogue(view.dialogue)),
            None => observe_dialogue(view.dialogue),
        };

        let mut surprise = 0.0;
        if let Some(obs) = observed {
            if let Some(idx) = lead(&self.expects) {
                let belief = self.expects[idx].level;
                if self.expects[idx].token == obs {
                    self.expects[idx].level = clamp01(belief + BELIEF_ON_HIT);
                } else {
                    surprise = clamp01(belief);
                    self.expects[idx].token = obs;
                    self.expects[idx].level = BELIEF_AFTER_MISS;
                }
            }
        } else if let Some(idx) = lead(&self.expects) {
            let arousal = axis(view.point, AROUSAL, 0.0, 1.0);
            let target = belief_target(arousal);
            let level = self.expects[idx].level;
            self.expects[idx].level = clamp01(level + LEVEL_STEP * (target - level));
        }

        if let Some(idx) = lead(&self.cares) {
            let valence = axis(view.point, VALENCE, -1.0, 1.0);
            let target = clamp01(CARE_LEVEL - CARE_VALENCE_GAIN * valence);
            let level = self.cares[idx].level;
            self.cares[idx].level = clamp01(level + LEVEL_STEP * (target - level));
        }

        if has_question(view.dialogue) {
            if let Some(idx) = lead(&self.wants) {
                let approach = axis(view.point, APPROACH, -1.0, 1.0);
                let extra = if approach > 0.0 { approach } else { 0.0 };
                let target = clamp01(WANT_QUESTION + WANT_APPROACH_GAIN * extra);
                let level = self.wants[idx].level;
                self.wants[idx].level = clamp01(level + LEVEL_STEP * (target - level));
            }
        }

        let learned_before = self.learned_count();
        if let Some(token) = novel_token(self, view.dialogue) {
            self.store_learned(token, learning_allowed());
        }
        let learned_now = self.learned_count() > learned_before;

        let outbound = self.compose(view.t_ms, view.consent, surprise);
        let valid = validate(&outbound);
        debug_assert!(valid.is_ok());
        let _ = valid;
        self.last_surprise = surprise;
        self.last_outbound = outbound;
        self.turns = self.turns.saturating_add(1);
        InnerTurn {
            outbound,
            surprise,
            learned: learned_now,
            learn_refused: self.learn_refused,
        }
    }

    fn compose(&self, t_ms: u32, consent: u8, surprise: f32) -> IntentMsg {
        let mut msg = IntentMsg::hold(t_ms);
        msg.source = IntentSource::Own;
        msg.consent = consent & 0x0f;
        if surprise > 0.0 {
            msg.verb = pack_token(b"surprise");
            msg.strength = clamp01(surprise);
            msg.pull[VALENCE] = SURPRISE_VALENCE;
            msg.pull[AROUSAL] = SURPRISE_AROUSAL;
            msg.pull[DOMINANCE] = SURPRISE_DOMINANCE;
            msg.pull[APPROACH] = SURPRISE_APPROACH;
            msg.pull_mask = PULL_MASK_ALL;
        } else if let Some(idx) = lead(&self.wants) {
            msg.verb = self.wants[idx].token;
            msg.strength = clamp01(self.wants[idx].level);
        }
        msg
    }

    fn holds(&self, token: &[u8; VERB_BYTES]) -> bool {
        self.cares.iter().any(|m| m.token == *token)
            || self.wants.iter().any(|m| m.token == *token)
            || self.expects.iter().any(|m| m.token == *token)
            || self.learned.iter().any(|m| m.token == *token)
    }

    /// `allowed` is the lock. The public turn passes [`learning_allowed`].
    fn store_learned(&mut self, token: [u8; VERB_BYTES], allowed: bool) {
        if !allowed {
            self.learn_refused = self.learn_refused.saturating_add(1);
            return;
        }
        if self.learned.iter().any(|m| m.token == token) {
            return;
        }
        for slot in &mut self.learned {
            if slot.is_empty() {
                *slot = Motive {
                    token,
                    level: LEARNED_LEVEL,
                };
                return;
            }
        }
    }
}

fn occupied(slots: &[Motive], index: usize) -> Option<Motive> {
    slots.get(index).copied().filter(|m| !m.is_empty())
}

fn lead(slots: &[Motive]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, motive) in slots.iter().enumerate() {
        if motive.is_empty() {
            continue;
        }
        match best {
            None => best = Some(i),
            Some(j) if motive.level > slots[j].level => best = Some(i),
            _ => {}
        }
    }
    best
}

fn axis(point: [f32; AXES], index: usize, lo: f32, hi: f32) -> f32 {
    let v = point[index];
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        0.0
    }
}

fn belief_target(arousal: f32) -> f32 {
    clamp01(0.5 + 0.5 * arousal)
}

fn has_question(text: &[u8]) -> bool {
    text.iter().any(|b| *b == b'?')
}

fn observe_dialogue(text: &[u8]) -> Option<[u8; VERB_BYTES]> {
    let mut i = 0;
    while let Some((start, end)) = next_word(text, i) {
        if let Some(token) = canon_cue(&text[start..end]) {
            return Some(token);
        }
        i = end;
    }
    if has_question(text) {
        Some(pack_token(b"question"))
    } else {
        None
    }
}

fn observe_verb(verb: &[u8; VERB_BYTES]) -> Option<[u8; VERB_BYTES]> {
    let n = token_len(verb);
    if n == 0 {
        return None;
    }
    let word = &verb[..n];
    if word_eq(word, b"hold") || word_eq(word, b"steady") {
        return None;
    }
    if let Some(token) = canon_cue(word) {
        return Some(token);
    }
    Some(pack_lower(word))
}

fn canon_cue(word: &[u8]) -> Option<[u8; VERB_BYTES]> {
    if word_eq(word, b"wait") {
        return Some(pack_token(b"pause"));
    }
    if word_eq(word, b"thank") || word_eq(word, b"thanks") {
        return Some(pack_token(b"thanks"));
    }
    if is_cue(word) {
        return Some(pack_lower(word));
    }
    None
}

fn is_cue(word: &[u8]) -> bool {
    CUES.iter().any(|cue| word_eq(word, cue))
}

fn novel_token(life: &InnerLife, text: &[u8]) -> Option<[u8; VERB_BYTES]> {
    let mut i = 0;
    while let Some((start, end)) = next_word(text, i) {
        let word = &text[start..end];
        if (3..=VERB_BYTES).contains(&word.len()) && !is_cue(word) {
            let packed = pack_lower(word);
            if !life.holds(&packed) {
                return Some(packed);
            }
        }
        i = end;
    }
    None
}

fn find_prefer(text: &[u8], prefer: &[&[u8]]) -> [u8; VERB_BYTES] {
    for needle in prefer {
        if contains_word(text, needle) {
            return pack_token(needle);
        }
    }
    if let Some((start, end)) = next_word(text, 0) {
        return pack_lower(&text[start..end]);
    }
    pack_token(b"hold")
}

fn contains_word(text: &[u8], needle: &[u8]) -> bool {
    let mut i = 0;
    while let Some((start, end)) = next_word(text, i) {
        if word_eq(&text[start..end], needle) {
            return true;
        }
        i = end;
    }
    false
}

fn next_word(text: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    while i < text.len() && !text[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i >= text.len() {
        return None;
    }
    let start = i;
    while i < text.len() && text[i].is_ascii_alphabetic() {
        i += 1;
    }
    Some((start, i))
}

fn word_eq(word: &[u8], needle: &[u8]) -> bool {
    if word.len() != needle.len() {
        return false;
    }
    word.iter()
        .zip(needle.iter())
        .all(|(a, b)| a.to_ascii_lowercase() == *b)
}

fn pack_lower(word: &[u8]) -> [u8; VERB_BYTES] {
    let mut out = [0u8; VERB_BYTES];
    let n = word.len().min(VERB_BYTES);
    for i in 0..n {
        out[i] = word[i].to_ascii_lowercase();
    }
    out
}

const fn pack_token(bytes: &[u8]) -> [u8; VERB_BYTES] {
    let mut out = [0u8; VERB_BYTES];
    let mut i = 0;
    while i < bytes.len() && i < VERB_BYTES {
        out[i] = bytes[i];
        i += 1;
    }
    out
}

fn token_len(token: &[u8; VERB_BYTES]) -> usize {
    let mut i = 0;
    while i < VERB_BYTES {
        if token[i] == 0 {
            return i;
        }
        i += 1;
    }
    VERB_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::INTENT_BYTES;

    fn point() -> [f32; AXES] {
        [0.15, 0.35, 0.48, 0.05]
    }

    fn view<'a>(t_ms: u32, dialogue: &'a [u8], inbound: Option<&'a IntentMsg>) -> TurnView<'a> {
        TurnView {
            t_ms,
            dialogue,
            point: point(),
            inbound,
            consent: 0,
        }
    }

    #[test]
    fn prose_seed_matches_the_const_seed() {
        let life = InnerLife::from_prose(
            "the person on the line is heard",
            "answer the question that was asked",
            "a short pause before speaking",
        );
        assert_eq!(life, InnerLife::seeded());
        assert_eq!(life.care(0).unwrap().token_str(), "heard");
        assert_eq!(life.want(0).unwrap().token_str(), "answer");
        assert_eq!(life.expect(0).unwrap().token_str(), "pause");
        assert_eq!(life.learned_count(), 0);
    }

    #[test]
    fn same_view_twice_is_the_same_state_and_the_same_bytes() {
        let run = || {
            let mut life = InnerLife::seeded();
            let mut last = [0u8; INTENT_BYTES];
            for (i, line) in [b"pause".as_slice(), b"what?", b"kettle please"]
                .into_iter()
                .enumerate()
            {
                let turn = life.turn(view(i as u32 * 80, line, None));
                last = turn.outbound.encode().unwrap();
            }
            (life, last)
        };
        let (a, a_bytes) = run();
        let (b, b_bytes) = run();
        assert_eq!(a, b);
        assert_eq!(a_bytes, b_bytes);
        assert_eq!(a.turns(), 3);
    }

    #[test]
    fn a_question_surprises_and_the_same_act_does_not() {
        let mut life = InnerLife::seeded();
        let first = life.turn(view(0, b"what?", None));
        assert_eq!(first.surprise.to_bits(), EXPECT_LEVEL.to_bits());
        assert_eq!(first.outbound.verb_str(), "surprise");
        assert_eq!(first.outbound.strength.to_bits(), 1.0f32.to_bits());
        assert_eq!(
            first.outbound.pull[AROUSAL].to_bits(),
            SURPRISE_AROUSAL.to_bits()
        );
        assert_eq!(first.outbound.pull_mask, PULL_MASK_ALL);
        assert_eq!(first.outbound.source, IntentSource::Own);
        assert_eq!(life.expect(0).unwrap().token_str(), "question");
        assert_eq!(
            life.expect(0).unwrap().level().to_bits(),
            BELIEF_AFTER_MISS.to_bits()
        );
        assert!(!first.learned);
        assert!(first.learn_refused >= 1);

        let second = life.turn(view(80, b"what?", None));
        assert_eq!(second.surprise.to_bits(), 0.0f32.to_bits());
        assert_eq!(second.outbound.verb_str(), "answer");
        assert_eq!(second.outbound.pull_mask, 0);
    }

    #[test]
    fn wait_matches_pause_and_an_inbound_verb_wins() {
        let mut life = InnerLife::seeded();
        let matched = life.turn(view(0, b"please wait", None));
        assert_eq!(matched.surprise.to_bits(), 0.0f32.to_bits());
        assert_eq!(life.expect(0).unwrap().token_str(), "pause");

        let mut inbound = IntentMsg::hold(80);
        inbound.source = IntentSource::Inbound;
        inbound.set_verb("rush").unwrap();
        let missed = life.turn(view(80, b"pause", Some(&inbound)));
        assert_eq!(missed.surprise.to_bits(), EXPECT_LEVEL.to_bits());
        assert_eq!(life.expect(0).unwrap().token_str(), "rush");
        assert_eq!(life.learned_count(), 0);
        assert_eq!(life.care(0).unwrap().token_str(), "heard");
        assert_eq!(life.want(0).unwrap().token_str(), "answer");
    }

    #[test]
    fn gemi_bytes_roundtrip_in_both_directions() {
        let mut life = InnerLife::seeded();
        let own = life.turn(view(0, b"what?", None));
        let own_bytes = own.outbound.encode().unwrap();
        assert_eq!(own_bytes.len(), INTENT_BYTES);
        assert_eq!(&own_bytes[0..4], b"GEMI");
        assert_eq!(IntentMsg::decode(&own_bytes).unwrap(), own.outbound);
        assert_eq!(life.emit_gemi().unwrap(), own_bytes);

        let mut inbound = IntentMsg::hold(80);
        inbound.source = IntentSource::Inbound;
        inbound.set_verb("rush").unwrap();
        let in_bytes = inbound.encode().unwrap();
        assert_eq!(IntentMsg::decode(&in_bytes).unwrap(), inbound);

        let mut other = InnerLife::seeded();
        let back = other.consume_gemi(&in_bytes, 80, b"", point(), 0).unwrap();
        assert!(back.surprise > 0.0);
        assert_eq!(back.outbound.source, IntentSource::Own);
        let reply = back.outbound.encode().unwrap();
        assert_eq!(&reply[0..4], b"GEMI");
        assert_eq!(IntentMsg::decode(&reply).unwrap(), back.outbound);
        assert_eq!(other.learned_count(), 0);

        let mut echo = InnerLife::seeded();
        let echoed = echo.consume_gemi(&reply, 0, b"", point(), 0).unwrap();
        assert_eq!(IntentMsg::decode(&reply).unwrap().verb_str(), "surprise");
        assert!(echoed.surprise > 0.0);
        assert_eq!(echo.expect(0).unwrap().token_str(), "surprise");
    }

    #[test]
    fn a_bad_gemi_buffer_does_not_change_the_life() {
        let mut life = InnerLife::seeded();
        let before = life;
        let err = life
            .consume_gemi(&[0u8; INTENT_BYTES], 0, b"what?", point(), 0)
            .unwrap_err();
        assert_eq!(err, CoreError::BadIntent);
        assert_eq!(life, before);
    }

    #[test]
    fn denied_consent_stores_no_motive_and_an_allow_flag_would() {
        assert_eq!(LEARN_LOCK, "deny");
        assert!(!learning_allowed());
        let mut life = InnerLife::seeded();
        let expect = life.expect(0).unwrap();
        for i in 0..8u32 {
            let turn = life.turn(view(i * 80, b"kettle please", None));
            assert_eq!(turn.surprise.to_bits(), 0.0f32.to_bits());
            assert!(!turn.learned);
        }
        assert_eq!(life.learned_count(), 0);
        assert_eq!(life.learn_refused(), 8);
        assert_eq!(life.care(0).unwrap().token_str(), "heard");
        assert_eq!(life.want(0).unwrap().token_str(), "answer");
        assert_eq!(life.expect(0).unwrap().token_str(), expect.token_str());
        for i in 0..LEARNED_SLOTS {
            assert!(life.learned(i).is_none());
        }

        let mut allowed = InnerLife::seeded();
        allowed.store_learned(pack_lower(b"kettle"), true);
        assert_eq!(allowed.learned_count(), 1);
        assert_eq!(allowed.learned(0).unwrap().token_str(), "kettle");
        assert_eq!(allowed.care(0).unwrap().token_str(), "heard");
        assert_eq!(allowed.want(0).unwrap().token_str(), "answer");
        allowed.store_learned(pack_lower(b"kettle"), true);
        assert_eq!(allowed.learned_count(), 1);
        let mut denied = InnerLife::seeded();
        denied.store_learned(pack_lower(b"kettle"), false);
        assert_eq!(denied.learned_count(), 0);
        assert_eq!(denied.learn_refused(), 1);
    }

    #[test]
    fn learn_lock_matches_locks_toml() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        let got = consent_learn(&text)
            .unwrap_or_else(|| panic!("no [consent] learn in {}", path.display()));
        assert_eq!(got, LEARN_LOCK);
        assert_eq!(got, "deny");
    }

    fn consent_learn(text: &str) -> Option<&str> {
        let mut in_consent = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_consent = trimmed == "[consent]";
                continue;
            }
            if !in_consent || trimmed.starts_with('#') {
                continue;
            }
            let Some(rest) = trimmed.strip_prefix("learn") else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('"') else {
                continue;
            };
            let end = rest.find('"')?;
            return Some(&rest[..end]);
        }
        None
    }
}
