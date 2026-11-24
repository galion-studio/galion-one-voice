// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Cares, wants, and expectations.
//!
//! This is the inner-life layer. A care or a want is an open token in a
//! fixed slot, not an emotion class. The slot counts are buffer sizes.
//! The emotion point is the moment on the four axes. Expectation mismatch
//! is a scalar, and the nudge it produces is a [`crate::IntentMsg`] pull.
//! [`crate::EmotionCore::step`] applies that pull. This module does not
//! hold a [`crate::VoiceBaseline`] and does not write one.
//!
//! `turn` does not allocate. Durable weights change only in [`InnerLife::learn`],
//! and only when `LOCKS.toml` `[consent] learn` is `allow`.

use crate::axes::{clamp_pos, AROUSAL, AXES, PULL_LIMIT, VALENCE};
use crate::baseline::VoiceBaseline;
use crate::intent::{validate, IntentMsg, IntentSource, VERB_BYTES};
use crate::CoreError;

/// Compiled lock text. The turn does not open the file.
const LOCKS_TEXT: &str = include_str!("../../../LOCKS.toml");

pub const CARE_SLOTS: usize = 4;
pub const WANT_SLOTS: usize = 4;
pub const LABEL_BYTES: usize = 32;
pub const CUE_BYTES: usize = 16;

/// Seed phrases. They match `standins/inner-life.txt`. The cue is the last word.
pub const SEED_CARE: &str = "the person on the line is heard";
pub const SEED_WANT: &str = "answer the question that was asked";
pub const SEED_EXPECT: &str = "a short pause before speaking";

/// Initial strength of a seeded care and a seeded want. Dyadic, so the
/// step `SEED_STRENGTH + STRENGTH_STEP` is exact in f32.
pub const SEED_STRENGTH: f32 = 0.5;
pub const STRENGTH_STEP: f32 = 0.25;
pub const STRENGTH_DECAY: f32 = 0.75;
pub const EXPECT_HOLD: f32 = 1.0;

/// Mean absolute axis gap plus this much when the expected token is missed.
pub const WORD_SURPRISE: f32 = 0.5;
/// Below this, the outbound verb stays `hold` and the pull stays zero.
pub const SURPRISE_MIN: f32 = 0.0625;
/// Arousal pull when the words missed and the point did not move.
pub const STARTLE_AROUSAL: f32 = 0.5;
/// How far a warm or cool cue shifts the next expected valence.
pub const LEX_VALENCE: f32 = 0.125;
/// How far the next expected point moves toward the strongest care.
pub const CARE_PULL: f32 = 0.125;
/// Fraction of an inbound pull folded into the expected point.
pub const INBOUND_FOLD: f32 = 0.25;
/// One dialogue turn, including encode of the intent it just built.
pub const TURN_ALLOC_BUDGET: usize = 0;

const VERB_SURPRISE: [u8; VERB_BYTES] = *b"surprise\0\0\0\0\0\0\0\0";

/// Lexical valence cues. Not an emotion inventory. A hit moves the next
/// expected valence by [`LEX_VALENCE`]. It does not pick a class.
const WARM: &[&[u8]] = &[b"thanks", b"good", b"yes", b"glad"];
const COOL: &[&[u8]] = &[b"no", b"not", b"stop", b"wrong"];

/// One care. `lean` is a preferred point on the axes, seeded from the
/// baseline and not moved by a turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Care {
    pub label: [u8; LABEL_BYTES],
    pub cue: [u8; CUE_BYTES],
    pub strength: f32,
    pub lean: [f32; AXES],
}

impl Care {
    pub fn label_str(&self) -> &str {
        token_str(&self.label)
    }
    pub fn cue_str(&self) -> &str {
        token_str(&self.cue)
    }
}

/// One want. `met` is 0 until the cue word shows up in a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    pub label: [u8; LABEL_BYTES],
    pub cue: [u8; CUE_BYTES],
    pub strength: f32,
    pub met: f32,
}

impl Want {
    pub fn label_str(&self) -> &str {
        token_str(&self.label)
    }
    pub fn cue_str(&self) -> &str {
        token_str(&self.cue)
    }
}

/// What the next turn was expected to feel like, and which token was expected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expect {
    pub label: [u8; LABEL_BYTES],
    pub cue: [u8; CUE_BYTES],
    pub point: [f32; AXES],
    pub hold: f32,
}

impl Expect {
    pub fn label_str(&self) -> &str {
        token_str(&self.label)
    }
    pub fn cue_str(&self) -> &str {
        token_str(&self.cue)
    }
}

