// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Per-80 ms mouth targets.
//!
//! Phonetic posture (jaw, tongue, lips, viseme) comes from the phone class.
//! Emotion and intent add a shift on top. A shift with an unknown step stays
//! null: an unknown size is not treated as zero, so it blocks the numeric sum.
//! Speech signs apply on the prosody track only.
//!
//! The viseme clock uses the rate of the feeling that covers t = 0. A later
//! feeling still reports its own rate target. Mid-utterance retiming is not
//! done; onset shape was not read. Speaking faster does not change the
//! coarticulation weight (Lindblom 1963 and Gay 1978 do not agree on a size).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::coart::{self, Blend};
use crate::direction::{self, GARNIER, HH, LINDBLOM_1963, OHMAN, TARTTER};
use crate::error::MouthError;
use crate::feeling::{self, FeelingTick};
use crate::intent::{IntentStub, PrecisionIntent, CONTRACT_SCHEMA, CONTRACT_TODO};
use crate::inventory::Category;
use crate::phone::{self, PhoneClass};
use crate::scales::Scales;
use crate::TICK_MS;

pub const SCHEMA: &str = "galion-mouth/1";
pub const CLONER_SCHEMA: &str = "galion-mouth-cloner/1";
pub const AVATAR_SCHEMA: &str = "galion-mouth-viseme/1";

pub const INTEGRATOR: &str = "Integrator bc-ed0d7d68 (draft PR 194) reads clone_mouth next to clone_request_patch. Apply one speaking-rate multiplier. emotion-render prosody.rate uses the same Juslin/Banse signs. This packet does not set exaggeration, cfg_weight, or temperature.";

pub const AVATAR_NOTE: &str = "Shapes are Rhubarb Lip Sync A-H and X. Any avatar maps those letters. This crate does not emit a face rig. cues are phone intervals. ticks are the shape at each 80 ms tick centre, the same rule as galion-articulation add_rhubarb.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub direction: String,
    pub amount: Option<f64>,
    pub cite: String,
    pub blocked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shift {
    pub direction: String,
    pub amount: Option<f64>,
    pub unit: String,
    pub components: Vec<Component>,
}

