// Galion Voice — Perpetual Open MIT License
//! Real live feeds (A11 follow-up), each behind its own cargo feature.
//!
//! * [`TensorTimingFeed`] (`tensor-feed`): every heartbeat runs a small
//!   codec-shaped pipeline on **real galion-tensor kernels** and times each stage
//!   with [`StageTimer`]: CAPTURE = mic frame (1920 samples @ 24 kHz) from a
//!   [`crate::mic::MicSource`] — [`crate::mic::SynthMic`] by default, or a WAV
//!   fixture via [`crate::mic::PcmMic`] (`--mic-wav`; live mic is stubbed) →
//!   `CausalConv1dStream` encoder → pooled latent; THINK = `layers` × `gemv_f32_into`
//!   (`d × d`, tanh); SPEAK = `gemv_f32_into` back to frame length →
//!   `CausalConv1dStream` decoder. Op counter = 2 × MACs actually executed;
//!   GFLOP/s = ops / wall time. Weights are deterministic synthetic values — there
//!   are no released Galion weights yet (A7 gq-pack deferred); swap in `.gq`
//!   tensors when they exist, the timing/counter path stays the same.
//! * [`EmotionAffectFeed`] (`emotion-feed`): galion-emotion
//!   `AffectDetector` (default [`EnergyStubDetector`] = `GatedDetector<LoudnessEstimator>`;
//!   any `GatedDetector<E: AffectEstimator>` via [`AffectSource`]) → A9
//!   `AffectStore<Consented>::record` → `latest()`. The store is revalidated
//!   against the `ConsentGate` once per heartbeat (A9 §2); `Unconsented`, a
//!   cleared SENSE bit or a withdrawn gate all yield `no reading`.
//!
//! Both implement the dependency-free traits in [`crate::live`], so tests and
//! stub builds use the same [`ComposedFeed`] path.

#[allow(unused_imports)]
use crate::mic::{MicSource, SynthMic};
#[allow(unused_imports)]
use crate::live::{AffectFeed, AffectReading, AffectSample, ComposedFeed, Stage, StageTimer, StageTiming, TimingFeed};

// ---------------------------------------------------------------------------
// galion-tensor timing feed
// ---------------------------------------------------------------------------

#[cfg(feature = "tensor-feed")]
pub use tensor_feed::{TensorTimingFeed, TensorFeedConfig};

#[cfg(feature = "tensor-feed")]
mod tensor_feed {
    use super::*;
    use galion_tensor::{gemv_f32_into, CausalConv1dStream, Exec, Tensor};

    /// Shape of the per-frame pipeline. Defaults are cheap enough for debug builds.
    #[derive(Clone, Copy, Debug)]
    pub struct TensorFeedConfig {
        pub sample_rate: u32,
        /// Samples per 80 ms frame.
        pub frame: usize,
        /// Encoder channels (conv `c_out`).
        pub channels: usize,
        pub kernel: usize,
        /// Latent width (`d_model`); must be a multiple of `channels`.
        pub d: usize,
        pub layers: usize,
        pub exec: Exec,
    }

    impl Default for TensorFeedConfig {
        fn default() -> Self {
            Self { sample_rate: 24_000, frame: 1920, channels: 8, kernel: 5, d: 256, layers: 4, exec: Exec::Auto }
        }
    }

    /// Real galion-tensor kernels, wall-clock timed per stage. See module docs.
    pub struct TensorTimingFeed {
        cfg: TensorFeedConfig,
        enc: CausalConv1dStream,
        dec: CausalConv1dStream,
        w_think: Vec<Vec<f32>>,
        w_out: Vec<f32>,
        pcm: Vec<f32>,
        enc_out: Vec<f32>,
        h: Vec<f32>,
        h2: Vec<f32>,
        frame_out: Vec<f32>,
        pcm_out: Vec<f32>,
        mic: Box<dyn MicSource>,
        /// Cumulative ops since start (2 × MACs).
        pub total_ops: u64,
    }

