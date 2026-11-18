// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One pass over an S1 intent document.
//!
//! feeling -> lungs, throat, mouth, nose -> sound out -> voice-guard slot -> listen-back.

use std::path::{Path, PathBuf};

use galion_feeling::{frames_from_document, Config as FeelCfg, Feeling, Inventory as FeelInv};
use galion_listen_back::contract::Intent as ListenIntent;
use galion_listen_back::decode;
use galion_listen_back::fixture::BreathSpan;
use galion_listen_back::score;
use galion_lungs::plan::{self, Moment, Request as BreathRequest};
use galion_mne_contract::mirror::load_dir;
use galion_mne_contract::{
    validate, BodyOrigin, BodyState, Confidence, DocKind, Document, Feeling as Heard, NoseState,
    ReadbackFrame, ThroatState, ValueOrigin, FRAME_MS, SCHEMA, SCHEMA_VERSION,
};
use galion_mouth::feeling::FeelingTick;
use galion_mouth::intent::IntentStub;
use galion_mouth::{load_scales, plan as mouth_plan};
use galion_nose::config::Config as NoseCfg;
use galion_nose::contract::request_from_document;
use galion_nose::inventory::Inventory as NoseInv;
use galion_nose::plan as nose_plan;
use galion_nose::synth as nose_synth;
use galion_sound_out::contract::MomentFixture;
use galion_sound_out::environment::Burst;
use galion_sound_out::{render as sound_render, Policy, SAMPLE_RATE};
use galion_throat::contract::{self as throat_contract, Speaker};
use galion_throat::lungs_in::parse_lungs;
use galion_throat::render::{self as throat_render, Request as ThroatReq};
use serde::Serialize;

const VOICE_F0_HZ: f64 = 180.0;

#[derive(Clone, Debug, Serialize)]
pub struct FixtureScore {
    pub id: String,
    pub feeling: f64,
    pub pitch_error_st: Option<f64>,
    pub breath: f64,
    pub pitch_quality: f64,
    pub samples: usize,
    pub backend: String,
    pub guard: String,
    pub readback_ok: bool,
    pub planned_inhales: usize,
}

pub fn fixture_dir(repo: &Path) -> PathBuf {
    repo.join("tools/symphony/mne-contract/fixtures")
}

pub fn run_fixtures(repo: &Path) -> Result<Vec<FixtureScore>, String> {
    run_fixtures_on(repo, "piper")
}

