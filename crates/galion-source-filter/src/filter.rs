// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Glue around `biquad::DirectForm1`. No filter arithmetic lives here.
//!
//! Direct Form 1 is the form that crate recommends when coefficients move
//! while audio is running. Each 80 ms tick may retune. The sample loop only
//! calls `run`.

use biquad::{Biquad, Coefficients, DirectForm1, Hertz, Type};

use crate::controls::{
    formant_q, nasal_q, section_eps, tilt_db, tilt_q, MouthControls, TractBaseline,
    BREATH_CENTER_HZ, F1_Q0, F2_Q0, FORMANT_DB, NASAL_DEPTH_DB, NASAL_HI_HZ, NASAL_LO_HZ,
    TILT_PIVOT_HZ,
};
use galion_emotion_core::{Breath, MomentState, VoiceBaseline};

/// Heap allocations one frame of [`SourceFilter::process`] plus
/// [`SourceFilter::set_controls`] is allowed to perform.
pub const FRAME_ALLOC_BUDGET: usize = 0;

const UNITY: Coefficients<f32> = Coefficients {
    a1: 0.0,
    a2: 0.0,
    b0: 1.0,
    b1: 0.0,
    b2: 0.0,
};

const BREATH_Q: f32 = 0.6;
const BREATH_GAIN: f32 = 0.22;
const NOISE_SEED: u32 = 0x51E0_5A4D;

struct Bank<const N: usize> {
    on: bool,
    stages: [DirectForm1<f32>; N],
}

impl<const N: usize> Bank<N> {
    fn new() -> Self {
        Self {
            on: false,
            stages: [DirectForm1::new(UNITY); N],
        }
    }

    fn apply(&mut self, coeffs: [Coefficients<f32>; N]) {
        let fresh = !self.on;
        for (stage, coeff) in self.stages.iter_mut().zip(coeffs) {
            stage.update_coefficients(coeff);
            if fresh {
                stage.reset_state();
            }
        }
        self.on = true;
    }

    fn bypass(&mut self) {
        if self.on {
            for stage in &mut self.stages {
                stage.reset_state();
            }
            self.on = false;
        }
    }

    fn run(&mut self, mut x: f32) -> f32 {
        for stage in &mut self.stages {
            x = stage.run(x);
        }
        x
    }
}

/// The configured persona's source-filter. Own it for the utterance; call [`Self::process`]
/// once per 80 ms frame.
pub struct SourceFilter {
    sample_rate: u32,
    tract: TractBaseline,
    controls: MouthControls,
    tilt: Bank<2>,
    formant: Bank<4>,
    nasal: Bank<2>,
    breath: Bank<1>,
    breath_gain: f32,
    noise: u32,
}

impl SourceFilter {
    pub fn new(sample_rate: u32, tract: TractBaseline) -> Self {
        let mut filter = Self {
            sample_rate: 0,
            tract: TractBaseline::REST,
            controls: MouthControls::neutral(),
            tilt: Bank::new(),
            formant: Bank::new(),
            nasal: Bank::new(),
            breath: Bank::new(),
            breath_gain: 0.0,
            noise: NOISE_SEED,
        };
        filter.set_baseline(tract);
        filter.set_sample_rate(sample_rate);
        filter
    }

