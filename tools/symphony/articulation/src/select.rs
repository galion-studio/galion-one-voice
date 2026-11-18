// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Pick clean target-speaker segments out of a real-life take.
//!
//! The phrase split and the frame-SNR idea are the ones `refprep` already
//! uses (`ecapa_consistent` phrases, pause 0.20 s). Speaker match against an
//! enrolment clip is acoustic (F0, F1, F2) unless a tool JSON supplies an
//! embedding cosine from galion-ecapa / pyannote. Music, overlap, and neural
//! denoise stay in `workers/scene_tools.py` (DeepFilterNet, Demucs, pyannote,
//! Silero) and are gaps when those tools are not installed. This file does
//! not reimplement them.

use crate::acoustic;
use crate::environment;
use crate::state::{self, SpeakerBaseline};
use serde::Serialize;
use serde_json::Value;

const PAUSE_S: f64 = 0.20;
const MIN_DUR_S: f64 = 0.35;
const MIN_SIM: f64 = 0.55;
const MIN_VOICED: f64 = 0.35;
const MIN_SNR_DB: f64 = 6.0;

#[derive(Clone, Debug, Serialize)]
pub struct Segment {
    pub start_s: f64,
    pub end_s: f64,
    pub speaker: String,
    pub speaker_sim: f64,
    pub embedding_cos: Option<f64>,
    pub usable: f64,
    pub snr_db: f64,
    pub music_noise: String,
    pub rt60_s: Option<f64>,
    pub emotion: String,
    pub kept: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentTable {
    pub schema: u32,
    pub built_on: String,
    pub n_in: usize,
    pub n_kept: usize,
    pub gaps: Vec<String>,
    pub segments: Vec<Segment>,
}

fn frame_db(x: &[f64], sr: f64) -> (Vec<f64>, usize) {
    let win = ((0.025 * sr) as usize).max(1);
    let hop = ((0.010 * sr) as usize).max(1);
    let mut levels = Vec::new();
    let mut i = 0;
    while i + win <= x.len() {
        let e = x[i..i + win].iter().map(|s| s * s).sum::<f64>() / win as f64;
        levels.push(if e <= 1e-12 { -120.0 } else { 10.0 * e.log10() });
        i += hop;
    }
    (levels, hop)
}

fn quantile(xs: &[f64], p: f64) -> f64 {
    if xs.is_empty() {
        return -120.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let t = p.clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let i = t.floor() as usize;
    let f = t - i as f64;
    if i + 1 >= v.len() {
        v[i]
    } else {
        v[i] * (1.0 - f) + v[i + 1] * f
    }
}

fn sim_f0(a: f64, b: f64) -> Option<f64> {
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    let st = (12.0 * (a / b).log2()).abs();
    Some(1.0 / (1.0 + st / 2.5))
}

fn sim_hz(a: f64, b: f64, scale: f64) -> Option<f64> {
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    Some(1.0 / (1.0 + (a - b).abs() / scale))
}

fn identity_score(seg: &SpeakerBaseline, enrol: &SpeakerBaseline) -> (f64, f64) {
    let mut acc = 0.0;
    let mut w = 0.0;
    if let Some(s) = sim_f0(seg.f0_hz, enrol.f0_hz) {
        acc += 5.0 * s;
        w += 5.0;
    }
    if let (Some(a), Some(b)) = (seg.vtl_cm, enrol.vtl_cm) {
        if let Some(s) = sim_hz(a, b, 1.2) {
            acc += 3.0 * s;
            w += 3.0;
        }
    }
    (acc, w)
}

fn formant_sim(seg: &crate::Articulation, enrol: &crate::Articulation) -> (f64, f64) {
    let mut acc = 0.0;
    let mut w = 0.0;
    for (name, scale, wt) in [("f1_hz", 80.0, 2.0), ("f2_hz", 150.0, 2.0)] {
        let m = |a: &crate::Articulation| a.summary.get(name).and_then(|s| s.q.get(4).copied());
        if let (Some(a), Some(b)) = (m(seg), m(enrol)) {
            if let Some(s) = sim_hz(a, b, scale) {
                acc += wt * s;
                w += wt;
            }
        }
    }
    (acc, w)
}

pub fn select(
    scene: &[f64],
    sr: f64,
    enrol: &[f64],
    tools: Option<&Value>,
) -> Result<SegmentTable, String> {
    if scene.is_empty() || enrol.is_empty() {
        return Err("select needs the scene and an enrolment clip".into());
    }
    let (enrol_art, _) = acoustic::analyse(enrol, sr);
    let mut enrol_done = enrol_art;
    enrol_done.finish();
    let enrol_base = state::speaker_baseline(&enrol_done);
    let (levels, hop) = frame_db(scene, sr);
    let floor = quantile(&levels, 0.10);
    let speech: Vec<bool> = levels.iter().map(|db| *db > floor + 8.0).collect();
    let min_pause = ((PAUSE_S * sr) / hop as f64).round().max(1.0) as usize;
    let mut spans = Vec::new();
    let mut i = 0;
    while i < speech.len() {
        if !speech[i] {
            i += 1;
            continue;
        }
        let st = i;
        let mut j = i;
        let mut sil = 0;
        while j < speech.len() && (speech[j] || sil < min_pause) {
            if speech[j] {
                sil = 0;
            } else {
                sil += 1;
            }
            j += 1;
        }
        let end = j - sil;
        let a = st * hop;
        let b = (end * hop).min(scene.len());
        if b > a {
            spans.push((a, b));
        }
        i = j;
    }
    let mut gaps = vec![
        "deep-filter: not run (DeepFilterNet stays out of process; install and call workers/scene_tools.py)".into(),
        "demucs: not run (htdemucs music separation is out of process)".into(),
        "pyannote: not run (diarization is out of process)".into(),
        "silero-vad: not run (neural VAD is out of process; phrase split uses the refprep energy mask)".into(),
    ];
    let tool_rows = tools
        .and_then(|v| v.get("segments"))
        .and_then(|s| s.as_array());
    if let Some(v) = tools {
        if let Some(g) = v.get("gaps").and_then(|g| g.as_array()) {
            gaps = g
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
        } else if tool_rows.is_some() {
            gaps.clear();
        }
    }
    let mut segments = Vec::new();
    for (a, b) in spans {
        let start_s = a as f64 / sr;
        let end_s = b as f64 / sr;
        let dur = end_s - start_s;
        let slice = &scene[a..b];
        let (mut art, _) = acoustic::analyse(slice, sr);
        art.finish();
        let base = state::speaker_baseline(&art);
        let (mut acc, mut w) = identity_score(&base, &enrol_base);
        if base.f0_hz <= 0.0 {
            acc = 0.0;
            w = 0.0;
        } else {
            let (f_acc, f_w) = formant_sim(&art, &enrol_done);
            acc += f_acc;
            w += f_w;
        }
        let mut speaker_sim = if w == 0.0 { 0.0 } else { acc / w };
        let voiced = art
            .tracks
            .get("voicing")
            .map(|t| {
                let v: Vec<f64> = t.ticks.iter().flatten().copied().collect();
                if v.is_empty() {
                    0.0
                } else {
                    v.iter().sum::<f64>() / v.len() as f64
                }
            })
            .unwrap_or(0.0);
        let seg_levels: Vec<f64> = {
            let (lv, _) = frame_db(slice, sr);
            lv
        };
        let level = quantile(&seg_levels, 0.60);
        let snr_db = level - floor;
        let mut music_noise = if voiced < MIN_VOICED {
            "noise"
        } else {
            "speech"
        }
        .to_string();
        let mut embedding_cos = None;
        if let Some(rows) = tool_rows {
            for row in rows {
                let rs = row.get("start_s").and_then(|x| x.as_f64()).unwrap_or(-1.0);
                let re = row.get("end_s").and_then(|x| x.as_f64()).unwrap_or(-1.0);
                let overlap = rs < end_s && re > start_s;
                if !overlap {
                    continue;
                }
                if let Some(c) = row.get("embedding_cos").and_then(|x| x.as_f64()) {
                    embedding_cos = Some(c);
                    speaker_sim = 0.7 * c.clamp(0.0, 1.0) + 0.3 * speaker_sim;
                }
                if let Some(k) = row.get("music_noise").and_then(|x| x.as_str()) {
                    music_noise = k.to_string();
                }
            }
        }
        let (fluid, _) = state::measure_fluid(
            &art,
            state::Avd {
                arousal: 0.0,
                valence: 0.0,
                dominance: 0.0,
            },
        );
        let emotion = fluid
            .first()
            .map(|t| state::top_blend(&t.blend))
            .unwrap_or_else(|| "neutral".into());
        let snr_f = (snr_db / 25.0).clamp(0.0, 1.0);
        let usable = (speaker_sim * voiced.clamp(0.0, 1.0) * snr_f).clamp(0.0, 1.0);
        let speaker = if base.f0_hz <= 0.0 {
            "unknown"
        } else if speaker_sim >= MIN_SIM {
            "target"
        } else {
            "other"
        };
        let kept = dur >= MIN_DUR_S
            && speaker_sim >= MIN_SIM
            && voiced >= MIN_VOICED
            && snr_db >= MIN_SNR_DB
            && music_noise != "music";
        let env = environment::measure(slice, sr, None).ok();
        segments.push(Segment {
            start_s,
            end_s,
            speaker: speaker.into(),
            speaker_sim,
            embedding_cos,
            usable,
            snr_db,
            music_noise,
            rt60_s: env.and_then(|e| e.rt60_s),
            emotion,
            kept,
        });
    }
    let n_kept = segments.iter().filter(|s| s.kept).count();
    Ok(SegmentTable {
        schema: 1,
        built_on: "refprep phrase split + enrolment F0/formant match; ECAPA/pyannote cosine when a tool JSON is passed".into(),
        n_in: segments.len(),
        n_kept,
        gaps,
        segments,
    })
}

pub fn concat_kept(scene: &[f64], sr: f64, table: &SegmentTable) -> Vec<f64> {
    let mut out = Vec::new();
    for s in table.segments.iter().filter(|s| s.kept) {
        let a = (s.start_s * sr).round().max(0.0) as usize;
        let b = ((s.end_s * sr).round() as usize).min(scene.len());
        if b > a {
            out.extend_from_slice(&scene[a..b]);
        }
    }
    out
}
