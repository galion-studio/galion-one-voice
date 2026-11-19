// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-articulation / galion-artic
//!   analyze --wav IN.wav [--sparc S.json] [--disvoice D.json] [--rhubarb R.json] --out CARD.json
//!   compare --ref A.json --clone B.json [--embedding-cosine C] [--out SCORE.json]
//!   decode  --wav IN.wav [--emotion NAME] [--env studio|close|cafe|hall]
//!           [--enrol E.wav] [--labels FILE.toml] [--emotion-json J] [--egemaps J]
//!           [--neutral-egemaps J] [--cpps DB] --out PROFILE.json
//!   select  --wav SCENE.wav --enrol E.wav [--tools SCENE.json] --out SEGMENTS.json
//!   loop    --ref REF.wav [--emotion NAME] [--env cafe] [--enrol E.wav]
//!           [--max-iter N] [--render-cmd CMD] --out REPORT.json

use galion_articulation::acoustic;
use galion_articulation::closed_loop::{self, Request};
use galion_articulation::decode::{self, Opts};
use galion_articulation::labels;
use galion_articulation::score;
use galion_articulation::select;
use galion_articulation::Articulation;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            m.insert(k.to_string(), it.next().unwrap_or_default());
        }
    }
    (cmd, m)
}

fn read_json(p: &str) -> Result<serde_json::Value, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("{p}: {e}"))
}

fn write(out: Option<&String>, s: String) -> Result<(), String> {
    match out {
        Some(p) => std::fs::write(p, s).map_err(|e| format!("{p}: {e}")),
        None => {
            println!("{s}");
            Ok(())
        }
    }
}

fn kept_or_all(
    samples: &[f64],
    sr: f64,
    enrol: Option<&str>,
) -> Result<(Vec<f64>, Option<select::SegmentTable>), String> {
    let Some(path) = enrol else {
        return Ok((samples.to_vec(), None));
    };
    let (e, esr) = acoustic::read_wav_mono(path)?;
    if (esr - sr).abs() > 1.0 {
        return Err("enrolment clip sample rate differs from the scene".into());
    }
    let table = select::select(samples, sr, &e, None)?;
    if table.n_kept == 0 {
        return Err("no clean target-speaker segment".into());
    }
    Ok((select::concat_kept(samples, sr, &table), Some(table)))
}

fn run() -> Result<(), String> {
    let (cmd, a) = args();
    match cmd.as_str() {
        "analyze" => {
            let wav = a.get("wav").ok_or("--wav required")?;
            let (x, sr) = acoustic::read_wav_mono(wav)?;
            let (mut art, settings) = acoustic::analyse(&x, sr);
            if let Some(p) = a.get("sparc") {
                galion_articulation::workers::add_sparc(&mut art, &read_json(p)?)?;
            }
            if let Some(p) = a.get("disvoice") {
                galion_articulation::workers::add_disvoice(&mut art, &read_json(p)?)?;
            }
            if let Some(p) = a.get("rhubarb") {
                galion_articulation::workers::add_rhubarb(&mut art, &read_json(p)?)?;
            }
            art.finish();
            let card = serde_json::json!({ "articulation": art, "settings": settings });
            write(a.get("out"), serde_json::to_string_pretty(&card).unwrap())
        }
        "compare" => {
            let load = |k: &str| -> Result<Articulation, String> {
                let v = read_json(a.get(k).ok_or(format!("--{k} required"))?)?;
                serde_json::from_value(v["articulation"].clone()).map_err(|e| e.to_string())
            };
            let cos = a
                .get("embedding-cosine")
                .map(|s| s.parse::<f64>().map_err(|e| e.to_string()))
                .transpose()?;
            let s = score::compare_with(&load("ref")?, &load("clone")?, cos);
            write(a.get("out"), serde_json::to_string_pretty(&s).unwrap())
        }
        "decode" => {
            let wav = a.get("wav").ok_or("--wav required")?;
            let (x, sr) = acoustic::read_wav_mono(wav)?;
            let (kept, segments) = kept_or_all(&x, sr, a.get("enrol").map(|s| s.as_str()))?;
            let (mut art, _) = acoustic::analyse(&kept, sr);
            if let Some(p) = a.get("disvoice") {
                galion_articulation::workers::add_disvoice(&mut art, &read_json(p)?)?;
            }
            art.finish();
            let emotion_json = a.get("emotion-json").map(|p| read_json(p)).transpose()?;
            let egemaps = a.get("egemaps").map(|p| read_json(p)).transpose()?;
            let neutral = a.get("neutral-egemaps").map(|p| read_json(p)).transpose()?;
            let labels = a
                .get("labels")
                .map(|p| labels::load(Path::new(p)))
                .transpose()?;
            let cpps = a
                .get("cpps")
                .map(|s| s.parse::<f64>().map_err(|e| e.to_string()))
                .transpose()?;
            let profile = decode::profile(
                &art,
                &kept,
                sr,
                &Opts {
                    emotion: a.get("emotion").map(|s| s.as_str()),
                    env_name: a.get("env").map(|s| s.as_str()),
                    emotion_json: emotion_json.as_ref(),
                    egemaps: egemaps.as_ref(),
                    neutral_egemaps: neutral.as_ref(),
                    labels: labels.as_ref(),
                    cpps_db: cpps,
                    breath_cues: None,
                    breath_source: None,
                },
            )?;
            let body = serde_json::json!({ "profile": profile, "segments": segments });
            write(a.get("out"), serde_json::to_string_pretty(&body).unwrap())
        }
        "select" => {
            let wav = a.get("wav").ok_or("--wav required")?;
            let enrol = a.get("enrol").ok_or("--enrol required")?;
            let (x, sr) = acoustic::read_wav_mono(wav)?;
            let (e, esr) = acoustic::read_wav_mono(enrol)?;
            if (esr - sr).abs() > 1.0 {
                return Err("enrolment clip sample rate differs from the scene".into());
            }
            let tools = a.get("tools").map(|p| read_json(p)).transpose()?;
            let table = select::select(&x, sr, &e, tools.as_ref())?;
            write(a.get("out"), serde_json::to_string_pretty(&table).unwrap())
        }
        "loop" => {
            let wav = a.get("ref").ok_or("--ref required")?;
            let (x, sr) = acoustic::read_wav_mono(wav)?;
            let enrol_samples = if let Some(p) = a.get("enrol") {
                let (e, esr) = acoustic::read_wav_mono(p)?;
                if (esr - sr).abs() > 1.0 {
                    return Err("enrolment clip sample rate differs from the reference".into());
                }
                Some(e)
            } else {
                None
            };
            let max_iter = a
                .get("max-iter")
                .map(|s| s.parse::<usize>().map_err(|e| e.to_string()))
                .transpose()?
                .unwrap_or(3);
            if let Some(cmd) = a.get("render-cmd") {
                return external_loop(&x, sr, &a, cmd, max_iter, enrol_samples.as_deref());
            }
            let report = closed_loop::run_standin(
                &x,
                sr,
                &Request {
                    emotion: a.get("emotion").map(|s| s.as_str()),
                    env_name: a.get("env").map(|s| s.as_str()),
                    enrol: enrol_samples.as_deref(),
                    max_iter,
                    start_exaggeration: None,
                },
            )?;
            write(a.get("out"), serde_json::to_string_pretty(&report).unwrap())
        }
        _ => Err("usage: galion-articulation analyze|compare|decode|select|loop".into()),
    }
}