    /// Rest formants fixed at the Burg medians. The moment starts neutral.
    pub fn rest(sample_rate: u32) -> Self {
        Self::new(sample_rate, TractBaseline::REST)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn baseline(&self) -> TractBaseline {
        self.tract
    }

    pub fn controls(&self) -> MouthControls {
        self.controls
    }

    /// Replace the steady tract. Does not read the moment.
    pub fn set_baseline(&mut self, tract: TractBaseline) {
        let f1 = sane_hz(tract.f1_hz, TractBaseline::REST.f1_hz, 120.0, 1_200.0);
        let f2 = sane_hz(
            tract.f2_hz,
            TractBaseline::REST.f2_hz,
            f1 + 200.0,
            4_000.0,
        );
        self.tract = TractBaseline {
            f1_hz: f1,
            f2_hz: f2,
        };
        self.retune();
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        if sample_rate == 0 || sample_rate == self.sample_rate {
            return;
        }
        self.sample_rate = sample_rate;
        self.retune();
    }

    pub fn set_controls(&mut self, controls: MouthControls) {
        self.controls = sanitize(controls);
        self.retune();
    }

    /// Fold one emotion-core frame into controls. `voice` is the rest point
    /// and is not stored as a feeling. The tract frequencies stay as they were.
    pub fn set_moment(
        &mut self,
        voice: &VoiceBaseline,
        moment: &MomentState,
        breath: Breath,
        velum: f32,
    ) {
        let tract = self.tract;
        self.set_controls(MouthControls::from_moment(voice, moment, breath, velum));
        debug_assert_eq!(self.tract.f1_hz.to_bits(), tract.f1_hz.to_bits());
        debug_assert_eq!(self.tract.f2_hz.to_bits(), tract.f2_hz.to_bits());
    }

    /// One frame, in place. Neutral controls return without writing.
    ///
    /// Does not allocate. Does not grow a buffer. The banks and the noise
    /// word are fields.
    pub fn process(&mut self, samples: &mut [f32]) {
        let tilt = self.tilt.on;
        let formant = self.formant.on;
        let nasal = self.nasal.on;
        let breath = self.breath.on && self.breath_gain > 0.0;
        if !tilt && !formant && !nasal && !breath {
            return;
        }
        for sample in samples.iter_mut() {
            let mut x = *sample;
            if tilt {
                x = self.tilt.run(x);
            }
            if formant {
                x = self.formant.run(x);
            }
            if nasal {
                x = self.nasal.run(x);
            }
            if breath {
                let noise = self.next_noise();
                let air = self.breath.run(noise);
                x += air * self.breath_gain;
            }
            *sample = clamp_audio(x);
        }
    }

    /// Apply `frames[i]` to frame `i`, holding the last control if the slice
    /// runs out. An empty slice keeps the controls already set.
    pub fn process_frames(&mut self, samples: &mut [f32], frames: &[MouthControls]) {
        let width = crate::frame_samples(self.sample_rate);
        if width == 0 || samples.is_empty() {
            return;
        }
        let mut offset = 0;
        let mut index = 0;
        while offset < samples.len() {
            if let Some(controls) = frames.get(index) {
                self.set_controls(*controls);
            }
            let end = offset.saturating_add(width).min(samples.len());
            self.process(&mut samples[offset..end]);
            offset = end;
            index = index.saturating_add(1);
        }
    }

    fn retune(&mut self) {
        if self.sample_rate < 2 {
            self.tilt.bypass();
            self.formant.bypass();
            self.nasal.bypass();
            self.breath.bypass();
            self.breath_gain = 0.0;
            return;
        }
        self.retune_tilt();
        self.retune_formant();
        self.retune_nasal();
        self.retune_breath();
    }

    fn retune_tilt(&mut self) {
        let db = tilt_db(self.controls);
        if db.abs() < section_eps() {
            self.tilt.bypass();
            return;
        }
        let q = tilt_q();
        let low = self.design(Type::LowShelf(-db), TILT_PIVOT_HZ, q);
        let high = self.design(Type::HighShelf(db), TILT_PIVOT_HZ, q);
        if let (Some(low), Some(high)) = (low, high) {
            self.tilt.apply([low, high]);
        } else {
            self.tilt.bypass();
        }
    }

    fn retune_formant(&mut self) {
        let openness = self.controls.openness;
        if openness.abs() < section_eps() {
            self.formant.bypass();
            return;
        }
        let (f1_at, f2_at) = self.tract.shifted(openness);
        // Depth stays put so a small opening is a small frequency move, not a
        // shallow bump. Q and the centre still follow openness.
        let gain = FORMANT_DB;
        let q1 = formant_q(F1_Q0, openness);
        let q2 = formant_q(F2_Q0, openness);
        let cut_1 = self.design(Type::PeakingEQ(-gain), self.tract.f1_hz, q1);
        let boost_1 = self.design(Type::PeakingEQ(gain), f1_at, q1);
        let cut_2 = self.design(Type::PeakingEQ(-gain), self.tract.f2_hz, q2);
        let boost_2 = self.design(Type::PeakingEQ(gain), f2_at, q2);
        if let (Some(cut_1), Some(boost_1), Some(cut_2), Some(boost_2)) =
            (cut_1, boost_1, cut_2, boost_2)
        {
            self.formant.apply([cut_1, boost_1, cut_2, boost_2]);
        } else {
            self.formant.bypass();
        }
    }

    fn retune_nasal(&mut self) {
        let nasal = self.controls.nasal;
        if nasal < section_eps() {
            self.nasal.bypass();
            return;
        }
        let depth = -NASAL_DEPTH_DB * nasal.min(1.0);
        let q = nasal_q();
        let low = self.design(Type::PeakingEQ(depth), NASAL_LO_HZ, q);
        let high = self.design(Type::PeakingEQ(depth), NASAL_HI_HZ, q);
        if let (Some(low), Some(high)) = (low, high) {
            self.nasal.apply([low, high]);
        } else {
            self.nasal.bypass();
        }
    }

    fn retune_breath(&mut self) {
        let gain = self.controls.breath_noise;
        if gain < section_eps() {
            self.breath.bypass();
            self.breath_gain = 0.0;
            return;
        }
        match self.design(Type::BandPass, BREATH_CENTER_HZ, BREATH_Q) {
            Some(coeff) => {
                self.breath.apply([coeff]);
                self.breath_gain = gain.min(1.0) * BREATH_GAIN;
            }
            None => {
                self.breath.bypass();
                self.breath_gain = 0.0;
            }
        }
    }

    fn design(&self, kind: Type<f32>, hz: f32, q: f32) -> Option<Coefficients<f32>> {
        let nyquist = self.sample_rate as f32 * 0.45;
        if nyquist < 80.0 {
            return None;
        }
        let hz = hz.clamp(40.0, nyquist);
        let q = q.clamp(0.15, 30.0);
        let fs = Hertz::<f32>::from_hz(self.sample_rate as f32).ok()?;
        let f0 = Hertz::<f32>::from_hz(hz).ok()?;
        Coefficients::<f32>::from_params(kind, fs, f0, q).ok()
    }

    fn next_noise(&mut self) -> f32 {
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 {
            x = NOISE_SEED;
        }
        self.noise = x;
        let signed = x as i32;
        signed as f32 * (1.0 / i32::MAX as f32)
    }
}

fn sanitize(controls: MouthControls) -> MouthControls {
    MouthControls {
        tension: finite_clamp(controls.tension, -1.0, 1.0),
        breathiness: finite_clamp(controls.breathiness, 0.0, 1.0),
        openness: finite_clamp(controls.openness, -1.0, 1.0),
        nasal: finite_clamp(controls.nasal, 0.0, 1.0),
        breath_noise: finite_clamp(controls.breath_noise, 0.0, 1.0),
    }
}

fn sane_hz(hz: f32, fallback: f32, lo: f32, hi: f32) -> f32 {
    if hz.is_finite() {
        hz.clamp(lo, hi)
    } else {
        fallback
    }
}

fn finite_clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        0.0
    }
}

fn clamp_audio(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
