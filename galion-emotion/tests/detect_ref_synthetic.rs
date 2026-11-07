// Galion Voice — Perpetual Open MIT License
//! Inbound detect against the hand-written synthetic reference track.
//! PCM is generated in the test. No wav files, no network, no model.

use galion_emotion::{
    parse_reference_track, Affect, AffectDetector, AffectEstimator, ConsentDecision,
    DefaultConsentGate, DetectError, EmotionFrame, EstimatorKind, GatedDetector, LoudnessEstimator,
    Purpose, RefEntry, RefLabel, ReferenceEstimator,
};

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/affect-ref-synthetic.txt"
));

const SR: u32 = 24_000;
const WINDOW: usize = 1_920;

fn allow() -> DefaultConsentGate {
    let mut g = DefaultConsentGate::default();
    g.grant(0, "en", Purpose::PersonalAssistant);
    g
}

fn open() -> GatedDetector<ReferenceEstimator> {
    let estimator = ReferenceEstimator::from_str_track(FIXTURE).expect("synthetic fixture parses");
    let mut detector = GatedDetector::new(estimator);
    detector.smoothing = 1.0;
    detector
}

fn silence() -> Vec<f32> {
    vec![0.0; WINDOW]
}

fn detect_fresh(pcm: &[f32], t_ms: u32) -> (GatedDetector<ReferenceEstimator>, EmotionFrame) {
    let mut detector = open();
    let gate = allow();
    let frame = detector
        .detect(pcm, SR, t_ms, &gate, Purpose::PersonalAssistant)
        .expect("consented detect");
    (detector, frame)
}

fn track() -> Vec<RefEntry> {
    parse_reference_track(FIXTURE).expect("synthetic fixture parses")
}

fn near(got: f32, want: f32) {
    assert!((got - want).abs() < 1e-5, "{got} != {want}");
}

fn blended_arousal(label: RefLabel, loud_arousal: f32) -> f32 {
    let (_, arousal, _) = label.vad().expect("labelled span");
    0.75 * arousal + 0.25 * loud_arousal
}

#[test]
fn fixture_parses_six_synthetic_entries() {
    assert!(FIXTURE.contains("HAND-WRITTEN SYNTHETIC DATA"));
    assert!(!FIXTURE.to_ascii_lowercase().contains(".wav"));

    let entries = track();
    let data_lines = FIXTURE
        .lines()
        .filter(|line| !line.split('#').next().unwrap_or("").trim().is_empty())
        .count();
    assert_eq!(data_lines, 6);
    assert_eq!(entries.len(), 6);

    let want = [
        (0u32, RefLabel::Neutral, 0.85f32),
        (480, RefLabel::Happy, 0.72),
        (1200, RefLabel::Sad, 0.64),
        (1920, RefLabel::Angry, 0.70),
        (2640, RefLabel::Unknown, 0.30),
        (3200, RefLabel::Neutral, 0.90),
    ];
    for (entry, (t_ms, label, posterior)) in entries.iter().zip(want) {
        assert_eq!(entry.t_ms, t_ms);
        assert_eq!(entry.label, label);
        near(entry.posterior, posterior);
        assert!(entry.label.vad().is_some() || entry.label == RefLabel::Unknown);
    }
    for pair in entries.windows(2) {
        assert!(pair[0].t_ms < pair[1].t_ms);
    }

    let estimator = ReferenceEstimator::from_str_track(FIXTURE).unwrap();
    assert_eq!(estimator.kind(), EstimatorKind::OfflineReference);
    assert_eq!(estimator.name(), "offline-reference");
    assert_eq!(estimator.kind().label(), "offline-reference");
    assert_eq!(estimator.entry_at(0).unwrap().label, RefLabel::Neutral);
    assert_eq!(estimator.entry_at(479).unwrap().label, RefLabel::Neutral);
    assert_eq!(estimator.entry_at(480).unwrap().label, RefLabel::Happy);
    assert_eq!(estimator.entry_at(3199).unwrap().label, RefLabel::Unknown);
    assert_eq!(estimator.entry_at(3200).unwrap().label, RefLabel::Neutral);
    assert_eq!(estimator.entry_at(u32::MAX).unwrap().t_ms, 3200);
}

