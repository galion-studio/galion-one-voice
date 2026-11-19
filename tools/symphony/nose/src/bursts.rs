// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Place wordless events and decide their shape.
//!
//! Placement comes from intent. Amusement on the burst track may add one laugh
//! when the caller asked for that. Amusement on the prosody blend does not.
//! A sigh is not collapsed to one of its four candidates. Moan, gasp, hum and
//! "mm" stay unnamed unless the caller names a category.
//!
//! Shape: a laugh uses the Bachorowski column for this speaker's F0 band and
//! the requested mouth. Other gestures have no acoustic target. Intensity and
//! arousal change level only through config slopes, which ship at zero.
//! Valence and dominance are recorded and do not move a knob.

use crate::baseline::{Band, Speaker};
use crate::config::{self, Config};
use crate::feeling::{self, OpenFeeling};
use crate::gestures::{self, SIGH_CANDIDATES};
use crate::inventory::Inventory;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct BurstAsk {
    pub gesture: String,
    #[serde(default)]
    pub at_s: Option<f64>,
    /// `start` or `end`, used when `at_s` is absent. Default `end`.
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    /// `open` or `closed`. Laugh only. Default open, the larger of Bachorowski's two sets.
    #[serde(default)]
    pub mouth: Option<String>,
    /// When set, the span is a detected one and the config duration is not used.
    #[serde(default)]
    pub end_s: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BurstEvent {
    pub gesture: String,
    pub recipe: String,
    pub category: Option<String>,
    pub candidates: Vec<String>,
    /// Prosody category at the start of the event. Not the burst's name.
    pub prosody: String,
    pub mouth: Option<String>,
    pub start_s: f64,
    pub end_s: f64,
    /// `planned` or `detected`.
    pub timing: String,
    pub calls: Option<u32>,
    pub call_s: Option<f64>,
    pub gap_s: Option<f64>,
    pub f0_hz: f64,
    pub f0_source: String,
    pub published_bout_s: Option<f64>,
    pub level_dbfs: f64,
    pub intensity: f64,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub opens_velum: bool,
}

pub fn level_db(feel: &OpenFeeling, cfg: &Config) -> f64 {
    cfg.level.base_db
        + cfg.level.db_per_intensity * feel.intensity
        + cfg.level.db_per_arousal * feel.arousal
}

struct Draft {
    gesture: String,
    category: Option<String>,
    candidates: Vec<String>,
    mouth: Option<String>,
    start_s: f64,
    end_s: f64,
    timing: String,
    feel: OpenFeeling,
}

pub fn place(
    asks: &[BurstAsk],
    speech_s: f64,
    speaker: &Speaker,
    feeling_at: &dyn Fn(f64) -> OpenFeeling,
    cfg: &Config,
    inventory: &Inventory,
) -> Result<Vec<BurstEvent>, String> {
    let mut drafts = Vec::new();
    for ask in asks {
        drafts.push(one(ask, speech_s, speaker, feeling_at, cfg, inventory)?);
    }
    drafts.sort_by(|a, b| {
        a.start_s
            .partial_cmp(&b.start_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut cursor = 0.0;
    for d in &mut drafts {
        if d.start_s < cursor {
            let dur = d.end_s - d.start_s;
            d.start_s = cursor;
            d.end_s = cursor + dur;
        }
        cursor = d.end_s;
    }
    let mut out = Vec::with_capacity(drafts.len());
    for d in drafts {
        out.push(shape(d, speaker, cfg)?);
    }
    Ok(out)
}

fn one(
    ask: &BurstAsk,
    speech_s: f64,
    speaker: &Speaker,
    feeling_at: &dyn Fn(f64) -> OpenFeeling,
    cfg: &Config,
    inventory: &Inventory,
) -> Result<Draft, String> {
    let gesture = gestures::canonical(&ask.gesture).ok_or_else(|| {
        format!(
            "unknown gesture {}. Wordless gestures are: {}",
            ask.gesture,
            gestures::GESTURES.join(", ")
        )
    })?;
    if let Some(t) = ask.at_s {
        if !t.is_finite() || t < 0.0 {
            return Err(format!("{gesture} at_s must be finite and >= 0"));
        }
    }
    if let Some(t) = ask.end_s {
        if !t.is_finite() {
            return Err(format!("{gesture} end_s must be finite"));
        }
    }
    let place = ask.place.as_deref().unwrap_or("end");
    let raw_start = if let Some(t) = ask.at_s {
        t
    } else {
        match place {
            "start" => 0.0,
            "end" => speech_s,
            other => return Err(format!("{gesture} place must be start or end, not {other}")),
        }
    };
    let start_s = config::snap_time(crate::TICK_S, raw_start);
    let detected = ask.end_s.is_some();
    let end_s = if let Some(end) = ask.end_s {
        let end = config::snap_time(crate::TICK_S, end);
        if end <= start_s {
            start_s + crate::TICK_S
        } else {
            end
        }
    } else {
        start_s + planned_duration(gesture, speaker, cfg)
    };
    let feel = feeling_at(start_s);
    let (category, candidates) = resolve_category(gesture, ask.category.as_deref(), inventory)?;
    let mouth = mouth_of(gesture, ask.mouth.as_deref())?;
    Ok(Draft {
        gesture: gesture.into(),
        category,
        candidates,
        mouth,
        start_s,
        end_s,
        timing: if detected { "detected" } else { "planned" }.into(),
        feel,
    })
}

fn planned_duration(gesture: &str, speaker: &Speaker, cfg: &Config) -> f64 {
    if gesture == "laugh" {
        let (calls, f0_unused, bout_unused) = laugh_column(speaker.band(cfg), true, cfg);
        let _ = (f0_unused, bout_unused);
        let n = calls.max(1);
        let raw = n as f64 * cfg.laugh.call_s + (n.saturating_sub(1) as f64) * cfg.laugh.gap_s;
        return config::snap_round(cfg.tick_s, raw);
    }
    config::snap_round(cfg.tick_s, configured_duration(gesture, cfg))
}

pub fn configured_duration(gesture: &str, cfg: &Config) -> f64 {
    match gesture {
        "sigh" => cfg.sigh.duration_s,
        "moan" => cfg.moan.duration_s,
        "gasp" => cfg.gasp.duration_s,
        "hum" => cfg.hum.duration_s,
        "mm" => cfg.mm.duration_s,
        "laugh" => {
            // A lower bound used before the sex column is known. `shape`
            // replaces a planned laugh's end from the real call count.
            cfg.laugh.call_s
        }
        _ => cfg.sigh.duration_s,
    }
}

fn resolve_category(
    gesture: &str,
    asked: Option<&str>,
    inventory: &Inventory,
) -> Result<(Option<String>, Vec<String>), String> {
    match gesture {
        "laugh" => match asked {
            None | Some("amusement") => Ok((Some("amusement".into()), Vec::new())),
            Some(other) => Err(format!(
                "a laugh is burst amusement (emotions-v2 map.laughter, the same assignment as PR 191 resolve). {other} is not that category"
            )),
        },
        "sigh" => match asked {
            None => Ok((
                None,
                SIGH_CANDIDATES.iter().map(|s| (*s).to_string()).collect(),
            )),
            Some(name) if SIGH_CANDIDATES.contains(&name) => {
                Ok((Some(name.to_string()), SIGH_CANDIDATES.iter().map(|s| (*s).to_string()).collect()))
            }
            Some(other) => Err(format!(
                "a sigh is a detector across {}; {other} is not one of them",
                SIGH_CANDIDATES.join(", ")
            )),
        },
        _ => match asked {
            None => Ok((None, Vec::new())),
            Some(name) => {
                inventory.check_burst_category(name)?;
                Ok((Some(name.to_string()), Vec::new()))
            }
        },
    }
}

fn mouth_of(gesture: &str, mouth: Option<&str>) -> Result<Option<String>, String> {
    if gesture != "laugh" {
        if mouth.is_some() {
            return Err(format!("{gesture} does not take a mouth. Mouth is a laugh distinction (Bachorowski Table II)."));
        }
        return Ok(None);
    }
    match mouth {
        None | Some("open") => Ok(Some("open".into())),
        Some("closed") => Ok(Some("closed".into())),
        Some(other) => Err(format!("laugh mouth must be open or closed, not {other}")),
    }
}

fn shape(draft: Draft, speaker: &Speaker, cfg: &Config) -> Result<BurstEvent, String> {
    let band = speaker.band(cfg);
    let (calls, call_s, gap_s, f0_hz, f0_source, published) = match draft.gesture.as_str() {
        "laugh" => laugh_shape(&draft, band, cfg),
        "moan" => {
            let f0 = shifted_f0(speaker.f0_hz, cfg.moan.f0_st);
            let source = if speaker.f0_hz <= 0.0 {
                "baseline-missing"
            } else if cfg.moan.f0_st == 0.0 {
                "speaker-baseline"
            } else {
                "speaker-baseline-plus-config-st"
            };
            (None, None, None, f0, source.into(), None)
        }
        "hum" | "mm" => {
            let source = if speaker.f0_hz <= 0.0 {
                "baseline-missing"
            } else {
                "speaker-baseline"
            };
            (
                None,
                None,
                None,
                speaker.f0_hz.max(0.0),
                source.into(),
                None,
            )
        }
        _ => (None, None, None, 0.0, "unvoiced".into(), None),
    };
    let (calls, call_s, gap_s) = fit_calls(calls, call_s, gap_s, draft.end_s - draft.start_s);
    let level_dbfs = level_db(&draft.feel, cfg);
    Ok(BurstEvent {
        gesture: draft.gesture.clone(),
        recipe: gestures::recipe(&draft.gesture).into(),
        category: draft.category,
        candidates: draft.candidates,
        prosody: draft.feel.category,
        mouth: draft.mouth,
        start_s: draft.start_s,
        end_s: draft.end_s,
        timing: draft.timing,
        calls,
        call_s,
        gap_s,
        f0_hz,
        f0_source,
        published_bout_s: published,
        level_dbfs,
        intensity: draft.feel.intensity,
        arousal: draft.feel.arousal,
        valence: draft.feel.valence,
        dominance: draft.feel.dominance,
        opens_velum: gestures::opens_velum(&draft.gesture),
    })
}

fn laugh_column(band: Band, open: bool, cfg: &Config) -> (u32, f64, Option<f64>) {
    match band {
        Band::Low => (
            cfg.laugh.calls_low,
            if open {
                cfg.laugh.f0_low_open_hz
            } else {
                cfg.laugh.f0_low_closed_hz
            },
            Some(cfg.laugh.published_bout_low_s),
        ),
        Band::High => (
            cfg.laugh.calls_high,
            if open {
                cfg.laugh.f0_high_open_hz
            } else {
                cfg.laugh.f0_high_closed_hz
            },
            Some(cfg.laugh.published_bout_high_s),
        ),
        Band::Unset => (
            cfg.laugh.calls_unset,
            cfg.laugh.f0_when_baseline_missing_hz,
            None,
        ),
    }
}

fn laugh_shape(
    draft: &Draft,
    band: Band,
    cfg: &Config,
) -> (
    Option<u32>,
    Option<f64>,
    Option<f64>,
    f64,
    String,
    Option<f64>,
) {
    let open = draft.mouth.as_deref() != Some("closed");
    let (calls, f0, bout) = laugh_column(band, open, cfg);
    let source = match band {
        Band::Unset => "baseline-missing",
        _ => "bachorowski2001-table-ii",
    };
    (
        Some(calls),
        Some(cfg.laugh.call_s),
        Some(cfg.laugh.gap_s),
        f0,
        source.into(),
        bout,
    )
}

fn fit_calls(
    calls: Option<u32>,
    call_s: Option<f64>,
    gap_s: Option<f64>,
    window: f64,
) -> (Option<u32>, Option<f64>, Option<f64>) {
    let (Some(n), Some(c), Some(g)) = (calls, call_s, gap_s) else {
        return (calls, call_s, gap_s);
    };
    if n == 0 || window <= 0.0 {
        return (Some(n), Some(c), Some(g));
    }
    let raw = n as f64 * c + (n.saturating_sub(1) as f64) * g;
    if raw <= window + 1e-9 {
        return (Some(n), Some(c), Some(g));
    }
    let scale = window / raw;
    (Some(n), Some(c * scale), Some(g * scale))
}

fn shifted_f0(f0: f64, st: f64) -> f64 {
    if f0 <= 0.0 {
        0.0
    } else {
        f0 * 2f64.powf(st / 12.0)
    }
}

/// One laugh, if burst-track amusement is strictly the top and above the
/// config floor. A tie places nothing. Prosody is ignored here; the caller
/// passes burst weights only.
pub fn laugh_from_feeling(feel: &OpenFeeling, cfg: &Config) -> Result<Option<BurstAsk>, String> {
    match feeling::exclusive_burst_top(&feel.burst) {
        Some(("amusement", w)) if w > cfg.place.amusement_min => Ok(Some(BurstAsk {
            gesture: "laugh".into(),
            at_s: None,
            place: Some("end".into()),
            category: None,
            mouth: None,
            end_s: None,
        })),
        Some(("amusement", _)) => Ok(None),
        _ => Ok(None),
    }
}

pub fn non_laugh_gap(feel: &OpenFeeling) -> Option<String> {
    match feeling::exclusive_burst_top(&feel.burst) {
        Some((id, _)) if id != "amusement" => Some(format!(
            "burst track tops {id}; no gesture was published for that category, and intent did not name one"
        )),
        None if feel.burst.iter().filter(|w| w.w > 0.0).count() > 1 => {
            let mut tops: Vec<_> = feel.burst.iter().filter(|w| w.w > 0.0).collect();
            if tops.len() >= 2 {
                tops.sort_by(|a, b| b.w.partial_cmp(&a.w).unwrap_or(std::cmp::Ordering::Equal));
                if (tops[0].w - tops[1].w).abs() <= 1e-12 {
                    return Some(
                        "burst weights tie; no gesture was placed from feeling".into(),
                    );
                }
            }
            None
        }
        _ => None,
    }
}
