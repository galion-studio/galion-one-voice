//! Cloning V2 branch A — reference fidelity.
//!
//! Selects and concatenates the best donor segments (SNR, voiced ratio, pitch coverage),
//! averages multi-reference speaker embeddings, and adapts conditioning for a ~6 s best
//! piece versus a 50 s ideal. Spectral-envelope match is an opt-in post layer.
//! Piper stays the TTS. This crate does not clone and does not evaluate consent.
//!
//! `CONSENT_EVALUATIONS` is zero on purpose. Clone-time consent remains one
//! `galion_consent::evaluate` call in `crates/galion-consent`.

#![forbid(unsafe_code)]

pub mod analyze;
pub mod audio;
pub mod embed;
pub mod envelope;
pub mod score;
pub mod select;
pub mod synth;

/// Fidelity prep and eval never call the consent gate.
pub const CONSENT_EVALUATIONS: u32 = 0;

pub use score::{emotion_match, evaluate, EvalInput, EvalReport};
pub use select::{prepare, Config, Prepared};

pub fn self_check() -> serde_json::Value {
    let vectors = vec![vec![3.0f32, 0.0], vec![0.0, 4.0]];
    let weights = vec![3.0f32, 1.0];
    let sr = audio::TARGET_SR;
    let clean = synth::harmonics(sr, 1.0, 180.0, 0.25);
    let noise = synth::lcg_noise(clean.len(), 0.08, 7);
    let short = short_case();
    let long = long_case();
    let cap = cap_case();
    let same_a = embed::embedding(&synth::harmonics(sr, 1.5, 150.0, 0.2), sr);
    let same_b = embed::embedding(&synth::harmonics(sr, 1.5, 150.0, 0.55), sr);
    let other = embed::embedding(&synth::harmonics(sr, 1.5, 280.0, 0.3), sr);
    let damaged = synth::emphasize_high(&clean, sr);
    let repaired = envelope::correct_envelope(&damaged, &clean, sr, 1.0);
    serde_json::json!({
        "schema": "galion-fidelity-self-check/1",
        "consent_evaluations": CONSENT_EVALUATIONS,
        "closed_form": {
            "vectors": vectors,
            "weights": null,
            "embedding_average": embed::average_embeddings(&vectors, None),
            "weighted_weights": weights,
            "embedding_average_weighted": embed::average_embeddings(&vectors, Some(&weights)),
            "category_blend": 0.9,
            "emb_sim": 0.8,
            "emotion_match": emotion_match(0.9, 0.8),
            "wer_ref": "the cat sat",
            "wer_hyp": "the cat sat down",
            "wer": score::word_error_rate("the cat sat", "the cat sat down"),
            "wer_exact_ref": "Hello, World!",
            "wer_exact_hyp": "hello world",
            "wer_exact": score::word_error_rate("Hello, World!", "hello world"),
            "segment_snr_db": 30.0,
            "segment_voiced": 1.0,
            "segment_coverage": 0.5,
            "segment_score": select::segment_score(30.0, 1.0, 0.5),
            "mos_snr_db": 30.0,
            "mos_voiced": 1.0,
            "mos_flatness": 0.0,
            "mos_clip": 0.0,
            "mos": score::mos_from_components(30.0, 1.0, 0.0, 0.0),
            "wer_gate": score::WER_GATE,
            "ecapa_gate_exclusive": score::ECAPA_GATE,
            "emotion_gate": score::EMOTION_GATE,
            "ecapa_at_threshold_passes": score::ecapa_pass(score::ECAPA_GATE),
            "ecapa_just_above_passes": score::ecapa_pass(0.7058),
            "wer_at_threshold_passes": score::wer_pass(score::WER_GATE),
            "wer_above_threshold_passes": score::wer_pass(0.019),
            "emotion_at_gate_passes": score::emotion_pass(score::EMOTION_GATE),
        },
        "audio": {
            "sine_f0_hz": analyze::median_f0(&clean, sr),
            "sine_voiced": analyze::voiced_ratio(&clean, sr),
            "noise_voiced": analyze::voiced_ratio(&noise, sr),
            "mos_sine": score::mos_proxy(&clean, sr),
            "mos_noise": score::mos_proxy(&noise, sr),
            "same_speaker_cosine": embed::cosine(&same_a, &same_b),
            "different_f0_cosine": embed::cosine(&same_a, &other),
            "short": plan_brief(&short),
            "long": plan_brief(&long),
            "cap": plan_brief(&cap),
            "envelope_ratio_clean": envelope::high_low_ratio(&clean, sr),
            "envelope_ratio_damaged": envelope::high_low_ratio(&damaged, sr),
            "envelope_ratio_repaired": envelope::high_low_ratio(&repaired, sr),
            "envelope_strength0_identical": envelope::correct_envelope(&damaged, &clean, sr, 0.0) == damaged,
        }
    })
}