    fn weights(seed: &mut u64, n: usize, scale: f32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (((*seed >> 40) as f32) / ((1u64 << 24) as f32) - 0.5) * 2.0 * scale
            })
            .collect()
    }

    impl TensorTimingFeed {
        /// Synthetic mic (deterministic tone) — the pre-wave-2 behaviour.
        pub fn new(cfg: TensorFeedConfig) -> galion_tensor::Result<Self> {
            Self::with_mic(cfg, Box::new(SynthMic { sample_rate: cfg.sample_rate }))
        }

        /// Any [`MicSource`] for CAPTURE (e.g. a WAV fixture). Its sample rate
        /// must equal `cfg.sample_rate` (resample when building the mic).
        pub fn with_mic(cfg: TensorFeedConfig, mic: Box<dyn MicSource>) -> galion_tensor::Result<Self> {
            assert_eq!(mic.sample_rate(), cfg.sample_rate, "mic sample rate must match the pipeline");
            assert!(cfg.d % cfg.channels == 0 && cfg.d / cfg.channels <= cfg.frame, "d must be channels × pooled steps");
            let mut seed = 0xA11_7E50u64;
            let (c, k, d, n) = (cfg.channels, cfg.kernel, cfg.d, cfg.frame);
            let enc_w = Tensor::from_vec(weights(&mut seed, c * k, 0.5), vec![c, 1, k])?;
            let dec_w = Tensor::from_vec(weights(&mut seed, k, 0.5), vec![1, 1, k])?;
            let s = 1.0 / (d as f32).sqrt();
            Ok(Self {
                enc: CausalConv1dStream::new(&enc_w, None, 1, 1, n)?,
                dec: CausalConv1dStream::new(&dec_w, None, 1, 1, n)?,
                w_think: (0..cfg.layers).map(|_| weights(&mut seed, d * d, s)).collect(),
                w_out: weights(&mut seed, n * d, s),
                pcm: vec![0.0; n],
                enc_out: vec![0.0; c * n],
                h: vec![0.0; d],
                h2: vec![0.0; d],
                frame_out: vec![0.0; n],
                pcm_out: vec![0.0; n],
                mic,
                total_ops: 0,
                cfg,
            })
        }

        /// Ops (2 × MACs) one `step` executes.
        pub fn ops_per_frame(&self) -> u64 {
            let c = &self.cfg;
            let enc = c.channels * c.frame * c.kernel;
            let think = c.layers * c.d * c.d;
            let speak = c.frame * c.d + c.frame * c.kernel;
            2 * (enc + think + speak) as u64
        }

        /// Name of the CAPTURE source (`synth`, `wav`, …).
        pub fn mic_name(&self) -> &'static str {
            self.mic.name()
        }

        /// Last decoded output frame (SPEAK), for tests.
        pub fn last_output(&self) -> &[f32] {
            &self.pcm_out
        }

        fn capture(&mut self, tick: u64) {
            let n = self.cfg.frame;
            self.mic.fill(tick, &mut self.pcm);
            self.enc.step(&self.pcm, n, &mut self.enc_out).expect("encoder shapes fixed at new()");
            // Mean-pool each channel into d/channels steps -> latent h.
            let (c, steps) = (self.cfg.channels, self.cfg.d / self.cfg.channels);
            let win = n / steps;
            for ch in 0..c {
                for s in 0..steps {
                    let seg = &self.enc_out[ch * n + s * win..ch * n + (s + 1) * win];
                    self.h[ch * steps + s] = seg.iter().sum::<f32>() / win as f32;
                }
            }
        }

        fn think(&mut self) {
            let d = self.cfg.d;
            for w in &self.w_think {
                gemv_f32_into(w, d, d, &self.h, None, &mut self.h2, self.cfg.exec).expect("think shapes");
                for (o, i) in self.h.iter_mut().zip(&self.h2) {
                    *o = i.tanh();
                }
            }
        }

        fn speak(&mut self) {
            let (n, d) = (self.cfg.frame, self.cfg.d);
            gemv_f32_into(&self.w_out, n, d, &self.h, None, &mut self.frame_out, self.cfg.exec).expect("speak shapes");
            self.dec.step(&self.frame_out, n, &mut self.pcm_out).expect("decoder shapes");
        }
    }

    impl TimingFeed for TensorTimingFeed {
        fn step(&mut self, tick: u64) -> StageTiming {
            let mut t = StageTimer::default();
            t.time(Stage::Capture, || self.capture(tick));
            t.time(Stage::Think, || self.think());
            t.time(Stage::Speak, || self.speak());
            let ops = self.ops_per_frame();
            self.total_ops += ops;
            let ms = t.lat.total() as f64;
            let gflops = if ms > 0.0 { ops as f64 / (ms / 1000.0) / 1e9 } else { 0.0 };
            StageTiming {
                capture: t.lat.get(Stage::Capture),
                think: t.lat.get(Stage::Think),
                speak: t.lat.get(Stage::Speak),
                ops,
                gflops,
            }
        }
        fn pcm(&self) -> &[f32] {
            &self.pcm
        }
        fn sample_rate(&self) -> u32 {
            self.cfg.sample_rate
        }
        fn name(&self) -> &'static str {
            "galion-tensor"
        }
    }
}

