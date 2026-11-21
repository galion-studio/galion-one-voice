// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Bounds, slew, tags, and the persona file this crate reuses.

use samantha_emotion_prosody::*;
use std::path::PathBuf;

fn assert_bounds(params: &SpeakingParams) {
    assert!(
        params.in_bounds(),
        "out of bounds: ex={} ls={} ns={} nw={} sil={} pause={} pitch={} energy={} rate={} worker={}",
        params.exaggeration,
        params.length_scale,
        params.noise_scale,
        params.noise_w,
        params.sentence_silence_s,
        params.pre_pause_ms,
        params.pitch_shift_st,
        params.energy,
        params.rate,
        params.worker_rate,
    );
    if let Some(tag) = params.tag {
        assert!(!tag.on_piper());
        assert!(tag.on_clone());
        assert!(!is_whisper_token(tag.token()));
        for banned in WHISPER_TOKENS {
            assert_ne!(tag.token(), *banned);
        }
    }
}

#[test]
fn grid_stays_inside_bounds_and_one_tick_cannot_cross_the_band() {
    let alpha = slew_alpha();
    // Lipschitz of the exaggeration map, times the largest one-tick Pad step.
    let max_ex_step = alpha * (0.10 / 0.70 + 2.0 * 0.06 / 0.75 + 0.02) + 1e-4;
    assert!(max_ex_step < EXAGGERATION_MAX - EXAGGERATION_MIN);

    for a in 0..=10 {
        for v in -4..=4 {
            for d in 0..=4 {
                let pad = Pad {
                    arousal: a as f32 / 10.0,
                    valence: v as f32 / 4.0,
                    dominance: d as f32 / 4.0,
                };
                let instant = map(pad);
                assert_bounds(&instant);
                assert!(instant.tag.is_none(), "map does not script a tag");

                let mut clock = ProsodyClock::new();
                clock.set_target(pad);
                let mut last = clock.tick();
                assert_bounds(&last);
                assert!((last.exaggeration - EXAGGERATION_REST).abs() <= max_ex_step);
                for _ in 0..24 {
                    let next = clock.tick();
                    assert_bounds(&next);
                    assert!((next.exaggeration - last.exaggeration).abs() <= max_ex_step);
                    last = next;
                }
            }
        }
    }
}

#[test]
fn sustained_play_can_giggle_and_then_waits() {
    let mut clock = ProsodyClock::new();
    clock.set_target(Pad {
        arousal: 0.72,
        valence: 0.80,
        dominance: 0.55,
    });
    let mut fires = Vec::new();
    for i in 0..200u32 {
        let params = clock.tick();
        assert_bounds(&params);
        if let Some(tag) = params.tag {
            assert_eq!(tag, Tag::Giggle);
            assert_eq!(tag.token(), "[giggle]");
            fires.push(i);
        }
    }
    assert!(
        !fires.is_empty(),
        "a sustained playful mood should open the hint"
    );
    assert!(
        fires.len() <= 5,
        "cooldown keeps it off most lines: {fires:?}"
    );
    for pair in fires.windows(2) {
        assert!(pair[1] - pair[0] >= u32::from(GIGGLE_COOLDOWN_TICKS));
    }
}

#[test]
fn one_hot_tick_does_not_giggle() {
    let mut clock = ProsodyClock::new();
    clock.set_target(Pad {
        arousal: 0.9,
        valence: 0.9,
        dominance: 0.55,
    });
    assert!(clock.tick().tag.is_none());
    clock.set_target(Pad::RESTING);
    for _ in 0..40 {
        let params = clock.tick();
        assert!(params.tag.is_none());
        assert_bounds(&params);
    }
}

#[test]
fn a_warm_unhurried_mood_can_inhale() {
    let mut clock = ProsodyClock::new();
    clock.set_target(Pad {
        arousal: 0.12,
        valence: 0.30,
        dominance: 0.45,
    });
    let mut inhales = 0u32;
    for _ in 0..80 {
        let params = clock.tick();
        assert_bounds(&params);
        match params.tag {
            Some(Tag::Inhale) => {
                assert_eq!(params.tag.unwrap().token(), "[inhale]");
                inhales += 1;
            }
            Some(Tag::Giggle) => panic!("this mood is not a giggle"),
            None => {}
        }
    }
    assert!(inhales >= 1);
    assert!(inhales <= 3);
}

#[test]
fn a_collapsed_pad_is_not_a_whisper() {
    let mut clock = ProsodyClock::new();
    clock.set_target(Pad {
        arousal: 0.02,
        valence: -0.8,
        dominance: 0.05,
    });
    for _ in 0..40 {
        let params = clock.tick();
        assert_bounds(&params);
        assert!(params.tag.is_none());
        assert!(params.energy >= ENERGY_MIN);
        assert!(params.exaggeration >= EXAGGERATION_MIN);
        assert!(!params.whisper());
    }
}

#[test]
fn tick_mean_is_under_the_80ms_bar() {
    let n = 100_000u32;
    let elapsed = measure_ticks(n);
    let mean_ns = elapsed.as_nanos() as f64 / f64::from(n);
    eprintln!(
        "measure_ticks n={n} elapsed_us={} mean_ns={mean_ns:.1}",
        elapsed.as_micros()
    );
    assert!(
        mean_ns < 1_000_000.0,
        "mean {mean_ns:.1} ns is over 1 ms; the 80 ms bar is 80_000_000 ns"
    );
    assert!(mean_ns < 80_000_000.0);
}

#[test]
fn persona_toml_e0_and_tag_policy_match_the_constants() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/galion-voice-v2/personas/voice/samantha.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    assert!((e0(&text, "pitch_st") - BASE_PITCH_ST).abs() < 1e-6);
    assert!((e0(&text, "rate") - BASE_LENGTH).abs() < 1e-6);
    assert!((e0(&text, "loudness") - BASE_ENERGY).abs() < 1e-6);
    assert!((e0(&text, "pre_pause_ms") - BASE_PRE_PAUSE_MS).abs() < 1e-6);

    let e1_pause = section_value(&text, "[register.prosody.e1]", "pre_pause_ms");
    assert!((e1_pause - PAUSE_CEILING_MS).abs() < 1e-6);

    assert!(flag(&text, "on_piper") == Some(false));
    assert!(flag(&text, "on_clone") == Some(true));
    let allowed = allowed_tags(&text);
    assert_eq!(allowed, PERSONA_ALLOWED_TAGS);
    assert!(!allowed.contains(&"[inhale]"));
    assert!(!allowed.contains(&"[giggle]"));
    assert!(!allowed.iter().any(|t| is_whisper_token(t)));
}

fn e0(text: &str, key: &str) -> f32 {
    section_value(text, "[register.prosody.e0]", key)
}

fn section_value(text: &str, header: &str, key: &str) -> f32 {
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == key {
                return v
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("parse {key} in {header}"));
            }
        }
    }
    panic!("missing {key} in {header}");
}

fn flag(text: &str, key: &str) -> Option<bool> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == key {
                return match v.split('#').next().unwrap_or("").trim() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
            }
        }
    }
    None
}

fn allowed_tags(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == "allowed" {
                let body = v.split('#').next().unwrap_or("");
                for tag in PERSONA_ALLOWED_TAGS {
                    if body.contains(tag) {
                        found.push(*tag);
                    }
                }
            }
        }
    }
    found
}
