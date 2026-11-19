// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Read mouth measurements that V1 already wrote.
//!
//! This does not run praatfan, SPARC, Phonet, or Rhubarb. It parses the
//! articulation card from `galion-articulation`, or the worker JSON those
//! bricks already emit (`workers.rs`: SPARC EMA, Phonet posteriors, Rhubarb
//! mouthCues). The 80 ms bin is the same median-of-frames rule as
//! `galion_articulation::to_ticks`, and the viseme-at-tick-centre rule is
//! the same as `add_rhubarb`.
//!
//! Optional: [`read_via_binary`] shells out to an already installed
//! `galion-articulation`. It does not download weights.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::MouthError;
use crate::TICK_MS;
use crate::TICK_S;

pub const SCHEMA: &str = "galion-mouth-read/1";

pub const NOTE: &str = "Read-only view of V1 output. f1-f4 are praatfan Burg tracks when the card says so. ema_* and lip_aperture are SPARC template units, not millimetres. phonet is a phonological posterior. phonet nasal is a speech-class posterior, not nasalance. visemes are Rhubarb A-H and X. No model is invoked unless read_via_binary is pointed at a local galion-articulation.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhonetTrack {
    pub section: String,
    pub ticks: Vec<Option<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouthRead {
    pub schema: String,
    pub tick_ms: u32,
    pub n_ticks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    pub f1_hz: Vec<Option<f64>>,
    pub f2_hz: Vec<Option<f64>>,
    pub f3_hz: Vec<Option<f64>>,
    pub f4_hz: Vec<Option<f64>>,
    pub sources: BTreeMap<String, String>,
    pub ema: BTreeMap<String, Vec<Option<f64>>>,
    pub lip_aperture: Vec<Option<f64>>,
    pub phonet: BTreeMap<String, PhonetTrack>,
    pub visemes: Vec<Option<String>>,
    pub bricks: Vec<String>,
    pub note: String,
}

pub fn read_json(text: &str) -> Result<MouthRead, MouthError> {
    let v: Value = serde_json::from_str(text).map_err(|e| MouthError::Json(e.to_string()))?;
    read_value(&v)
}

pub fn read_value(v: &Value) -> Result<MouthRead, MouthError> {
    if let Some(card) = v.get("articulation") {
        return read_card(card);
    }
    if v.get("tracks").is_some() {
        return read_card(v);
    }
    if v.get("sparc").is_some()
        || v.get("rhubarb").is_some()
        || v.get("disvoice").is_some()
        || v.get("phonet").is_some()
    {
        return read_bundle(v);
    }
    Err(MouthError::Json(
        "expected an articulation card or a sparc/phonet/rhubarb bundle".into(),
    ))
}