impl Shift {
    fn not_applied(unit: &str) -> Self {
        Self {
            direction: "not_applied".into(),
            amount: None,
            unit: unit.into(),
            components: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouthTick {
    pub t_ms: u32,
    pub phone: String,
    pub phone_known: bool,
    pub viseme: String,
    pub f1: Shift,
    pub f2: Shift,
    pub f3: Shift,
    pub lips: String,
    /// +1 spread, 0 neutral, −1 round, after coarticulation. Not a millimetre
    /// opening and not a SPARC unit. The phone's own word stays in `lips`.
    pub lips_blended: Option<f64>,
    pub lip_intent: Shift,
    pub lip_opening: Shift,
    pub jaw: String,
    /// Planning coordinate in −1..=1 after coarticulation. Null when the
    /// weight is unknown, or when a consonant has weight 0. Not millimetres.
    /// +1 open, −1 close.
    pub jaw_blended: Option<f64>,
    pub jaw_emotion: Shift,
    pub tongue_front: String,
    pub tongue_front_blended: Option<f64>,
    pub tongue_height: String,
    pub tongue_height_blended: Option<f64>,
    pub tongue_tip: String,
    pub precision: String,
    pub precision_amount: Option<f64>,
    pub coarticulation_weight: Option<f64>,
    pub rate: Shift,
    pub feeling: String,
    pub track: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prosody_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_from: Option<String>,
    /// Set on a burst when the prosody row would have moved F1. Not a command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld_f1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld_rate: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisemeCue {
    pub shape: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub phone: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvatarTiming {
    pub schema: String,
    pub tick_ms: u32,
    pub shapes: String,
    pub cues: Vec<VisemeCue>,
    pub ticks: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub direction: String,
    pub amount: Option<f64>,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloneMouth {
    pub schema: String,
    pub tick_ms: u32,
    /// Rate that built the viseme clock. The feeling at t = 0.
    pub speaking_rate: Summary,
    pub timing_note: String,
    pub f1: Summary,
    pub f2: Summary,
    pub f3: Summary,
    pub lip_intent: Summary,
    pub lip_opening: Summary,
    pub jaw_emotion: Summary,
    pub precision: String,
    pub coarticulation_weight: Option<f64>,
    pub integrator: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntentEcho {
    pub schema: String,
    pub todo: String,
    pub text: String,
    pub smile: bool,
    pub address_in_noise: bool,
    pub precision: Option<String>,
    pub phone_count: usize,
    pub phones_are_caller_supplied: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouthPlan {
    pub schema: String,
    pub tick_ms: u32,
    pub n_ticks: usize,
    pub duration_ms: u32,
    pub intent: IntentEcho,
    pub clone_mouth: CloneMouth,
    pub avatar: AvatarTiming,
    pub ticks: Vec<MouthTick>,
    pub unknown: Vec<String>,
    pub citations: Vec<String>,
}

struct Placed {
    class: PhoneClass,
    start: u32,
    end: u32,
}

struct Fold {
    value: f64,
    blocked: bool,
    signs: Vec<i8>,
    components: Vec<Component>,
}

impl Fold {
    fn new() -> Self {
        Self {
            value: 0.0,
            blocked: false,
            signs: Vec::new(),
            components: Vec::new(),
        }
    }

    fn add(
        &mut self,
        name: &str,
        sign: i8,
        step: Option<f64>,
        weight: f64,
        cite: &str,
        positive: &str,
        negative: &str,
    ) {
        if sign == 0 || weight == 0.0 {
            return;
        }
        self.signs.push(sign);
        let direction = if sign > 0 { positive } else { negative };
        match step {
            Some(step) => {
                let amount = sign as f64 * step * weight;
                self.value += amount;
                self.components.push(Component {
                    name: name.into(),
                    direction: direction.into(),
                    amount: Some(amount),
                    cite: cite.into(),
                    blocked: false,
                });
            }
            None => {
                self.blocked = true;
                self.components.push(Component {
                    name: name.into(),
                    direction: direction.into(),
                    amount: None,
                    cite: cite.into(),
                    blocked: true,
                });
            }
        }
    }

    fn finish(self, unit: &str, positive: &str, negative: &str) -> (Shift, Vec<String>) {
        let mut unknown = Vec::new();
        if self.signs.is_empty() {
            return (Shift::not_applied(unit), unknown);
        }
        for c in &self.components {
            if c.blocked {
                unknown.push(c.name.clone());
            }
        }
        let same = self.signs.iter().all(|s| *s == self.signs[0]);
        let direction = if self.blocked {
            if same {
                if self.signs[0] > 0 {
                    positive
                } else {
                    negative
                }
            } else {
                "unresolved"
            }
        } else if self.value > 0.0 {
            positive
        } else if self.value < 0.0 {
            negative
        } else {
            "cancelled"
        };
        let amount = if self.blocked { None } else { Some(self.value) };
        (
            Shift {
                direction: direction.into(),
                amount,
                unit: unit.into(),
                components: self.components,
            },
            unknown,
        )
    }
}

pub fn plan_value(
    intent: &Value,
    feeling: &Value,
    scales: &Scales,
) -> Result<MouthPlan, MouthError> {
    let intent: IntentStub =
        serde_json::from_value(intent.clone()).map_err(|e| MouthError::Json(e.to_string()))?;
    let feelings = feeling::parse_feelings(feeling)?;
    plan(&intent, &feelings, scales)
}

pub fn plan(
    intent: &IntentStub,
    feelings: &[FeelingTick],
    scales: &Scales,
) -> Result<MouthPlan, MouthError> {
    let precision = intent.precision_intent()?;
    let rate_feel = feeling::feeling_at(feelings, 0);
    let (rate_shift, rate_unknown, _) = fold_rate(&rate_feel, scales);
    let timing_multiplier = rate_multiplier(&rate_shift)?;

    let specs = if intent.phones.is_empty() {
        vec![crate::intent::PhoneSpec {
            symbol: "sil".into(),
            duration_ms: None,
        }]
    } else {
        intent.phones.clone()
    };

    let mut cursor = 0u32;
    let mut placed = Vec::with_capacity(specs.len());
    for spec in &specs {
        if spec.duration_ms == Some(0) {
            return Err(MouthError::BadScale(
                "phone duration_ms must be at least 1".into(),
            ));
        }
        let base = spec.duration_ms.unwrap_or(scales.phone_ms);
        let dur = scale_duration(base, timing_multiplier)?;
        placed.push(Placed {
            class: phone::classify(&spec.symbol),
            start: cursor,
            end: cursor + dur,
        });
        cursor += dur;
    }
    let duration_ms = cursor;
    let n_ticks = ((duration_ms as f64) / TICK_MS as f64).ceil().max(1.0) as usize;

    let (weight, precision_amount, precision_word) = effective_weight(scales, precision.as_ref());
    let vowel_units: Vec<Option<crate::phone::Units>> =
        placed.iter().map(|p| p.class.units).collect();
    let blended = coart::blend(&vowel_units, weight);

    let mut unknown = BTreeSet::new();
    for name in rate_unknown {
        unknown.insert(name);
    }
    if weight.is_none() {
        unknown.insert("coarticulation".into());
    }
    if precision.is_some() && precision_amount.is_none() {
        unknown.insert("precision".into());
    }

    let mut ticks = Vec::with_capacity(n_ticks);
    for i in 0..n_ticks {
        let t_ms = i as u32 * TICK_MS;
        let centre = t_ms + TICK_MS / 2;
        let feel = feeling::feeling_at(feelings, centre);
        let (tick, more) = make_tick(
            t_ms,
            centre,
            &placed,
            &blended,
            &feel,
            scales,
            intent,
            precision_word,
            precision_amount,
            weight,
        );
        for name in more {
            unknown.insert(name);
        }
        ticks.push(tick);
    }

    let cues: Vec<VisemeCue> = placed
        .iter()
        .map(|p| VisemeCue {
            shape: p.class.viseme.into(),
            start_ms: p.start,
            end_ms: p.end,
            phone: p.class.symbol.clone(),
        })
        .collect();
    let avatar_ticks: Vec<String> = ticks.iter().map(|t| t.viseme.clone()).collect();

    Ok(MouthPlan {
        schema: SCHEMA.into(),
        tick_ms: TICK_MS,
        n_ticks,
        duration_ms,
        intent: IntentEcho {
            schema: CONTRACT_SCHEMA.into(),
            todo: CONTRACT_TODO.into(),
            text: intent.text.clone(),
            smile: intent.smile,
            address_in_noise: intent.address_in_noise,
            precision: intent.precision.clone(),
            phone_count: intent.phones.len(),
            phones_are_caller_supplied: !intent.phones.is_empty(),
        },
        clone_mouth: CloneMouth {
            schema: CLONER_SCHEMA.into(),
            tick_ms: TICK_MS,
            speaking_rate: Summary {
                direction: rate_shift.direction.clone(),
                amount: timing_multiplier,
                unit: "multiplier".into(),
            },
            timing_note: "Viseme cues use the rate of the feeling at t = 0. Each tick still reports the rate target of the feeling that covers that tick. A later change does not rebuild the cue clock.".into(),
            f1: summary_of(ticks.iter().map(|t| &t.f1)),
            f2: summary_of(ticks.iter().map(|t| &t.f2)),
            f3: summary_of(ticks.iter().map(|t| &t.f3)),
            lip_intent: summary_of(ticks.iter().map(|t| &t.lip_intent)),
            lip_opening: summary_of(ticks.iter().map(|t| &t.lip_opening)),
            jaw_emotion: summary_of(ticks.iter().map(|t| &t.jaw_emotion)),
            precision: precision_word.into(),
            coarticulation_weight: weight,
            integrator: INTEGRATOR.into(),
        },
        avatar: AvatarTiming {
            schema: AVATAR_SCHEMA.into(),
            tick_ms: TICK_MS,
            shapes: "Rhubarb A-H, X".into(),
            cues,
            ticks: avatar_ticks,
            note: AVATAR_NOTE.into(),
        },
        citations: citation_list(&ticks),
        ticks,
        unknown: unknown.into_iter().collect(),
    })
}

fn summary_of<'a>(shifts: impl Iterator<Item = &'a Shift>) -> Summary {
    let rows: Vec<&Shift> = shifts.collect();
    let Some(first) = rows.first() else {
        return Summary {
            direction: "not_applied".into(),
            amount: None,
            unit: String::new(),
        };
    };
    let same = rows
        .iter()
        .all(|s| s.direction == first.direction && amounts_eq(s.amount, first.amount));
    if same {
        Summary {
            direction: first.direction.clone(),
            amount: first.amount,
            unit: first.unit.clone(),
        }
    } else {
        Summary {
            direction: "varies".into(),
            amount: None,
            unit: first.unit.clone(),
        }
    }
}

fn amounts_eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => false,
    }
}

fn citation_list(ticks: &[MouthTick]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for c in phone::cite() {
        set.insert((*c).to_string());
    }
    set.insert(LINDBLOM_1963.to_string());
    set.insert(OHMAN.to_string());
    set.insert(HH.to_string());
    for tick in ticks {
        for shift in [
            &tick.f1,
            &tick.f2,
            &tick.f3,
            &tick.lip_intent,
            &tick.lip_opening,
            &tick.jaw_emotion,
            &tick.rate,
        ] {
            for c in &shift.components {
                if !c.cite.is_empty() {
                    set.insert(c.cite.clone());
                }
            }
        }
    }
    set.into_iter().collect()
}

#[allow(clippy::too_many_arguments)]
fn make_tick(
    t_ms: u32,
    centre: u32,
    placed: &[Placed],
    blended: &[Blend],
    feel: &FeelingTick,
    scales: &Scales,
    intent: &IntentStub,
    precision_word: &str,
    precision_amount: Option<f64>,
    weight: Option<f64>,
) -> (MouthTick, Vec<String>) {
    let (f1, u1, withheld_f1) = fold_formant(feel, scales, "f1", intent);
    let (f2, u2, _) = fold_formant(feel, scales, "f2", intent);
    let (f3, u3, _) = fold_formant(feel, scales, "f3", intent);
    let (jaw_emotion, uj) = fold_jaw(feel, scales, intent.address_in_noise);
    let (lip_intent, us) = fold_smile(intent.smile, scales);
    let (lip_opening, uo) = fold_opening(intent.address_in_noise, scales);
    let (rate, ur, withheld_rate) = fold_rate(feel, scales);

    let mut unknown = Vec::new();
    unknown.extend(u1);
    unknown.extend(u2);
    unknown.extend(u3);
    unknown.extend(uj);
    unknown.extend(us);
    unknown.extend(uo);
    unknown.extend(ur);

    let (phone, known, viseme, lips, jaw, tongue_front, tongue_height, tongue_tip, blend) =
        match placed
            .iter()
            .position(|p| centre >= p.start && centre < p.end)
        {
            Some(i) => {
                let class = &placed[i].class;
                (
                    class.symbol.clone(),
                    class.known,
                    class.viseme.to_string(),
                    class.lips.to_string(),
                    class.jaw.to_string(),
                    class.tongue_front.to_string(),
                    class.tongue_height.to_string(),
                    class.tongue_tip.to_string(),
                    blended[i],
                )
            }
            None => (
                "sil".into(),
                true,
                "X".into(),
                "rest".into(),
                "rest".into(),
                "unspecified".into(),
                "unspecified".into(),
                "unspecified".into(),
                Blend {
                    jaw: None,
                    front: None,
                    height: None,
                    lips: None,
                },
            ),
        };

    let (pole, tier, alias, feeling) = label_feeling(feel);
    (
        MouthTick {
            t_ms,
            phone,
            phone_known: known,
            viseme,
            f1,
            f2,
            f3,
            lips,
            lips_blended: blend.lips,
            lip_intent,
            lip_opening,
            jaw,
            jaw_blended: blend.jaw,
            jaw_emotion,
            tongue_front,
            tongue_front_blended: blend.front,
            tongue_height,
            tongue_height_blended: blend.height,
            tongue_tip,
            precision: precision_word.into(),
            precision_amount,
            coarticulation_weight: weight,
            rate,
            feeling,
            track: feel.track.clone(),
            pole,
            prosody_tier: tier,
            alias_from: alias,
            withheld_f1,
            withheld_rate,
        },
        unknown,
    )
}

fn label_feeling(feel: &FeelingTick) -> (Option<String>, Option<String>, Option<String>, String) {
    match feel.parts.as_slice() {
        [] => (None, None, None, "neutral".into()),
        [one] => {
            let tier = Category::parse(&one.category).map(|c| c.tier().as_str().to_string());
            (
                one.pole.clone(),
                tier,
                one.alias_from.clone(),
                one.category.clone(),
            )
        }
        _ => (None, None, None, "mixture".into()),
    }
}

fn fold_formant(
    feel: &FeelingTick,
    scales: &Scales,
    which: &str,
    intent: &IntentStub,
) -> (Shift, Vec<String>, Option<String>) {
    let mut fold = Fold::new();
    let mut withheld = None;
    if which == "f1" {
        if feel.is_prosody() {
            for part in &feel.parts {
                let Some(cat) = Category::parse(&part.category) else {
                    continue;
                };
                let sign = direction::f1_sign(cat);
                if sign == 0 {
                    continue;
                }
                fold.add(
                    "f1_hz",
                    sign,
                    scales.get(part.pole.as_deref(), "f1_hz"),
                    part.weight,
                    direction::f1_cite(cat),
                    "higher",
                    "lower",
                );
            }
        } else if feel.is_burst() {
            withheld = single_withheld(feel, direction::f1_sign, "higher", "lower");
        }
        if intent.address_in_noise {
            fold.add(
                "address_f1_hz",
                1,
                scales.get(None, "address_f1_hz"),
                1.0,
                GARNIER,
                "higher",
                "lower",
            );
        }
    }
    if intent.smile {
        fold.add(
            "smile_formant_hz",
            1,
            scales.get(None, "smile_formant_hz"),
            1.0,
            TARTTER,
            "higher",
            "lower",
        );
    }
    let (shift, unknown) = fold.finish("Hz", "higher", "lower");
    (shift, unknown, withheld)
}

fn fold_jaw(feel: &FeelingTick, scales: &Scales, address: bool) -> (Shift, Vec<String>) {
    let mut fold = Fold::new();
    if feel.is_prosody() {
        for part in &feel.parts {
            let Some(cat) = Category::parse(&part.category) else {
                continue;
            };
            let sign = direction::f1_sign(cat);
            if sign == 0 {
                continue;
            }
            fold.add(
                "jaw_emotion",
                sign,
                scales.get(part.pole.as_deref(), "jaw_emotion"),
                part.weight,
                direction::f1_cite(cat),
                "more_open",
                "more_closed",
            );
        }
    }
    if address {
        fold.add(
            "address_jaw",
            1,
            scales.get(None, "address_jaw"),
            1.0,
            GARNIER,
            "more_open",
            "more_closed",
        );
    }
    fold.finish("planning", "more_open", "more_closed")
}

fn fold_smile(smile: bool, scales: &Scales) -> (Shift, Vec<String>) {
    let mut fold = Fold::new();
    if smile {
        fold.add(
            "smile_spread",
            1,
            scales.get(None, "smile_spread"),
            1.0,
            TARTTER,
            "spread",
            "round",
        );
    }
    fold.finish("planning", "spread", "round")
}

fn fold_opening(address: bool, scales: &Scales) -> (Shift, Vec<String>) {
    let mut fold = Fold::new();
    if address {
        fold.add(
            "address_lip_opening",
            1,
            scales.get(None, "address_lip_opening"),
            1.0,
            GARNIER,
            "more_open",
            "more_closed",
        );
    }
    fold.finish("planning", "more_open", "more_closed")
}

fn fold_rate(feel: &FeelingTick, scales: &Scales) -> (Shift, Vec<String>, Option<String>) {
    let mut fold = Fold::new();
    let mut withheld = None;
    if feel.is_prosody() {
        for part in &feel.parts {
            let Some(cat) = Category::parse(&part.category) else {
                continue;
            };
            let sign = direction::rate_sign(cat);
            if sign == 0 {
                continue;
            }
            fold.add(
                "rate",
                sign,
                scales.get(part.pole.as_deref(), "rate"),
                part.weight,
                direction::rate_cite(cat),
                "faster",
                "slower",
            );
        }
    } else if feel.is_burst() {
        withheld = single_withheld(feel, direction::rate_sign, "faster", "slower");
    }
    let (mut shift, unknown) = fold.finish("delta", "faster", "slower");
    if !feel.is_prosody() {
        shift = Shift::not_applied("multiplier");
    } else if shift.direction == "not_applied" {
        shift.unit = "multiplier".into();
    } else if let Some(delta) = shift.amount {
        shift.amount = Some(1.0 + delta);
        shift.unit = "multiplier".into();
    } else {
        shift.unit = "multiplier".into();
    }
    (shift, unknown, withheld)
}

fn single_withheld(
    feel: &FeelingTick,
    sign_of: fn(Category) -> i8,
    positive: &str,
    negative: &str,
) -> Option<String> {
    if feel.parts.len() != 1 {
        return None;
    }
    let cat = Category::parse(&feel.parts[0].category)?;
    let sign = sign_of(cat);
    if sign > 0 {
        Some(positive.into())
    } else if sign < 0 {
        Some(negative.into())
    } else {
        None
    }
}

fn rate_multiplier(shift: &Shift) -> Result<Option<f64>, MouthError> {
    match shift.amount {
        Some(m) if m <= 0.0 => Err(MouthError::Rate(
            "configured rate step makes a non-positive multiplier".into(),
        )),
        Some(m) => Ok(Some(m)),
        None => Ok(None),
    }
}

fn scale_duration(base: u32, multiplier: Option<f64>) -> Result<u32, MouthError> {
    match multiplier {
        Some(m) if m > 0.0 => {
            let ms = (base as f64 / m).round();
            if !ms.is_finite() || ms < 1.0 {
                return Err(MouthError::Rate(
                    "configured rate step collapses the phone duration".into(),
                ));
            }
            Ok(ms as u32)
        }
        Some(_) => Err(MouthError::Rate(
            "speaking-rate multiplier must be positive".into(),
        )),
        None => Ok(base),
    }
}

fn effective_weight(
    scales: &Scales,
    precision: Option<&PrecisionIntent>,
) -> (Option<f64>, Option<f64>, &'static str) {
    let w = scales.get(None, "coarticulation");
    let amount = scales.get(None, "precision");
    let word = match precision {
        Some(PrecisionIntent::Clear) => "hyper",
        Some(PrecisionIntent::Slur) => "hypo",
        None => "unknown",
    };
    let effective = match (w, amount, precision) {
        (None, _, _) => None,
        (Some(w), _, None) => Some(w),
        (Some(w), None, Some(_)) => Some(w),
        (Some(w), Some(a), Some(PrecisionIntent::Clear)) => Some((w * (1.0 - a)).clamp(0.0, 1.0)),
        (Some(w), Some(a), Some(PrecisionIntent::Slur)) => {
            Some((w + (1.0 - w) * a).clamp(0.0, 1.0))
        }
    };
    (effective, amount, word)
}
