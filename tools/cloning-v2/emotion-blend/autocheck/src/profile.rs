//! Load base-recording profiles. Accepts voicecheck schema or worker-A blend schema.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::gate::GateThresholds;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerProbs {
    pub neu: f64,
    pub hap: f64,
    pub ang: f64,
    pub sad: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Avd {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseProsody {
    pub f0_mean_hz: f64,
    pub f0_sd_st: f64,
    pub f0_range_st: f64,
    pub energy_sd_db: f64,
    pub rms_dbfs: f64,
    pub pace_syl_s: f64,
    pub ser: SerProbs,
    pub avd: Avd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    pub schema: String,
    pub persona: String,
    pub base_recording: String,
    pub reference_wav: String,
    pub baselines: GateThresholds,
    pub base: BaseProsody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineDelta {
    pub line_id: String,
    pub wav: String,
    pub f0_mean_hz: f64,
    pub f0_sd_st: f64,
    pub f0_range_st: f64,
    pub energy_sd_db: f64,
    pub rms_dbfs: f64,
    pub pace_syl_s: f64,
    pub ser: SerProbs,
    pub avd: Avd,
    pub delta_f0_mean_hz: f64,
    pub delta_f0_sd_st: f64,
    pub delta_energy_sd_db: f64,
    pub delta_pace_syl_s: f64,
    pub delta_ser: SerProbs,
    pub delta_avd: Avd,
}

pub fn default_baselines_for(persona: &str) -> GateThresholds {
    match persona {
        "samantha" | "own-samantha" => GateThresholds {
            wer_max: 0.018,
            ecapa_min: 0.7057,
        },
        "jarvis" | "own-jarvis" => GateThresholds {
            wer_max: 0.019,
            ecapa_min: 0.7331,
        },
        _ => GateThresholds::default(),
    }
}

pub fn persona_from_voice(voice: &str) -> String {
    voice
        .trim()
        .trim_start_matches("own-")
        .split('-')
        .next()
        .unwrap_or(voice)
        .to_string()
}

fn f64_field(v: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(x) = v.get(*k) {
            if let Some(n) = x.as_f64() {
                return Some(n);
            }
            if let Some(n) = x.as_i64() {
                return Some(n as f64);
            }
        }
    }
    None
}

fn ser_from(v: &Value) -> Option<SerProbs> {
    let probs = v.get("probs").unwrap_or(v);
    Some(SerProbs {
        neu: f64_field(probs, &["neu"])?,
        hap: f64_field(probs, &["hap"])?,
        ang: f64_field(probs, &["ang"])?,
        sad: f64_field(probs, &["sad"])?,
    })
}

fn avd_from(v: &Value) -> Option<Avd> {
    Some(Avd {
        arousal: f64_field(v, &["arousal"])?,
        valence: f64_field(v, &["valence"])?,
        dominance: f64_field(v, &["dominance"])?,
    })
}

fn base_from_emotion_file(file: &Value) -> Result<BaseProsody> {
    let proxies = file.get("proxies").context("missing proxies")?;
    let ser = file.get("ser").and_then(ser_from).context("missing ser")?;
    let avd = file.get("avd").and_then(avd_from).context("missing avd")?;
    Ok(BaseProsody {
        f0_mean_hz: f64_field(proxies, &["f0_mean_hz"]).unwrap_or(0.0),
        f0_sd_st: f64_field(proxies, &["f0_sd_st"]).unwrap_or(0.0),
        f0_range_st: f64_field(proxies, &["f0_range_st_p5_p95", "f0_range_st"]).unwrap_or(0.0),
        energy_sd_db: f64_field(proxies, &["energy_sd_db"]).unwrap_or(0.0),
        rms_dbfs: f64_field(proxies, &["rms_dbfs"]).unwrap_or(0.0),
        pace_syl_s: f64_field(proxies, &["speech_rate_syl_s", "pace_syl_s"]).unwrap_or(0.0),
        ser,
        avd,
    })
}

/// Build a LineDelta comparing one galion-emotion file entry to the base profile.
pub fn line_delta(line_id: &str, file: &Value, base: &BaseProsody) -> Result<LineDelta> {
    let proxies = file.get("proxies").context("line missing proxies")?;
    let ser = file.get("ser").and_then(ser_from).context("line missing ser")?;
    let avd = file.get("avd").and_then(avd_from).context("line missing avd")?;
    let f0 = f64_field(proxies, &["f0_mean_hz"]).unwrap_or(0.0);
    let f0_sd = f64_field(proxies, &["f0_sd_st"]).unwrap_or(0.0);
    let f0_r = f64_field(proxies, &["f0_range_st_p5_p95", "f0_range_st"]).unwrap_or(0.0);
    let energy = f64_field(proxies, &["energy_sd_db"]).unwrap_or(0.0);
    let rms = f64_field(proxies, &["rms_dbfs"]).unwrap_or(0.0);
    let pace = f64_field(proxies, &["speech_rate_syl_s", "pace_syl_s"]).unwrap_or(0.0);
    Ok(LineDelta {
        line_id: line_id.into(),
        wav: file
            .get("file")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        f0_mean_hz: f0,
        f0_sd_st: f0_sd,
        f0_range_st: f0_r,
        energy_sd_db: energy,
        rms_dbfs: rms,
        pace_syl_s: pace,
        delta_f0_mean_hz: f0 - base.f0_mean_hz,
        delta_f0_sd_st: f0_sd - base.f0_sd_st,
        delta_energy_sd_db: energy - base.energy_sd_db,
        delta_pace_syl_s: pace - base.pace_syl_s,
        delta_ser: SerProbs {
            neu: ser.neu - base.ser.neu,
            hap: ser.hap - base.ser.hap,
            ang: ser.ang - base.ser.ang,
            sad: ser.sad - base.ser.sad,
        },
        delta_avd: Avd {
            arousal: avd.arousal - base.avd.arousal,
            valence: avd.valence - base.avd.valence,
            dominance: avd.dominance - base.avd.dominance,
        },
        ser,
        avd,
    })
}

fn load_voicecheck_schema(v: &Value) -> Result<VoiceProfile> {
    let persona = v
        .get("persona")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let baselines = if let Some(b) = v.get("baselines") {
        GateThresholds {
            wer_max: f64_field(b, &["wer_max"]).unwrap_or(default_baselines_for(&persona).wer_max),
            ecapa_min: f64_field(b, &["ecapa_min"])
                .unwrap_or(default_baselines_for(&persona).ecapa_min),
        }
    } else {
        default_baselines_for(&persona)
    };
    let base_val = v.get("base").context("missing base")?;
    // Prefer nested ser/avd; else treat base as emotion file.
    let base = if base_val.get("ser").is_some() && base_val.get("f0_mean_hz").is_some() {
        BaseProsody {
            f0_mean_hz: f64_field(base_val, &["f0_mean_hz"]).unwrap_or(0.0),
            f0_sd_st: f64_field(base_val, &["f0_sd_st"]).unwrap_or(0.0),
            f0_range_st: f64_field(base_val, &["f0_range_st"]).unwrap_or(0.0),
            energy_sd_db: f64_field(base_val, &["energy_sd_db"]).unwrap_or(0.0),
            rms_dbfs: f64_field(base_val, &["rms_dbfs"]).unwrap_or(0.0),
            pace_syl_s: f64_field(base_val, &["pace_syl_s"]).unwrap_or(0.0),
            ser: ser_from(base_val.get("ser").unwrap_or(base_val)).context("base.ser")?,
            avd: avd_from(base_val.get("avd").unwrap_or(base_val)).context("base.avd")?,
        }
    } else {
        base_from_emotion_file(base_val)?
    };
    Ok(VoiceProfile {
        schema: "galion-voicecheck-profile/1".into(),
        persona,
        base_recording: v
            .get("base_recording")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        reference_wav: v
            .get("reference_wav")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        baselines,
        base,
        source: Some("galion-voicecheck-profile/1".into()),
    })
}

fn load_blend_schema(v: &Value, persona: &str) -> Result<VoiceProfile> {
    let voices = v.get("voices").context("blend profile missing voices")?;
    let voice = voices
        .get(persona)
        .with_context(|| format!("blend profile missing voices.{persona}"))?;
    let original = voice
        .get("original")
        .context("blend voice missing original")?;
    let whole = original
        .get("whole")
        .context("blend original missing whole")?;
    let base = base_from_emotion_file(whole)?;
    let base_recording = whole
        .get("file")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Ok(VoiceProfile {
        schema: "galion-voicecheck-profile/1".into(),
        persona: persona.into(),
        base_recording,
        reference_wav: String::new(),
        baselines: default_baselines_for(persona),
        base,
        source: Some("galion-emotion-blend/1".into()),
    })
}

pub fn load_profile(path: &Path, persona: &str) -> Result<VoiceProfile> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let v: Value = serde_json::from_str(&raw).context("parse profile json")?;
    let schema = v
        .get("schema")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if schema.starts_with("galion-voicecheck-profile") || v.get("base").is_some() {
        let mut p = load_voicecheck_schema(&v)?;
        if p.persona == "unknown" {
            p.persona = persona.into();
        }
        return Ok(p);
    }
    if schema.starts_with("galion-emotion-blend") || v.get("voices").is_some() {
        return load_blend_schema(&v, persona);
    }
    // Raw galion-emotion score of a single file / files[0]
    if let Some(files) = v.get("files").and_then(|x| x.as_array()) {
        if let Some(f0) = files.first() {
            let base = base_from_emotion_file(f0)?;
            return Ok(VoiceProfile {
                schema: "galion-voicecheck-profile/1".into(),
                persona: persona.into(),
                base_recording: f0
                    .get("file")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .into(),
                reference_wav: String::new(),
                baselines: default_baselines_for(persona),
                base,
                source: Some("galion-emotion/1".into()),
            });
        }
    }
    if v.get("proxies").is_some() {
        let base = base_from_emotion_file(&v)?;
        return Ok(VoiceProfile {
            schema: "galion-voicecheck-profile/1".into(),
            persona: persona.into(),
            base_recording: v
                .get("file")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .into(),
            reference_wav: String::new(),
            baselines: default_baselines_for(persona),
            base,
            source: Some("galion-emotion-file".into()),
        });
    }
    bail!("unrecognised profile schema at {}", path.display());
}