/// Run an already installed `galion-articulation analyze` and parse its card.
/// `bin` must exist. This function does not download anything.
pub fn read_via_binary(bin: &Path, wav: &Path) -> Result<MouthRead, MouthError> {
    if !bin.exists() {
        return Err(MouthError::Binary(format!(
            "{} is not on disk. The V1 reader was not downloaded.",
            bin.display()
        )));
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let out = std::env::temp_dir().join(format!(
        "galion-mouth-read-{}-{nanos}.json",
        std::process::id()
    ));
    let output = Command::new(bin)
        .arg("analyze")
        .arg("--wav")
        .arg(wav)
        .arg("--out")
        .arg(&out)
        .output()
        .map_err(|e| MouthError::Binary(format!("{}: {e}", bin.display())))?;
    if !output.status.success() {
        let _ = fs::remove_file(&out);
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        return Err(MouthError::Binary(format!(
            "galion-articulation exited {}: {err}",
            output.status
        )));
    }
    let text =
        fs::read_to_string(&out).map_err(|e| MouthError::Io(format!("{}: {e}", out.display())))?;
    let _ = fs::remove_file(&out);
    read_json(&text)
}

fn read_card(card: &Value) -> Result<MouthRead, MouthError> {
    let n_ticks = card.get("n_ticks").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
    let tick_ms = card
        .get("tick_ms")
        .and_then(|n| n.as_u64())
        .unwrap_or(TICK_MS as u64) as u32;
    if tick_ms != TICK_MS {
        return Err(MouthError::Json(format!(
            "articulation tick_ms is {tick_ms}, the mouth clock is {TICK_MS}"
        )));
    }
    let tracks = card.get("tracks").and_then(|t| t.as_object());
    let mut sources = BTreeMap::new();
    let mut ema = BTreeMap::new();
    if let Some(tracks) = tracks {
        for (key, track) in tracks {
            if let Some(rest) = key.strip_prefix("ema_") {
                if let Some(src) = track.get("source").and_then(|s| s.as_str()) {
                    sources.insert(key.clone(), src.to_string());
                }
                ema.insert(
                    rest.to_string(),
                    fit(opt_series(track.get("ticks")), n_ticks),
                );
            }
        }
    }
    let lip = take_track(tracks, "lip_aperture", n_ticks, &mut sources);
    let f1 = take_track(tracks, "f1_hz", n_ticks, &mut sources);
    let f2 = take_track(tracks, "f2_hz", n_ticks, &mut sources);
    let f3 = take_track(tracks, "f3_hz", n_ticks, &mut sources);
    let f4 = take_track(tracks, "f4_hz", n_ticks, &mut sources);
    let visemes = card
        .get("visemes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let visemes = fit_strings(visemes, n_ticks);
    let mut phonet = BTreeMap::new();
    if let Some(tracks) = tracks {
        for (key, track) in tracks {
            if let Some(rest) = key.strip_prefix("phon_") {
                if let Some(src) = track.get("source").and_then(|s| s.as_str()) {
                    sources.insert(key.clone(), src.to_string());
                }
                phonet.insert(
                    rest.to_string(),
                    PhonetTrack {
                        section: phonet_section(rest).into(),
                        ticks: fit(opt_series(track.get("ticks")), n_ticks),
                    },
                );
            }
        }
    }
    let mut bricks = brick_names(card.get("bricks"));
    if bricks.is_empty()
        && sources
            .values()
            .any(|s| s.to_ascii_lowercase().contains("praatfan"))
    {
        bricks.push("praatfan_rust".into());
    }
    Ok(MouthRead {
        schema: SCHEMA.into(),
        tick_ms,
        n_ticks,
        duration_s: card.get("duration_s").and_then(|n| n.as_f64()),
        f1_hz: f1,
        f2_hz: f2,
        f3_hz: f3,
        f4_hz: f4,
        sources,
        ema,
        lip_aperture: lip,
        phonet,
        visemes,
        bricks,
        note: NOTE.into(),
    })
}

fn take_track(
    tracks: Option<&serde_json::Map<String, Value>>,
    key: &str,
    n_ticks: usize,
    sources: &mut BTreeMap<String, String>,
) -> Vec<Option<f64>> {
    let Some(tracks) = tracks else {
        return vec![None; n_ticks];
    };
    let Some(track) = tracks.get(key) else {
        return vec![None; n_ticks];
    };
    if let Some(src) = track.get("source").and_then(|s| s.as_str()) {
        sources.insert(key.to_string(), src.to_string());
    }
    fit(opt_series(track.get("ticks")), n_ticks)
}

fn read_bundle(v: &Value) -> Result<MouthRead, MouthError> {
    let duration_s = v
        .get("duration_s")
        .and_then(|n| n.as_f64())
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| MouthError::Json("worker bundle needs duration_s".into()))?;
    let n_ticks = (duration_s / TICK_S).ceil().max(1.0) as usize;
    let mut read = MouthRead {
        schema: SCHEMA.into(),
        tick_ms: TICK_MS,
        n_ticks,
        duration_s: Some(duration_s),
        f1_hz: vec![None; n_ticks],
        f2_hz: vec![None; n_ticks],
        f3_hz: vec![None; n_ticks],
        f4_hz: vec![None; n_ticks],
        sources: BTreeMap::new(),
        ema: BTreeMap::new(),
        lip_aperture: vec![None; n_ticks],
        phonet: BTreeMap::new(),
        visemes: vec![None; n_ticks],
        bricks: Vec::new(),
        note: NOTE.into(),
    };
    if let Some(sparc) = v.get("sparc") {
        add_sparc(&mut read, sparc)?;
        read.bricks.push("SPARC".into());
    }
    if let Some(phonet) = v.get("phonet") {
        add_phonet(&mut read, phonet)?;
        read.bricks.push("Phonet".into());
    } else if let Some(phonet) = v.get("disvoice").and_then(|d| d.get("phonet")) {
        add_phonet(&mut read, phonet)?;
        read.bricks.push("Phonet".into());
    }
    if let Some(rhubarb) = v.get("rhubarb") {
        add_rhubarb(&mut read, rhubarb)?;
        read.bricks.push("Rhubarb Lip Sync".into());
    }
    Ok(read)
}

fn add_sparc(read: &mut MouthRead, v: &Value) -> Result<(), MouthError> {
    let rate = v
        .get("rate_hz")
        .and_then(|n| n.as_f64())
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| MouthError::Json("sparc.rate_hz".into()))?;
    let names: Vec<String> = v
        .get("ema_names")
        .and_then(|a| a.as_array())
        .ok_or_else(|| MouthError::Json("sparc.ema_names".into()))?
        .iter()
        .map(|x| x.as_str().unwrap_or("").to_string())
        .collect();
    let rows = v
        .get("ema")
        .and_then(|a| a.as_array())
        .ok_or_else(|| MouthError::Json("sparc.ema".into()))?;
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    for row in rows {
        let vals = num_row(row);
        for (i, name) in names.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            cols[i].push(vals.get(i).copied().unwrap_or(f64::NAN));
        }
    }
    for (i, name) in names.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let key = name.to_ascii_lowercase();
        read.sources
            .insert(format!("ema_{key}"), "SPARC ema".into());
        read.ema
            .insert(key, rate_to_ticks(rate, &cols[i], read.n_ticks));
    }
    let idx = |k: &str| names.iter().position(|n| n == k);
    if let (Some(u), Some(l)) = (idx("ULY"), idx("LLY")) {
        let ap: Vec<f64> = cols[u].iter().zip(&cols[l]).map(|(a, b)| a - b).collect();
        read.sources
            .insert("lip_aperture".into(), "glue: ULY - LLY".into());
        read.lip_aperture = rate_to_ticks(rate, &ap, read.n_ticks);
    }
    Ok(())
}