pub fn run_fixtures_on(repo: &Path, backend: &str) -> Result<Vec<FixtureScore>, String> {
    let loaded = load_dir(&fixture_dir(repo))?;
    let mut scores = Vec::new();
    for (path, doc) in loaded {
        if doc.kind != DocKind::Intent {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fixture")
            .trim_end_matches(".intent")
            .to_string();
        scores.push(run_one(repo, &id, &doc, backend)?);
    }
    if scores.len() != 8 {
        return Err(format!("expected 8 intent fixtures, ran {}", scores.len()));
    }
    Ok(scores)
}

fn run_one(
    repo: &Path,
    id: &str,
    doc: &Document,
    backend_name: &str,
) -> Result<FixtureScore, String> {
    validate(doc).map_err(|e| format!("{id}: intent: {e}"))?;
    let feel_cfg = FeelCfg::load(&repo.join("configs/symphony/feeling.toml"))?;
    let feel_inv = FeelInv::load(&FeelInv::bundled_path())?;
    let frames = frames_from_document(doc).map_err(|e| format!("{id}: feeling: {e}"))?;
    let felt =
        Feeling::run(&frames, &feel_cfg, &feel_inv).map_err(|e| format!("{id}: feeling: {e}"))?;

    let category = lungs_name(&felt.lungs.category);
    let mut breath_req = BreathRequest::constant(
        &doc.text,
        &category,
        felt.lungs.arousal,
        felt.lungs.energy,
        felt.lungs.fatigue,
    );
    breath_req.trajectory = felt
        .lungs
        .trajectory
        .iter()
        .map(|m| Moment {
            at_s: m.at_s,
            category: Some(lungs_name(&m.category)),
            arousal: Some(m.arousal),
            energy: Some(m.energy),
            fatigue: Some(m.fatigue),
        })
        .collect();

    let mouth_scales = load_scales(&repo.join("configs/symphony/mouth.toml"))
        .map_err(|e| format!("{id}: mouth: {e}"))?;
    let mouth = mouth_plan(
        &IntentStub::from_document(doc),
        &FeelingTick::from_document(doc),
        &mouth_scales,
    )
    .map_err(|e| format!("{id}: mouth: {e}"))?;
    let rate = mouth.clone_mouth.speaking_rate.amount.unwrap_or(1.0);

    let nose_cfg = NoseCfg::bundled()?;
    let nose_inv = NoseInv::bundled()?;
    let nose_req = request_from_document(doc, VOICE_F0_HZ)?;
    let nose =
        nose_plan::plan(&nose_req, &nose_cfg, &nose_inv).map_err(|e| format!("{id}: nose: {e}"))?;
    let nose_audio = nose_synth::render(&nose, SAMPLE_RATE as f64, &[], &nose_cfg)?;
    let bursts = nose
        .bursts
        .iter()
        .filter_map(|ev| {
            let sr = SAMPLE_RATE as f64;
            let a = (ev.start_s * sr).round().max(0.0) as usize;
            let b = (ev.end_s * sr).round().max(a as f64) as usize;
            let b = b.min(nose_audio.bursts.len());
            if b <= a {
                return None;
            }
            Some(Burst {
                category: ev.category.clone().unwrap_or_else(|| ev.gesture.clone()),
                start_s: ev.start_s,
                samples: nose_audio.bursts[a..b].to_vec(),
                sample_rate: sr,
            })
        })
        .collect::<Vec<_>>();

    let lungs_cfg = galion_lungs::config::Config::bundled()?;
    let lungs_inv = galion_lungs::inventory::Inventory::bundled()?;
    let lungs_plan =
        plan::plan(&breath_req, &lungs_cfg, &lungs_inv).map_err(|e| format!("{id}: lungs: {e}"))?;
    let throat_cfg = galion_throat::config::load_path(&repo.join("configs/symphony/throat.toml"))?;
    let (throat, _) = throat_render::render(
        &ThroatReq {
            speaker: Speaker {
                f0_hz: VOICE_F0_HZ,
                rd: None,
            },
            feeling: throat_contract::drive_from_document(doc)?,
            lungs: parse_lungs(&serde_json::to_value(&lungs_plan).map_err(|e| e.to_string())?)?,
        },
        &throat_cfg,
        false,
    )
    .map_err(|e| format!("{id}: throat: {e}"))?;
    let voiced: Vec<f64> = throat
        .ticks
        .iter()
        .filter_map(|t| t.f0_st_vs_baseline)
        .collect();
    let pitch_st = if voiced.is_empty() {
        0.0
    } else {
        voiced.iter().sum::<f64>() / voiced.len() as f64
    };

    let first = doc.intent_frames.first().ok_or("no frames")?;
    let (pin, track) = match &first.feeling.pin {
        Some(p) => (
            p.name.as_str().to_string(),
            match p.track {
                galion_mne_contract::Track::Burst => "burst",
                galion_mne_contract::Track::Prosody => "prosody",
            },
        ),
        None => ("neutral".into(), "prosody"),
    };
    let mut moment = MomentFixture::new(&doc.text);
    moment.category = category.clone();
    moment.arousal = first.feeling.arousal;
    moment.energy = first.inner.energy;
    moment.fatigue = first.inner.fatigue;
    moment.controls.track = track.into();
    moment.controls.prosody.rate = rate.clamp(0.8, 1.2);
    moment.controls.prosody.pitch_st = pitch_st.clamp(-4.0, 4.0);
    moment.lungs_trajectory = Some(serde_json::Value::Array(
        breath_req
            .trajectory
            .iter()
            .map(|m| {
                serde_json::json!({
                    "at_s": m.at_s,
                    "category": m.category,
                    "arousal": m.arousal,
                    "energy": m.energy,
                    "fatigue": m.fatigue,
                })
            })
            .collect(),
    ));
    moment.clone_mouth = Some(serde_json::to_value(&mouth.clone_mouth).map_err(|e| e.to_string())?);

    let policy = Policy::load(&repo.join("configs/symphony/sound-out.toml"))
        .unwrap_or_else(|_| Policy::builtin());
    if policy.default_backend != "piper" {
        return Err("sound-out default backend must be piper".into());
    }
    let mut backend = policy
        .open(Some(backend_name), Some("cpu"))
        .map_err(|e| format!("{id}: sound-out: {e}"))?;
    let sounded = sound_render(&moment, &mut *backend, &bursts, Some("cpu"))?;
    let (audio, guard) = voice_guard(repo, id, sounded.samples)?;

    let read =
        decode::analyse(&audio, SAMPLE_RATE as f64).map_err(|e| format!("{id}: listen: {e}"))?;
    let listen_intent = ListenIntent {
        schema: SCHEMA.into(),
        source: "galion-mne-contract".into(),
        todo: String::new(),
        feeling_source: "galion-feeling".into(),
        id: id.into(),
        text: doc.text.clone(),
        track: if pin == "neutral" {
            "baseline".into()
        } else {
            track.into()
        },
        category: pin.clone(),
        prosody_tier: String::new(),
        baseline_f0_hz: VOICE_F0_HZ,
        f1_hz: 700.0,
        f2_hz: 1200.0,
        f3_hz: 2800.0,
        pitch_st: pitch_st.clamp(-4.0, 4.0),
        speech_s: audio.len() as f64 / SAMPLE_RATE as f64,
        breaths: lungs_plan
            .segments
            .iter()
            .filter(|s| s.kind == "inhale")
            .map(|s| BreathSpan {
                start_s: s.start_s,
                dur_s: (s.end_s - s.start_s).max(0.0),
            })
            .collect(),
        feeling: Vec::new(),
    };
    let (feeling_score, _) = score::feeling(&listen_intent, &read);
    let err = score::pitch_error_st(&read, &listen_intent);
    let heard = score::breath_events(&read.voicing_ticks, &read.rms_db_ticks);
    let breath = score::breath_plan_match(&listen_intent.breaths, &heard);
    let quality = score::pitch_voice_quality(err, read.hnr_db);

    let readback = readback_doc(doc, &read)?;
    validate(&readback).map_err(|e| format!("{id}: readback: {e}"))?;

    Ok(FixtureScore {
        id: id.into(),
        feeling: feeling_score,
        pitch_error_st: err,
        breath,
        pitch_quality: quality,
        samples: audio.len(),
        backend: sounded.backend,
        guard,
        readback_ok: true,
        planned_inhales: listen_intent.breaths.len(),
    })
}

fn lungs_name(name: &str) -> String {
    if name.is_empty() {
        "neutral".into()
    } else {
        name.to_string()
    }
}

fn voice_guard(repo: &Path, id: &str, samples: Vec<f64>) -> Result<(Vec<f64>, String), String> {
    let cfg = galion_voice_guard::load_config(&repo.join("configs/symphony/voice-guard.toml"))
        .map_err(|e| format!("{id}: guard: {e}"))?;
    let pcm: Vec<f32> = samples.iter().copied().map(|s| s as f32).collect();
    match galion_voice_guard::before_listen(&pcm, SAMPLE_RATE, &cfg, None, None) {
        Ok(report) => Ok((samples, report.verdict.to_string())),
        Err(report) => Err(format!("{id}: guard: {}", report.reason)),
    }
}

fn readback_doc(
    intent: &Document,
    read: &galion_listen_back::decode::ReadBack,
) -> Result<Document, String> {
    let n = read.n_ticks.max(1);
    let mut frames = Vec::with_capacity(n);
    for i in 0..n {
        let t_ms = (i as u32).saturating_mul(FRAME_MS as u32);
        let src = intent
            .intent_frames
            .iter()
            .rev()
            .find(|f| f.t_ms <= t_ms)
            .or(intent.intent_frames.first())
            .ok_or("no intent frame")?;
        let f0 = read.f0_hz_ticks.get(i).copied().flatten();
        let measured = f0.is_some();
        frames.push(ReadbackFrame {
            t_ms,
            feeling: Heard {
                arousal: src.feeling.arousal,
                valence: src.feeling.valence,
                dominance: src.feeling.dominance,
                pin: src.feeling.pin.clone(),
            },
            body: BodyState {
                origin: if measured {
                    BodyOrigin::Measured
                } else {
                    BodyOrigin::Unmeasured
                },
                lungs: Default::default(),
                throat: ThroatState {
                    f0_hz: f0.filter(|_| measured),
                    hnr_db: read.hnr_db.filter(|_| measured),
                },
                mouth: Default::default(),
                nose: NoseState::default(),
            },
            confidence: Confidence {
                feeling: 1.0,
                body: if measured { 1.0 } else { 0.0 },
                confounded: false,
            },
        });
    }
    Ok(Document {
        schema: SCHEMA.into(),
        schema_version: SCHEMA_VERSION,
        kind: DocKind::Readback,
        frame_ms: FRAME_MS,
        value_origin: ValueOrigin::Measured,
        note: format!("read-back of {}", intent.moment.moment_id),
        persona: intent.persona.clone(),
        moment: intent.moment.clone(),
        text: intent.text.clone(),
        baseline: intent.baseline,
        intent_frames: Vec::new(),
        readback_frames: frames,
    })
}