/// One dialogue turn. `point` is the emotion point, copied, not a baseline.
#[derive(Clone, Copy, Debug)]
pub struct TurnIn<'a> {
    pub line: &'a str,
    pub point: [f32; AXES],
    pub t_ms: u32,
    pub consent: u8,
}

/// What the turn measured and the GEMI message for the MnE side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnOut {
    pub surprise: f32,
    pub point_gap: f32,
    pub word_gap: f32,
    pub intent: IntentMsg,
}

#[derive(Clone, Copy, Debug)]
struct Scan {
    care_hit: [bool; CARE_SLOTS],
    want_hit: [bool; WANT_SLOTS],
    expect_hit: bool,
    saw_word: bool,
    first: [u8; CUE_BYTES],
    first_len: usize,
    lex: f32,
}

/// Live cares, wants, and expectations, plus the kept copy `learn` may write.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnerLife {
    cares: [Care; CARE_SLOTS],
    wants: [Want; WANT_SLOTS],
    expect: Expect,
    kept_cares: [Care; CARE_SLOTS],
    kept_wants: [Want; WANT_SLOTS],
    kept_expect: Expect,
    permit: bool,
    last_surprise: f32,
    last_outbound: IntentMsg,
    last_inbound: Option<IntentMsg>,
    inbound_frames: u32,
    turns: u32,
}

impl InnerLife {
    /// Stand-in speaker, seed phrases, learning from the compiled lock.
    pub fn at_rest() -> Self {
        Self::seed(
            VoiceBaseline::standin(),
            SEED_CARE,
            SEED_WANT,
            SEED_EXPECT,
            learn_permitted(LOCKS_TEXT),
        )
    }

    /// Seed from the stand-in record. Persona and memory are not read here.
    pub fn from_standin(baseline: VoiceBaseline, inner: &crate::standin::InnerLifeStandin) -> Self {
        Self::seed(
            baseline,
            &inner.care,
            &inner.want,
            &inner.expect,
            learn_permitted(LOCKS_TEXT),
        )
    }

    fn seed(baseline: VoiceBaseline, care: &str, want: &str, expect: &str, permit: bool) -> Self {
        let mut cares = [blank_care(); CARE_SLOTS];
        let mut wants = [blank_want(); WANT_SLOTS];
        cares[0] = care_from(care, baseline.pos);
        wants[0] = want_from(want);
        let expect = expect_from(expect, baseline.pos);
        Self {
            cares,
            wants,
            expect,
            kept_cares: cares,
            kept_wants: wants,
            kept_expect: expect,
            permit,
            last_surprise: 0.0,
            last_outbound: IntentMsg::steady(0),
            last_inbound: None,
            inbound_frames: 0,
            turns: 0,
        }
    }

    pub fn cares(&self) -> &[Care; CARE_SLOTS] {
        &self.cares
    }
    pub fn wants(&self) -> &[Want; WANT_SLOTS] {
        &self.wants
    }
    pub fn expect(&self) -> Expect {
        self.expect
    }
    pub fn kept_cares(&self) -> &[Care; CARE_SLOTS] {
        &self.kept_cares
    }
    pub fn kept_wants(&self) -> &[Want; WANT_SLOTS] {
        &self.kept_wants
    }
    pub fn kept_expect(&self) -> Expect {
        self.kept_expect
    }
    pub fn surprise(&self) -> f32 {
        self.last_surprise
    }
    pub fn last_outbound(&self) -> IntentMsg {
        self.last_outbound
    }
    pub fn last_inbound(&self) -> Option<IntentMsg> {
        self.last_inbound
    }
    pub fn inbound_frames(&self) -> u32 {
        self.inbound_frames
    }
    pub fn turns(&self) -> u32 {
        self.turns
    }
    pub fn learn_allowed(&self) -> bool {
        self.permit
    }

