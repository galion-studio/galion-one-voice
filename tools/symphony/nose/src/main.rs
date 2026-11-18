// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-nose plan    --intent intent.json [--config nose.toml] [--emotions emotions.toml] [--out plan.json]
//! galion-nose plan    --text "..." [--category amusement] [--intensity 0] [--arousal 0]
//!                      [--burst laugh] [--burst-at end] [--mouth open] [--f0 120] [--from-feeling]
//!                      [--out plan.json]
//! galion-nose render  --plan plan.json [--sr 24000] [--reference-laugh ref.wav] [--out-nose n.wav] [--out-bursts b.wav]
//! galion-nose read    --json worker.json
//!
//! A reference WAV is read at runtime and is not written back into the repository.
//! Burst audio is synthesized, or taken from that reference, and is not a committed asset.
#![forbid(unsafe_code)]

use galion_nose::baseline::Speaker;
use galion_nose::bursts::BurstAsk;
use galion_nose::config::Config;
use galion_nose::contract::{self, Intent};
use galion_nose::feeling::OpenFeeling;
use galion_nose::inventory::Inventory;
use galion_nose::plan::{self, Request};
use galion_nose::read;
use galion_nose::synth::{self, GestureRef};
use galion_nose::wav;
use std::collections::HashMap;
use std::path::Path;

fn args() -> (String, HashMap<String, Vec<String>>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            let v = it.next().unwrap_or_default();
            m.entry(k.to_string()).or_default().push(v);
        }
    }
    (cmd, m)
}

fn one<'a>(m: &'a HashMap<String, Vec<String>>, k: &str) -> Option<&'a str> {
    m.get(k).and_then(|v| v.first()).map(|s| s.as_str())
}

fn flag(m: &HashMap<String, Vec<String>>, k: &str) -> bool {
    m.contains_key(k)
}

fn num(m: &HashMap<String, Vec<String>>, k: &str, default: f64) -> Result<f64, String> {
    match one(m, k) {
        None => Ok(default),
        Some(s) => s
            .parse::<f64>()
            .map_err(|_| format!("--{k} is not a number"))
            .and_then(|n| {
                if n.is_finite() {
                    Ok(n)
                } else {
                    Err(format!("--{k} must be finite"))
                }
            }),
    }
}

fn load_config(m: &HashMap<String, Vec<String>>) -> Result<Config, String> {
    match one(m, "config") {
        Some(p) => Config::load(Path::new(p)),
        None => Config::bundled(),
    }
}

fn load_inventory(m: &HashMap<String, Vec<String>>) -> Result<Inventory, String> {
    match one(m, "emotions") {
        Some(p) => Inventory::load(Path::new(p)),
        None => Inventory::bundled(),
    }
}

fn write_text(path: &str, body: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    std::fs::write(path, body).map_err(|e| format!("{path}: {e}"))
}

fn emit(path: Option<&str>, body: &str) -> Result<(), String> {
    match path {
        Some(p) => write_text(p, body),
        None => {
            print!("{body}");
            Ok(())
        }
    }
}

