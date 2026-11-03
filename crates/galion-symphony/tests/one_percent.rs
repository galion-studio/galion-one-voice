// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::{FrameIn, HeartbeatConfig, FRAME_SAMPLES, RIKA_BITS};
use galion_symphony::{
    gate_report, SampleKind, Symphony, TickSample, P1_BUDGET_NS, P3_LL_NS, PRODUCT_CLAIM, TICK_NS,
};

fn silence() -> [f32; FRAME_SAMPLES] {
    [0f32; FRAME_SAMPLES]
}

fn frame<'a>(pcm: &'a [f32], late: bool) -> FrameIn<'a> {
    FrameIn {
        you_pcm: pcm,
        affect: None,
        tension: None,
        consent_bits: RIKA_BITS,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking: false,
        late,
    }
}

#[test]
fn stub_script_is_under_budget_and_the_late_frame_pauses() {
    let mut s = Symphony::granted_rika();
    s.load_stub_script();
    let report = s.report();
    assert!(!report.product_claim);
    assert!(report.late_is_pause);
    assert_eq!(report.catch_up_steps, 0);
    assert_eq!(s.catch_up_steps(), 0);
    let stub = &report.stub;
    assert_eq!(stub.kind, SampleKind::Stub);
    assert_eq!(stub.n, 8);
    assert_eq!(stub.pauses, 1);
    assert_eq!(stub.over_budget, 0);
    assert_eq!(stub.p99_ns, Some(50_000));
    assert_eq!(stub.p1_pass, Some(true));
    assert_eq!(stub.p1_note, "ok");
    assert_eq!(stub.p1_instrument, "injected");
    assert_eq!(stub.p3_instrument, "injected");
    assert_eq!(stub.p3_under_budget, Some(true));
    assert_eq!(stub.p3_much_less, Some(true));
    assert!(stub.p3_ratio.unwrap() >= 10);
    assert_eq!(stub.cpu_source, None);
    assert_eq!(report.measured.n, 0);
    assert_eq!(report.measured.kind, SampleKind::Measured);
    assert_eq!(report.measured.p1_pass, None);
    assert_eq!(report.measured.p1_note, "unavailable");
}

#[test]
fn exact_budget_passes_p1_and_is_not_much_less() {
    let mut s = Symphony::<DefaultConsentGate>::new(
        HeartbeatConfig::default(),
        DefaultConsentGate::default(),
    );
    let v = s.observe(TickSample {
        engine_ns: P1_BUDGET_NS,
        kind: SampleKind::Stub,
        late: false,
    });
    assert!(!v.paused);
    assert!(!v.over_budget);
    assert_eq!(v.catch_up, 0);
    let stub = s.report().stub;
    assert_eq!(stub.p1_pass, Some(true));
    assert_eq!(stub.p3_under_budget, Some(false));
    assert_eq!(stub.p3_much_less, Some(false));
    assert_eq!(stub.p3_ratio, Some(1));
    assert_eq!(stub.pauses, 0);
}

#[test]
fn a_spike_pauses_that_frame_and_the_next_frame_is_not_catch_up() {
    let mut s = Symphony::<DefaultConsentGate>::new(
        HeartbeatConfig::default(),
        DefaultConsentGate::default(),
    );
    let v = s.observe(TickSample {
        engine_ns: P1_BUDGET_NS + 1,
        kind: SampleKind::Stub,
        late: false,
    });
    assert!(v.paused);
    assert!(v.over_budget);
    assert_eq!(v.catch_up, 0);
    let next = s.observe(TickSample {
        engine_ns: 1_000,
        kind: SampleKind::Stub,
        late: false,
    });
    assert!(
        !next.paused,
        "the next frame is the next frame, not a catch-up"
    );
    let stub = s.report().stub;
    assert_eq!(stub.n, 2);
    assert_eq!(stub.pauses, 1);
    // One spike and one cheap frame still average under 1% of two ticks.
    // P1 is the window share. The spike already paused on its own.
    assert_eq!(stub.p1_pass, Some(true));
    assert_eq!(stub.p1_note, "ok");
    assert_eq!(s.catch_up_steps(), 0);
}

#[test]
fn a_window_of_over_budget_frames_fails_p1() {
    let mut s = Symphony::<DefaultConsentGate>::new(
        HeartbeatConfig::default(),
        DefaultConsentGate::default(),
    );
    let v = s.observe(TickSample {
        engine_ns: P1_BUDGET_NS + 1,
        kind: SampleKind::Stub,
        late: false,
    });
    assert!(v.paused);
    assert!(v.over_budget);
    let stub = s.report().stub;
    assert_eq!(stub.n, 1);
    assert_eq!(stub.pauses, 1);
    assert_eq!(stub.p1_pass, Some(false));
    assert_eq!(stub.p1_note, "over");
    assert_eq!(s.catch_up_steps(), 0);
}