fn plan_brief(prepared: &select::Prepared) -> serde_json::Value {
    let prefix = 1.2 * audio::TARGET_SR as f64;
    let n = (prefix as usize).min(prepared.samples.len());
    serde_json::json!({
        "mode": prepared.meta.mode,
        "input_s": prepared.meta.input_s,
        "assembled_s": prepared.meta.assembled_s,
        "lm_conditioning_s": prepared.meta.lm_conditioning_s,
        "speaker_token_s": prepared.meta.speaker_token_s,
        "speaker_pool_s": prepared.meta.speaker_pool_s,
        "repeated": prepared.meta.repeated,
        "recommended_timbre_strength": prepared.meta.recommended_timbre_strength,
        "timbre_applied": prepared.meta.timbre_applied,
        "segments": prepared.meta.segments.len(),
        "first_start_s": prepared.meta.segments.first().map(|s| s.start_s),
        "first_score": prepared.meta.segments.first().map(|s| s.score),
        "prefix_f0_hz": analyze::median_f0(&prepared.samples[..n], prepared.meta.sample_rate),
        "embedding_dim": prepared.meta.embedding_dim,
    })
}

pub fn short_case() -> select::Prepared {
    let sr = audio::TARGET_SR;
    let noise = synth::lcg_noise((sr as f32 * 3.0) as usize, 0.02, 1);
    let vowel = synth::harmonics(sr, 3.0, 180.0, 0.25);
    let input = synth::concat(&[noise, vowel]);
    select::prepare(&[(input, sr)], &select::Config::default()).expect("short case")
}

pub fn long_case() -> select::Prepared {
    let sr = audio::TARGET_SR;
    let noise = synth::lcg_noise((sr as f32 * 4.0) as usize, 0.02, 2);
    let low = synth::harmonics(sr, 4.0, 120.0, 0.12);
    let bright = synth::glide(sr, 4.0, 180.0, 280.0, 0.28);
    let input = synth::concat(&[noise, low, bright]);
    select::prepare(&[(input, sr)], &select::Config::default()).expect("long case")
}

pub fn cap_case() -> select::Prepared {
    let sr = audio::TARGET_SR;
    let tone = synth::harmonics(sr, 8.0, 200.0, 0.22);
    let mut cfg = select::Config::default();
    cfg.ideal_s = 3.0;
    select::prepare(&[(tone, sr)], &cfg).expect("cap case")
}

