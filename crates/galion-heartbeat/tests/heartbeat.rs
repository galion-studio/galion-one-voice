// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
mod common;
use common::*;
use galion_emotion::band::consent_bits;
use galion_emotion::{
    ConsentDecision, Cue, DefaultConsentGate, EmotionFrame, Manner, OffReason, ProsodyControl, Purpose, RikaState, VoicePersona,
};
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

fn tired() -> EmotionFrame {
    EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: -0.6, arousal: 0.2, dominance: 0.4, confidence: 1.0 }
}

fn frame_with(pcm: &[f32], affect: Option<EmotionFrame>, bits: u8, late: bool) -> FrameIn<'_> {
    FrameIn {
        you_pcm: pcm,
        affect,
        tension: None,
        consent_bits: bits,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking: false,
        late,
    }
}

#[test]
fn feeling_tracks_live_empathy_inside_step() {
    let pcm = vec![0.0f32; FRAME_SAMPLES];
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), granted());
    let off = hb.step(&frame_with(&pcm, Some(tired()), consent_bits::SENSE, false));
    assert_eq!(off.decided.state, RikaState::Off, "feeling does not require Rika");
    assert!((off.feeling.e - 1.0).abs() < 1e-6);
    assert_eq!(off.feeling.manner, Manner::Answered);
    assert_eq!(off.feeling.persona, VoicePersona::Samantha);
    assert!(!off.feeling.hub.enabled);
    assert_eq!(off.voice_prosody().pre_pause_ms, 250);
    assert_eq!(galion_heartbeat::FRAME_MS, 80);

    hb.set_persona(VoicePersona::Jarvis);
    let jar = hb.step(&frame_with(&pcm, Some(tired()), consent_bits::SENSE, false));
    assert_eq!(jar.feeling.persona, VoicePersona::Jarvis);
    assert!(jar.feeling.hub.enabled);
    assert!((jar.feeling.prosody.warmth - 0.55).abs() < 1e-4);
    assert_eq!(jar.feeling.prosody.pre_pause_ms, 120);
    assert!((jar.voice_prosody().pitch_shift_st - -0.4).abs() < 1e-4);
    assert!((off.voice_prosody().pitch_shift_st - -0.5).abs() < 1e-4);
    assert!(jar.voice_prosody().warmth < off.voice_prosody().warmth);

    let quiet = hb.step(&frame_with(&pcm, Some(tired()), 0, false));
    assert_eq!(quiet.feeling.e, 0.0, "no SENSE, no empathy");
    assert_eq!(quiet.feeling.manner, Manner::Plain);
}

#[test]
fn feeling_hub_toggle_leaves_user_audio_and_prosody() {
    let you = vec![0.2f32; FRAME_SAMPLES];
    let snapshot = you.clone();
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), granted());
    hb.set_persona(VoicePersona::Jarvis);
    let on = hb.step(&frame_with(&you, Some(tired()), consent_bits::SENSE, false));
    assert!(on.feeling.hub.enabled);
    let mut me = vec![0.0f32; 128];
    on.feeling.hub.mix_me(&mut me);
    assert!(me.iter().any(|s| *s != 0.0));
    assert_eq!(you, snapshot);

    hb.set_hub_layer(false);
    let off = hb.step(&frame_with(&you, Some(tired()), consent_bits::SENSE, false));
    assert!(!off.feeling.hub.enabled);
    assert_eq!(off.feeling.e, on.feeling.e);
    assert_eq!(off.feeling.prosody, on.feeling.prosody);
    let mut muted = vec![0.0f32; 128];
    off.feeling.hub.mix_me(&mut muted);
    assert!(muted.iter().all(|s| *s == 0.0));
    assert_eq!(you, snapshot);

    hb.set_persona(VoicePersona::Samantha);
    hb.set_hub_layer(true);
    let sam = hb.step(&frame_with(&you, Some(tired()), consent_bits::SENSE, false));
    assert!(!sam.feeling.hub.enabled);
    assert_eq!(you, snapshot);
}

#[test]
fn feeling_late_frame_holds() {
    let pcm = vec![0.0f32; FRAME_SAMPLES];
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), granted());
    let first = hb.step(&frame_with(&pcm, Some(tired()), consent_bits::SENSE, false));
    assert!((first.feeling.e - 1.0).abs() < 1e-6);
    let joy = EmotionFrame { seq: 2, t_ms: 160, dur_ms: 80, valence: 0.8, arousal: 0.4, dominance: 0.5, confidence: 1.0 };
    let held = hb.step(&frame_with(&pcm, Some(joy), consent_bits::SENSE, true));
    assert!(held.paused);
    assert!((held.feeling.e - 1.0).abs() < 1e-6, "a late frame does not retune the voice");
    assert_eq!(held.feeling.manner, Manner::Answered);
}

#[test]
fn feeling_step_fits_the_cpu_tick() {
    let pcm = vec![0.0f32; FRAME_SAMPLES];
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), granted());
    hb.set_persona(VoicePersona::Jarvis);
    let input = frame_with(&pcm, Some(tired()), consent_bits::SENSE, false);
    for _ in 0..16 {
        let _ = hb.step(&input);
    }
    let n = 200u32;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        let out = hb.step(&input);
        assert!(out.feeling.hub.enabled);
        assert!(out.feeling.e > 0.9);
    }
    let elapsed = t0.elapsed();
    let avg_ns = elapsed.as_nanos() / u128::from(n);
    eprintln!("feeling steps={n} elapsed={elapsed:?} avg_ns={avg_ns}");
    assert!(
        elapsed.as_millis() < 50,
        "200 feeling steps took {elapsed:?} (avg {avg_ns} ns); the non-inference tick has to stay inside the 80 ms frame"
    );
}

#[test]
fn hot_path_source_has_no_blocking_or_allocating_calls() {
    for (label, src) in [("src/lib.rs", include_str!("../src/lib.rs")), ("src/symphony.rs", include_str!("../src/symphony.rs"))] {
        let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        for bad in ["Mutex", "RwLock", "Condvar", "sleep", "std::fs", "std::net", "Instant", "SystemTime", "mpsc", "Vec<", "vec!", "String", "Box::new", "format!", "println!", "thread::"] {
            assert!(!code.contains(bad), "{label} contains `{bad}`");
        }
    }
}