fn external_loop(
    samples: &[f64],
    sr: f64,
    a: &HashMap<String, String>,
    render_cmd: &str,
    max_iter: usize,
    enrol: Option<&[f64]>,
) -> Result<(), String> {
    let (kept, segments) = if let Some(e) = enrol {
        let table = select::select(samples, sr, e, None)?;
        if table.n_kept == 0 {
            return Err("no clean target-speaker segment".into());
        }
        (select::concat_kept(samples, sr, &table), Some(table))
    } else {
        (samples.to_vec(), None)
    };
    let dir = std::env::temp_dir().join(format!("galion-artic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ref_path = dir.join("ref.wav");
    let out_path = dir.join("clone.wav");
    let profile_path = dir.join("profile.json");
    write_wav(&ref_path, &kept, sr)?;
    let (mut art, _) = acoustic::analyse(&kept, sr);
    art.finish();
    let mut profile = decode::profile(
        &art,
        &kept,
        sr,
        &Opts {
            emotion: a.get("emotion").map(|s| s.as_str()),
            env_name: a.get("env").map(|s| s.as_str()),
            emotion_json: None,
            egemaps: None,
            neutral_egemaps: None,
            labels: None,
            cpps_db: None,
            breath_cues: None,
            breath_source: None,
        },
    )?;
    let goal = profile.chatterbox;
    let goal_f0 = profile.speaker.f0_hz;
    let goal_rate = profile.lungs.voiced_runs_per_s * goal.speed;
    let goal_rms = decode::sample_rms_db(&kept) + (goal.exaggeration - 0.5) * 12.0;
    let mut params = goal;
    let mut applied = Vec::new();
    let mut rows = Vec::new();
    for _ in 0..max_iter {
        applied.push(params);
        profile.chatterbox = params;
        std::fs::write(
            &profile_path,
            serde_json::to_string_pretty(&profile).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        let status = Command::new("sh")
            .arg("-c")
            .arg(render_cmd)
            .env("GALION_PROFILE", &profile_path)
            .env("GALION_REF", &ref_path)
            .env("GALION_OUT", &out_path)
            .status()
            .map_err(|e| format!("render: {e}"))?;
        if !status.success() {
            return Err(format!("render command exited {status}"));
        }
        let (y, ysr) = acoustic::read_wav_mono(out_path.to_str().unwrap())?;
        if (ysr - sr).abs() > 1.0 {
            return Err("renderer changed the sample rate".into());
        }
        let (mut got, _) = acoustic::analyse(&y, ysr);
        got.finish();
        let got_profile = decode::profile(
            &got,
            &y,
            ysr,
            &Opts {
                emotion: None,
                env_name: None,
                emotion_json: None,
                egemaps: None,
                neutral_egemaps: None,
                labels: None,
                cpps_db: None,
                breath_cues: None,
                breath_source: None,
            },
        )?;
        rows = closed_loop::deltas(
            goal_f0,
            goal_rms,
            goal_rate,
            &closed_loop::Observation {
                f0_hz: got_profile.speaker.f0_hz,
                rms_dbfs: decode::sample_rms_db(&y),
                voiced_runs_per_s: got_profile.lungs.voiced_runs_per_s,
            },
        );
        if rows.iter().all(|d| d.pass) {
            break;
        }
        let next = closed_loop::correct(params, &rows);
        if (next.exaggeration - params.exaggeration).abs() < 1e-3
            && (next.speed - params.speed).abs() < 1e-3
        {
            break;
        }
        params = next;
    }
    let report = serde_json::json!({
        "schema": 1,
        "renderer": "external",
        "passed": rows.iter().all(|d| d.pass),
        "iterations": applied.len(),
        "profile": profile,
        "segments": segments,
        "applied": applied,
        "deltas": rows,
    });
    write(a.get("out"), serde_json::to_string_pretty(&report).unwrap())
}

fn write_wav(path: &Path, samples: &[f64], sr: f64) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr.round() as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("galion-articulation: {e}");
        std::process::exit(2);
    }
}