#[test]
fn late_sample_pauses_without_a_second_step() {
    let mut s = Symphony::granted_rika();
    let before = s.heartbeat().frames();
    let v = s.observe(TickSample {
        engine_ns: 1_000,
        kind: SampleKind::Stub,
        late: true,
    });
    assert!(v.paused);
    assert!(!v.over_budget);
    assert_eq!(s.heartbeat().frames(), before, "observe does not step");
    assert_eq!(s.report().stub.pauses, 1);
    assert!(s.report().late_is_pause);
}

#[test]
fn tick_late_and_short_pcm_pause_and_do_not_catch_up() {
    let mut s = Symphony::granted_rika();
    let pcm = silence();
    let (out, v) = s.tick(&frame(&pcm, true));
    assert!(out.paused);
    assert!(v.paused);
    assert_eq!(v.kind, SampleKind::Measured);
    assert_eq!(v.catch_up, 0);
    assert_eq!(s.heartbeat().paused_frames(), 1);
    assert_eq!(s.heartbeat().frames(), 1);

    let short = vec![0f32; FRAME_SAMPLES / 2];
    let (out, v) = s.tick(&frame(&short, false));
    assert!(out.paused);
    assert!(v.paused);
    assert_eq!(s.heartbeat().frames(), 2);
    assert_eq!(s.heartbeat().paused_frames(), 2);
    assert_eq!(s.catch_up_steps(), 0);
    let measured = s.report().measured;
    assert_eq!(measured.n, 2);
    assert_eq!(measured.pauses, 2);
    assert_eq!(measured.kind, SampleKind::Measured);
    assert_eq!(
        s.report().stub.n,
        0,
        "a measured tick must not land in the stub series"
    );
}

#[test]
fn measured_window_holds_p1_and_p3_on_this_process() {
    let mut s = Symphony::granted_rika();
    let report = s.run_budget(64);
    assert!(!report.product_claim);
    assert!(report.late_is_pause);
    assert_eq!(report.catch_up_steps, 0);
    assert_eq!(report.stub.n, 0);
    let m = &report.measured;
    assert_eq!(m.kind, SampleKind::Measured);
    assert_eq!(m.n, 64);
    assert_eq!(m.p3_instrument, "instant");
    assert_eq!(
        m.p1_pass,
        Some(true),
        "P1 note {} cpu {:?}",
        m.p1_note,
        m.cpu_ns
    );
    assert!(
        m.p1_instrument == "schedstat" || m.p1_instrument == "stat",
        "{}",
        m.p1_instrument
    );
    assert_eq!(m.p3_under_budget, Some(true), "p99 {:?} ns", m.p99_ns);
    let p99 = m.p99_ns.unwrap();
    assert!(p99 < P1_BUDGET_NS, "p99 {p99} ns is not under 800 µs");
    assert!(p99 < TICK_NS);
    assert_eq!(m.pauses, 0, "silence frames are not late");
    assert!(m.cpu_ns.unwrap() > 0, "process CPU counter did not advance");
    assert_eq!(
        s.heartbeat().frames(),
        4 + 64,
        "warmup steps plus the window, no extras"
    );
}

#[test]
fn a_second_window_does_not_pretend_the_span_matches() {
    let mut s = Symphony::granted_rika();
    let first = s.run_budget(8);
    assert_eq!(first.measured.p1_pass, Some(true));
    let second = s.run_budget(8);
    assert_eq!(second.measured.n, 8, "the second window must not append");
    assert_eq!(second.measured.p1_pass, Some(true));
    assert_eq!(second.measured.cpu_ns, first.measured.cpu_ns);
}

#[test]
fn gate_hook_labels_stub_and_measured_separately() {
    let report = gate_report(32);
    assert!(!PRODUCT_CLAIM);
    assert!(!report.product_claim);
    assert!(report.late_is_pause);
    assert_eq!(report.catch_up_steps, 0);
    assert_eq!(report.stub.kind, SampleKind::Stub);
    assert_eq!(report.measured.kind, SampleKind::Measured);
    assert_eq!(report.stub.p1_pass, Some(true));
    assert_eq!(report.stub.pauses, 1);
    assert_eq!(report.stub.p99_ns, Some(50_000));
    assert!(report.stub.p99_ns.unwrap() < P3_LL_NS);
    assert_eq!(
        report.measured.p1_pass,
        Some(true),
        "{}",
        report.measured.p1_note
    );
    assert_eq!(
        report.measured.p3_under_budget,
        Some(true),
        "p99 {:?}",
        report.measured.p99_ns
    );
    assert!(report.measured.p99_ns.unwrap() < P1_BUDGET_NS);
    let json = report.to_json();
    assert!(json.contains("\"engine\":\"v6-one-percent\""));
    assert!(json.contains("\"product_claim\":false"));
    assert!(json.contains("\"kind\":\"stub\""));
    assert!(json.contains("\"kind\":\"measured\""));
    assert!(!json.contains("\"product_claim\":true"));
    let parsed_ok = json.starts_with('{') && json.ends_with('}');
    assert!(parsed_ok);
}