// ---------------------------------------------------------------------------
// galion-emotion affect feed
// ---------------------------------------------------------------------------

#[cfg(feature = "emotion-feed")]
pub use emotion_feed::{AffectSource, EmotionAffectFeed};

#[cfg(feature = "emotion-feed")]
mod emotion_feed {
    use super::*;
    use galion_emotion::{
        consent_bits, AffectDetector, AffectStore, Consented, DefaultConsentGate, EnergyStubDetector, GatedDetector, Purpose,
        RefEntry, ReferenceEstimator, Revalidated, SyntheticEstimator, SyntheticMode, Unconsented, WipeReceipt,
    };

    /// Which estimator sits behind the gated SENSE detector. All are stubs /
    /// replays — no model, no download, no training.
    #[derive(Clone, Debug, PartialEq, Default)]
    pub enum AffectSource {
        /// RMS/ZCR loudness (A9 `EnergyStubDetector`): arousal only, valence unknown (0).
        #[default]
        Loudness,
        /// Deterministic fixture mode with valence (`sweep`, `joy`, `valence:<v>`, …).
        Synthetic(SyntheticMode),
        /// Replay of an offline reference label track (SenseVoice-Small dev-oracle seam).
        Reference(Vec<RefEntry>),
    }

    impl AffectSource {
        /// CLI: `loudness` | `synthetic:<mode>` | `reference:<path>`.
        pub fn parse(s: &str) -> Result<AffectSource, String> {
            if s == "loudness" {
                return Ok(AffectSource::Loudness);
            }
            if let Some(m) = s.strip_prefix("synthetic:") {
                return SyntheticMode::parse(m)
                    .map(AffectSource::Synthetic)
                    .ok_or_else(|| format!("unknown synthetic mode {m:?} (neutral|sweep|joy|angry|sad|tired|calm|valence:<v>)"));
            }
            if let Some(p) = s.strip_prefix("reference:") {
                let src = std::fs::read_to_string(p).map_err(|e| format!("reference track {p}: {e}"))?;
                return galion_emotion::parse_reference_track(&src).map(AffectSource::Reference);
            }
            Err(format!("unknown --affect {s:?} (loudness|synthetic:<mode>|reference:<path>)"))
        }

        pub fn detector(self) -> Box<dyn AffectDetector> {
            match self {
                AffectSource::Loudness => Box::new(EnergyStubDetector::default()),
                AffectSource::Synthetic(m) => Box::new(GatedDetector::new(SyntheticEstimator::new(m))),
                AffectSource::Reference(t) => Box::new(GatedDetector::new(ReferenceEstimator::new(t))),
            }
        }
    }
    use std::time::Instant;

    enum Slot {
        Off(AffectStore<Unconsented>),
        On(AffectStore<Consented>),
        /// Transient while moving the typestate; never observed outside `read`.
        Moving,
    }

