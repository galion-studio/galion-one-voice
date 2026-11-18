// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The closed loop. Fixtures go through the intent stub (or S1's binary),
//! the feeling stub (or S2), an optional lungs plan (PR #195), a render,
//! the articulation decoder, one correction, and a second score.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::contract::{self, Intent};
use crate::decode::{self, ReadBack};
use crate::fixture::{self, Fixture};
use crate::render::{self, Control};
use crate::report::{Correction, LungsNote, PassReport, Report, ScenarioReport};
use crate::round4;
use crate::score::{self, LayerScores};

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub render_cmd: Option<String>,
    pub articulation_bin: Option<String>,
    pub lungs_bin: Option<String>,
    pub feeling_bin: Option<String>,
    pub contract_bin: Option<String>,
}

impl Options {
    /// CI path. No sibling binary, no cloner, no environment variables.
    pub fn synthetic() -> Self {
        Options::default()
    }

    pub fn from_env() -> Self {
        Options {
            render_cmd: env_nonempty("GALION_RENDER_CMD"),
            articulation_bin: env_nonempty("GALION_ARTICULATION"),
            lungs_bin: env_nonempty("GALION_LUNGS"),
            feeling_bin: env_nonempty("GALION_FEELING"),
            contract_bin: env_nonempty("GALION_MNE_CONTRACT"),
        }
    }
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn run_all(opt: &Options) -> Result<Report, String> {
    let mut scenarios = Vec::new();
    let mut contract_ran = false;
    let mut feeling_ran = false;
    let mut lungs_ran = false;
    for fixture in fixture::builtin() {
        let mut intent = if let Some(bin) = &opt.contract_bin {
            contract_ran = true;
            load_contract(bin, &fixture)?
        } else {
            contract::from_fixture(&fixture)?
        };
        if let Some(bin) = &opt.feeling_bin {
            feeling_ran = true;
            load_feeling(bin, &mut intent)?;
        }
        let lungs = if let Some(bin) = &opt.lungs_bin {
            lungs_ran = true;
            load_lungs(bin, &intent)?
        } else {
            LungsNote {
                status: "stub".into(),
                inhales: Vec::new(),
                agreement: None,
                note: "GALION_LUNGS is unset. Breath scores compare the read-back with the fixture. The planner in PR #195 is not reimplemented here.".into(),
            }
        };
        let reference_samples = render::reference(&intent);
        let reference = decode::analyse(&reference_samples, render::SR)?;
        let first = if opt.render_cmd.is_some() {
            Control {
                pitch_st: intent.pitch_st,
                place_breaths: !intent.breaths.is_empty(),
                renderer: "render-cmd".into(),
            }
        } else {
            Control::standin_first(&intent)
        };
        let pass1 = one_pass(opt, &intent, &first, &reference, &lungs, 1)?;
        let correction = correct_once(&intent, &pass1);
        let second = Control {
            pitch_st: correction.to_pitch_st,
            place_breaths: correction.place_breaths,
            renderer: first.renderer.clone(),
        };
        let pass2 = one_pass(opt, &intent, &second, &reference, &lungs, 2)?;
        scenarios.push(ScenarioReport {
            id: intent.id.clone(),
            text: intent.text.clone(),
            track: intent.track.clone(),
            category: intent.category.clone(),
            prosody_tier: intent.prosody_tier.clone(),
            intent_source: intent.source.clone(),
            feeling_source: intent.feeling_source.clone(),
            lungs,
            correction,
            passes: vec![pass1, pass2],
        });
    }
    Ok(Report {
        schema: "galion-listen-back/1".into(),
        correction_passes: 1,
        rollout_gate: crate::rollout_gate(),
        scenarios,
        stubs: stub_lines(opt, contract_ran, feeling_ran, lungs_ran),
    })
}

fn correct_once(intent: &Intent, pass: &PassReport) -> Correction {
    // Flat targets absorb analyser bias. A non-zero intended offset is
    // always corrected once. The baseline F0 on the intent is not written.
    let delta = match pass.pitch_error_st {
        Some(err) if intent.pitch_st.abs() < 0.25 && err.abs() <= 0.6 => 0.0,
        Some(err) => -err,
        None => 0.0,
    };
    let next = (pass.applied_pitch_st + delta).clamp(-6.0, 6.0);
    let missing = !intent.breaths.is_empty() && pass.scores.breath_plan_match < 0.999;
    Correction {
        from_pitch_st: pass.applied_pitch_st,
        to_pitch_st: round4(next),
        pitch_st_delta: round4(next - pass.applied_pitch_st),
        place_breaths: !intent.breaths.is_empty(),
        breaths_inserted: if missing { intent.breaths.len() } else { 0 },
        speaker_f0_steered: false,
    }
}

fn one_pass(
    opt: &Options,
    intent: &Intent,
    control: &Control,
    reference: &ReadBack,
    lungs: &LungsNote,
    index: u32,
) -> Result<PassReport, String> {
    let (samples, sr) = match &opt.render_cmd {
        Some(cmd) => {
            let dir =
                std::env::temp_dir().join(format!("galion-listen-back-{}-{}", intent.id, index));
            render::render_external(cmd, intent, control, &dir)?
        }
        None => (render::synthesize(intent, control), render::SR),
    };
    let mut read = decode::analyse(&samples, sr)?;
    if let Some(bin) = &opt.articulation_bin {
        match external_profile(bin, &samples, sr, intent, index) {
            Ok(value) => {
                if let Err(err) = decode::merge_profile(&mut read, &value) {
                    read.decoder = format!("{}; profile skipped: {err}", read.decoder);
                }
            }
            Err(err) => {
                read.decoder = format!(
                    "{}; GALION_ARTICULATION decode skipped: {err}",
                    read.decoder
                );
            }
        }
    }
    let heard = score::breath_events(&read.voicing_ticks, &read.rms_db_ticks);
    let (feeling_score, feeling) = score::feeling(intent, &read);
    let err = score::pitch_error_st(&read, intent).map(round4);
    let scores = LayerScores {
        feeling_match: feeling_score,
        breath_plan_match: score::breath_plan_match(&intent.breaths, &heard),
        pitch_voice_quality: score::pitch_voice_quality(err, read.hnr_db),
        articulation: score::articulation(&reference.art, &read.art),
        speaker_kept: score::speaker_kept(&reference.art, &read.art),
    };
    Ok(PassReport {
        index,
        renderer: control.renderer.clone(),
        decoder: read.decoder.clone(),
        applied_pitch_st: round4(control.pitch_st),
        place_breaths: control.place_breaths,
        pitch_error_st: err,
        scores,
        feeling,
        body: score::body_view(intent, &read, &lungs.status, &heard),
        baseline: score::baseline_view(intent, &read),
    })
}

fn stub_lines(
    opt: &Options,
    contract_ran: bool,
    feeling_ran: bool,
    lungs_ran: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if !contract_ran {
        lines.push(contract::CONTRACT_TODO.into());
    }
    if !feeling_ran {
        lines.push(
            "TODO(S2 feeling): the write-side trajectory is the fixture's flat pitch offset. Set GALION_FEELING to that agent's binary. Arousal, valence and dominance stay 0 until it answers. This crate does not invent Cowen coordinates.".into(),
        );
    }
    if !lungs_ran {
        lines.push(
            "TODO(lungs, PR #195): set GALION_LUNGS to galion-lungs. This crate does not replan inhalations. breath_plan_match is the decoded audio against the fixture.".into(),
        );
    }
    lines.push(
        "TODO(S3 throat): no throat planner is linked. Throat F0 and HNR in the report are galion-articulation's praatfan tracks.".into(),
    );
    lines.push(
        "TODO(S4 mouth): no mouth planner is linked. F1-F3 are the decoder's formant tracks. TractPrint v1 compare is galion-articulation's score::compare. TractPrint v2 stays on PR #191.".into(),
    );
    lines.push("TODO(S5 nose): phon_nasal and A1-P0 are not measured. nose status is stub.".into());
    if opt.render_cmd.is_none() {
        lines.push(
            "TODO(S6 sound-out): CI renders a synthetic vowel. The first pass keeps 0.55 of the asked pitch offset and drops inhalations. That undershoot is not Chatterbox. Set GALION_RENDER_CMD, or pass --render-cmd, to a local cloner. Nothing is downloaded.".into(),
        );
    }
    if opt.articulation_bin.is_none() {
        lines.push(
            "TODO(decoder PR #191): GALION_ARTICULATION is unset, so the feeling category is not read from galion-voice-decode/2. Acoustics still come from galion-articulation analyse, the praatfan path that profile is built on.".into(),
        );
    }
    lines
}

fn load_contract(bin: &str, fixture: &Fixture) -> Result<Intent, String> {
    let dir = temp_dir(&format!("contract-{}", fixture.id));
    let fixture_path = dir.join("fixture.json");
    let out_path = dir.join("intent.json");
    let body = serde_json::json!({
        "id": fixture.id,
        "text": fixture.text,
        "name": fixture.name,
        "baseline_f0_hz": fixture.baseline_f0_hz,
        "f1_hz": fixture.f1_hz,
        "f2_hz": fixture.f2_hz,
        "f3_hz": fixture.f3_hz,
        "pitch_st": fixture.pitch_st,
        "speech_s": fixture.speech_s,
        "breaths": fixture.breaths,
    });
    write_json(&fixture_path, &body)?;
    run_bin(
        bin,
        &[
            "--fixture",
            &fixture_path.display().to_string(),
            "--out",
            &out_path.display().to_string(),
        ],
    )?;
    let intent: Intent = read_json(&out_path)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(intent)
}

fn load_feeling(bin: &str, intent: &mut Intent) -> Result<(), String> {
    let dir = temp_dir(&format!("feeling-{}", intent.id));
    let in_path = dir.join("intent.json");
    let out_path = dir.join("feeling.json");
    write_json(&in_path, intent)?;
    run_bin(
        bin,
        &[
            "--intent",
            &in_path.display().to_string(),
            "--out",
            &out_path.display().to_string(),
        ],
    )?;
    let value: serde_json::Value = read_json(&out_path)?;
    contract::apply_feeling(intent, &value)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn load_lungs(bin: &str, intent: &Intent) -> Result<LungsNote, String> {
    let dir = temp_dir(&format!("lungs-{}", intent.id));
    let out_path = dir.join("plan.json");
    let category = if intent.category == "neutral" {
        "neutral"
    } else {
        intent.category.as_str()
    };
    run_bin(
        bin,
        &[
            "plan",
            "--text",
            &intent.text,
            "--category",
            category,
            "--arousal",
            "0",
            "--energy",
            "0.5",
            "--fatigue",
            "0",
            "--out",
            &out_path.display().to_string(),
        ],
    )?;
    let plan: serde_json::Value = read_json(&out_path)?;
    let inhales = score::lungs_inhales(&plan)?;
    let agreement = Some(score::breath_plan_match(&intent.breaths, &inhales));
    let _ = std::fs::remove_dir_all(&dir);
    Ok(LungsNote {
        status: "galion-lungs".into(),
        inhales,
        agreement,
        note: "Plan from GALION_LUNGS (PR #195). agreement is that plan against the fixture. The audio score stays on the read-back.".into(),
    })
}

fn external_profile(
    bin: &str,
    samples: &[f64],
    sr: f64,
    intent: &Intent,
    index: u32,
) -> Result<serde_json::Value, String> {
    let dir = temp_dir(&format!("decode-{}-{index}", intent.id));
    let wav_path = dir.join("in.wav");
    let out_path = dir.join("profile.json");
    write_wav(&wav_path, samples, sr)?;
    let emotion = if intent.track == "burst" && intent.category == "amusement" {
        "laughter"
    } else if intent.track == "baseline" {
        "neutral"
    } else {
        intent.category.as_str()
    };
    run_bin(
        bin,
        &[
            "decode",
            "--wav",
            &wav_path.display().to_string(),
            "--emotion",
            emotion,
            "--out",
            &out_path.display().to_string(),
        ],
    )?;
    let value = read_json(&out_path)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(value)
}

fn run_bin(bin: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(bin)
        .args(args)
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .output()
        .map_err(|e| format!("{bin}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        Err(format!("{bin} failed: {err}{stdout}"))
    }
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("galion-listen-back-{name}"))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_wav(path: &Path, samples: &[f64], sr: f64) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr.round() as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w =
        hound::WavWriter::create(path, spec).map_err(|e| format!("{}: {e}", path.display()))?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}
