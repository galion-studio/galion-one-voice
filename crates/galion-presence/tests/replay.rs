// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
use galion_presence::backend::{AffectOut, FrameIn, FrameOut, MockBackend, MockConfig, ReplayBackend};
use galion_presence::replay::Status;
use galion_presence::spec::{parse_presence, PresenceFile};
use galion_presence::{percentile, run, Verdict};

const SPEC: &str = include_str!("../../../specs/presence.json");
const LOCKS: &str = include_str!("../../../LOCKS.toml");
/// Golden digest of the shipped presence.json on the default mock. Update only with a ledger entry.
const GOLDEN_DIGEST: &str = "9f657f360c565dbf";

fn file() -> PresenceFile {
    parse_presence(SPEC).expect("shipped presence.json must validate")
}

fn mock(cfg: MockConfig) -> MockBackend {
    MockBackend::new(cfg)
}

#[test]
fn shipped_spec_is_green_on_default_mock() {
    let f = file();
    assert!(f.specs.len() >= 6);
    let r = run(&f, &mut mock(MockConfig::default()));
    assert_eq!(r.verdict, Verdict::Green, "{}", r.summary());
    let p1 = &r.specs[0];
    assert_eq!(p1.metrics.barge_in_ms, Some(80.0));
    assert_eq!(r.onset_p50_ms, Some(240.0));
    let p3 = r.specs.iter().find(|s| s.id.starts_with("P3")).unwrap();
    assert_eq!(p3.metrics.e_to_a_frames, Some(1.0));
    assert_eq!(p3.metrics.attunement, Some(1.0));
}

#[test]
fn replay_is_deterministic_and_matches_golden() {
    let f = file();
    let a = run(&f, &mut mock(MockConfig::default()));
    let b = run(&f, &mut mock(MockConfig::default()));
    assert_eq!(a.deterministic_json(), b.deterministic_json());
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.digest(), GOLDEN_DIGEST, "golden changed:\n{}", a.summary());
    // reusing one backend across runs must not leak state
    let mut m = mock(MockConfig::default());
    let c1 = run(&f, &mut m).digest();
    let c2 = run(&f, &mut m).digest();
    assert_eq!(c1, c2);
}

#[test]
fn slow_yield_fails_barge_in() {
    let r = run(&file(), &mut mock(MockConfig { yield_frames: 4, ..Default::default() }));
    assert_eq!(r.verdict, Verdict::Red);
    let p1 = &r.specs[0];
    assert_eq!(p1.metrics.barge_in_ms, Some(320.0));
    assert!(p1.must.iter().any(|c| c.assertion.metric == "barge_in_ms" && c.status == Status::Fail));
}

#[test]
fn slow_endpoint_fails_onset_p50() {
    let r = run(&file(), &mut mock(MockConfig { endpoint_frames: 5, ..Default::default() }));
    assert_eq!(r.verdict, Verdict::Red);
    assert_eq!(r.onset_p50_ms, Some(400.0));
    assert!(r.run_must.iter().any(|c| c.assertion.metric == "onset_p50_ms" && c.status == Status::Fail));
}

#[test]
fn slow_affect_fails_e_to_a_and_attunement() {
    let r = run(&file(), &mut mock(MockConfig { affect_lag_frames: 3, ..Default::default() }));
    assert_eq!(r.verdict, Verdict::Red);
    let p3 = r.specs.iter().find(|s| s.id.starts_with("P3")).unwrap();
    assert_eq!(p3.metrics.e_to_a_frames, Some(3.0));
    assert!(p3.metrics.attunement.unwrap() < 0.9);
}

#[test]
fn consent_and_prohibited_purpose_freeze_affect() {
    let r = run(&file(), &mut mock(MockConfig::default()));
    for id in ["P4", "P5", "P2"] {
        let s = r.specs.iter().find(|s| s.id.starts_with(id)).unwrap();
        assert_eq!(s.metrics.affect_out_changes, 0.0, "{id}");
        assert_eq!(s.metrics.attunement, None, "{id}: attunement undefined without consent");
    }
}

#[test]
fn lean_in_is_pending_on_mock() {
    let r = run(&file(), &mut mock(MockConfig::default()));
    let p6 = r.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(p6.metrics.lean_in_ms, None);
    assert_eq!(p6.metrics.late_frames, 1.0);
    let c = p6.should.iter().find(|c| c.assertion.metric == "lean_in_ms").unwrap();
    assert_eq!(c.status, Status::Pending);
}

/// Test double: a backend that claims Rika and leans in `delay` frames after the first breath frame.
struct FakeRika {
    inner: MockBackend,
    delay: Option<u32>,
    breath_at: Option<u32>,
}
impl ReplayBackend for FakeRika {
    fn name(&self) -> &'static str {
        "fake-rika"
    }
    fn supports_rika(&self) -> bool {
        true
    }
    fn reset(&mut self) {
        self.inner.reset();
        self.breath_at = None;
    }
    fn step(&mut self, f: &FrameIn) -> FrameOut {
        if f.you == galion_presence::spec::YouState::Breath && self.breath_at.is_none() {
            self.breath_at = Some(f.index);
        }
        let mut o = self.inner.step(f);
        o.rika_lean_in = matches!((self.delay, self.breath_at), (Some(d), Some(b)) if f.index >= b + d);
        o
    }
}

