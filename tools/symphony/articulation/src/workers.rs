// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Readers for the out-of-process bricks' JSON (written by `workers/*.py` and Rhubarb).
//! The workers call the upstream libraries as-is; this file only maps their output
//! onto the 80 ms tick and the throat / mouth / nose sections.

use crate::{rate_to_ticks, Articulation, Brick, Section};
use serde_json::Value;

fn arr(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}

fn brick(a: &mut Articulation, v: &Value) {
    if let Some(bs) = v.get("bricks").and_then(|b| b.as_array()) {
        for b in bs {
            if let Ok(b) = serde_json::from_value::<Brick>(b.clone()) {
                if !a.bricks.contains(&b) {
                    a.bricks.push(b);
                }
            }
        }
    }
}

/// SPARC encode() output: 12 EMA channels at 50 Hz in SPARC's speaker-agnostic template space.
/// Channel order (SPARC README): TDX TDY TBX TBY TTX TTY LIX LIY ULX ULY LLX LLY.
pub fn add_sparc(a: &mut Articulation, v: &Value) -> Result<(), String> {
    let rate = v["rate_hz"].as_f64().ok_or("sparc: rate_hz")?;
    let names: Vec<String> = v["ema_names"]
        .as_array()
        .ok_or("sparc: ema_names")?
        .iter()
        .map(|x| x.as_str().unwrap_or("?").to_string())
        .collect();
    let ema = v["ema"].as_array().ok_or("sparc: ema")?;
    let n = a.n_ticks;
    let mut chans: Vec<Vec<f64>> = vec![Vec::with_capacity(ema.len()); names.len()];
    for row in ema {
        let r = arr(row);
        for (c, x) in r.iter().enumerate().take(names.len()) {
            chans[c].push(*x);
        }
    }
    for (c, name) in names.iter().enumerate() {
        a.add(
            &format!("ema_{}", name.to_lowercase()),
            Section::Mouth,
            "SPARC template units",
            "SPARC ema",
            rate_to_ticks(rate, &chans[c], n),
        );
    }
    let idx = |k: &str| names.iter().position(|x| x == k);
    if let (Some(u), Some(l)) = (idx("ULY"), idx("LLY")) {
        let ap: Vec<f64> = chans[u].iter().zip(&chans[l]).map(|(a, b)| a - b).collect();
        a.add(
            "lip_aperture",
            Section::Mouth,
            "SPARC template units",
            "glue: ULY - LLY",
            rate_to_ticks(rate, &ap, n),
        );
    }
    for k in ["periodicity", "loudness"] {
        if v.get(k).is_some() {
            let sec = Section::Throat;
            a.add(
                &format!("sparc_{k}"),
                sec,
                "SPARC",
                "SPARC source features",
                rate_to_ticks(rate, &arr(&v[k]), n),
            );
        }
    }
    brick(a, v);
    Ok(())
}

/// DisVoice static glottal + phonation features, and Phonet per-frame posteriors.
pub fn add_disvoice(a: &mut Articulation, v: &Value) -> Result<(), String> {
    for group in ["glottal", "phonation"] {
        if let Some(m) = v.get(group).and_then(|g| g.as_object()) {
            for (k, x) in m {
                if let Some(x) = x.as_f64().filter(|x| x.is_finite()) {
                    a.statics
                        .insert(format!("{group}.{k}"), (Section::Throat, x));
                }
            }
        }
    }
    if let Some(p) = v.get("phonet") {
        let rate = p["rate_hz"].as_f64().ok_or("phonet: rate_hz")?;
        if let Some(cls) = p["classes"].as_object() {
            for (k, series) in cls {
                let sec = match k.as_str() {
                    "nasal" => Section::Nose,
                    "voice" => Section::Throat,
                    _ => Section::Mouth,
                };
                a.add(
                    &format!("phon_{k}"),
                    sec,
                    "posterior",
                    "Phonet",
                    rate_to_ticks(rate, &arr(series), a.n_ticks),
                );
            }
        }
    }
    brick(a, v);
    Ok(())
}

/// Rhubarb Lip Sync `--exportFormat json` output -> mouth shape per tick (tick centre).
pub fn add_rhubarb(a: &mut Articulation, v: &Value) -> Result<(), String> {
    let cues = v["mouthCues"].as_array().ok_or("rhubarb: mouthCues")?;
    let mut out = vec!["X".to_string(); a.n_ticks];
    for (i, slot) in out.iter_mut().enumerate() {
        let t = (i as f64 + 0.5) * crate::TICK_S;
        for c in cues {
            let (s, e) = (
                c["start"].as_f64().unwrap_or(0.0),
                c["end"].as_f64().unwrap_or(0.0),
            );
            if t >= s && t < e {
                *slot = c["value"].as_str().unwrap_or("X").to_string();
                break;
            }
        }
    }
    a.visemes = Some(out);
    a.bricks.push(Brick {
        name: "Rhubarb Lip Sync".into(),
        version: "1.13.0".into(),
        licence: "MIT".into(),
        lang: "C++".into(),
        process: "out of process (CLI)".into(),
    });
    Ok(())
}