    /// Update live state from one line and one emotion point.
    /// The returned intent is the nudge. The caller passes it to
    /// [`crate::EmotionCore::step`]. This function does not step the core.
    pub fn turn(&mut self, input: TurnIn<'_>) -> TurnOut {
        let scan = scan_line(input.line, &self.cares, &self.wants, &self.expect.cue);
        let mut delta = [0.0; AXES];
        for i in 0..AXES {
            delta[i] = input.point[i] - self.expect.point[i];
        }
        let point_gap = mean_abs(&delta);
        let word_gap = if scan.saw_word && !cue_empty(&self.expect.cue) && !scan.expect_hit {
            1.0
        } else {
            0.0
        };
        let surprise = (self.expect.hold * (point_gap + word_gap * WORD_SURPRISE)).clamp(0.0, 1.0);
        let intent = outbound(input.t_ms, input.consent, surprise, &delta);

        for i in 0..CARE_SLOTS {
            if cue_empty(&self.cares[i].cue) {
                continue;
            }
            self.cares[i].strength = if scan.care_hit[i] {
                (self.cares[i].strength + STRENGTH_STEP).clamp(0.0, 1.0)
            } else {
                (self.cares[i].strength * STRENGTH_DECAY).clamp(0.0, 1.0)
            };
        }
        for i in 0..WANT_SLOTS {
            if cue_empty(&self.wants[i].cue) {
                continue;
            }
            if scan.want_hit[i] {
                self.wants[i].strength = (self.wants[i].strength + STRENGTH_STEP).clamp(0.0, 1.0);
                self.wants[i].met = 1.0;
            } else {
                self.wants[i].strength = (self.wants[i].strength * STRENGTH_DECAY).clamp(0.0, 1.0);
            }
        }

        for i in 0..AXES {
            self.expect.point[i] = clamp_pos(i, input.point[i]);
        }
        self.expect.point[VALENCE] =
            clamp_pos(VALENCE, self.expect.point[VALENCE] + LEX_VALENCE * scan.lex);
        if let Some(lean) = strongest_lean(&self.cares) {
            for i in 0..AXES {
                let toward = lean[i] - self.expect.point[i];
                self.expect.point[i] = clamp_pos(i, self.expect.point[i] + CARE_PULL * toward);
            }
        }
        if scan.first_len > 0 {
            self.expect.cue = scan.first;
        }

        self.last_surprise = surprise;
        self.last_outbound = intent;
        self.turns = self.turns.wrapping_add(1);
        TurnOut {
            surprise,
            point_gap,
            word_gap,
            intent,
        }
    }

    /// Copy live cares, wants, and the expectation into the kept weights.
    /// Returns false, and writes nothing, unless the lock allows it.
    pub fn learn(&mut self) -> bool {
        if !self.permit {
            return false;
        }
        self.kept_cares = self.cares;
        self.kept_wants = self.wants;
        self.kept_expect = self.expect;
        true
    }

    /// Inbound GEMI from the MnE side. Same message type as [`Self::turn`].
    pub fn take_intent(&mut self, msg: &IntentMsg) -> Result<(), CoreError> {
        validate(msg)?;
        self.fold_inbound(msg);
        Ok(())
    }

    pub fn take_bytes(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        let msg = IntentMsg::decode(bytes)?;
        self.fold_inbound(&msg);
        Ok(())
    }

    fn fold_inbound(&mut self, msg: &IntentMsg) {
        self.last_inbound = Some(*msg);
        self.inbound_frames = self.inbound_frames.wrapping_add(1);
        let scale = INBOUND_FOLD * msg.strength;
        if scale == 0.0 {
            return;
        }
        for i in 0..AXES {
            if msg.pull_mask & (1 << i) != 0 {
                self.expect.point[i] = clamp_pos(i, self.expect.point[i] + scale * msg.pull[i]);
            }
        }
    }
}

