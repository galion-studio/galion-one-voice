// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-01 integration: the live demo frame loop on the real Heartbeat + RikaReplayBackend path.
use galion_emotion::band::consent_bits;
use galion_emotion::Purpose;
use galion_heartbeat::symphony::NanoPcmRing;
use galion_heartbeat::FRAME_SAMPLES;
use galion_live_demo::mimi::{MimiMode, MimiVoice};
use galion_live_demo::{run_demo, run_frames, run_frames_draining, synth_you, Config, FrameRecord, FrameSink, RunCfg, ToneVoice};
use galion_presence::wav;
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("galion-live-demo-test-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

const SA: u8 = consent_bits::SENSE | consent_bits::ADAPT;

#[test]
fn n_frames_stereo_length_no_panic() {
    let you = synth_you();
    let frames = you.len().div_ceil(FRAME_SAMPLES);
    for cfg in [
        RunCfg::default(),
        RunCfg { consent_bits: SA, ..RunCfg::default() },
        RunCfg { consent_bits: SA, rika_opener: true, ..RunCfg::default() },
    ] {
        let (me, s) = run_frames(&you, &cfg, &mut ToneVoice::default(), &mut []);
        assert_eq!(me.len(), frames * FRAME_SAMPLES);
        assert_eq!(s.frames as usize, frames);
        assert_eq!(s.paused_frames, 0);
        assert!(s.me_onsets >= 2, "mock turn-taking replied: {s:?}");
        assert!(me.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    }
}

#[test]
fn run_demo_writes_stereo_capture_and_sums() {
    let out = tmp("files");
    let cfg = Config { input: None, out_dir: out.clone(), run: RunCfg { consent_bits: SA, rika_opener: true, ..RunCfg::default() }, echo: false };
    let a = run_demo(&cfg, &mut ToneVoice::default(), &mut []).expect("demo runs");
    let st = wav::read_wav(&a.stereo_wav).unwrap();
    assert_eq!(st.sample_rate, 24_000);
    assert_eq!(st.channels, 2);
    assert_eq!(st.samples.len(), a.stereo_frames); // downmixed mono length == per-channel length
    assert_eq!(a.stereo_frames, a.summary.frames as usize * FRAME_SAMPLES);
    let bytes = std::fs::metadata(&a.stereo_wav).unwrap().len() as usize;
    assert_eq!(bytes, 44 + a.stereo_frames * 2 * 2);
    let cap = std::fs::read_to_string(&a.capture).unwrap();
    assert!(cap.contains("summary: frames="));
    assert_eq!(cap.lines().filter(|l| l.starts_with(' ') || l.chars().next().is_some_and(|c| c.is_ascii_digit())).count() as u32, a.summary.frames);
    let sums = std::fs::read_to_string(&a.sums).unwrap();
    for f in ["live_input_24k.wav", "live_output_stereo.wav", "console_capture.txt"] {
        let line = sums.lines().find(|l| l.ends_with(f)).expect(f);
        let want = galion_live_demo::sha256::hex(&std::fs::read(out.join(f)).unwrap());
        assert!(line.starts_with(&want), "{f}");
    }
    // deterministic offline run
    let cfg2 = Config { out_dir: tmp("files2"), ..cfg };
    let b = run_demo(&cfg2, &mut ToneVoice::default(), &mut []).unwrap();
    assert_eq!(std::fs::read(&a.sums).unwrap(), std::fs::read(&b.sums).unwrap());
}

#[test]
fn opener_lean_in_160ms_and_single_soft_hum() {
    let you = synth_you();
    let (_, s) = run_frames(&you, &RunCfg { consent_bits: SA, rika_opener: true, ..RunCfg::default() }, &mut ToneVoice::default(), &mut []);
    assert!(s.rika_enabled);
    assert_eq!(s.lean_in_ms, Some(160));
    assert_eq!(s.soft_hum_frames, 1, "soft ack at most once, only after the user has spoken");
    assert!(s.yields >= 1);
}

#[test]
fn rika_off_by_default_and_without_consent() {
    let you = synth_you();
    let (_, s) = run_frames(&you, &RunCfg::default(), &mut ToneVoice::default(), &mut []);
    assert!(!s.rika_enabled);
    assert_eq!(s.lean_in_frames, 0);
    let (_, s) = run_frames(&you, &RunCfg { rika_opener: true, ..RunCfg::default() }, &mut ToneVoice::default(), &mut []);
    assert_eq!(s.lean_in_frames, 0, "no SENSE/ADAPT → no lean-in");
    assert_eq!(s.soft_hum_frames, 0);
}

struct OffSink(u32);
impl FrameSink for OffSink {
    fn on_frame(&mut self, r: &FrameRecord) {
        if matches!(r.off, Some(galion_emotion::OffReason::Gate(_))) {
            self.0 += 1;
        }
    }
}

#[test]
fn workplace_and_education_hard_off_equal_default_me_band() {
    let you = synth_you();
    let (base, _) = run_frames(&you, &RunCfg::default(), &mut ToneVoice::default(), &mut []);
    for p in [Purpose::Workplace, Purpose::Education] {
        let mut sink = OffSink(0);
        let (me, s) = run_frames(&you, &RunCfg { consent_bits: SA, rika_opener: true, purpose: p, ..RunCfg::default() }, &mut ToneVoice::default(), &mut [&mut sink]);
        assert_eq!(me, base, "{p:?}: me band must equal the no-consent default");
        assert_eq!(s.lean_in_frames, 0);
        assert_eq!(sink.0, s.frames, "{p:?}: every frame gate-OFF");
    }
}

#[test]
fn sixteen_khz_input_is_resampled_to_24k() {
    let dir = tmp("16k");
    std::fs::create_dir_all(&dir).unwrap();
    let sr = 16_000u32;
    let pcm: Vec<i16> = (0..sr as usize * 2).map(|i| if i > 8000 && i < 20000 { ((i as f32 * 0.07).sin() * 6000.0) as i16 } else { 0 }).collect();
    let inp = dir.join("in16k.wav");
    std::fs::write(&inp, wav::encode_pcm16(sr, 1, &pcm)).unwrap();
    let cfg = Config { input: Some(inp), out_dir: dir.join("out"), run: RunCfg::default(), echo: false };
    let a = run_demo(&cfg, &mut ToneVoice::default(), &mut []).unwrap();
    assert_eq!(a.summary.frames, 25); // 2 s = 48000 samples @ 24 kHz = 25 frames
    assert_eq!(wav::read_wav(&a.input_wav).unwrap().sample_rate, 24_000);
}

#[test]
fn mimi_bypass_lane_runs_and_labels() {
    let you = synth_you();
    let short = &you[..FRAME_SAMPLES * 60]; // through turn 1 and the start of the reply
    let mut v = MimiVoice::new(ToneVoice::default(), MimiMode::NoKernels).expect("lane");
    let (me, s) = run_frames(short, &RunCfg::default(), &mut v, &mut []);
    assert_eq!(me.len(), 60 * FRAME_SAMPLES);
    assert_eq!(v.errors, 0);
    assert!(s.me_voiced_frames > 0);
    assert!(me.iter().any(|x| x.abs() > 0.01), "DryBypass is audible");
    assert!(galion_live_demo::MeVoice::tag(&v).contains("IS_STUB"));
}

/// FP-02: a spoken reply in the me band keeps turn-taking `me_voiced` up for the whole
/// sentence (via `MeVoice::planned` → `me_planned`), so the dot shows *speaking* while the
/// words play — not just the mock's 8-frame reply. Barge-in (turn 2 of `synth_you`) cuts it.
#[test]
fn reply_words_hold_me_voiced_and_yield_on_barge_in() {
    use galion_live_demo::ReplyVoice;
    struct Rec(Vec<bool>);
    impl FrameSink for Rec {
        fn on_frame(&mut self, r: &FrameRecord) {
            self.0.push(r.me_voiced);
        }
    }
    let you = synth_you();
    let words: Vec<f32> = (0..24 * FRAME_SAMPLES).map(|i| 0.2 * (i as f32 * 0.03).sin()).collect(); // 1.92 s
    let mut v = ReplyVoice::new(words);
    let mut rec = Rec(Vec::new());
    let cfg = RunCfg { consent_bits: SA, rika_opener: true, ..RunCfg::default() };
    let (me, s) = run_frames(&you, &cfg, &mut v, &mut [&mut rec]);
    // longest voiced run = the full first reply (24 frames), far beyond the mock's 8.
    let (mut best, mut run) = (0, 0);
    for &m in &rec.0 {
        run = if m { run + 1 } else { 0 };
        best = best.max(run);
    }
    assert!(best >= 24, "reply held for {best} frames: {s:?}");
    assert!(s.me_onsets >= 2 && s.lean_in_frames > 0, "{s:?}");
    // the voiced frames carry the words (not silence)
    let first = rec.0.iter().position(|&m| m).unwrap();
    let e: f32 = me[first * FRAME_SAMPLES..(first + 1) * FRAME_SAMPLES].iter().map(|x| x * x).sum();
    assert!(e > 1.0, "words audible in me band");
    // turn-2 reply is cut by the barge-in: some voiced run is shorter than the sentence
    // (then the barge-in itself gets a full reply).
    let mut runs = Vec::new();
    let mut r = 0;
    for &m in rec.0.iter().chain([false].iter()) {
        if m { r += 1 } else if r > 0 { runs.push(r); r = 0 }
    }
    assert!(runs.iter().any(|&r| r < 24), "barge-in yields mid-reply: runs {runs:?}");
}

#[test]
fn live_loop_drains_one_nano_frame_and_leaves_the_tail() {
    let you = vec![0.0f32; FRAME_SAMPLES * 3];
    let mut ring = NanoPcmRing::new();
    let mut pcm = vec![0.25f32; FRAME_SAMPLES * 2 + 10];
    pcm[FRAME_SAMPLES..FRAME_SAMPLES * 2].fill(0.5);
    assert_eq!(ring.push(&pcm), pcm.len());
    let (me, summary) = run_frames_draining(&you, &RunCfg::default(), &mut ToneVoice::default(), &mut [], Some(&mut ring));
    assert_eq!(summary.frames, 3);
    assert_eq!(summary.paused_frames, 0);
    assert!(me[..FRAME_SAMPLES].iter().all(|s| (*s - 0.25).abs() < 1e-6));
    assert!(me[FRAME_SAMPLES..2 * FRAME_SAMPLES].iter().all(|s| (*s - 0.5).abs() < 1e-6));
    assert_eq!(ring.queued(), 10, "the tail waits for a later frame");
}