fn add_phonet(read: &mut MouthRead, v: &Value) -> Result<(), MouthError> {
    let rate = v
        .get("rate_hz")
        .and_then(|n| n.as_f64())
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| MouthError::Json("phonet.rate_hz".into()))?;
    let classes = v
        .get("classes")
        .and_then(|c| c.as_object())
        .ok_or_else(|| MouthError::Json("phonet.classes".into()))?;
    for (name, series) in classes {
        let values = num_row(series);
        read.sources.insert(format!("phon_{name}"), "Phonet".into());
        read.phonet.insert(
            name.clone(),
            PhonetTrack {
                section: phonet_section(name).into(),
                ticks: rate_to_ticks(rate, &values, read.n_ticks),
            },
        );
    }
    Ok(())
}

fn add_rhubarb(read: &mut MouthRead, v: &Value) -> Result<(), MouthError> {
    let cues = v
        .get("mouthCues")
        .and_then(|c| c.as_array())
        .ok_or_else(|| MouthError::Json("rhubarb.mouthCues".into()))?;
    let mut out = vec![None; read.n_ticks];
    for (i, slot) in out.iter_mut().enumerate() {
        let t = (i as f64 + 0.5) * TICK_S;
        for c in cues {
            let start = c.get("start").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let end = c.get("end").and_then(|n| n.as_f64()).unwrap_or(0.0);
            if t >= start && t < end {
                *slot = c.get("value").and_then(|s| s.as_str()).map(str::to_string);
                break;
            }
        }
    }
    read.visemes = out;
    read.sources
        .insert("visemes".into(), "Rhubarb Lip Sync".into());
    Ok(())
}

fn phonet_section(name: &str) -> &'static str {
    match name {
        "nasal" => "nose",
        "voice" => "throat",
        _ => "mouth",
    }
}

fn rate_to_ticks(rate_hz: f64, values: &[f64], n_ticks: usize) -> Vec<Option<f64>> {
    let times: Vec<f64> = (0..values.len()).map(|i| i as f64 / rate_hz).collect();
    to_ticks(&times, values, n_ticks)
}

/// Median of finite samples whose time falls in the tick. Same rule as
/// `galion_articulation::to_ticks`.
pub fn to_ticks(times: &[f64], values: &[f64], n_ticks: usize) -> Vec<Option<f64>> {
    let mut bins: Vec<Vec<f64>> = vec![Vec::new(); n_ticks];
    for (t, v) in times.iter().zip(values) {
        if !v.is_finite() || *t < 0.0 {
            continue;
        }
        let i = (*t / TICK_S).floor() as usize;
        if i < n_ticks {
            bins[i].push(*v);
        }
    }
    bins.into_iter().map(|mut b| median(&mut b)).collect()
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

fn opt_series(v: Option<&Value>) -> Vec<Option<f64>> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| x.as_f64().filter(|n| n.is_finite()))
                .collect()
        })
        .unwrap_or_default()
}

fn num_row(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}

fn fit(mut v: Vec<Option<f64>>, n: usize) -> Vec<Option<f64>> {
    v.truncate(n);
    v.resize(n, None);
    v
}

fn fit_strings(mut v: Vec<Option<String>>, n: usize) -> Vec<Option<String>> {
    v.truncate(n);
    v.resize(n, None);
    v
}

fn brick_names(v: Option<&Value>) -> Vec<String> {
    v.and_then(|b| b.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|b| b.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
