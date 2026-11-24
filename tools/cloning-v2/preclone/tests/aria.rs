// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Aria blend gates. Profiles are generated here. No recording, no audio file.

use std::fs;
use std::path::Path;
use std::process::Command;

use galion_preclone::blend::{
    baseline_distance, design_aria, design_from_profiles, parse_source, parse_spec, source_bytes,
    spec_bytes, SourceConsent, SourceProfile, MIN_BASELINE_DISTANCE, TARGET_AGE_YEARS,
};
use galion_preclone::{analyse, profile_bytes, synth, Request, VoiceProfile};

fn voice(f0: f64, f1: f64, f2: f64, f3: f64, centroid: f64, hnr: f64, voiced: f64) -> VoiceProfile {
    VoiceProfile {
        f0_hz: f0,
        f1_hz: f1,
        f2_hz: f2,
        f3_hz: f3,
        spectral_centroid_hz: centroid,
        hnr_db: hnr,
        voiced_fraction: voiced,
        kept_samples: 16_000,
    }
}

/// Synthetic warmth role. Numbers, not a person.
fn warmth() -> VoiceProfile {
    voice(196.0, 560.0, 1700.0, 2800.0, 1900.0, 12.0, 0.68)
}

/// Synthetic calm role. Numbers, not a person.
fn calm() -> VoiceProfile {
    voice(110.0, 400.0, 1350.0, 2400.0, 1250.0, 20.0, 0.82)
}

fn marked(consent: SourceConsent, voice: VoiceProfile) -> SourceProfile {
    SourceProfile { consent, voice }
}

fn fictional(voice: VoiceProfile) -> SourceProfile {
    marked(SourceConsent::FictionalReference, voice)
}

fn between(x: f64, a: f64, b: f64) -> bool {
    let lo = a.min(b);
    let hi = a.max(b);
    x > lo && x < hi
}

#[test]
fn blend_lands_inside_the_parent_ranges() {
    let spec = design_aria(&fictional(warmth()), &fictional(calm())).unwrap();
    let sam = &spec.samantha.baseline;
    let jar = &spec.jarvis.baseline;
    let child = &spec.baseline;
    assert!(
        between(child.f0_centre_hz, sam.f0_centre_hz, jar.f0_centre_hz),
        "f0 {}",
        child.f0_centre_hz
    );
    assert!(between(child.f1_hz, sam.f1_hz, jar.f1_hz));
    assert!(between(child.f2_hz, sam.f2_hz, jar.f2_hz));
    assert!(between(child.f3_hz, sam.f3_hz, jar.f3_hz));
    assert!(between(
        child.spectral_centroid_hz,
        sam.spectral_centroid_hz,
        jar.spectral_centroid_hz
    ));
    assert!(between(child.hnr_db, sam.hnr_db, jar.hnr_db));
    assert!(between(child.breathiness, sam.breathiness, jar.breathiness));
    assert!(child.f0_lo_hz < child.f0_centre_hz && child.f0_centre_hz < child.f0_hi_hz);
    assert!((70.0..=400.0).contains(&child.f0_lo_hz));
    assert!((70.0..=400.0).contains(&child.f0_hi_hz));
    assert!(child.f0_span_hz > 0.0);
    assert!(child.f1_hz < child.f2_hz && child.f2_hz < child.f3_hz);
    assert!((0.0..=1.0).contains(&child.breathiness));
    assert!((0.80..=1.20).contains(&child.rate));
    assert_eq!(spec.age.target_years, TARGET_AGE_YEARS);
    assert_eq!(spec.age.target_years, 23.0);
    assert!(spec.age.shift_years < 0.0);
    assert_eq!(spec.emotion.pitch_centre_hz, child.f0_centre_hz);
    assert_eq!(spec.emotion.persona_id, "aria");
    assert_eq!(spec.emotion.voice_id, "aria-steady");
    assert!((-1.0..=1.0).contains(&spec.emotion.valence));
    assert!((0.0..=1.0).contains(&spec.emotion.arousal));
    assert!((0.0..=1.0).contains(&spec.emotion.dominance));
    assert!((-1.0..=1.0).contains(&spec.emotion.approach));
    assert!((spec.emotion.valence - 0.2488).abs() < 1e-6);
    assert!((spec.emotion.arousal - 0.352).abs() < 1e-6);
    assert!((spec.emotion.dominance - 0.472).abs() < 1e-6);
    assert!((spec.emotion.approach - 0.234).abs() < 1e-6);
    assert!(spec.distance_samantha >= MIN_BASELINE_DISTANCE);
    assert!(spec.distance_jarvis >= MIN_BASELINE_DISTANCE);
    let d_s = baseline_distance(child, sam);
    let d_j = baseline_distance(child, jar);
    assert!((spec.distance_samantha - d_s).abs() < 5e-7, "{d_s}");
    assert!((spec.distance_jarvis - d_j).abs() < 5e-7, "{d_j}");
    let text = String::from_utf8(spec_bytes(&spec)).unwrap();
    assert!(text.contains("\"schema\": \"galion-voice-design/1\""));
    assert!(text.contains("\"compute\": \"cpu\""));
    assert!(text.contains("\"not_a_person\": true"));
    assert!(text.contains("\"feeds\": \"galion-emotion-core.VoiceBaseline\""));
    assert!(text.contains("\"target_years\": 23"));
    assert!(text.contains("playful"));
    assert!(text.contains("honest"));
    assert!(text.contains("emotionally-attentive"));
    assert!(text.contains("steady-at-work"));
    assert!(!text.contains(".wav"));

    let owned = design_aria(
        &marked(SourceConsent::OwnVoice, warmth()),
        &marked(SourceConsent::OwnVoice, calm()),
    )
    .unwrap();
    assert!(owned.distance_samantha >= MIN_BASELINE_DISTANCE);
    assert_ne!(spec_bytes(&owned), spec_bytes(&spec));
}