#[test]
fn silence_boundaries_follow_the_label_table() {
    let pcm = silence();
    let mut loud = LoudnessEstimator::default();
    let loud_est = loud.estimate(&pcm, SR, 0);
    assert_eq!(loud_est.arousal, 0.0, "silence must not invent loudness");

    let cases = [
        (0u32, RefLabel::Neutral, Affect::Calm),
        (480, RefLabel::Happy, Affect::Joy),
        (1200, RefLabel::Sad, Affect::Tired),
        (1920, RefLabel::Angry, Affect::Angry),
        (3200, RefLabel::Neutral, Affect::Calm),
    ];
    for (t_ms, label, affect) in cases {
        let (detector, frame) = detect_fresh(&pcm, t_ms);
        let (valence, _, dominance) = label.vad().unwrap();
        let entry = detector.estimator.entry_at(t_ms).unwrap();
        assert_eq!(entry.label, label);
        assert_eq!(frame.seq, 0);
        assert_eq!(frame.t_ms, t_ms);
        assert_eq!(frame.dur_ms, 80);
        assert!(detector.valence_known());
        assert_eq!(frame.valence, valence);
        near(frame.arousal, blended_arousal(label, 0.0));
        assert_eq!(frame.arousal, blended_arousal(label, loud_est.arousal));
        assert_eq!(frame.dominance, dominance);
        near(frame.confidence, entry.posterior);
        assert_eq!(
            frame.label(),
            affect,
            "{label:?} at {t_ms} with silence -> {frame:?}"
        );
    }

    let (detector, frame) = detect_fresh(&pcm, 2640);
    let entry = detector.estimator.entry_at(2640).unwrap();
    assert_eq!(entry.label, RefLabel::Unknown);
    near(entry.posterior, 0.30);
    assert!(!detector.valence_known());
    assert_eq!(frame.valence, 0.0);
    assert_eq!(frame.confidence, 0.0);
    assert_eq!(frame.arousal, loud_est.arousal);
    assert_eq!(frame.dominance, loud_est.dominance);
    assert_eq!(frame.label(), Affect::Neutral);
}

#[test]
fn label_holds_until_the_next_fixture_timestamp() {
    let pcm = silence();
    let mut detector = open();
    let gate = allow();
    let entries = track();
    for (i, t_ms) in (0u32..=3_280).step_by(80).enumerate() {
        let frame = detector
            .detect(&pcm, SR, t_ms, &gate, Purpose::PersonalAssistant)
            .expect("hold");
        let (fresh, raw) = detect_fresh(&pcm, t_ms);
        let active = entries
            .iter()
            .rposition(|entry| entry.t_ms <= t_ms)
            .unwrap();
        assert_eq!(
            detector.estimator.entry_at(t_ms).unwrap().t_ms,
            entries[active].t_ms
        );
        if active + 1 < entries.len() {
            assert!(t_ms < entries[active + 1].t_ms);
        }
        assert_eq!(frame.seq, i as u32);
        assert_eq!(frame.t_ms, raw.t_ms);
        assert_eq!(frame.dur_ms, raw.dur_ms);
        // smoothing 1.0 still passes through EMA, so a held span is not bit-identical.
        near(frame.valence, raw.valence);
        near(frame.arousal, raw.arousal);
        near(frame.dominance, raw.dominance);
        near(frame.confidence, raw.confidence);
        assert_eq!(frame.label(), raw.label());
        assert_eq!(detector.valence_known(), fresh.valence_known());
    }
}

#[test]
fn window_length_does_not_change_the_fixture_valence() {
    let (_, long) = detect_fresh(&vec![0.0; WINDOW], 480);
    let (_, short) = detect_fresh(&vec![0.0; 480], 480);
    assert_eq!(long.dur_ms, 80);
    assert_eq!(short.dur_ms, 20);
    assert_eq!(long.valence, short.valence);
    assert_eq!(long.arousal, short.arousal);
    assert_eq!(long.dominance, short.dominance);
    assert_eq!(long.confidence, short.confidence);
    assert_eq!(long.label(), Affect::Joy);
}

#[test]
fn loud_window_nudges_arousal_and_not_valence() {
    let quiet = silence();
    let loud = vec![0.3; WINDOW];
    let mut loudness = LoudnessEstimator::default();
    let loud_arousal = loudness.estimate(&loud, SR, 0).arousal;
    assert!(
        loud_arousal > 0.9,
        "constant 0.3 should sit near full-scale rms, got {loud_arousal}"
    );

    let spans = [
        (0u32, RefLabel::Neutral, Affect::Calm, Affect::Neutral),
        (480, RefLabel::Happy, Affect::Joy, Affect::Excited),
        (1200, RefLabel::Sad, Affect::Tired, Affect::Sad),
        (1920, RefLabel::Angry, Affect::Angry, Affect::Angry),
        (3200, RefLabel::Neutral, Affect::Calm, Affect::Neutral),
    ];
    for (t_ms, label, quiet_affect, loud_affect) in spans {
        let (_, low) = detect_fresh(&quiet, t_ms);
        let (_, high) = detect_fresh(&loud, t_ms);
        let (valence, _, dominance) = label.vad().unwrap();
        assert_eq!(low.valence, valence);
        assert_eq!(high.valence, valence);
        assert_eq!(low.dominance, dominance);
        assert_eq!(high.dominance, dominance);
        assert_eq!(low.confidence, high.confidence);
        assert!(high.arousal > low.arousal, "{label:?} arousal did not rise");
        near(high.arousal, blended_arousal(label, loud_arousal));
        assert_eq!(low.label(), quiet_affect, "quiet {label:?} -> {low:?}");
        assert_eq!(high.label(), loud_affect, "loud {label:?} -> {high:?}");
    }

    let (_, unknown) = detect_fresh(&loud, 2640);
    assert_eq!(unknown.valence, 0.0);
    assert_eq!(unknown.confidence, 0.0);
    near(unknown.arousal, loud_arousal);
    assert_eq!(unknown.label(), Affect::Neutral);
}

