// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The feeling is a point. Every 80 ms it steps toward a target.
//! Inner factors (breath, energy, fatigue, health) and outer factors
//! (room, noise, listener distance) move that point. The baseline voice
//! is not a field of the point and this module never writes it.

use serde::{Deserialize, Serialize};

use crate::emotion::{EmotionId, Feeling, ProsodyTier, Track};
use crate::TICK_S;

/// Arousal 0..1, valence −1..1, dominance 0..1. Same ranges as `galion-emotion`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Avd {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
}

impl Avd {
    pub fn lerp(self, toward: Avd, t: f64) -> Avd {
        let t = t.clamp(0.0, 1.0);
        Avd {
            arousal: self.arousal + (toward.arousal - self.arousal) * t,
            valence: self.valence + (toward.valence - self.valence) * t,
            dominance: self.dominance + (toward.dominance - self.dominance) * t,
        }
        .clamped()
    }

    pub fn clamped(self) -> Avd {
        fn c(x: f64, lo: f64, hi: f64, d: f64) -> f64 {
            if x.is_finite() {
                x.clamp(lo, hi)
            } else {
                d
            }
        }
        Avd {
            arousal: c(self.arousal, 0.0, 1.0, NEUTRAL_AROUSAL),
            valence: c(self.valence, -1.0, 1.0, 0.0),
            dominance: c(self.dominance, 0.0, 1.0, 0.5),
        }
    }
}

pub const NEUTRAL_AROUSAL: f64 = 0.35;

pub const NEUTRAL_AVD: Avd = Avd {
    arousal: NEUTRAL_AROUSAL,
    valence: 0.0,
    dominance: 0.5,
};

/// Approach time. Pipeline default, not a cited feeling time constant.
const TAU_S: f64 = 0.240;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct InnerFactors {
    /// 0..1. Physiology. Not one of the 24 categories.
    pub breath: f64,
    pub energy: f64,
    /// 0 rested, 1 spent. Pulls arousal down.
    pub fatigue: f64,
    /// 1 well, 0 unwell. Pulls arousal down. Not a diagnosis.
    pub health: f64,
}

impl Default for InnerFactors {
    fn default() -> Self {
        Self {
            breath: 0.35,
            energy: 0.70,
            fatigue: 0.0,
            health: 1.0,
        }
    }
}

