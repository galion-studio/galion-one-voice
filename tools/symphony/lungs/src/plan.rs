// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Plan inhalations from the text, then step lung volume and relative
//! subglottal pressure on the 80 ms grid.
//!
//! Order, for one breath group:
//! 1. Phrase length sets inspiration duration and depth.
//! 2. The category modifier is applied (amusement, disgust, fear). Other
//!    categories have no published breath row and do not invent one.
//! 3. Fatigue scales depth and the group limit by its config coefficients.
//! 4. Positive arousal shortens the group limit.
//! 5. Across the spoken ticks, volume falls, and pressure, loudness and F0
//!    fall with it unless energy or arousal drive them back up.

use crate::config::{self, Config};
use crate::inventory::Inventory;
use crate::{DIRECTIONS, REUSES, TICK_MS};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct Moment {
    pub at_s: f64,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub arousal: Option<f64>,
    #[serde(default)]
    pub energy: Option<f64>,
    #[serde(default)]
    pub fatigue: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub text: String,
    pub category: String,
    pub arousal: f64,
    pub energy: f64,
    pub fatigue: f64,
    pub trajectory: Vec<Moment>,
}

impl Request {
    pub fn constant(text: &str, category: &str, arousal: f64, energy: f64, fatigue: f64) -> Self {
        Request {
            text: text.to_string(),
            category: category.to_string(),
            arousal,
            energy,
            fatigue,
            trajectory: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct State {
    category: String,
    arousal: f64,
    energy: f64,
    fatigue: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phrase {
    pub index: usize,
    pub text: String,
    /// Start of this phrase on the speech clock, breaths excluded.
    pub speech_t0: f64,
    pub speech_s: f64,
    pub category: String,
    pub arousal: f64,
    pub energy: f64,
    pub fatigue: f64,
    pub insp_s: f64,
    pub depth: f64,
    pub hold_s: f64,
    /// `structural` or `faster`. Faster means the group limit was shortened.
    pub cycle: String,
    /// `phrase`, `short`, `quick`, or `deep`. A description of the inhale, not an emotion.
    pub form: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub kind: String,
    pub form: String,
    pub start_s: f64,
    pub end_s: f64,
    pub phrase: usize,
    pub loudness_dbfs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tick {
    pub t_ms: u32,
    pub lung_volume_rel: f64,
    /// Relative pressure during speech. Absent when the folds are not being driven.
    pub subglottal_pressure_rel: Option<f64>,
    /// Loudness offset in dB from the pressure reference. Not dB SPL.
    pub loudness_db: Option<f64>,
    /// F0 offset in semitones from the pressure reference. Not a speaking F0.
    pub f0_st: Option<f64>,
    pub arousal: f64,
    pub energy: f64,
    pub fatigue: f64,
    /// Emotion state at this tick. Never the string "breath".
    pub category: String,
    pub segment: String,
    pub phrase: usize,
    /// Position on the speech clock. Absent during inhale, hold and exhale.
    pub speech_t_s: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub schema: String,
    pub kind: String,
    pub not_emotion: bool,
    pub tick_ms: u32,
    pub duration_s: f64,
    pub n_ticks: usize,
    pub phrases: Vec<Phrase>,
    pub segments: Vec<Segment>,
    pub ticks: Vec<Tick>,
    pub directions: Vec<String>,
    pub reuses: Vec<String>,
}

struct Word {
    spoken: String,
    strong: bool,
    weak: bool,
}

struct Draft {
    words: Vec<String>,
    speech_s: f64,
    speech_t0: f64,
    faster: bool,
}

pub fn plan(req: &Request, cfg: &Config, inventory: &Inventory) -> Result<Plan, String> {
    inventory.check_state(&req.category)?;
    if !req.arousal.is_finite() || !req.energy.is_finite() || !req.fatigue.is_finite() {
        return Err("arousal, energy and fatigue must be finite".into());
    }
    for m in &req.trajectory {
        if !m.at_s.is_finite() {
            return Err("a trajectory moment has a non-finite time".into());
        }
        if let Some(c) = &m.category {
            inventory.check_state(c)?;
        }
        for (name, v) in [
            ("arousal", m.arousal),
            ("energy", m.energy),
            ("fatigue", m.fatigue),
        ] {
            if let Some(x) = v {
                if !x.is_finite() {
                    return Err(format!("a trajectory moment has a non-finite {name}"));
                }
            }
        }
    }

    let words = words_of(&req.text);
    let drafts = pack(&words, cfg, req);
    let mut phrases = Vec::new();
    let mut segments = Vec::new();
    let mut ticks = Vec::new();
    let mut t = 0.0;
    let mut volume = cfg.volume.rest;

    for (index, draft) in drafts.into_iter().enumerate() {
        let start = resolve(req, draft.speech_t0);
        let mut insp = (cfg.inspiration.base_s + cfg.inspiration.per_phrase_s * draft.speech_s)
            .clamp(cfg.inspiration.min_s, cfg.inspiration.max_s);
        let mut depth = (cfg.inspiration.depth_base
            + cfg.inspiration.depth_per_phrase_s * draft.speech_s)
            .clamp(cfg.inspiration.depth_min, cfg.inspiration.depth_max);
        let mut hold = 0.0;
        if start.category == "amusement" {
            insp *= cfg.categories.amusement.insp_time_scale;
            depth *= cfg.categories.amusement.depth_scale;
        }
        if start.category == "disgust" {
            hold = config::snap(cfg.tick_s, cfg.categories.disgust.hold_s);
        }
        let fatigue_depth = (1.0 + cfg.fatigue.depth_per_unit * start.fatigue).clamp(0.2, 2.0);
        depth = (depth * fatigue_depth).clamp(cfg.inspiration.depth_min, cfg.inspiration.depth_max);
        insp = config::snap(cfg.tick_s, insp);
        let form = if start.category == "amusement" {
            "short"
        } else if draft.faster {
            "quick"
        } else if depth >= cfg.inspiration.deep_depth {
            "deep"
        } else {
            "phrase"
        };
        let cycle = if draft.faster { "faster" } else { "structural" };
        let loudness = breath_dbfs(cfg, depth, start.arousal, start.energy, start.fatigue);

        let from = volume;
        let peak = (from + depth * cfg.volume.span).min(cfg.volume.rest + cfg.volume.span);
        let rise = (peak - from).max(0.0);
        let speech_end = (peak - cfg.volume.spend_fraction * rise).max(cfg.volume.floor);
        let exhale_s = config::snap(cfg.tick_s, cfg.audible.exhale_s);
        let exhale_end = if exhale_s > 0.0 {
            (speech_end - cfg.volume.exhale_spend * (speech_end - from)).max(cfg.volume.floor)
        } else {
            speech_end
        };

        phrases.push(Phrase {
            index,
            text: draft.words.join(" "),
            speech_t0: draft.speech_t0,
            speech_s: draft.speech_s,
            category: start.category.clone(),
            arousal: start.arousal,
            energy: start.energy,
            fatigue: start.fatigue,
            insp_s: insp,
            depth,
            hold_s: hold,
            cycle: cycle.to_string(),
            form: form.to_string(),
        });

        t = push_span(
            &mut segments,
            &mut ticks,
            cfg,
            req,
            Span {
                kind: "inhale",
                form,
                t0: t,
                dur: insp,
                vol0: from,
                vol1: peak,
                phrase: index,
                speech_t0: None,
                state: Some(start.clone()),
                loudness: Some(loudness),
                pressure: false,
            },
        );
        if hold > 0.0 {
            t = push_span(
                &mut segments,
                &mut ticks,
                cfg,
                req,
                Span {
                    kind: "hold",
                    form: "pause",
                    t0: t,
                    dur: hold,
                    vol0: peak,
                    vol1: peak,
                    phrase: index,
                    speech_t0: None,
                    state: Some(start.clone()),
                    loudness: None,
                    pressure: false,
                },
            );
        }
        t = push_span(
            &mut segments,
            &mut ticks,
            cfg,
            req,
            Span {
                kind: "speech",
                form: "phonation",
                t0: t,
                dur: draft.speech_s,
                vol0: peak,
                vol1: speech_end,
                phrase: index,
                speech_t0: Some(draft.speech_t0),
                state: None,
                loudness: None,
                pressure: true,
            },
        );
        if exhale_s > 0.0 {
            t = push_span(
                &mut segments,
                &mut ticks,
                cfg,
                req,
                Span {
                    kind: "exhale",
                    form: "release",
                    t0: t,
                    dur: exhale_s,
                    vol0: speech_end,
                    vol1: exhale_end,
                    phrase: index,
                    speech_t0: None,
                    state: Some(start),
                    loudness: Some(loudness + cfg.audible.exhale_db_offset),
                    pressure: false,
                },
            );
        }
        volume = exhale_end;
    }

    Ok(Plan {
        schema: "galion-lungs/1".into(),
        kind: "physiology".into(),
        not_emotion: true,
        tick_ms: TICK_MS,
        duration_s: t,
        n_ticks: ticks.len(),
        phrases,
        segments,
        ticks,
        directions: DIRECTIONS.iter().map(|s| (*s).to_string()).collect(),
        reuses: REUSES.iter().map(|s| (*s).to_string()).collect(),
    })
}

struct Span<'a> {
    kind: &'a str,
    form: &'a str,
    t0: f64,
    dur: f64,
    vol0: f64,
    vol1: f64,
    phrase: usize,
    speech_t0: Option<f64>,
    state: Option<State>,
    loudness: Option<f64>,
    pressure: bool,
}

fn push_span(
    segments: &mut Vec<Segment>,
    ticks: &mut Vec<Tick>,
    cfg: &Config,
    req: &Request,
    span: Span<'_>,
) -> f64 {
    let n = (span.dur / cfg.tick_s).round() as usize;
    if n == 0 {
        return span.t0;
    }
    segments.push(Segment {
        kind: span.kind.to_string(),
        form: span.form.to_string(),
        start_s: span.t0,
        end_s: span.t0 + n as f64 * cfg.tick_s,
        phrase: span.phrase,
        loudness_dbfs: span.loudness,
    });
    for i in 0..n {
        let u = (i + 1) as f64 / n as f64;
        let vol = span.vol0 + (span.vol1 - span.vol0) * u;
        let speech_t = span.speech_t0.map(|t0| t0 + (i as f64 + 0.5) * cfg.tick_s);
        let state = if let Some(t_s) = speech_t {
            resolve(req, t_s)
        } else {
            span.state.clone().unwrap_or_else(|| resolve(req, 0.0))
        };
        let (ps, loud, f0) = if span.pressure {
            let ps = pressure(cfg, vol, state.energy, state.arousal);
            (Some(ps), Some(loudness_db(cfg, ps)), Some(f0_st(cfg, ps)))
        } else {
            (None, None, None)
        };
        ticks.push(Tick {
            t_ms: ticks.len() as u32 * TICK_MS,
            lung_volume_rel: vol,
            subglottal_pressure_rel: ps,
            loudness_db: loud,
            f0_st: f0,
            arousal: state.arousal,
            energy: state.energy,
            fatigue: state.fatigue,
            category: state.category,
            segment: span.kind.to_string(),
            phrase: span.phrase,
            speech_t_s: speech_t,
        });
    }
    span.t0 + n as f64 * cfg.tick_s
}

fn pressure(cfg: &Config, volume: f64, energy: f64, arousal: f64) -> f64 {
    cfg.pressure.ps_ref
        + cfg.energy.ps_per_unit * (energy - cfg.energy.at)
        + cfg.arousal.ps_per_unit * arousal
        + cfg.pressure.ps_per_volume * (volume - cfg.volume.rest)
}

fn loudness_db(cfg: &Config, ps: f64) -> f64 {
    cfg.pressure.loudness_db_per_ps * (ps - cfg.pressure.ps_ref)
}

fn f0_st(cfg: &Config, ps: f64) -> f64 {
    cfg.pressure.f0_st_per_ps * (ps - cfg.pressure.ps_ref)
}

pub fn breath_dbfs(cfg: &Config, depth: f64, arousal: f64, energy: f64, fatigue: f64) -> f64 {
    let neutral = 20.0 * cfg.audible.neutral_rms.log10();
    neutral
        + cfg.audible.db_per_depth * (depth - 0.5)
        + cfg.audible.db_per_arousal * arousal
        + cfg.audible.db_per_energy * (energy - cfg.energy.at)
        + cfg.audible.db_per_fatigue * fatigue
}

fn resolve(req: &Request, speech_t: f64) -> State {
    let mut category = req.category.clone();
    let mut arousal = req.arousal;
    let mut energy = req.energy;
    let mut fatigue = req.fatigue;
    let mut moments: Vec<&Moment> = req.trajectory.iter().collect();
    moments.sort_by(|a, b| {
        a.at_s
            .partial_cmp(&b.at_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for m in moments {
        if m.at_s <= speech_t + 1e-9 {
            if let Some(c) = &m.category {
                category = c.clone();
            }
            if let Some(a) = m.arousal {
                arousal = a;
            }
            if let Some(e) = m.energy {
                energy = e;
            }
            if let Some(f) = m.fatigue {
                fatigue = f;
            }
        }
    }
    State {
        category,
        arousal: arousal.clamp(-1.0, 1.0),
        energy: energy.clamp(0.0, 1.0),
        fatigue: fatigue.clamp(0.0, 1.0),
    }
}

fn pack(words: &[Word], cfg: &Config, req: &Request) -> Vec<Draft> {
    let mut groups = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut cur_s = 0.0;
    let mut speech_t = 0.0;
    let mut group_t0 = 0.0;
    let mut limit = cfg.phrase.max_group_s;
    let mut clause = cfg.phrase.clause_break_s;
    let mut faster = false;

    for w in words {
        let ds = word_seconds(cfg, &w.spoken);
        if ds == 0.0 {
            continue;
        }
        if cur.is_empty() {
            let st = resolve(req, speech_t);
            let lim = limits(cfg, &st);
            limit = lim.0;
            clause = lim.1;
            faster = lim.2;
            group_t0 = speech_t;
        } else if cur_s + ds > limit + 1e-9 {
            groups.push(Draft {
                words: std::mem::take(&mut cur),
                speech_s: cur_s,
                speech_t0: group_t0,
                faster,
            });
            cur_s = 0.0;
            let st = resolve(req, speech_t);
            let lim = limits(cfg, &st);
            limit = lim.0;
            clause = lim.1;
            faster = lim.2;
            group_t0 = speech_t;
        }
        cur.push(w.spoken.clone());
        cur_s += ds;
        speech_t += ds;
        if w.strong || (w.weak && cur_s + 1e-9 >= clause) {
            groups.push(Draft {
                words: std::mem::take(&mut cur),
                speech_s: cur_s,
                speech_t0: group_t0,
                faster,
            });
            cur_s = 0.0;
        }
    }
    if !cur.is_empty() {
        groups.push(Draft {
            words: cur,
            speech_s: cur_s,
            speech_t0: group_t0,
            faster,
        });
    }
    groups
}

fn limits(cfg: &Config, st: &State) -> (f64, f64, bool) {
    let mut scale = 1.0 + cfg.fatigue.group_per_unit * st.fatigue;
    if st.category == "fear" {
        scale *= cfg.categories.fear.group_scale;
    }
    if st.arousal > 0.0 {
        scale /= 1.0 + cfg.arousal.rate_k * st.arousal;
    }
    scale = scale.max(0.05);
    let max_g = (cfg.phrase.max_group_s * scale).max(cfg.phrase.min_group_s);
    let clause = (cfg.phrase.clause_break_s * scale).max(cfg.tick_s);
    (max_g, clause, scale < 1.0 - 1e-9)
}

pub fn word_seconds(cfg: &Config, spoken: &str) -> f64 {
    let n = spoken.chars().filter(|c| c.is_alphanumeric()).count();
    if n == 0 {
        return 0.0;
    }
    config::snap(cfg.tick_s, n as f64 / cfg.phrase.chars_per_s)
}

fn words_of(text: &str) -> Vec<Word> {
    let lines: Vec<&str> = text
        .split(['\n', '\r'])
        .filter(|l| !l.trim().is_empty())
        .collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut line_words = words_in_line(line);
        if i + 1 < lines.len() {
            if let Some(last) = line_words.last_mut() {
                last.strong = true;
                last.weak = false;
            }
        }
        out.extend(line_words);
    }
    out
}

fn words_in_line(line: &str) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    for raw in line.split_whitespace() {
        let strong = raw
            .chars()
            .any(|c| c == '.' || c == '!' || c == '?' || c == '…');
        let weak = !strong && raw.chars().any(|c| c == ',' || c == ';' || c == ':');
        let spoken: String = raw
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'')
            .collect();
        if spoken.chars().all(|c| c == '\'') {
            if let Some(prev) = out.last_mut() {
                prev.strong |= strong;
                prev.weak = prev.weak || (weak && !prev.strong);
            }
            continue;
        }
        out.push(Word {
            spoken,
            strong,
            weak,
        });
    }
    out
}
