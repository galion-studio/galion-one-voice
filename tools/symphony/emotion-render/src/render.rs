// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One 80 ms target → controls the existing cloner already has.
//!
//! Knobs, and where they already live:
//! - Chatterbox `exaggeration` (`prepare_conditionals` in
//!   `scripts/clone/chatterbox_worker.py`)
//! - Chatterbox `cfg_weight` and `temperature`
//!   (`scripts/clone/nano_stream.py`, `_iter_turbo_tokens`)
//! - `prosody.pitch_st`, `prosody.rate`, `prosody.energy`,
//!   `prosody.pre_pause_ms`, `prosody.range`
//!   (`log_prosody` / `speaking_rate_of` in the same worker)
//!
//! Pitch is a shift against this speaker's own neutral: 0 means no shift.
//! The speaker's hertz are not an input and are not invented.
//!
//! Valence is recorded and does not move a knob. Juslin & Laukka (2003)
//! show anger and happiness sharing the high-arousal pattern, so valence
//! is not one feature. Arousal is recorded and does not multiply a gain:
//! no source read here publishes hertz, or a multiplier, per arousal unit.
//! The category sign already carries the modal arousal pattern.
//!
//! TODO(bc-56b9f22c, PR 191): `galion_articulation::state::TickState` is the
//! decoder's per-tick blend (neutral plus the 12 NHB names). This crate does
//! not depend on that workspace. [`TargetTick::from_decoder_value`] reads the
//! same JSON object (`t_ms`, `arousal`, `valence`, `dominance`, `intensity`,
//! `blend[{id,w}]`) and also accepts the other 12 names. Their
//! `decode::params_for` writes exaggeration, cfg and speed from fixed
//! formulas. Those formulas are not used here. `speed` in this output is
//! the speaking-rate multiplier, which is their field name for rate.
//!
//! TODO(bc-ed0d7d68): [`clone_request_patch`] is the object to merge into
//! the worker's stdin JSON. The worker applies `prosody.rate` only, and
//! clamps it to 0.8..=1.2. It logs pitch, range, energy and pre_pause_ms.
//! It does not read exaggeration, cfg_weight or temperature from the request.
//!
//! TODO(PR 190, bc-9621c1b7): the eval harness does not call this crate.
//! Direction words match `configs/symphony/emotion-labels-v2.toml`.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{json, Value};

use crate::direction::{sign_word, speech_directions, Acoustics, Sign};
use crate::error::RenderError;
use crate::inventory::{resolve_name, Category, Resolved, Track, TICK_MS};
use crate::scales::Scales;

pub const SCHEMA: &str = "galion-emotion-render/1";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Avd {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
}