#[test]
fn lean_in_measured_when_backend_supports_rika() {
    let f = file();
    let mut ok = FakeRika { inner: mock(MockConfig::default()), delay: Some(2), breath_at: None };
    let r = run(&f, &mut ok);
    let p6 = r.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(p6.metrics.lean_in_ms, Some(160.0));
    assert_eq!(p6.should.iter().find(|c| c.assertion.metric == "lean_in_ms").unwrap().status, Status::Pass);

    let mut never = FakeRika { inner: mock(MockConfig::default()), delay: None, breath_at: None };
    let r = run(&f, &mut never);
    let p6 = r.specs.iter().find(|s| s.id.starts_with("P6")).unwrap();
    assert_eq!(p6.should.iter().find(|c| c.assertion.metric == "lean_in_ms").unwrap().status, Status::NotMeasured);
}

#[test]
fn late_frame_is_a_pause_not_a_glitch() {
    let mut m = mock(MockConfig::default());
    let base = FrameIn { index: 0, you: galion_presence::spec::YouState::Silence, affect_in: None, adapt: false, late: false, me_planned: true };
    let a = m.step(&base);
    assert!(a.me_voiced);
    let late = m.step(&FrameIn { index: 1, late: true, me_planned: false, ..base });
    assert_eq!(late, a, "late frame holds the previous output");
    assert_eq!(late.affect_out, AffectOut::NEUTRAL_WARM);
}

fn with(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "fixture edit target missing: {from}");
    src.replacen(from, to, 1)
}

#[test]
fn validator_rejects_engagement_everywhere() {
    let e = parse_presence(&with(SPEC, "\"barge_in_ms <= 200\"", "\"engagement_score >= 0.5\"")).unwrap_err();
    assert!(e.contains("engagement"), "{e}");
    let e = parse_presence(&with(SPEC, "\"description\": \"Galion is mid", "\"session_length_s\": 1, \"description\": \"Galion is mid")).unwrap_err();
    assert!(e.contains("session_length"), "{e}");
    let e = parse_presence(&with(SPEC, "\"late_frames\": \"scripted", "\"user_retention\": \"x\", \"late_frames\": \"scripted")).unwrap_err();
    assert!(e.contains("retention"), "{e}");
}

#[test]
fn validator_requires_attunement_and_structure() {
    assert!(parse_presence(&with(SPEC, "\"attunement\": \"min(", "\"attune_x\": \"min(")).unwrap_err().contains("attunement"));
    assert!(parse_presence(&with(SPEC, "\"frame_ms\": 80", "\"frame_ms\": 40")).is_err());
    assert!(parse_presence(&with(SPEC, "\"verdict_budget_s\": 300", "\"verdict_budget_s\": 301")).is_err());
    assert!(parse_presence(&with(SPEC, "\"barge_in_ms <= 200\"", "\"wer <= 0.1\"")).unwrap_err().contains("unknown metric"));
    assert!(parse_presence(&with(SPEC, "\"onset_ms <= 300\", \"me_onsets", "\"onset_p50_ms <= 300\", \"me_onsets")).is_err(), "aggregates are run-level only");
    assert!(parse_presence(&with(SPEC, "\"should\": [\"barge_in_ms <= 80\"]", "\"x\": []")).unwrap_err().contains("should"));
    assert!(parse_presence(&with(SPEC, "\"schema\": \"galion-presence/v1\"", "\"schema\": \"v0\"")).is_err());
    assert!(parse_presence(&with(SPEC, "{\"from\": 12, \"to\": 20", "{\"from\": 20, \"to\": 12")).is_err());
    let empty = r#"{"schema":"galion-presence/v1","frame_ms":80,"verdict_budget_s":300,
      "targets":{"barge_in_max_ms":200,"onset_p50_max_ms":300,"affect_to_voice_max_frames":2,"affect_to_voice_max_ms":160},
      "metrics":{"attunement":"x"},"must":[],"should":[],"specs":[]}"#;
    assert!(parse_presence(empty).unwrap_err().contains("zero specs"));
}

#[test]
fn targets_match_locks() {
    let f = file();
    let lock = |k: &str| -> f64 {
        let line = LOCKS.lines().find(|l| l.trim_start().starts_with(&format!("{k} "))).unwrap_or_else(|| panic!("LOCKS.toml missing {k}"));
        line.split('=').nth(1).unwrap().split('#').next().unwrap().trim().parse().unwrap()
    };
    assert_eq!(f.targets.barge_in_max_ms, lock("barge_in_max_ms"));
    assert_eq!(f.targets.onset_p50_max_ms, lock("onset_p50_max_ms"));
    assert_eq!(f.targets.affect_to_voice_max_ms, lock("affect_to_voice_max_ms"));
    assert_eq!(f.targets.affect_to_voice_max_frames as f64, lock("affect_to_voice_max_frames"));
    assert_eq!(f.verdict_budget_s as f64, lock("verdict_budget_s"));
    assert_eq!(f.frame_ms as f64, lock("frame_ms"));
}

#[test]
fn percentile_nearest_rank() {
    assert_eq!(percentile(&[], 50.0), None);
    assert_eq!(percentile(&[1.0], 95.0), Some(1.0));
    assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 50.0), Some(2.0));
    assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 95.0), Some(4.0));
}