/// `true` only when `[consent]` contains `learn = "allow"`.
/// A missing key, `deny`, and the same key under another table are deny.
pub fn learn_permitted(locks_toml: &str) -> bool {
    let mut in_consent = false;
    let mut allow = false;
    for raw in locks_toml.lines() {
        let line = strip_comment(raw);
        if line.is_empty() {
            continue;
        }
        if line.as_bytes().first() == Some(&b'[') {
            in_consent = line == "[consent]";
            continue;
        }
        if !in_consent {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "learn" {
            continue;
        }
        allow = value_is_allow(value);
    }
    allow
}

fn outbound(t_ms: u32, consent: u8, surprise: f32, delta: &[f32; AXES]) -> IntentMsg {
    let mut msg = IntentMsg::steady(t_ms);
    msg.consent = consent & 0x0f;
    msg.source = IntentSource::Own;
    if surprise < SURPRISE_MIN {
        return msg;
    }
    let mut pull = [0.0; AXES];
    let mut any = false;
    for i in 0..AXES {
        if delta[i] != 0.0 {
            pull[i] = clamp_pull(delta[i]);
            any = true;
        }
    }
    if !any {
        pull[AROUSAL] = STARTLE_AROUSAL;
    }
    let mut mask = 0u8;
    for i in 0..AXES {
        if pull[i] != 0.0 {
            mask |= 1 << i;
        }
    }
    msg.verb = VERB_SURPRISE;
    msg.strength = surprise;
    msg.pull = pull;
    msg.pull_mask = mask;
    if validate(&msg).is_err() {
        let mut hold = IntentMsg::steady(t_ms);
        hold.consent = consent & 0x0f;
        return hold;
    }
    msg
}

fn scan_line(
    line: &str,
    cares: &[Care; CARE_SLOTS],
    wants: &[Want; WANT_SLOTS],
    expect_cue: &[u8; CUE_BYTES],
) -> Scan {
    let bytes = line.as_bytes();
    let mut scan = Scan {
        care_hit: [false; CARE_SLOTS],
        want_hit: [false; WANT_SLOTS],
        expect_hit: false,
        saw_word: false,
        first: [0; CUE_BYTES],
        first_len: 0,
        lex: 0.0,
    };
    let mut i = 0;
    let mut warm = false;
    let mut cool = false;
    let mut saw_first = false;
    while i < bytes.len() {
        while i < bytes.len() && !is_word(bytes[i]) {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && is_word(bytes[i]) {
            i += 1;
        }
        if start == i {
            break;
        }
        let word = &bytes[start..i];
        scan.saw_word = true;
        if !saw_first {
            saw_first = true;
            if word.len() <= CUE_BYTES {
                for (n, b) in word.iter().enumerate() {
                    scan.first[n] = b.to_ascii_lowercase();
                }
                scan.first_len = word.len();
            }
        }
        if eq_cue(word, expect_cue) {
            scan.expect_hit = true;
        }
        for n in 0..CARE_SLOTS {
            if eq_cue(word, &cares[n].cue) {
                scan.care_hit[n] = true;
            }
        }
        for n in 0..WANT_SLOTS {
            if eq_cue(word, &wants[n].cue) {
                scan.want_hit[n] = true;
            }
        }
        if eq_list(word, WARM) {
            warm = true;
        }
        if eq_list(word, COOL) {
            cool = true;
        }
    }
    if warm {
        scan.lex += 1.0;
    }
    if cool {
        scan.lex -= 1.0;
    }
    scan
}

fn care_from(text: &str, lean: [f32; AXES]) -> Care {
    let cue = last_cue(text);
    if cue_empty(&cue) {
        return blank_care();
    }
    Care {
        label: pack_label(text),
        cue,
        strength: SEED_STRENGTH,
        lean,
    }
}

fn want_from(text: &str) -> Want {
    let cue = last_cue(text);
    if cue_empty(&cue) {
        return blank_want();
    }
    Want {
        label: pack_label(text),
        cue,
        strength: SEED_STRENGTH,
        met: 0.0,
    }
}

fn expect_from(text: &str, point: [f32; AXES]) -> Expect {
    Expect {
        label: pack_label(text),
        cue: last_cue(text),
        point,
        hold: EXPECT_HOLD,
    }
}

fn blank_care() -> Care {
    Care {
        label: [0; LABEL_BYTES],
        cue: [0; CUE_BYTES],
        strength: 0.0,
        lean: [0.0; AXES],
    }
}

fn blank_want() -> Want {
    Want {
        label: [0; LABEL_BYTES],
        cue: [0; CUE_BYTES],
        strength: 0.0,
        met: 0.0,
    }
}

fn pack_label(text: &str) -> [u8; LABEL_BYTES] {
    let mut out = [0u8; LABEL_BYTES];
    for (i, b) in text.as_bytes().iter().take(LABEL_BYTES).enumerate() {
        if !(0x20..=0x7e).contains(b) {
            break;
        }
        out[i] = *b;
    }
    out
}

fn last_cue(text: &str) -> [u8; CUE_BYTES] {
    let bytes = text.as_bytes();
    let mut end = bytes.len();
    while end > 0 && !is_word(bytes[end - 1]) {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let word = &bytes[start..end];
    let mut out = [0u8; CUE_BYTES];
    if word.is_empty() || word.len() > CUE_BYTES {
        return out;
    }
    for (i, b) in word.iter().enumerate() {
        out[i] = b.to_ascii_lowercase();
    }
    out
}

fn cue_empty(cue: &[u8; CUE_BYTES]) -> bool {
    cue[0] == 0
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'\'' || b == b'-'
}

fn eq_cue(word: &[u8], cue: &[u8; CUE_BYTES]) -> bool {
    let n = cue.iter().position(|b| *b == 0).unwrap_or(CUE_BYTES);
    eq_bytes(word, &cue[..n])
}

fn eq_list(word: &[u8], list: &[&[u8]]) -> bool {
    list.iter().any(|cue| eq_bytes(word, cue))
}

fn eq_bytes(word: &[u8], cue: &[u8]) -> bool {
    if cue.is_empty() || word.len() != cue.len() {
        return false;
    }
    for (a, b) in word.iter().zip(cue.iter()) {
        if a.to_ascii_lowercase() != *b {
            return false;
        }
    }
    true
}

fn strongest_lean(cares: &[Care; CARE_SLOTS]) -> Option<[f32; AXES]> {
    let mut best = -1.0;
    let mut lean = [0.0; AXES];
    for care in cares {
        if cue_empty(&care.cue) {
            continue;
        }
        if care.strength > best {
            best = care.strength;
            lean = care.lean;
        }
    }
    if best < 0.0 {
        None
    } else {
        Some(lean)
    }
}

fn mean_abs(delta: &[f32; AXES]) -> f32 {
    let mut sum = 0.0;
    for v in delta {
        sum += if *v < 0.0 { -*v } else { *v };
    }
    sum * 0.25
}

fn clamp_pull(v: f32) -> f32 {
    if v > PULL_LIMIT {
        PULL_LIMIT
    } else if v < -PULL_LIMIT {
        -PULL_LIMIT
    } else {
        v
    }
}

fn token_str(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..end]).unwrap_or("")
}

fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_str = !in_str;
        } else if bytes[i] == b'#' && !in_str {
            return line[..i].trim();
        }
        i += 1;
    }
    line.trim()
}