#[test]
fn guard_refuses_a_result_too_close_to_a_parent() {
    let sam = fictional(warmth());
    let err = design_aria(&sam, &sam).unwrap_err();
    assert!(err.contains("identity distance"), "{err}");
    assert!(err.contains("minimum"), "{err}");

    let mut near = warmth();
    near.f0_hz += 1.0;
    near.f1_hz += 5.0;
    let err = design_aria(&sam, &fictional(near)).unwrap_err();
    assert!(err.contains("identity distance"), "{err}");
    assert!(err.contains("refused"), "{err}");
}

#[test]
fn refuses_a_source_without_consent() {
    let good = String::from_utf8(source_bytes(&fictional(calm()))).unwrap();
    let bare = r#"{
      "schema": "galion-preclone/1",
      "voice": {
        "f0_hz": 196, "f1_hz": 560, "f2_hz": 1700, "f3_hz": 2800,
        "spectral_centroid_hz": 1900, "hnr_db": 12, "voiced_fraction": 0.68
      }
    }"#;
    let err = design_from_profiles(bare, &good).unwrap_err();
    assert!(err.contains("samantha-slot"), "{err}");
    assert!(err.contains("refused"), "{err}");
    assert!(err.contains("consent"), "{err}");

    let err = design_from_profiles(&good, bare).unwrap_err();
    assert!(err.contains("jarvis-slot"), "{err}");
    assert!(err.contains("consent"), "{err}");
}

#[test]
fn refuses_a_real_third_party() {
    let mut bad = String::from_utf8(source_bytes(&fictional(warmth()))).unwrap();
    bad = bad.replace("fictional-reference", "third-party");
    let good = String::from_utf8(source_bytes(&fictional(calm()))).unwrap();
    let err = design_from_profiles(&bad, &good).unwrap_err();
    assert!(
        err.contains("real third-party voices are never allowed"),
        "{err}"
    );

    bad = String::from_utf8(source_bytes(&fictional(calm()))).unwrap();
    bad = bad.replace("fictional-reference", "cc0-donor");
    let err = design_from_profiles(&good, &bad).unwrap_err();
    assert!(err.contains("jarvis-slot"), "{err}");
    assert!(
        err.contains("real third-party voices are never allowed"),
        "{err}"
    );

    let unknown = good.replace("fictional-reference", "public-web");
    let err = design_from_profiles(&unknown, &good).unwrap_err();
    assert!(err.contains("not an allowed source"), "{err}");
}