    /// galion-emotion SENSE path: detector → `AffectStore<Consented>` → reading.
    /// `D` defaults to the loudness stub; any consent-gated `AffectDetector` fits.
    pub struct EmotionAffectFeed<D: AffectDetector = EnergyStubDetector> {
        gate: DefaultConsentGate,
        purpose: Purpose,
        slot: Slot,
        det: D,
        /// Wipe receipts (counts only) — Art. 30-style record, for tests/ops.
        pub wipes: Vec<WipeReceipt>,
    }

    impl EmotionAffectFeed {
        /// SENSE off: default-deny gate, store stays `Unconsented`; PCM is never read.
        pub fn unconsented() -> Self {
            Self::unconsented_with(EnergyStubDetector::default())
        }

        /// Grant consent for `PersonalAssistant` with in-band `bits` (must include
        /// SENSE to reach `Consented`; otherwise this stays SENSE-off).
        pub fn consented(bits: u8) -> Self {
            Self::consented_with(EnergyStubDetector::default(), bits)
        }

        /// Default live mode: SENSE + ADAPT.
        pub fn sense_on() -> Self {
            Self::consented(consent_bits::SENSE | consent_bits::ADAPT)
        }
    }

    impl<D: AffectDetector> EmotionAffectFeed<D> {
        /// SENSE off with a chosen detector.
        pub fn unconsented_with(det: D) -> Self {
            Self {
                gate: DefaultConsentGate::default(),
                purpose: Purpose::PersonalAssistant,
                slot: Slot::Off(AffectStore::new(64)),
                det,
                wipes: Vec::new(),
            }
        }

        /// Consent with `bits` and a chosen detector.
        pub fn consented_with(det: D, bits: u8) -> Self {
            let mut f = Self::unconsented_with(det);
            f.gate.grant(0, "en", f.purpose);
            f.slot = match std::mem::replace(&mut f.slot, Slot::Moving) {
                Slot::Off(s) => match s.grant(&f.gate, f.purpose, bits) {
                    Ok(c) => Slot::On(c),
                    Err((u, _)) => Slot::Off(u),
                },
                other => other,
            };
            f
        }

        /// SENSE + ADAPT with a chosen detector.
        pub fn sense_on_with(det: D) -> Self {
            Self::consented_with(det, consent_bits::SENSE | consent_bits::ADAPT)
        }

        /// Withdraw at the gate (GDPR Art. 7(3)); the next heartbeat revalidation wipes.
        pub fn withdraw(&mut self, at_ms: u64) {
            self.gate.withdraw(at_ms);
        }

        /// Change in-band bits now (clearing SENSE wipes immediately).
        pub fn set_bits(&mut self, bits: u8, at_ms: u64) {
            self.slot = match std::mem::replace(&mut self.slot, Slot::Moving) {
                Slot::On(s) => self.settle(s.set_bits(bits, at_ms)),
                other => other,
            };
        }

        /// Frames currently held by the store (0 when unconsented / wiped).
        pub fn stored_frames(&self) -> usize {
            match &self.slot {
                Slot::On(s) => s.len(),
                Slot::Off(s) => s.len(),
                Slot::Moving => 0,
            }
        }

        fn settle(&mut self, r: Revalidated) -> Slot {
            match r {
                Revalidated::StillConsented(s) => Slot::On(s),
                Revalidated::Wiped(u, receipt) => {
                    self.det.reset();
                    self.wipes.push(receipt);
                    Slot::Off(u)
                }
            }
        }
    }

    impl<D: AffectDetector> AffectFeed for EmotionAffectFeed<D> {
        fn sense(&self) -> bool {
            matches!(&self.slot, Slot::On(s) if s.bits() & consent_bits::SENSE != 0)
        }

