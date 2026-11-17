// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Decode. Throat and mouth come from `galion-articulation` (praatfan).
//! Nose, breath stage, nasality, breathiness and eGeMAPS are named and left
//! empty until the features agent lands them.
//!
//! A tick is a feeling only when the room, the noise and the distance match
//! the baseline. Otherwise the z is reported and the label is withheld.

use std::collections::BTreeMap;

use galion_articulation::Section;

use crate::audio::{median, rms_ticks};
use crate::emotion::EmotionId;
use crate::fluid::{BaselineVoice, InnerFactors, OuterFactors, TickFeeling};
use crate::profile::{
    BreathBlock, EnvironmentBlock, SectionBlock, TaxonomyBlock, VoiceFile, VoiceProfile,
};
use crate::TICK_MS;

/// |z| below this stays the speaker's own neutral. Pipeline default.
const Z_NEUTRAL: f64 = 0.75;

pub fn label_from_z(z_f0: Option<f64>, confounded: bool) -> Option<EmotionId> {
    if confounded {
        return None;
    }
    let z = z_f0?;
    if !z.is_finite() || z.abs() < Z_NEUTRAL {
        return None;
    }
    Some(nearest_f0_z(z))
}

fn expected_z(id: EmotionId) -> f64 {
    (id.prototype().arousal - crate::fluid::NEUTRAL_AROUSAL) * 4.0
}

fn nearest_f0_z(z: f64) -> EmotionId {
    let mut best = EmotionId::Interest;
    let mut best_d = f64::MAX;
    for id in EmotionId::ALL {
        let d = (z - expected_z(id)).abs();
        if d < best_d {
            best = id;
            best_d = d;
        }
    }
    best
}

pub struct DecodeIn<'a> {
    pub id: &'a str,
    pub role: &'a str,
    pub samples: &'a [f64],
    pub sample_rate_hz: f64,
    pub voice: &'a VoiceFile,
    /// Conditions of this take. Compared with the baseline, not copied over it.
    pub outer: OuterFactors,
    pub inner: InnerFactors,
}