#[test]
fn same_inputs_write_the_same_bytes() {
    let sam = fictional(warmth());
    let jar = fictional(calm());
    let once = spec_bytes(&design_aria(&sam, &jar).unwrap());
    let twice = spec_bytes(&design_aria(&sam, &jar).unwrap());
    assert_eq!(once, twice);

    let sam_json = String::from_utf8(source_bytes(&sam)).unwrap();
    let jar_json = String::from_utf8(source_bytes(&jar)).unwrap();
    assert_eq!(source_bytes(&sam), sam_json.as_bytes());
    let from_json = spec_bytes(&design_from_profiles(&sam_json, &jar_json).unwrap());
    let from_json_again = spec_bytes(&design_from_profiles(&sam_json, &jar_json).unwrap());
    assert_eq!(from_json, from_json_again);
    assert_eq!(from_json, once);

    let parsed = parse_source(&sam_json).unwrap();
    assert_eq!(parsed.consent, SourceConsent::FictionalReference);
    assert_eq!(parsed.voice.f0_hz, sam.voice.f0_hz);
}

#[test]
fn spec_roundtrip() {
    let spec = design_aria(&fictional(warmth()), &fictional(calm())).unwrap();
    let bytes = spec_bytes(&spec);
    let text = std::str::from_utf8(&bytes).unwrap();
    let back = parse_spec(text).unwrap();
    assert_eq!(spec_bytes(&back), bytes);
    assert_eq!(back.emotion.persona_id, "aria");
    assert_eq!(back.emotion.voice_id, "aria-steady");
    assert_eq!(back.age.target_years, 23.0);
    assert_eq!(back.samantha.consent, SourceConsent::FictionalReference);
    assert_eq!(back.jarvis.consent, SourceConsent::FictionalReference);
    assert!((back.baseline.f0_centre_hz - spec.baseline.f0_centre_hz).abs() < 1e-6);
    assert!((back.emotion.pitch_centre_hz - spec.emotion.pitch_centre_hz).abs() < 1e-6);
    assert!((back.distance_samantha - spec.distance_samantha).abs() < 1e-6);
    assert!((back.distance_jarvis - spec.distance_jarvis).abs() < 1e-6);
    assert_eq!(back.baseline.timbre, spec.baseline.timbre);
}

#[test]
fn a_preclone_profile_without_consent_is_refused() {
    let samples = synth::formant_voice(
        16_000,
        16_000,
        140.0,
        0.05,
        &[(500.0, 80.0), (1500.0, 110.0), (2500.0, 150.0)],
    );
    let profile = analyse(&Request {
        samples: &samples,
        sample_rate_hz: 16_000,
        target_f0_hz: Some(140.0),
        moments: &[],
    })
    .unwrap();
    let text = String::from_utf8(profile_bytes(&profile)).unwrap();
    let err = parse_source(&text).unwrap_err();
    assert!(err.contains("refused"), "{err}");
    assert!(err.contains("consent"), "{err}");

    let marked = SourceProfile::from_profile(SourceConsent::FictionalReference, &profile);
    let other = fictional(calm());
    let spec = design_aria(&marked, &other).unwrap();
    assert!(spec.distance_samantha >= MIN_BASELINE_DISTANCE);
    assert_eq!(spec.emotion.persona_id, "aria");
}

#[test]
fn cli_writes_the_library_bytes() {
    let dir = std::env::temp_dir().join(format!("galion-aria-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let warmth_path = dir.join("warmth-profile.json");
    let calm_path = dir.join("calm-profile.json");
    let out_path = dir.join("aria-spec.json");
    let sam = fictional(warmth());
    let jar = fictional(calm());
    fs::write(&warmth_path, source_bytes(&sam)).unwrap();
    fs::write(&calm_path, source_bytes(&jar)).unwrap();
    let mut bin = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    bin.push(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });
    bin.push("galion-aria-blend");
    let status = Command::new(&bin)
        .arg("--samantha")
        .arg(&warmth_path)
        .arg("--jarvis")
        .arg(&calm_path)
        .arg("--out")
        .arg(&out_path)
        .status()
        .unwrap();
    assert!(status.success(), "{status}");
    let from_cli = fs::read(&out_path).unwrap();
    let from_lib = spec_bytes(&design_aria(&sam, &jar).unwrap());
    assert_eq!(from_cli, from_lib);
    let text = String::from_utf8(from_cli).unwrap();
    assert!(!text.contains("warmth-profile"));
    assert!(!text.contains("calm-profile"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_stage_holds_no_audio() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    walk(root);
}

fn walk(dir: &Path) {
    for ent in fs::read_dir(dir).unwrap() {
        let path = ent.unwrap().path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            walk(&path);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        assert!(
            !matches!(
                ext.as_str(),
                "wav" | "wave" | "mp3" | "flac" | "ogg" | "opus" | "m4a" | "aac"
            ),
            "voice audio must not be committed: {}",
            path.display()
        );
    }
}