        fn read(&mut self, tick: u64, pcm: &[f32], sample_rate: u32) -> AffectSample {
            let at_ms = tick * crate::heartbeat::FRAME_MS;
            // Once per heartbeat: re-check the gate; anything but Allow wipes.
            self.slot = match std::mem::replace(&mut self.slot, Slot::Moving) {
                Slot::On(s) => {
                    let r = s.revalidate(&self.gate, at_ms);
                    self.settle(r)
                }
                other => other,
            };
            let Slot::On(store) = &mut self.slot else {
                return AffectSample::default();
            };
            let t0 = Instant::now();
            let frame = self.det.detect(pcm, sample_rate, at_ms as u32, &self.gate, self.purpose);
            if let Ok(f) = &frame {
                store.record(f);
            }
            let latency = t0.elapsed().as_secs_f32() * 1000.0;
            let reading = match frame {
                Ok(_) => store.latest().map(|f| AffectReading { valence: f.valence, arousal: f.arousal, confidence: f.confidence }),
                Err(_) => None,
            };
            AffectSample { latency_ms: Some(latency), reading }
        }

        fn name(&self) -> &'static str {
            "galion-emotion"
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

/// The real live feed (`tensor-feed` + `emotion-feed`): galion-tensor timing,
/// galion-emotion SENSE. `sense = false` keeps the store `Unconsented`.
#[cfg(all(feature = "tensor-feed", feature = "emotion-feed"))]
pub fn real_feed(sense: bool) -> ComposedFeed<TensorTimingFeed, EmotionAffectFeed> {
    let affect = if sense { EmotionAffectFeed::sense_on() } else { EmotionAffectFeed::unconsented() };
    let timing = TensorTimingFeed::new(TensorFeedConfig::default()).expect("default tensor feed config is valid");
    ComposedFeed::new(timing, affect, "real")
}

/// Real feed with a chosen CAPTURE mic and SENSE estimator (`label` → footer
/// `source: live <label>`).
#[cfg(all(feature = "tensor-feed", feature = "emotion-feed"))]
pub fn real_feed_with(
    sense: bool,
    mic: Box<dyn MicSource>,
    affect: AffectSource,
    label: &'static str,
) -> ComposedFeed<TensorTimingFeed, EmotionAffectFeed<Box<dyn galion_emotion::AffectDetector>>> {
    let det = affect.detector();
    let affect = if sense { EmotionAffectFeed::sense_on_with(det) } else { EmotionAffectFeed::unconsented_with(det) };
    let timing = TensorTimingFeed::with_mic(TensorFeedConfig::default(), mic).expect("default tensor feed config is valid");
    ComposedFeed::new(timing, affect, label)
}

/// Which real feeds were compiled in (for `--feed` diagnostics).
pub fn compiled_feeds() -> &'static str {
    match (cfg!(feature = "tensor-feed"), cfg!(feature = "emotion-feed")) {
        (true, true) => "tensor+emotion",
        (true, false) => "tensor",
        (false, true) => "emotion",
        (false, false) => "none (stub-only build)",
    }
}

/// Is WAV fixture mic input compiled in (feature `wav-mic`)?
pub fn wav_mic_compiled() -> bool {
    cfg!(feature = "wav-mic")
}

#[cfg(all(test, feature = "tensor-feed", feature = "emotion-feed"))]
mod tests {
    use super::*;
    use crate::live::{LiveFeed, LiveSource, LiveView, NO_READING};
    use crate::mic::SynthMic;
    use crate::{ui, ConsoleState, MetricsSource, SyntheticSource};
    use galion_emotion::consent_bits;
    use ratatui::{backend::TestBackend, Terminal};

    fn run<F: LiveFeed>(feed: F, ticks: u64) -> (ConsoleState, LiveSource<F, SyntheticSource>) {
        let mut s = ConsoleState::default();
        let mut src = LiveSource { feed, inner: SyntheticSource::default() };
        for _ in 0..ticks {
            src.poll(&mut s);
            s.advance();
        }
        (s, src)
    }

