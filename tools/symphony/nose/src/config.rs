// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Magnitudes from `configs/symphony/nose.toml`.
//! Cited directions are checked here. The numbers themselves are not.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub tick_s: f64,
    pub speech: SpeechCfg,
    pub speaker: SpeakerCfg,
    pub velum: VelumCfg,
    pub feeling: FeelingCfg,
    pub murmur: MurmurCfg,
    pub a1_p0: A1P0Cfg,
    pub place: PlaceCfg,
    pub laugh: LaughCfg,
    pub sigh: DurCfg,
    pub moan: MoanCfg,
    pub gasp: GaspCfg,
    pub hum: DurCfg,
    pub mm: DurCfg,
    pub level: LevelCfg,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechCfg {
    pub chars_per_s: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeakerCfg {
    pub f0_band_hz: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VelumCfg {
    pub span_open: f64,
    pub hum_open: f64,
    pub mm_open: f64,
    pub laugh_add: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeelingCfg {
    pub velum_per_intensity: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MurmurCfg {
    pub hz: f64,
    pub rms_at_open: f64,
    pub literature_hz: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct A1P0Cfg {
    pub db_per_velum: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceCfg {
    pub amusement_min: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaughCfg {
    pub call_s: f64,
    pub calls_low: u32,
    pub calls_high: u32,
    pub calls_unset: u32,
    pub gap_s: f64,
    pub published_bout_low_s: f64,
    pub published_bout_high_s: f64,
    pub f0_low_open_hz: f64,
    pub f0_low_closed_hz: f64,
    pub f0_high_open_hz: f64,
    pub f0_high_closed_hz: f64,
    pub f0_when_baseline_missing_hz: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurCfg {
    pub duration_s: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoanCfg {
    pub duration_s: f64,
    pub f0_st: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GaspCfg {
    pub duration_s: f64,
    pub attack_fraction: f64,
    pub decay: f64,
    pub literature: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelCfg {
    pub base_db: f64,
    pub db_per_intensity: f64,
    pub db_per_arousal: f64,
    pub literature: String,
    pub seed: u64,
}

impl Config {
    pub fn bundled() -> Result<Self, String> {
        Self::load(&bundled_path())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let cfg: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        cfg.check()?;
        Ok(cfg)
    }

    pub fn check(&self) -> Result<(), String> {
        if self.schema != "galion-nose-config/1" {
            return Err(format!(
                "nose config schema {} (want galion-nose-config/1)",
                self.schema
            ));
        }
        if (self.tick_s - crate::TICK_S).abs() > 1e-12 {
            return Err("tick_s must stay 0.08, the Symphony clock".into());
        }
        pos("speech.chars_per_s", self.speech.chars_per_s)?;
        if (self.speaker.f0_band_hz - 165.0).abs() > 1e-9 {
            return Err(
                "speaker.f0_band_hz must stay 165, the decoder's label-column cut (PR 191)".into(),
            );
        }
        unit_open("velum.span_open", self.velum.span_open)?;
        unit_open("velum.hum_open", self.velum.hum_open)?;
        unit_open("velum.mm_open", self.velum.mm_open)?;
        if !(0.0..=1.0).contains(&self.velum.laugh_add) {
            return Err("velum.laugh_add must be in [0, 1]".into());
        }
        if !self.feeling.velum_per_intensity.is_finite() {
            return Err("feeling.velum_per_intensity must be finite".into());
        }
        named("feeling.literature", &self.feeling.literature)?;
        pos("murmur.hz", self.murmur.hz)?;
        pos("murmur.rms_at_open", self.murmur.rms_at_open)?;
        named("murmur.literature_hz", &self.murmur.literature_hz)?;
        if !self.a1_p0.db_per_velum.is_finite() {
            return Err("a1_p0.db_per_velum must be finite".into());
        }
        named("a1_p0.literature", &self.a1_p0.literature)?;
        if !self.place.amusement_min.is_finite() || self.place.amusement_min < 0.0 {
            return Err("place.amusement_min must be finite and >= 0".into());
        }
        self.check_laugh()?;
        self.check_dur("sigh", &self.sigh)?;
        self.check_dur("hum", &self.hum)?;
        self.check_dur("mm", &self.mm)?;
        pos("moan.duration_s", self.moan.duration_s)?;
        if !self.moan.f0_st.is_finite() {
            return Err("moan.f0_st must be finite".into());
        }
        named("moan.literature", &self.moan.literature)?;
        pos("gasp.duration_s", self.gasp.duration_s)?;
        if !(self.gasp.attack_fraction > 0.0 && self.gasp.attack_fraction < 1.0) {
            return Err("gasp.attack_fraction must lie in (0, 1)".into());
        }
        pos("gasp.decay", self.gasp.decay)?;
        named("gasp.literature", &self.gasp.literature)?;
        if !self.level.base_db.is_finite()
            || !self.level.db_per_intensity.is_finite()
            || !self.level.db_per_arousal.is_finite()
        {
            return Err("level coefficients must be finite".into());
        }
        named("level.literature", &self.level.literature)?;
        Ok(())
    }

    fn check_dur(&self, name: &str, d: &DurCfg) -> Result<(), String> {
        pos(&format!("{name}.duration_s"), d.duration_s)?;
        named(&format!("{name}.literature"), &d.literature)
    }

    fn check_laugh(&self) -> Result<(), String> {
        let l = &self.laugh;
        pos("laugh.call_s", l.call_s)?;
        if l.gap_s < 0.0 || !l.gap_s.is_finite() {
            return Err("laugh.gap_s must be finite and >= 0".into());
        }
        for (name, n) in [
            ("calls_low", l.calls_low),
            ("calls_high", l.calls_high),
            ("calls_unset", l.calls_unset),
        ] {
            if n < 1 {
                return Err(format!("laugh.{name} must be >= 1"));
            }
        }
        // Table II: open-mouth F0 is higher than closed-mouth, and the women's
        // open-mouth mean is higher than the men's. The hertz may be edited.
        // The inequality is the finding.
        higher(
            "laugh.f0_low_open_hz",
            l.f0_low_open_hz,
            "laugh.f0_low_closed_hz",
            l.f0_low_closed_hz,
        )?;
        higher(
            "laugh.f0_high_open_hz",
            l.f0_high_open_hz,
            "laugh.f0_high_closed_hz",
            l.f0_high_closed_hz,
        )?;
        higher(
            "laugh.f0_high_open_hz",
            l.f0_high_open_hz,
            "laugh.f0_low_open_hz",
            l.f0_low_open_hz,
        )?;
        if l.f0_when_baseline_missing_hz != 0.0 {
            return Err(
                "laugh.f0_when_baseline_missing_hz must stay 0 so a missing baseline is not a sex mean"
                    .into(),
            );
        }
        pos("laugh.published_bout_low_s", l.published_bout_low_s)?;
        pos("laugh.published_bout_high_s", l.published_bout_high_s)?;
        Ok(())
    }
}

fn pos(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be a positive finite magnitude"))
    }
}

fn unit_open(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 && v <= 1.0 {
        Ok(())
    } else {
        Err(format!("{name} must lie in (0, 1]"))
    }
}

fn named(name: &str, v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        Err(format!("{name} must name a source, or \"none\""))
    } else {
        Ok(())
    }
}

fn higher(hi_name: &str, hi: f64, lo_name: &str, lo: f64) -> Result<(), String> {
    pos(hi_name, hi)?;
    pos(lo_name, lo)?;
    if hi > lo {
        Ok(())
    } else {
        Err(format!(
            "{hi_name} ({hi}) must stay above {lo_name} ({lo}); that inequality is the cited direction"
        ))
    }
}

pub fn bundled_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../configs/symphony/nose.toml")
}

/// Snap a positive duration onto the 80 ms grid by rounding. At least one tick.
pub fn snap_round(tick_s: f64, seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    let n = ((seconds / tick_s) + 1e-9).round().max(1.0);
    n * tick_s
}

/// Snap a time onto the grid. Zero stays zero. Negative is rejected by callers.
pub fn snap_time(tick_s: f64, seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    let n = ((seconds / tick_s) + 1e-9).round().max(0.0);
    n * tick_s
}

pub fn ticks_covering(tick_s: f64, seconds: f64) -> usize {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    ((seconds / tick_s) + 1e-9).ceil().max(1.0) as usize
}