/// Resolve the best available profile for a persona under the emotion-blend tree.
pub fn resolve_profile(
    autocheck_root: &Path,
    persona: &str,
    explicit: Option<&Path>,
) -> Result<(PathBuf, VoiceProfile)> {
    if let Some(p) = explicit {
        return Ok((p.to_path_buf(), load_profile(p, persona)?));
    }
    let blend = autocheck_root
        .parent()
        .unwrap_or(autocheck_root);
    let candidates = [
        blend.join(format!("analysis/profile-{persona}.json")),
        blend.join("analysis/profile.json"),
        blend.join("A-global/profile.json"),
        autocheck_root.join(format!("fixtures/profile-{persona}.json")),
    ];
    for c in &candidates {
        if c.is_file() {
            return Ok((c.clone(), load_profile(c, persona)?));
        }
    }
    bail!(
        "no profile found for persona={persona}; looked in analysis/profile-{persona}.json, A-global/profile.json"
    );
}

pub fn write_profile(path: &Path, profile: &VoiceProfile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(profile)?)?;
    Ok(())
}

pub fn aggregate_ser(lines: &[LineDelta]) -> BTreeMap<String, f64> {
    let n = lines.len().max(1) as f64;
    let mut m = BTreeMap::new();
    m.insert("neu".into(), lines.iter().map(|l| l.ser.neu).sum::<f64>() / n);
    m.insert("hap".into(), lines.iter().map(|l| l.ser.hap).sum::<f64>() / n);
    m.insert("ang".into(), lines.iter().map(|l| l.ser.ang).sum::<f64>() / n);
    m.insert("sad".into(), lines.iter().map(|l| l.ser.sad).sum::<f64>() / n);
    m
}