pub fn clean_vowel() -> Vec<f32> {
    synth::harmonics(audio::TARGET_SR, 3.0, 180.0, 0.25)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{ecapa_from_json, emotion_from_persona_json};
    use serde_json::json;

    #[test]
    fn consent_is_not_evaluated_here() {
        assert_eq!(CONSENT_EVALUATIONS, 0);
    }

    #[test]
    fn defaults_match_the_6s_and_50s_policy() {
        let cfg = select::Config::default();
        assert_eq!(cfg.ideal_s, 50.0);
        assert_eq!(cfg.lm_s, 6.0);
        assert_eq!(cfg.speaker_token_s, 10.0);
        assert_eq!(cfg.short_below_s, 8.0);
        assert!((score::EMOTION_GATE - 0.83).abs() < 1e-15);
    }

    #[test]
    fn persona_scorer_formula_is_the_one_we_apply() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scorers/persona/galion-persona/src/lib.rs"
        ))
        .unwrap();
        assert!(src.contains("0.70 * category_blend + 0.30 * emb_sim"));
        assert!(src.contains("pub const GATE: f64 = 0.83"));
        assert!(src.contains("CATEGORY_WEIGHT * category_blend + EMB_WEIGHT * emb_sim"));
    }

    #[test]
    fn sine_pitch_and_noise_voicing() {
        let sr = audio::TARGET_SR;
        let sine = synth::harmonics(sr, 1.0, 180.0, 0.25);
        let f0 = analyze::median_f0(&sine, sr).unwrap();
        assert!((f0 - 180.0).abs() < 5.0, "f0 {f0}");
        assert!(analyze::voiced_ratio(&sine, sr) > 0.8);
        let noise = synth::lcg_noise(sine.len(), 0.08, 9);
        assert!(analyze::voiced_ratio(&noise, sr) < 0.25);
    }

    #[test]
    fn embedding_average_matches_the_closed_form() {
        let vectors = vec![vec![3.0f32, 0.0], vec![0.0, 4.0]];
        let even = embed::average_embeddings(&vectors, None);
        let expect = std::f32::consts::FRAC_1_SQRT_2;
        assert!((even[0] - expect).abs() < 1e-5);
        assert!((even[1] - expect).abs() < 1e-5);
        let weighted = embed::average_embeddings(&vectors, Some(&[3.0, 1.0]));
        let n = (0.75f64 * 0.75 + 0.25 * 0.25).sqrt();
        assert!((weighted[0] as f64 - 0.75 / n).abs() < 1e-5);
        assert!((weighted[1] as f64 - 0.25 / n).abs() < 1e-5);
    }

    #[test]
    fn same_timbre_survives_gain_change() {
        let sr = audio::TARGET_SR;
        let a = embed::embedding(&synth::harmonics(sr, 1.5, 150.0, 0.15), sr);
        let b = embed::embedding(&synth::harmonics(sr, 1.5, 150.0, 0.55), sr);
        let c = embed::embedding(&synth::harmonics(sr, 1.5, 280.0, 0.3), sr);
        let same = embed::cosine(&a, &b).unwrap();
        let different = embed::cosine(&a, &c).unwrap();
        assert!(same > 0.95, "same {same}");
        assert!(different < same - 0.02, "different {different} same {same}");
        assert!((embed::l2(&a) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn short_clip_keeps_the_vowel_and_does_not_repeat() {
        let prepared = short_case();
        assert_eq!(prepared.meta.mode, "short");
        assert!(!prepared.meta.repeated);
        assert!(
            prepared.meta.assembled_s < 5.2,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(
            prepared.meta.assembled_s > 1.5,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(prepared.meta.assembled_s < 50.0);
        assert!((prepared.meta.recommended_timbre_strength - 0.35).abs() < 1e-9);
        assert!(!prepared.meta.timbre_applied);
        let first = &prepared.meta.segments[0];
        assert!(first.start_s >= 2.5, "start {}", first.start_s);
        assert!(first.voiced_ratio > 0.5);
        assert!(prepared.meta.lm_conditioning_s <= 6.0 + 1e-6);
        assert_eq!(prepared.meta.speaker_pool_s, prepared.meta.assembled_s);
    }

    #[test]
    fn clean_six_seconds_stays_short() {
        let sr = audio::TARGET_SR;
        let clip = synth::harmonics(sr, 6.0, 180.0, 0.22);
        let prepared = select::prepare(&[(clip, sr)], &select::Config::default()).unwrap();
        assert_eq!(prepared.meta.mode, "short");
        assert!(!prepared.meta.repeated);
        assert!(
            prepared.meta.assembled_s > 5.5,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(
            prepared.meta.assembled_s <= 6.05,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(prepared.meta.lm_conditioning_s <= 6.0 + 1e-6);
        assert!((prepared.meta.recommended_timbre_strength - 0.35).abs() < 1e-9);
    }

    #[test]
    fn long_reference_front_loads_the_best_segment() {
        let prepared = long_case();
        assert_eq!(prepared.meta.mode, "long");
        assert!(!prepared.meta.repeated);
        assert!((prepared.meta.recommended_timbre_strength - 0.55).abs() < 1e-9);
        assert!(
            prepared.meta.assembled_s <= 12.2,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(
            prepared.meta.assembled_s >= 5.0,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(prepared.meta.lm_conditioning_s <= 6.0 + 1e-6);
        let first = &prepared.meta.segments[0];
        assert!(first.start_s >= 7.5, "start {}", first.start_s);
        assert!(prepared.meta.segments.iter().all(|seg| seg.start_s >= 3.9));
        let sr = audio::TARGET_SR;
        let prefix = &prepared.samples[..sr as usize];
        assert!(analyze::voiced_ratio(prefix, sr) > 0.8);
        let f0 = analyze::median_f0(prefix, sr).unwrap();
        assert!((100.0..320.0).contains(&f0), "prefix f0 {f0}");
    }

    #[test]
    fn assembled_length_respects_the_cap() {
        let prepared = cap_case();
        assert_eq!(prepared.meta.mode, "long");
        assert!(
            prepared.meta.assembled_s <= 3.05,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(
            prepared.meta.assembled_s >= 1.8,
            "{}",
            prepared.meta.assembled_s
        );
        assert!(!prepared.meta.repeated);
    }

    #[test]
    fn multi_reference_average_stays_near_either_clip() {
        let sr = audio::TARGET_SR;
        let quiet = synth::harmonics(sr, 3.0, 180.0, 0.12);
        let loud = synth::harmonics(sr, 3.0, 180.0, 0.40);
        let prepared = select::prepare(
            &[(quiet.clone(), sr), (loud, sr)],
            &select::Config::default(),
        )
        .unwrap();
        assert_eq!(prepared.meta.inputs, 2);
        assert_eq!(prepared.meta.mode, "short");
        assert!(!prepared.meta.repeated);
        let one = embed::embedding(&quiet, sr);
        let cos = embed::cosine(&prepared.meta.embedding, &one).unwrap();
        assert!(cos > 0.9, "cosine {cos}");
    }

    #[test]
    fn envelope_strength_zero_is_identical_and_match_moves_the_high_band() {
        let sr = audio::TARGET_SR;
        let clean = synth::harmonics(sr, 2.0, 160.0, 0.22);
        let damaged = synth::emphasize_high(&clean, sr);
        assert_eq!(
            envelope::correct_envelope(&damaged, &clean, sr, 0.0),
            damaged
        );
        let ones = vec![1.0f64; 513];
        let rebuilt = envelope::apply_gain(&clean, &ones);
        let err = clean
            .iter()
            .zip(&rebuilt)
            .skip(800)
            .take(clean.len().saturating_sub(1600))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1e-3, "reconstruction {err}");
        let clean_r = envelope::high_low_ratio(&clean, sr);
        let before = (envelope::high_low_ratio(&damaged, sr) - clean_r).abs();
        let after =
            (envelope::high_low_ratio(&envelope::correct_envelope(&damaged, &clean, sr, 1.0), sr)
                - clean_r)
                .abs();
        assert!(after < before, "before {before} after {after}");
    }

    #[test]
    fn wer_emotion_and_gates() {
        assert_eq!(
            score::word_error_rate("Hello, World!", "hello world"),
            Some(0.0)
        );
        let wer = score::word_error_rate("the cat sat", "the cat sat down").unwrap();
        assert!((wer - 1.0 / 3.0).abs() < 1e-12);
        assert!(score::word_error_rate("   ", "hello").is_none());
        assert!(score::wer_pass(0.018));
        assert!(!score::wer_pass(0.019));
        assert!(!score::ecapa_pass(0.7057));
        assert!(score::ecapa_pass(0.7058));
        let em = emotion_match(0.9, 0.8);
        assert!((em - (0.70 * 0.9 + 0.30 * 0.8)).abs() < 1e-12);
        assert!(score::emotion_pass(0.83));
        assert!(!score::emotion_pass(0.829));
        assert!((score::mos_from_components(30.0, 1.0, 0.0, 0.0) - 5.0).abs() < 1e-12);
        let sr = audio::TARGET_SR;
        let sine = synth::harmonics(sr, 1.0, 180.0, 0.25);
        let noise = synth::lcg_noise(sine.len(), 0.08, 3);
        let mos_sine = score::mos_proxy(&sine, sr);
        let mos_noise = score::mos_proxy(&noise, sr);
        assert!(mos_sine > 4.0 && mos_sine <= 5.0, "{mos_sine}");
        assert!(mos_noise < 2.0 && mos_noise >= 1.0, "{mos_noise}");
    }

    #[test]
    fn eval_keeps_ecapa_separate_from_the_dsp_embedding() {
        let sr = audio::TARGET_SR;
        let reference = clean_vowel();
        let hypothesis = short_case().samples;
        let persona = json!({"category_blend": 0.90, "emb_sim": 0.80});
        let ecapa = json!({"mean": 0.71, "scores": {"hyp.wav": 0.71}});
        let report = evaluate(EvalInput {
            reference: &reference,
            reference_sr: sr,
            hypothesis: &hypothesis,
            hypothesis_sr: sr,
            reference_text: Some("the cat sat"),
            hypothesis_text: Some("the cat sat"),
            ecapa_json: Some(&ecapa),
            persona_json: Some(&persona),
            apply_timbre: false,
            timbre_strength: None,
        });
        assert_eq!(report.wer, Some(0.0));
        assert_eq!(report.wer_pass, Some(true));
        assert_eq!(report.ecapa, Some(0.71));
        assert_eq!(report.ecapa_pass, Some(true));
        assert!(report.embed_cosine.unwrap() > 0.8);
        assert!((report.emotion_match.unwrap() - emotion_match(0.9, 0.8)).abs() < 1e-12);
        assert_eq!(report.emotion_match_pass, Some(true));
        let missing = evaluate(EvalInput {
            reference: &reference,
            reference_sr: sr,
            hypothesis: &hypothesis,
            hypothesis_sr: sr,
            reference_text: None,
            hypothesis_text: None,
            ecapa_json: None,
            persona_json: None,
            apply_timbre: false,
            timbre_strength: None,
        });
        assert!(missing.ecapa.is_none());
        assert!(missing.wer.is_none());
        assert!(missing.emotion_match.is_none());
        assert!(missing.embed_cosine.is_some());
    }

    #[test]
    fn persona_and_ecapa_json_parsers() {
        let sets = json!({"sets": {"a": {"emotion_match": 0.80}, "b": {"emotion_match": 0.90}}});
        let (value, source) = emotion_from_persona_json(&sets).unwrap();
        assert!((value - 0.85).abs() < 1e-12);
        assert_eq!(source, "persona_json.sets.mean");
        let direct = json!({"emotion_match": 0.84});
        assert_eq!(emotion_from_persona_json(&direct).unwrap().0, 0.84);
        let scores = json!({"scores": {"a.wav": 0.70}});
        assert_eq!(ecapa_from_json(&scores).unwrap().0, 0.70);
        let vectors = json!({"ref_embedding": [1.0, 0.0], "hyp_embedding": [1.0, 0.0]});
        assert!((ecapa_from_json(&vectors).unwrap().0 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn refuses_writes_inside_the_repo() {
        let inside = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("not-allowed.wav");
        assert!(audio::ensure_output_allowed(&inside).is_err());
        let outside =
            std::env::temp_dir().join(format!("galion-fidelity-ok-{}.wav", std::process::id()));
        assert!(audio::ensure_output_allowed(&outside).is_ok());
    }

    #[test]
    fn wav_roundtrip_keeps_the_waveform() {
        let sr = audio::TARGET_SR;
        let y = synth::harmonics(sr, 0.3, 200.0, 0.4);
        let path =
            std::env::temp_dir().join(format!("galion-fidelity-rt-{}.wav", std::process::id()));
        audio::ensure_output_allowed(&path).unwrap();
        audio::write_wav_s16(&path, &y, sr).unwrap();
        let (back, back_sr) = audio::read_wav_mono(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back_sr, sr);
        assert_eq!(back.len(), y.len());
        let err = y
            .iter()
            .zip(&back)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(err < 1.0 / 32768.0 * 2.0, "{err}");
    }

    #[test]
    fn self_check_closed_form_is_stable() {
        let value = self_check();
        let closed = &value["closed_form"];
        assert!(
            (closed["emotion_match"].as_f64().unwrap() - emotion_match(0.9, 0.8)).abs() < 1e-12
        );
        assert!((closed["segment_score"].as_f64().unwrap() - 0.9).abs() < 1e-12);
        assert_eq!(closed["ecapa_at_threshold_passes"], false);
        assert_eq!(closed["wer_above_threshold_passes"], false);
        assert_eq!(value["consent_evaluations"], 0);
        assert_eq!(value["audio"]["envelope_strength0_identical"], true);
    }
}