    fn rows(s: &mut ConsoleState) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(120, 24)).unwrap();
        term.draw(|f| ui::draw(f, s)).unwrap();
        let b = term.backend().buffer().clone();
        (0..b.area.height).map(|y| (0..b.area.width).map(|x| b[(x, y)].symbol()).collect()).collect()
    }

    fn one_number_per_stage(v: &LiveView, n_ms: usize) {
        let t = v.text();
        for st in Stage::ALL {
            assert_eq!(t.matches(&format!(" {} ", st.label())).count(), 1, "{t}");
        }
        assert_eq!(t.matches("ms").count(), n_ms, "{t}");
    }

    #[test]
    fn real_feed_sense_on_one_number_per_stage() {
        let (mut s, src) = run(real_feed(true), 6);
        let v = s.live.clone().unwrap();
        one_number_per_stage(&v, 4);
        for st in Stage::ALL {
            assert!(v.lat.get(st).is_some(), "{st:?}");
        }
        let a = v.affect.expect("SENSE on => affect from galion-emotion");
        assert!(a.arousal > 0.0 && a.confidence > 0.0, "{a:?}");
        assert!(src.feed.affect.stored_frames() == 6, "AffectStore recorded one frame per tick");
        assert_eq!(s.tensor.ops, src.feed.timing.ops_per_frame());
        assert_eq!(src.feed.timing.total_ops, 6 * src.feed.timing.ops_per_frame());
        assert!(s.tensor.gflops > 0.0);
        assert!(src.feed.timing.last_output().iter().any(|x| *x != 0.0), "speak stage produced output");
        let r = rows(&mut s);
        assert!(r.iter().any(|l| l.contains("affect v")), "{r:#?}");
        assert!(r.iter().any(|l| l.contains("source: live real")), "{r:#?}");
    }

    #[test]
    fn real_feed_sense_off_no_reading() {
        let (mut s, src) = run(real_feed(false), 5);
        let v = s.live.clone().unwrap();
        assert!(!v.sense && v.affect.is_none() && v.lat.get(Stage::Sense).is_none());
        assert!(v.text().contains("sense no reading") && v.text().contains("affect no reading"), "{}", v.text());
        one_number_per_stage(&v, 3);
        assert_eq!(src.feed.affect.stored_frames(), 0, "Unconsented store holds nothing");
        assert!(rows(&mut s).iter().any(|l| l.contains("affect no reading")));
    }

    #[test]
    fn grant_without_sense_bit_stays_off() {
        let (s, _) = run(
            ComposedFeed::new(TensorTimingFeed::new(TensorFeedConfig::default()).unwrap(), EmotionAffectFeed::consented(consent_bits::ADAPT), "x"),
            2,
        );
        assert_eq!(s.live.unwrap().affect_text(), NO_READING);
    }

    #[test]
    fn withdraw_mid_run_wipes_and_shows_no_reading() {
        let (mut s, mut src) = run(real_feed(true), 4);
        assert!(s.live.as_ref().unwrap().affect.is_some());
        assert_eq!(src.feed.affect.stored_frames(), 4);
        src.feed.affect.withdraw(400);
        src.poll(&mut s);
        let v = s.live.as_ref().unwrap();
        assert!(v.affect.is_none() && v.text().contains("sense no reading"), "{}", v.text());
        assert_eq!(src.feed.affect.stored_frames(), 0);
        assert_eq!(src.feed.affect.wipes.len(), 1);
        assert_eq!(src.feed.affect.wipes[0].frames_wiped, 4);
    }

    #[test]
    fn clearing_sense_bit_wipes_immediately() {
        let mut f = EmotionAffectFeed::sense_on();
        let pcm: Vec<f32> = (0..1920).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
        assert!(f.read(0, &pcm, 24_000).reading.is_some());
        f.set_bits(consent_bits::ADAPT, 80);
        assert!(!f.sense());
        assert_eq!(f.read(1, &pcm, 24_000), AffectSample::default());
        assert_eq!(f.wipes.len(), 1);
    }

    #[test]
    fn tensor_feed_scalar_exec_matches_auto_output() {
        let mut a = TensorTimingFeed::new(TensorFeedConfig { exec: galion_tensor::Exec::Scalar, ..Default::default() }).unwrap();
        let mut b = TensorTimingFeed::new(TensorFeedConfig::default()).unwrap();
        for t in 0..3 {
            a.step(t);
            b.step(t);
        }
        let diff = a.last_output().iter().zip(b.last_output()).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
        assert!(diff < 1e-3, "scalar vs auto diverged: {diff}");
    }

    #[test]
    fn synthetic_sweep_gives_signed_valence_through_store() {
        let feed = real_feed_with(
            true,
            Box::new(SynthMic::default()),
            AffectSource::parse("synthetic:sweep").unwrap(),
            "real",
        );
        let (_, mut src) = run(feed, 1);
        let mut vals = Vec::new();
        for t in 1..60 {
            let s = src.feed.sample(t);
            vals.push(s.affect.expect("SENSE on").valence);
        }
        assert!(vals.iter().any(|v| *v > 0.3) && vals.iter().any(|v| *v < -0.3), "{vals:?}");
        assert!(AffectSource::parse("synthetic:nope").is_err());
        assert!(AffectSource::parse("bogus").is_err());
        assert!(AffectSource::parse("reference:/nonexistent").is_err());
    }

    #[test]
    fn synthetic_sense_off_still_no_reading() {
        let feed = real_feed_with(false, Box::new(SynthMic::default()), AffectSource::parse("synthetic:joy").unwrap(), "real");
        let (s, src) = run(feed, 3);
        assert_eq!(s.live.unwrap().affect_text(), NO_READING);
        assert_eq!(src.feed.affect.stored_frames(), 0);
    }

    #[test]
    fn reference_track_replays_labels() {
        let track = galion_emotion::parse_reference_track("0 NEUTRAL 0.9\n160 <|HAPPY|> 0.8\n").unwrap();
        let feed = real_feed_with(true, Box::new(SynthMic::default()), AffectSource::Reference(track), "real");
        let (s, _) = run(feed, 6);
        let a = s.live.unwrap().affect.unwrap();
        assert!(a.valence > 0.5, "happy label replayed (EMA-smoothed): {a:?}");
    }

    #[test]
    fn default_real_feed_uses_synth_mic_and_matches_legacy() {
        let mut a = real_feed(true);
        let mut b = real_feed_with(true, Box::new(SynthMic::default()), AffectSource::Loudness, "real");
        assert_eq!(a.timing.mic_name(), "synth");
        for t in 0..4 {
            let (x, y) = (a.sample(t), b.sample(t));
            assert_eq!(x.affect, y.affect, "loudness via Box<dyn AffectDetector> == EnergyStubDetector");
        }
        assert_eq!(a.timing.last_output(), b.timing.last_output());
    }

    #[cfg(feature = "wav-mic")]
    #[test]
    fn wav_fixture_drives_capture_and_sense() {
        use crate::mic::PcmMic;
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/fixtures/p7-turn-mono-8k16.wav");
        let mic = PcmMic::from_wav(&p, 24_000).unwrap();
        let frames = (mic.len() / 1920) as u64;
        let (mut s, src) = run(real_feed_with(true, Box::new(mic), AffectSource::Loudness, "real+wav"), frames);
        assert_eq!(src.feed.timing.mic_name(), "wav");
        assert_eq!(src.feed.affect.stored_frames() as u64, frames.min(64));
        assert!(rows(&mut s).iter().any(|l| l.contains("source: live real+wav")));
        // Arousal follows the fixture's speech/silence pattern: not constant.
        let mut f = real_feed_with(true, Box::new(PcmMic::from_wav(&p, 24_000).unwrap()), AffectSource::Loudness, "x");
        let ar: Vec<f32> = (0..frames).map(|t| f.sample(t).affect.map_or(0.0, |a| a.arousal)).collect();
        let (lo, hi) = ar.iter().fold((1.0f32, 0.0f32), |(l, h), x| (l.min(*x), h.max(*x)));
        assert!(hi - lo > 0.05, "arousal should move with the WAV: {ar:?}");
    }

    #[test]
    fn trait_objects_compose() {
        let mut f: Box<dyn LiveFeed> = Box::new(real_feed(true));
        let s = f.sample(0);
        assert!(s.sense && s.affect.is_some());
        assert_eq!(f.name(), "real");
    }
}