impl InnerFactors {
    fn clamped(self) -> Self {
        fn c(x: f64) -> f64 {
            if x.is_finite() {
                x.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        Self {
            breath: c(self.breath),
            energy: c(self.energy),
            fatigue: c(self.fatigue),
            health: c(self.health),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Room {
    Quiet,
    Small,
    Large,
    Outdoor,
}

impl Default for Room {
    fn default() -> Self {
        Self::Quiet
    }
}

impl Room {
    /// Gain applied in the environment mix. Pipeline default, not a cited room curve.
    pub fn gain(self) -> f64 {
        match self {
            Self::Quiet | Self::Small => 1.0,
            Self::Large => 0.85,
            Self::Outdoor => 0.70,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OuterFactors {
    pub room: Room,
    pub noise_db: f64,
    pub listener_distance_m: f64,
}

impl Default for OuterFactors {
    fn default() -> Self {
        Self {
            room: Room::Quiet,
            noise_db: 0.0,
            listener_distance_m: 1.0,
        }
    }
}

/// Tolerances so a float does not mark a matched take as confounded.
/// Not a cited just-noticeable difference.
const NOISE_MATCH_DB: f64 = 6.0;
const DISTANCE_MATCH_M: f64 = 0.5;

impl OuterFactors {
    pub fn matches(self, baseline: OuterFactors) -> bool {
        self.room == baseline.room
            && (self.noise_db - baseline.noise_db).abs() <= NOISE_MATCH_DB
            && (self.listener_distance_m - baseline.listener_distance_m).abs() <= DISTANCE_MATCH_M
    }

    /// Level relative to the baseline distance. 1 m against 1 m is unity.
    pub fn distance_gain(self, baseline: OuterFactors) -> f64 {
        let ref_d = baseline.listener_distance_m.max(0.3);
        let d = self.listener_distance_m.max(0.3);
        (ref_d / d).clamp(0.4, 1.8)
    }
}

/// Stable voice. Sex, age, tract length and personality live here, not in the feeling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineVoice {
    pub f0_hz_mean: f64,
    pub f0_hz_sd: f64,
    pub energy_mean: f64,
    pub energy_sd: f64,
    pub hnr_db_mean: f64,
    pub hnr_db_sd: f64,
    pub f1_hz_mean: f64,
    pub f1_hz_sd: f64,
    pub f2_hz_mean: f64,
    pub conditions: OuterFactors,
}

impl Default for BaselineVoice {
    fn default() -> Self {
        Self {
            f0_hz_mean: 180.0,
            f0_hz_sd: 8.0,
            energy_mean: 0.15,
            energy_sd: 0.05,
            hnr_db_mean: 12.0,
            hnr_db_sd: 3.0,
            f1_hz_mean: 700.0,
            f1_hz_sd: 40.0,
            f2_hz_mean: 1200.0,
            conditions: OuterFactors::default(),
        }
    }
}

impl BaselineVoice {
    pub fn z(&self, value: f64, mean: f64, sd: f64) -> Option<f64> {
        if sd > 1e-6 && value.is_finite() {
            Some((value - mean) / sd)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetState {
    pub feeling: Feeling,
    pub track: Track,
    /// 0 stays at neutral, 1 sits on the category prototype.
    pub intensity: f64,
    pub inner: InnerFactors,
    pub outer: OuterFactors,
}

impl TargetState {
    /// A spoken word carrying this category. Legal for all 24.
    pub fn spoken(id: EmotionId, intensity: f64) -> Self {
        Self {
            feeling: Feeling::Emotion(id),
            track: Track::Prosody,
            intensity,
            inner: InnerFactors::default(),
            outer: OuterFactors::default(),
        }
    }

    pub fn goal_avd(&self) -> Avd {
        let proto = match self.feeling {
            Feeling::Neutral => NEUTRAL_AVD,
            Feeling::Emotion(id) => id.prototype(),
        };
        let mut goal = NEUTRAL_AVD.lerp(proto, self.intensity);
        let inner = self.inner.clamped();
        let health = 0.55 + 0.45 * inner.health;
        let fatigue = 1.0 - 0.45 * inner.fatigue;
        goal.arousal = (goal.arousal * health * fatigue).clamp(0.0, 1.0);
        goal
    }
}

/// One 80 ms sample of the feeling. Both tracks are always present in the schema.
/// `burst` is null when the word is spoken rather than burst. `prosody` is null
/// at the speaker's own neutral.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TickFeeling {
    pub t_ms: u32,
    pub prosody: Option<EmotionId>,
    pub prosody_tier: Option<ProsodyTier>,
    pub burst: Option<EmotionId>,
    pub avd: Avd,
    pub breath: Option<f64>,
    pub z_f0: Option<f64>,
    pub z_energy: Option<f64>,
    pub confounded: bool,
    /// `plan` is the write. `read` is the clone heard back.
    pub source: String,
}

pub struct FluidCore {
    pub baseline: BaselineVoice,
    pub point: Avd,
    pub inner: InnerFactors,
    pub breath: f64,
}

impl FluidCore {
    pub fn new(baseline: BaselineVoice) -> Self {
        let inner = InnerFactors::default();
        let breath = inner.breath;
        Self {
            baseline,
            point: NEUTRAL_AVD,
            inner,
            breath,
        }
    }

    /// One tick. Does not write [`FluidCore::baseline`].
    pub fn step(&mut self, t_ms: u32, target: &TargetState) -> TickFeeling {
        let goal = target.goal_avd();
        let alpha = 1.0 - (-TICK_S / TAU_S).exp();
        self.point = self.point.lerp(goal, alpha);
        self.inner = target.inner.clamped();
        self.breath =
            (goal.arousal * self.inner.energy * (1.0 - 0.35 * self.inner.fatigue)).clamp(0.0, 1.0);
        let confounded = !target.outer.matches(self.baseline.conditions);
        plan_tick(t_ms, target, self.point, self.breath, confounded)
    }
}

pub fn plan_tick(
    t_ms: u32,
    target: &TargetState,
    avd: Avd,
    breath: f64,
    confounded: bool,
) -> TickFeeling {
    let (prosody, burst) = match (target.track, target.feeling) {
        (_, Feeling::Neutral) => (None, None),
        (Track::Prosody, Feeling::Emotion(id)) => (Some(id), None),
        (Track::Burst, Feeling::Emotion(id)) => (None, Some(id)),
    };
    let prosody_tier = prosody.map(EmotionId::tier);
    TickFeeling {
        t_ms,
        prosody,
        prosody_tier,
        burst,
        avd,
        breath: Some(breath),
        z_f0: None,
        z_energy: None,
        confounded,
        source: "plan".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion::EmotionId;

    #[test]
    fn baseline_stays_put_while_the_point_moves() {
        let baseline = BaselineVoice::default();
        let snapshot = baseline.clone();
        let mut core = FluidCore::new(baseline);
        let target = TargetState::spoken(EmotionId::Embarrassment, 1.0);
        let mut last = core.step(0, &target);
        for i in 1..30 {
            last = core.step(i * 80, &target);
        }
        assert_eq!(core.baseline, snapshot);
        assert!(last.avd.arousal > 0.7, "arousal {}", last.avd.arousal);
        assert_eq!(last.prosody, Some(EmotionId::Embarrassment));
        assert_eq!(
            last.prosody_tier,
            Some(crate::emotion::ProsodyTier::Extended)
        );
        assert!(last.burst.is_none());
        assert!(last.breath.unwrap() > 0.0);
        assert!(!last.confounded);
    }

    #[test]
    fn fatigue_and_noise_do_what_they_are_for() {
        let mut rested = FluidCore::new(BaselineVoice::default());
        let mut tired = FluidCore::new(BaselineVoice::default());
        let fresh = TargetState::spoken(EmotionId::Triumph, 1.0);
        let mut worn = fresh.clone();
        worn.inner.fatigue = 1.0;
        worn.inner.health = 0.2;
        let mut noisy = fresh.clone();
        noisy.outer.noise_db = 40.0;
        for i in 0..30 {
            rested.step(i * 80, &fresh);
            tired.step(i * 80, &worn);
        }
        assert!(tired.point.arousal < rested.point.arousal);
        let tick = rested.step(2400, &noisy);
        assert!(tick.confounded);
    }

    #[test]
    fn every_category_roundtrips_as_a_spoken_word() {
        for id in EmotionId::ALL {
            let target = TargetState::spoken(id, 0.8);
            let tick = plan_tick(80, &target, target.goal_avd(), 0.4, false);
            let json = serde_json::to_string(&tick).unwrap();
            let back: TickFeeling = serde_json::from_str(&json).unwrap();
            assert_eq!(back.prosody, Some(id));
            assert!(back.burst.is_none());
            assert!(json.contains(id.as_str()));
        }
    }

    #[test]
    fn a_burst_does_not_steal_a_spoken_word() {
        let mut core = FluidCore::new(BaselineVoice::default());
        let mut burst = TargetState::spoken(EmotionId::Amusement, 1.0);
        burst.track = Track::Burst;
        let tick = core.step(0, &burst);
        assert!(tick.prosody.is_none());
        assert_eq!(tick.burst, Some(EmotionId::Amusement));
    }
}
