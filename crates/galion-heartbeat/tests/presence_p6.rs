// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! presence.json through the real heartbeat: P6 measures lean_in_ms when Rika is ON.
use galion_heartbeat::presence::RikaReplayBackend;
use galion_heartbeat::HeartbeatConfig;
use galion_presence::backend::{MockBackend, MockConfig};
use galion_presence::replay::Status;
use galion_presence::spec::parse_presence;
use galion_presence::{run, Verdict};

const SPEC: &str = include_str!("../../../specs/presence.json");

#[test]
fn p6_lean_in_measured_160ms_from_real_rika() {
    let f = parse_presence(SPEC).unwrap();
    let mut b = RikaReplayBackend::new();
    let r = run(&f, &mut b);
    assert_eq!(r.verdict, Verdict::Green, "{}", r.summary());
    let p6 = r.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(p6.metrics.lean_in_ms, Some(160.0));
    assert!(p6.metrics.rika_supported);
    let c = p6.should.iter().find(|c| c.assertion.metric == "lean_in_ms").unwrap();
    assert_eq!(c.status, Status::Pass);
    // P6 is the last spec, so the heartbeat's report is P6's: harness and RikaReport agree.
    assert!(r.specs.last().unwrap().id.starts_with("P6"));
    assert_eq!(b.heartbeat().lean_in_ms(), Some(160));
    // Spec file carries the RikaReport value.
    let spec_p6 = f.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(spec_p6.rika_lean_in_ms, Some(160.0));
    // Every other spec: identical barge/onset to the plain mock (Rika never hurts turn-taking).
    let m = run(&f, &mut MockBackend::new(MockConfig::default()));
    for (a, b) in r.specs.iter().zip(&m.specs) {
        assert_eq!(a.metrics.barge_in_ms, b.metrics.barge_in_ms, "{}", a.id);
        assert_eq!(a.metrics.onset_ms, b.metrics.onset_ms, "{}", a.id);
        assert_eq!(a.metrics.me_onsets_while_user_speaking, 0.0, "{}", a.id);
    }
    // No Rika in no-consent / prohibited specs.
    for id in ["P4", "P5"] {
        let s = r.specs.iter().find(|s| s.id.starts_with(id)).unwrap();
        assert_eq!(s.metrics.lean_in_ms, None, "{id}");
    }
}

#[test]
fn rika_off_backend_reports_pending_like_mock() {
    let f = parse_presence(SPEC).unwrap();
    let mut b = RikaReplayBackend::with_config(HeartbeatConfig::default());
    let r = run(&f, &mut b);
    let p6 = r.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(p6.metrics.lean_in_ms, None);
    assert_eq!(p6.should.iter().find(|c| c.assertion.metric == "lean_in_ms").unwrap().status, Status::Pending);
}