pub fn decode_samples(input: DecodeIn<'_>) -> Result<VoiceProfile, String> {
    if input.samples.is_empty() || input.sample_rate_hz < 1000.0 {
        return Err("decode: need samples and a sample rate".into());
    }
    let (art, _settings) =
        galion_articulation::acoustic::analyse(input.samples, input.sample_rate_hz);
    let mut art = art;
    art.finish();
    let n = art.n_ticks;
    let confounded = !input.outer.matches(input.voice.baseline.conditions);
    let f0 = art
        .tracks
        .get("f0_hz")
        .map(|t| t.ticks.clone())
        .unwrap_or_default();
    let energy = rms_ticks(input.samples, input.sample_rate_hz, n);
    let voicing = art
        .tracks
        .get("voicing")
        .map(|t| t.ticks.clone())
        .unwrap_or_default();

    let mut ticks = Vec::with_capacity(n);
    let mut unvoiced = 0usize;
    let mut seen = 0usize;
    for i in 0..n {
        let z_f0 = f0
            .get(i)
            .copied()
            .flatten()
            .and_then(|hz| z_f0(&input.voice.baseline, hz));
        let z_energy = energy
            .get(i)
            .copied()
            .flatten()
            .and_then(|e| z_energy(&input.voice.baseline, e));
        if let Some(v) = voicing.get(i).copied().flatten() {
            seen += 1;
            if v < 0.5 {
                unvoiced += 1;
            }
        }
        let prosody = label_from_z(z_f0, confounded);
        ticks.push(TickFeeling {
            t_ms: (i as u32).saturating_mul(TICK_MS),
            prosody,
            prosody_tier: prosody.map(EmotionId::tier),
            burst: None,
            avd: crate::fluid::NEUTRAL_AVD,
            breath: None,
            z_f0,
            z_energy,
            confounded,
            source: "read".into(),
        });
    }

    let mut throat = SectionBlock {
        status: "measured".into(),
        source: "galion-articulation / praatfan_rust".into(),
        features: BTreeMap::new(),
        todo: None,
    };
    let mut mouth = SectionBlock {
        status: "measured".into(),
        source: "galion-articulation / praatfan_rust".into(),
        features: BTreeMap::new(),
        todo: None,
    };
    let mut nose = SectionBlock {
        status: "stub".into(),
        source: "none".into(),
        features: BTreeMap::new(),
        todo: Some(
            "TODO(features): nasality. V1's nasal output is a phonological posterior, not nasalance. Not called from this glue."
                .into(),
        ),
    };
    for (name, summary) in &art.summary {
        let block = match summary.section {
            Section::Throat => &mut throat,
            Section::Mouth => &mut mouth,
            Section::Nose => &mut nose,
        };
        block.features.insert(name.clone(), summary.mean);
        if summary.section == Section::Nose {
            block.status = "measured".into();
            block.todo = None;
        }
    }
    if throat.features.is_empty() {
        throat.status = "empty".into();
    }
    if mouth.features.is_empty() {
        mouth.status = "empty".into();
    }

    let unvoiced_fraction = if seen == 0 {
        None
    } else {
        Some(unvoiced as f64 / seen as f64)
    };
    let mut stubs = vec![
        "eGeMAPS names are not computed here. Throat and mouth are praatfan via galion-articulation. TODO(features).".into(),
        "Breath stage, breathiness and the four non-eGeMAPS breath quantities are not measured. TODO(features).".into(),
        "TractPrint v2 speaker/emotion split is not called. V1 compare stays on galion-articulation and is not a gate. TODO(features).".into(),
    ];
    if nose.status == "stub" {
        stubs.push(nose.todo.clone().unwrap_or_default());
    }

    Ok(VoiceProfile {
        schema: 1,
        id: input.id.into(),
        role: input.role.into(),
        sample_rate_hz: input.sample_rate_hz.round() as u32,
        duration_s: input.samples.len() as f64 / input.sample_rate_hz,
        tick_ms: TICK_MS,
        n_ticks: n,
        taxonomy: TaxonomyBlock::cowen24(),
        baseline: input.voice.baseline.clone(),
        inner: input.inner,
        inner_source: "not measured from the audio; the caller passes it".into(),
        throat,
        mouth,
        nose,
        breath: BreathBlock {
            status: "stub".into(),
            unvoiced_fraction,
            todo: "TODO(features): breath stage. unvoiced_fraction is the praatfan voicing track, not breaths per minute.".into(),
        },
        environment: EnvironmentBlock {
            outer: input.outer,
            mix: "gain for distance and room; noise is not injected".into(),
            reverb: "stub".into(),
            todo: "TODO(features): room and noise. A mismatch sets confounded and withholds the label.".into(),
        },
        ticks,
        stubs,
    })
}

fn z_f0(baseline: &BaselineVoice, hz: f64) -> Option<f64> {
    baseline.z(hz, baseline.f0_hz_mean, baseline.f0_hz_sd)
}

fn z_energy(baseline: &BaselineVoice, energy: f64) -> Option<f64> {
    baseline.z(energy, baseline.energy_mean, baseline.energy_sd)
}

pub fn median_f0(profile: &VoiceProfile) -> Option<f64> {
    let values: Vec<f64> = profile
        .ticks
        .iter()
        .filter_map(|t| t.z_f0.map(|z| baseline_hz(&profile.baseline, z)))
        .collect();
    median(&values)
}

fn baseline_hz(baseline: &BaselineVoice, z: f64) -> f64 {
    baseline.f0_hz_mean + z * baseline.f0_hz_sd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_cover_high_and_low_and_a_confound_withholds_them() {
        let high = label_from_z(Some(3.0), false).unwrap();
        assert!(high.prototype().arousal > 0.7, "{}", high.as_str());
        let low = label_from_z(Some(-3.0), false).unwrap();
        assert!(low.prototype().arousal < 0.3, "{}", low.as_str());
        assert!(label_from_z(Some(0.1), false).is_none());
        assert!(label_from_z(Some(3.0), true).is_none());
        assert!(label_from_z(None, false).is_none());
    }
}