impl Avd {
    pub fn zero() -> Self {
        Self {
            arousal: 0.0,
            valence: 0.0,
            dominance: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlendPart {
    pub category: Category,
    pub weight: f64,
    pub alias_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetTick {
    pub t_ms: u32,
    pub track: Track,
    pub avd: Avd,
    /// Copied onto the output. Blend weights, when present, are the mixture.
    /// This field is not applied a second time.
    pub intensity: f64,
    pub blend: Vec<BlendPart>,
}

impl TargetTick {
    pub fn baseline(t_ms: u32) -> Self {
        Self {
            t_ms,
            track: Track::Prosody,
            avd: Avd::zero(),
            intensity: 0.0,
            blend: Vec::new(),
        }
    }

    /// One category on one track. `intensity` is that category's weight.
    pub fn named(
        category: Category,
        track: Track,
        intensity: f64,
        avd: Avd,
    ) -> Result<Self, RenderError> {
        check_unit("intensity", intensity)?;
        check_axis(&avd)?;
        Ok(Self {
            t_ms: 0,
            track,
            avd,
            intensity,
            blend: vec![BlendPart {
                category,
                weight: intensity,
                alias_from: None,
            }],
        })
    }

    pub fn from_name(
        name: &str,
        track: Option<Track>,
        intensity: f64,
        avd: Avd,
    ) -> Result<Self, RenderError> {
        check_unit("intensity", intensity)?;
        check_axis(&avd)?;
        match resolve_name(name)? {
            Resolved::Baseline => Ok(Self::baseline(0)),
            Resolved::One {
                category,
                track: alias_track,
                alias_from,
            } => {
                let track = track.or(alias_track).unwrap_or(Track::Prosody);
                Ok(Self {
                    t_ms: 0,
                    track,
                    avd,
                    intensity,
                    blend: vec![BlendPart {
                        category,
                        weight: intensity,
                        alias_from,
                    }],
                })
            }
        }
    }

    /// Decoder `TickState` JSON from bc-56b9f22c. Extra `track` is optional
    /// and defaults to prosody. `emotion` is used only when `blend` is absent.
    pub fn from_decoder_value(v: &Value) -> Result<Self, RenderError> {
        let t_ms = v.get("t_ms").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let avd = Avd {
            arousal: num_or(v, "arousal", 0.0)?,
            valence: num_or(v, "valence", 0.0)?,
            dominance: num_or(v, "dominance", 0.0)?,
        };
        check_axis(&avd)?;
        let intensity = num_or(v, "intensity", 0.0)?;
        check_unit("intensity", intensity)?;
        let explicit = v.get("track").and_then(|x| x.as_str()).map(Track::parse);
        if v.get("track").and_then(|x| x.as_str()).is_some() && explicit.is_none() {
            return Err(RenderError::BadBlend(
                "track must be prosody or burst".into(),
            ));
        }
        let mut track = explicit.flatten().unwrap_or(Track::Prosody);
        let blend = if let Some(arr) = v.get("blend").and_then(|b| b.as_array()) {
            let mut parts = Vec::new();
            for row in arr {
                let id = row
                    .get("id")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| RenderError::BadBlend("each blend entry needs an id".into()))?;
                let weight = row
                    .get("w")
                    .and_then(|x| x.as_f64())
                    .ok_or_else(|| RenderError::BadBlend(format!("{id} needs a weight")))?;
                if !weight.is_finite() || weight < 0.0 {
                    return Err(RenderError::BadNumber("blend weight"));
                }
                if weight == 0.0 {
                    continue;
                }
                match resolve_name(id)? {
                    Resolved::Baseline => {}
                    Resolved::One {
                        category,
                        track: alias_track,
                        alias_from,
                    } => {
                        if explicit.is_none() {
                            if let Some(t) = alias_track {
                                track = t;
                            }
                        }
                        parts.push(BlendPart {
                            category,
                            weight,
                            alias_from,
                        });
                    }
                }
            }
            parts
        } else if let Some(name) = v.get("emotion").and_then(|x| x.as_str()) {
            match resolve_name(name)? {
                Resolved::Baseline => Vec::new(),
                Resolved::One {
                    category,
                    track: alias_track,
                    alias_from,
                } => {
                    if explicit.is_none() {
                        if let Some(t) = alias_track {
                            track = t;
                        }
                    }
                    vec![BlendPart {
                        category,
                        weight: intensity,
                        alias_from,
                    }]
                }
            }
        } else {
            Vec::new()
        };
        Ok(Self {
            t_ms,
            track,
            avd,
            intensity,
            blend,
        })
    }
}

fn num_or(v: &Value, key: &'static str, default: f64) -> Result<f64, RenderError> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or(RenderError::BadNumber(key)),
    }
}

fn check_unit(what: &'static str, x: f64) -> Result<(), RenderError> {
    if x.is_finite() && (0.0..=1.0).contains(&x) {
        Ok(())
    } else {
        Err(RenderError::BadNumber(what))
    }
}

fn check_axis(avd: &Avd) -> Result<(), RenderError> {
    for (name, x) in [
        ("arousal", avd.arousal),
        ("valence", avd.valence),
        ("dominance", avd.dominance),
    ] {
        if !x.is_finite() || !(-1.0..=1.0).contains(&x) {
            return Err(RenderError::BadNumber(name));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatterboxKnobs {
    pub exaggeration: f64,
    pub cfg_weight: f64,
    /// Same number as `cfg_weight`. Decoder field name (`decode::RenderParams.cfg`).
    pub cfg: f64,
    pub temperature: f64,
    /// Same number as `prosody.rate`. Decoder field name (`speed`).
    pub speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProsodyKnobs {
    /// Semitones relative to this speaker's neutral. 0 is the baseline.
    pub pitch_st: f64,
    /// Multiplier. 1 is the speaker's own range.
    pub range: f64,
    /// Multiplier. 1 is the speaker's own rate. The worker clamps 0.8..=1.2.
    pub rate: f64,
    /// Multiplier. 1 is the speaker's own energy. The worker logs this.
    pub energy: f64,
    /// Extra silence before the utterance, milliseconds. The worker logs this.
    /// A "fewer pauses" direction cannot subtract here; the worker knob only adds.
    pub pre_pause_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Directions {
    pub pitch_st: String,
    pub pitch_range: String,
    pub rate: String,
    pub energy: String,
    pub pause: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Applied {
    pub pitch_st: bool,
    pub pitch_range: bool,
    pub rate: bool,
    pub energy: bool,
    pub pause: bool,
    pub exaggeration: bool,
    pub cfg_weight: bool,
    pub temperature: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TickControls {
    pub schema: String,
    pub t_ms: u32,
    pub tick_ms: u32,
    pub track: String,
    /// Top blend weight, or `neutral` for the speaker baseline.
    pub emotion: String,
    pub prosody_tier: Option<String>,
    pub pole: Option<String>,
    pub alias_from: Option<String>,
    pub relative_to: String,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    pub chatterbox: ChatterboxKnobs,
    pub prosody: ProsodyKnobs,
    pub directions: Directions,
    pub applied: Applied,
    pub citations: Vec<String>,
    pub open: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WordRender {
    /// Passed through. This crate does not synthesise it.
    pub text: String,
    pub controls: TickControls,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UtteranceChatterbox {
    pub exaggeration: f64,
    pub cfg: f64,
    pub cfg_weight: f64,
    pub temperature: f64,
    pub speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UtteranceControls {
    pub schema: String,
    pub tick_ms: u32,
    pub ticks: Vec<TickControls>,
    pub chatterbox: UtteranceChatterbox,
}

struct Acc {
    pitch: f64,
    pitch_on: bool,
    range: f64,
    range_on: bool,
    rate: f64,
    rate_on: bool,
    energy: f64,
    energy_on: bool,
    pause_ms: f64,
    pause_on: bool,
    signs_pitch: Vec<Sign>,
    signs_range: Vec<Sign>,
    signs_rate: Vec<Sign>,
    signs_energy: Vec<Sign>,
    signs_pause: Vec<Sign>,
    citations: BTreeSet<String>,
    open: BTreeSet<String>,
    top_weight: f64,
    top: Option<Category>,
    top_pole: Option<String>,
    top_alias: Option<String>,
}

impl Acc {
    fn new() -> Self {
        Self {
            pitch: 0.0,
            pitch_on: false,
            range: 0.0,
            range_on: false,
            rate: 0.0,
            rate_on: false,
            energy: 0.0,
            energy_on: false,
            pause_ms: 0.0,
            pause_on: false,
            signs_pitch: Vec::new(),
            signs_range: Vec::new(),
            signs_rate: Vec::new(),
            signs_energy: Vec::new(),
            signs_pause: Vec::new(),
            citations: BTreeSet::new(),
            open: BTreeSet::new(),
            top_weight: -1.0,
            top: None,
            top_pole: None,
            top_alias: None,
        }
    }
}

pub fn render_tick(tick: &TargetTick, scales: &Scales) -> Result<TickControls, RenderError> {
    check_axis(&tick.avd)?;
    check_unit("intensity", tick.intensity)?;
    let mut acc = Acc::new();
    // Always recorded. They do not enter the arithmetic.
    acc.open.insert("valence_not_a_feature".into());
    acc.open.insert("arousal_no_per_unit".into());
    let apply_speech = tick.track == Track::Prosody;
    if !apply_speech {
        acc.open.insert("burst_withholds_speech".into());
    }
    for part in &tick.blend {
        if part.weight == 0.0 {
            continue;
        }
        if !part.weight.is_finite() || part.weight < 0.0 {
            return Err(RenderError::BadNumber("blend weight"));
        }
        let dir = speech_directions(part.category, tick.avd.dominance);
        if part.weight > acc.top_weight {
            acc.top_weight = part.weight;
            acc.top = Some(part.category);
            acc.top_pole = dir.pole.map(str::to_string);
            acc.top_alias = part.alias_from.clone();
        }
        for cite in dir.cite {
            if !cite.is_empty() {
                acc.citations.insert((*cite).to_string());
            }
        }
        if apply_speech {
            fold_feature(&mut acc, &dir, part.weight, scales);
        } else {
            // The signs stay visible. The knobs stay at the speaker baseline.
            note_signs(&mut acc, &dir);
        }
    }
    let weight_sum = tick.blend.iter().map(|p| p.weight).sum::<f64>();
    let engine = &scales.engine;
    let (exaggeration, ex_on) = engine_knob(
        engine.exaggeration,
        scales.scale_for(None, "exaggeration"),
        weight_sum,
        apply_speech,
    );
    let (cfg_weight, cfg_on) = engine_knob(
        engine.cfg_weight,
        scales.scale_for(None, "cfg_weight"),
        weight_sum,
        apply_speech,
    );
    let (temperature, temp_on) = engine_knob(
        engine.temperature,
        scales.scale_for(None, "temperature"),
        weight_sum,
        apply_speech,
    );
    if !ex_on {
        acc.open
            .insert("engine_knob_not_literature:exaggeration".into());
    }
    if !cfg_on {
        acc.open
            .insert("engine_knob_not_literature:cfg_weight".into());
    }
    if !temp_on {
        acc.open
            .insert("engine_knob_not_literature:temperature".into());
    }
    let rate = 1.0 + acc.rate;
    if acc.rate_on && !(0.8..=1.2).contains(&rate) {
        // The worker will clamp. The intended multiplier is left intact.
        acc.open.insert("worker_clamps_rate_0.8_1.2".into());
    }
    let pause_ms = if acc.pause_ms <= 0.0 {
        0
    } else {
        acc.pause_ms.round() as u32
    };
    let emotion = acc
        .top
        .map(|c| c.as_str().to_string())
        .unwrap_or_else(|| "neutral".into());
    let tier = acc.top.map(|c| c.tier().as_str().to_string());
    Ok(TickControls {
        schema: SCHEMA.into(),
        t_ms: tick.t_ms,
        tick_ms: TICK_MS,
        track: tick.track.as_str().into(),
        emotion,
        prosody_tier: tier,
        pole: acc.top_pole,
        alias_from: acc.top_alias,
        relative_to: "speaker_own_neutral".into(),
        arousal: tick.avd.arousal,
        valence: tick.avd.valence,
        dominance: tick.avd.dominance,
        intensity: tick.intensity,
        chatterbox: ChatterboxKnobs {
            exaggeration,
            cfg_weight,
            cfg: cfg_weight,
            temperature,
            speed: rate,
        },
        prosody: ProsodyKnobs {
            pitch_st: acc.pitch,
            range: 1.0 + acc.range,
            rate,
            energy: 1.0 + acc.energy,
            pre_pause_ms: pause_ms,
        },
        directions: Directions {
            pitch_st: consensus("pitch_st", &acc.signs_pitch),
            pitch_range: consensus("pitch_range", &acc.signs_range),
            rate: consensus("rate", &acc.signs_rate),
            energy: consensus("energy", &acc.signs_energy),
            pause: consensus("pause", &acc.signs_pause),
        },
        applied: Applied {
            pitch_st: acc.pitch_on,
            pitch_range: acc.range_on,
            rate: acc.rate_on,
            energy: acc.energy_on,
            pause: acc.pause_on,
            exaggeration: ex_on,
            cfg_weight: cfg_on,
            temperature: temp_on,
        },
        citations: acc.citations.into_iter().collect(),
        open: acc.open.into_iter().collect(),
    })
}

fn engine_knob(baseline: f64, scale: Option<f64>, weight: f64, apply: bool) -> (f64, bool) {
    match (apply, scale) {
        (true, Some(step)) if weight > 0.0 => (baseline + step * weight, true),
        _ => (baseline, false),
    }
}

fn note_signs(acc: &mut Acc, dir: &Acoustics) {
    acc.signs_pitch.push(dir.pitch.sign);
    acc.signs_range.push(dir.pitch_range.sign);
    acc.signs_rate.push(dir.rate.sign);
    acc.signs_energy.push(dir.energy.sign);
    acc.signs_pause.push(dir.pause.sign);
}

fn fold_feature(acc: &mut Acc, dir: &Acoustics, weight: f64, scales: &Scales) {
    apply_signed(
        dir.pitch.sign,
        scales.scale_for(dir.pole, "pitch_st"),
        weight,
        &mut SignedSlot {
            feature: "pitch_st",
            acc: &mut acc.pitch,
            on: &mut acc.pitch_on,
            signs: &mut acc.signs_pitch,
        },
        &mut acc.open,
        false,
    );
    apply_signed(
        dir.pitch_range.sign,
        scales.scale_for(dir.pole, "pitch_range"),
        weight,
        &mut SignedSlot {
            feature: "pitch_range",
            acc: &mut acc.range,
            on: &mut acc.range_on,
            signs: &mut acc.signs_range,
        },
        &mut acc.open,
        false,
    );
    apply_signed(
        dir.rate.sign,
        scales.scale_for(dir.pole, "rate"),
        weight,
        &mut SignedSlot {
            feature: "rate",
            acc: &mut acc.rate,
            on: &mut acc.rate_on,
            signs: &mut acc.signs_rate,
        },
        &mut acc.open,
        false,
    );
    apply_signed(
        dir.energy.sign,
        scales.scale_for(dir.pole, "energy"),
        weight,
        &mut SignedSlot {
            feature: "energy",
            acc: &mut acc.energy,
            on: &mut acc.energy_on,
            signs: &mut acc.signs_energy,
        },
        &mut acc.open,
        false,
    );
    // "Fewer" cannot subtract `pre_pause_ms` (the knob only adds silence).
    let fewer = dir.pause.sign == Sign::Lower;
    apply_signed(
        dir.pause.sign,
        scales.scale_for(dir.pole, "pause_ms"),
        weight,
        &mut SignedSlot {
            feature: "pause",
            acc: &mut acc.pause_ms,
            on: &mut acc.pause_on,
            signs: &mut acc.signs_pause,
        },
        &mut acc.open,
        fewer,
    );
    if fewer {
        acc.open.insert("pause_fewer_not_pre_pause_ms".into());
    }
    for feat in [dir.pitch, dir.pitch_range, dir.rate, dir.energy, dir.pause] {
        if !feat.cite.is_empty() && feat.sign != Sign::Unknown {
            acc.citations.insert(feat.cite.to_string());
        }
    }
}

struct SignedSlot<'a> {
    feature: &'a str,
    acc: &'a mut f64,
    on: &'a mut bool,
    signs: &'a mut Vec<Sign>,
}

fn apply_signed(
    sign: Sign,
    scale: Option<f64>,
    weight: f64,
    slot: &mut SignedSlot<'_>,
    open: &mut BTreeSet<String>,
    withhold_subtraction: bool,
) {
    let feature = slot.feature;
    slot.signs.push(sign);
    match sign {
        Sign::Unknown => {
            open.insert(format!("sign_unknown:{feature}"));
        }
        Sign::Unresolved => {
            open.insert(format!("sign_unresolved:{feature}"));
        }
        Sign::Higher | Sign::Lower => match scale {
            None => {
                open.insert(format!("magnitude_unknown:{feature}"));
            }
            Some(_step) if withhold_subtraction => {
                open.insert(format!("magnitude_unused:{feature}"));
            }
            Some(step) => {
                let s = if sign == Sign::Higher { 1.0 } else { -1.0 };
                *slot.acc += s * step * weight;
                *slot.on = true;
            }
        },
    }
}

fn consensus(feature: &str, signs: &[Sign]) -> String {
    let mut up = false;
    let mut down = false;
    let mut unresolved = false;
    for s in signs {
        match s {
            Sign::Higher => up = true,
            Sign::Lower => down = true,
            Sign::Unresolved => unresolved = true,
            Sign::Unknown => {}
        }
    }
    if up && down {
        return "mixed".into();
    }
    if up {
        return sign_word(feature, Sign::Higher).into();
    }
    if down {
        return sign_word(feature, Sign::Lower).into();
    }
    if unresolved {
        return "unresolved".into();
    }
    "unknown".into()
}

/// The word is carried through. Any of the 24 may be the target, including
/// embarrassment. Nothing is synthesised.
pub fn render_word(
    text: &str,
    tick: &TargetTick,
    scales: &Scales,
) -> Result<WordRender, RenderError> {
    Ok(WordRender {
        text: text.to_string(),
        controls: render_tick(tick, scales)?,
    })
}

pub fn render_utterance(
    ticks: &[TargetTick],
    scales: &Scales,
) -> Result<UtteranceControls, RenderError> {
    if ticks.is_empty() {
        return Err(RenderError::Empty);
    }
    let ticks: Result<Vec<_>, _> = ticks.iter().map(|t| render_tick(t, scales)).collect();
    let ticks = ticks?;
    Ok(UtteranceControls {
        schema: SCHEMA.into(),
        tick_ms: TICK_MS,
        chatterbox: UtteranceChatterbox {
            exaggeration: median(ticks.iter().map(|t| t.chatterbox.exaggeration)),
            cfg: median(ticks.iter().map(|t| t.chatterbox.cfg)),
            cfg_weight: median(ticks.iter().map(|t| t.chatterbox.cfg_weight)),
            temperature: median(ticks.iter().map(|t| t.chatterbox.temperature)),
            speed: median(ticks.iter().map(|t| t.prosody.rate)),
        },
        ticks,
    })
}

fn median(xs: impl Iterator<Item = f64>) -> f64 {
    let mut v: Vec<f64> = xs.collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// JSON to merge into one chatterbox worker request line.
/// See the module note for what that worker actually applies today.
pub fn clone_request_patch(controls: &TickControls) -> Value {
    json!({
        "emotion": controls.emotion,
        "track": controls.track,
        "exaggeration": controls.chatterbox.exaggeration,
        "cfg_weight": controls.chatterbox.cfg_weight,
        "temperature": controls.chatterbox.temperature,
        "prosody": {
            "pitch_st": controls.prosody.pitch_st,
            "range": controls.prosody.range,
            "rate": controls.prosody.rate,
            "energy": controls.prosody.energy,
            "pre_pause_ms": controls.prosody.pre_pause_ms,
        }
    })
}
