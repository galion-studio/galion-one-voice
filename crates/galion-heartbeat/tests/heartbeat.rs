// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
mod common;
use common::*;
use galion_emotion::band::consent_bits;
use galion_emotion::{ConsentDecision, Cue, DefaultConsentGate, OffReason, ProsodyControl, Purpose, RikaState};
use galion_heartbeat::{FrameIn, FrameOut, Heartbeat, HeartbeatConfig, FRAME_SAMPLES, RIKA_BITS};

fn granted() -> DefaultConsentGate {
    let mut g = DefaultConsentGate::default();
    g.grant(0, "hr", Purpose::PersonalAssistant);
    g
}

fn inp(pcm: &[f32], bits: u8) -> FrameIn<'_> {
    FrameIn {
        you_pcm: pcm,
        affect: None,
        tension: None,
        consent_bits: bits,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking: false,
        late: false,
    }
}

fn drive(hb: &mut Heartbeat<DefaultConsentGate>, frames: &[Vec<f32>], f: impl Fn(usize, FrameIn<'_>) -> FrameIn<'_>) -> Vec<FrameOut> {
    let mut outs = Vec::with_capacity(frames.len());
    for (i, x) in frames.iter().enumerate() {
        outs.push(hb.step(&f(i, inp(x, RIKA_BITS))));
    }
    outs
}

#[test]
fn frame_constants_match_locks() {
    assert_eq!(galion_heartbeat::FRAME_MS, 80);
    assert_eq!(FRAME_SAMPLES, 1920);
    let locks = include_str!("../../../LOCKS.toml");
    assert!(locks.contains("frame_ms = 80"));
    assert!(locks.contains("rika_land_max_ms = 160"));
    assert!(locks.contains("rika_yield_max_frames = 1"));
}

#[test]
fn rika_is_off_by_default_and_output_is_identity() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), granted());
    let outs = drive(&mut hb, &frames, |_, i| i);
    let base = ProsodyControl::default();
    for o in &outs {
        assert_eq!(o.decided.state, RikaState::Off);
        assert_eq!(o.decided.off, Some(OffReason::Disabled));
        assert!(!o.lean_in_audible());
        assert_eq!(o.me_gain(), 1.0);
        assert_eq!(o.prosody(base), base);
        assert!(o.me_onset_allowed);
    }
    assert_eq!(hb.rika_report().enabled_frames, 0);
    assert_eq!(hb.lean_in_ms(), None);
}

#[test]
fn r1_through_heartbeat_lean_in_160ms_and_yield_one_frame() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    let outs = drive(&mut hb, &frames, |_, i| i);
    let rep = hb.rika_report();
    assert_eq!(rep.evidence_frame, Some(13));
    assert_eq!(rep.armed_frame, Some(14));
    assert_eq!(hb.lean_in_ms(), Some(160), "R1 = 160 ms (LOCKS rika_land_max_ms)");
    // Decided at 14, applied (audible) at 15 = evidence + 2 frames.
    assert_eq!(outs[14].decided.state, RikaState::Armed);
    assert!(!outs[14].lean_in_audible());
    let first_audible = outs.iter().position(|o| o.lean_in_audible()).unwrap() as u32;
    assert_eq!((first_audible - 13) * 80, hb.lean_in_ms().unwrap());
    // Held breath: no me onset while armed.
    for o in &outs[14..18] {
        assert!(!o.me_onset_allowed, "frame {}", o.index);
        assert_eq!(o.decided.cue, Cue::LeanIn);
    }
    // User onset at 18 → yield_now decided at 18, applied at 19 (≤ 1 frame).
    assert!(outs[18].decided.yield_now);
    assert!(outs[19].yield_now());
    assert_eq!(rep.yield_latency_frames, Some(1));
    assert_eq!(rep.lexical_tokens_emitted, 0);
    // Lean-in prosody is bounded and never louder.
    let p = outs[15].prosody(ProsodyControl::default());
    assert!(p.energy <= ProsodyControl::default().energy);
    assert!(outs.iter().all(|o| o.me_gain() <= 1.0 && o.onset_threshold_db() <= 0.0));
}