#[test]
fn consent_deny_resets_reference_state() {
    let pcm = silence();
    let mut detector = open();
    detector.smoothing = 0.5;
    let gate = allow();
    let happy = detector
        .detect(&pcm, SR, 480, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(happy.seq, 0);
    assert_eq!(happy.valence, 0.7);
    assert!(detector.valence_known());

    let deny = DefaultConsentGate::default();
    let denied = detector
        .detect(&pcm, SR, 960, &deny, Purpose::PersonalAssistant)
        .unwrap_err();
    assert_eq!(
        denied,
        DetectError::NotPermitted(ConsentDecision::DenyNoConsent)
    );
    assert!(!detector.valence_known());

    let after = detector
        .detect(&pcm, SR, 0, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(after.seq, 1, "a refused window must not consume seq");
    assert_eq!(
        after.valence, 0.0,
        "withdrawn prev must not blend into the next allow"
    );
    assert!(detector.valence_known());

    let mut blocked = open();
    let mut workplace = DefaultConsentGate::default();
    workplace.grant(1, "en", Purpose::Workplace);
    let err = blocked
        .detect(&pcm, SR, 480, &workplace, Purpose::Workplace)
        .unwrap_err();
    assert_eq!(
        err,
        DetectError::NotPermitted(ConsentDecision::DenyProhibitedPurpose)
    );
    let err = blocked
        .detect(&pcm, SR, 480, &workplace, Purpose::Education)
        .unwrap_err();
    assert_eq!(
        err,
        DetectError::NotPermitted(ConsentDecision::DenyProhibitedPurpose)
    );

    let mut mismatch = open();
    let mut other = DefaultConsentGate::default();
    other.grant(1, "en", Purpose::Accessibility);
    let err = mismatch
        .detect(&pcm, SR, 1200, &other, Purpose::PersonalAssistant)
        .unwrap_err();
    assert_eq!(
        err,
        DetectError::NotPermitted(ConsentDecision::DenyPurposeMismatch)
    );
    assert!(!mismatch.valence_known());
}

#[test]
fn bad_input_does_not_advance_or_wipe() {
    let pcm = silence();
    let mut detector = open();
    let gate = allow();
    let ok = detector
        .detect(&pcm, SR, 1920, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(ok.seq, 0);
    assert!(detector.valence_known());
    assert_eq!(ok.label(), Affect::Angry);

    assert!(matches!(
        detector.detect(&[], SR, 1920, &gate, Purpose::PersonalAssistant),
        Err(DetectError::BadInput(_))
    ));
    assert!(matches!(
        detector.detect(&pcm, 0, 1920, &gate, Purpose::PersonalAssistant),
        Err(DetectError::BadInput(_))
    ));
    assert!(detector.valence_known());

    let next = detector
        .detect(&pcm, SR, 1200, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(next.seq, 1);
    assert_eq!(next.valence, RefLabel::Sad.vad().unwrap().0);
}

#[test]
fn trait_object_replays_the_same_fixture_frame() {
    let pcm = silence();
    let gate = allow();
    let (typed, direct) = detect_fresh(&pcm, 1920);
    let mut boxed: Box<dyn AffectDetector> = Box::new(open());
    let via_trait = boxed
        .detect(&pcm, SR, 1920, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(direct, via_trait);
    assert_eq!(via_trait.label(), Affect::Angry);
    assert!(typed.valence_known());

    let mut stub = galion_emotion::EnergyStubDetector::default();
    let energy = stub
        .detect(&pcm, SR, 1920, &gate, Purpose::PersonalAssistant)
        .unwrap();
    assert_eq!(energy.valence, 0.0);
    assert!(!stub.inner().valence_known());
    assert!(direct.valence < 0.0);
    near(direct.confidence, 0.70);
    boxed.reset();
}