fn value_is_allow(value: &str) -> bool {
    let v = value.trim();
    let v = v.strip_prefix('"').unwrap_or(v);
    let v = v.strip_suffix('"').unwrap_or(v);
    v == "allow"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_gate_reads_only_the_consent_table() {
        assert!(!learn_permitted(""));
        assert!(!learn_permitted("[consent]\nlearn = \"deny\"\n"));
        assert!(!learn_permitted("[consent]\n# learn = \"allow\"\n"));
        assert!(learn_permitted("[consent]\nlearn = \"allow\"\n"));
        assert!(!learn_permitted(
            "[formats]\nlearn = \"allow\"\n[consent]\nshare = \"deny\"\n"
        ));
        assert!(!learn_permitted(LOCKS_TEXT));
        assert!(!InnerLife::at_rest().learn_allowed());
        assert_eq!(InnerLife::at_rest().cares()[0].cue_str(), "heard");
        assert_eq!(InnerLife::at_rest().wants()[0].cue_str(), "asked");
        assert_eq!(InnerLife::at_rest().expect().cue_str(), "speaking");
    }

    #[test]
    fn learn_copies_weights_only_when_the_gate_is_open() {
        let point = InnerLife::at_rest().expect().point;
        let input = TurnIn {
            line: "heard speaking",
            point,
            t_ms: 0,
            consent: 0,
        };
        let mut denied = InnerLife::seed(
            VoiceBaseline::standin(),
            SEED_CARE,
            SEED_WANT,
            SEED_EXPECT,
            false,
        );
        denied.turn(input);
        assert!(denied.cares()[0].strength > denied.kept_cares()[0].strength);
        assert_eq!(denied.kept_cares()[0].strength, SEED_STRENGTH);
        assert!(!denied.learn());
        assert_eq!(denied.kept_cares()[0].strength, SEED_STRENGTH);
        assert_eq!(denied.kept_expect().cue_str(), "speaking");

        let mut allowed = InnerLife::seed(
            VoiceBaseline::standin(),
            SEED_CARE,
            SEED_WANT,
            SEED_EXPECT,
            true,
        );
        allowed.turn(input);
        assert!(allowed.learn());
        assert_eq!(
            allowed.kept_cares()[0].strength,
            allowed.cares()[0].strength
        );
        assert_eq!(allowed.kept_expect().cue_str(), allowed.expect().cue_str());
    }

    #[test]
    fn a_want_cue_marks_the_want_met() {
        let mut life = InnerLife::at_rest();
        let point = life.expect().point;
        let out = life.turn(TurnIn {
            line: "asked speaking",
            point,
            t_ms: 0,
            consent: 0,
        });
        assert_eq!(out.surprise, 0.0);
        assert_eq!(life.wants()[0].met, 1.0);
        assert!(life.wants()[0].strength > SEED_STRENGTH);
        assert_eq!(life.kept_wants()[0].met, 0.0);
    }
}