#[test]
fn no_sense_or_no_adapt_keeps_rika_off_and_reads_no_affect() {
    let frames = render(&r1());
    for (bits, why) in [(consent_bits::ADAPT, OffReason::NoSense), (consent_bits::SENSE, OffReason::NoAdapt), (0, OffReason::NoSense)] {
        let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
        let outs = drive(&mut hb, &frames, |_, mut i| {
            i.consent_bits = bits;
            i
        });
        assert!(outs.iter().all(|o| o.decided.off == Some(why) && !o.lean_in_audible()));
        assert_eq!(hb.rika_report().enabled_frames, 0);
    }
}

#[test]
fn workplace_and_education_are_denied_even_with_bits() {
    let frames = render(&r1());
    for p in [Purpose::Workplace, Purpose::Education] {
        let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
        let outs = drive(&mut hb, &frames, |_, mut i| {
            i.purpose = p;
            i
        });
        assert!(outs.iter().all(|o| o.decided.off == Some(OffReason::Gate(ConsentDecision::DenyProhibitedPurpose))));
        assert!(hb.explain().contains("workplace"));
    }
}

#[test]
fn no_session_no_rika() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    let outs = drive(&mut hb, &frames, |_, mut i| {
        i.session_open = false;
        i
    });
    assert!(outs.iter().all(|o| o.decided.off == Some(OffReason::NoSession)));
}

#[test]
fn late_frame_is_a_pause_no_extra_trigger_no_burst() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    // Late on the confirm frame (14): hysteresis not advanced, arms one frame later.
    let outs = drive(&mut hb, &frames, |n, mut i| {
        i.late = n == 14;
        i
    });
    assert!(outs[14].paused);
    assert!(!outs[14].me_onset_allowed, "late = pause: nothing new starts");
    assert_eq!(outs[14].decided.state, RikaState::Candidate);
    assert_eq!(hb.rika_report().armed_count, 1);
    assert_eq!(hb.rika_report().late_frames, 1);
    assert_eq!(hb.paused_frames(), 1);
    assert!(outs.iter().all(|o| (0.0..=1.0).contains(&o.applied.lean_in)), "no catch-up burst");
}

#[test]
fn short_pcm_frame_is_treated_as_late_pause() {
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    let short = vec![0.0f32; FRAME_SAMPLES / 2];
    let o = hb.step(&inp(&short, RIKA_BITS));
    assert!(o.paused);
    assert!(!o.me_onset_allowed);
}

#[test]
fn consent_withdrawn_mid_moment_wipes_and_falls_back_next_frame() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    let outs = drive(&mut hb, &frames, |n, mut i| {
        if n >= 16 {
            i.consent_bits = 0;
        }
        i
    });
    assert_eq!(outs[15].decided.state, RikaState::Armed);
    assert_eq!(outs[16].decided.off, Some(OffReason::NoSense), "OFF the same frame");
    assert!(outs[16].lean_in_audible(), "frame 16 still plays what frame 15 decided");
    assert!(!outs[17].lean_in_audible(), "neutral fallback at m+1");
    assert_eq!(hb.rika().state(), RikaState::Off);
}

#[test]
fn reset_starts_a_fresh_session() {
    let frames = render(&r1());
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    drive(&mut hb, &frames, |_, i| i);
    assert!(hb.lean_in_ms().is_some());
    hb.reset();
    assert_eq!(hb.frames(), 0);
    assert_eq!(hb.lean_in_ms(), None);
    assert_eq!(hb.rika().state(), RikaState::Off);
}

#[test]
fn hot_path_source_has_no_blocking_or_allocating_calls() {
    let src = include_str!("../src/lib.rs");
    let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    for bad in ["Mutex", "RwLock", "Condvar", "sleep", "std::fs", "std::net", "Instant", "SystemTime", "mpsc", "Vec<", "vec!", "String", "Box::new", "format!", "println!", "thread::"] {
        assert!(!code.contains(bad), "hot-path crate source contains `{bad}`");
    }
}