fn run() -> Result<(), String> {
    let (cmd, m) = args();
    match cmd.as_str() {
        "plan" => {
            let cfg = load_config(&m)?;
            let inventory = load_inventory(&m)?;
            if let Some(path) = one(&m, "intent") {
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let intent = Intent::parse(&text)?;
                let (planned, reading) = contract::plan_intent(&intent, &cfg, &inventory)?;
                let mut value = galion_nose::handoff::profile_patch(&planned);
                if let Some(r) = reading {
                    value["reading"] = serde_json::to_value(&r).map_err(|e| e.to_string())?;
                }
                value["plan"] = serde_json::to_value(&planned).map_err(|e| e.to_string())?;
                let body = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
                emit(one(&m, "out"), &format!("{body}\n"))?;
            } else {
                let text = if let Some(p) = one(&m, "text-file") {
                    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?
                } else {
                    one(&m, "text").unwrap_or("").to_string()
                };
                let feeling = OpenFeeling {
                    arousal: num(&m, "arousal", 0.0)?,
                    valence: num(&m, "valence", 0.0)?,
                    dominance: num(&m, "dominance", 0.0)?,
                    intensity: num(&m, "intensity", 0.0)?,
                    category: one(&m, "category").unwrap_or("neutral").to_string(),
                    burst: Vec::new(),
                }
                .sanitized()?;
                let speaker = Speaker {
                    f0_hz: num(&m, "f0", 0.0)?,
                    velum_rest: num(&m, "velum-rest", 0.0)?,
                    phon_nasal: None,
                };
                let mut req = Request::new(&text, feeling, speaker);
                req.place_from_feeling = flag(&m, "from-feeling");
                if let Some(g) = one(&m, "burst") {
                    let at = one(&m, "burst-at").unwrap_or("end");
                    let (place, at_s) = if let Ok(t) = at.parse::<f64>() {
                        (None, Some(t))
                    } else {
                        (Some(at.to_string()), None)
                    };
                    req.bursts.push(BurstAsk {
                        gesture: g.to_string(),
                        at_s,
                        place,
                        category: one(&m, "burst-category").map(|s| s.to_string()),
                        mouth: one(&m, "mouth").map(|s| s.to_string()),
                        end_s: None,
                    });
                }
                let planned = plan::plan(&req, &cfg, &inventory)?;
                let body = serde_json::to_string_pretty(&planned).map_err(|e| e.to_string())?;
                emit(one(&m, "out"), &format!("{body}\n"))?;
            }
        }
        "render" => {
            let cfg = load_config(&m)?;
            let path = one(&m, "plan").ok_or("render needs --plan")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let planned: galion_nose::plan::Plan = if value.get("ticks").is_some() {
                serde_json::from_value(value).map_err(|e| format!("plan: {e}"))?
            } else if let Some(p) = value.get("plan") {
                serde_json::from_value(p.clone()).map_err(|e| format!("plan: {e}"))?
            } else {
                return Err("plan JSON needs ticks, or a plan object".into());
            };
            let sr = num(&m, "sr", 24_000.0)?;
            let mut owned: Vec<(String, galion_nose::wav::Wav)> = Vec::new();
            for gesture in ["laugh", "sigh", "moan", "gasp", "hum", "mm"] {
                let key = format!("reference-{gesture}");
                if let Some(p) = one(&m, &key) {
                    owned.push((gesture.to_string(), wav::read_wav(Path::new(p))?));
                }
            }
            let refs: Vec<GestureRef<'_>> = owned
                .iter()
                .map(|(g, w)| GestureRef {
                    gesture: g.as_str(),
                    samples: w.samples.as_slice(),
                    sample_rate: w.sample_rate,
                })
                .collect();
            let rendered = synth::render(&planned, sr, &refs, &cfg)?;
            if let Some(p) = one(&m, "out-nose") {
                wav::write_wav_mono_i16(Path::new(p), sr.round() as u32, &rendered.nose)?;
            }
            if let Some(p) = one(&m, "out-bursts") {
                wav::write_wav_mono_i16(Path::new(p), sr.round() as u32, &rendered.bursts)?;
            }
            let note = serde_json::json!({
                "sample_rate": rendered.sample_rate,
                "nose_samples": rendered.nose.len(),
                "burst_samples": rendered.bursts.len(),
                "burst_events": rendered.burst_events,
            });
            emit(
                one(&m, "out"),
                &format!(
                    "{}\n",
                    serde_json::to_string_pretty(&note).map_err(|e| e.to_string())?
                ),
            )?;
        }
        "read" => {
            let path = one(&m, "json").ok_or("read needs --json")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let reading = read::parse_reading(&value)?;
            let body = serde_json::to_string_pretty(&reading).map_err(|e| e.to_string())?;
            emit(one(&m, "out"), &format!("{body}\n"))?;
        }
        _ => {
            return Err("usage: galion-nose plan | render | read  (see the crate README)".into());
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
