// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic signals only (no voice audio in git).
use galion_articulation::{acoustic, score, Section, TICK_S};

/// Glottal-like pulse train at f0 through two resonators (a crude vowel). Test fixture only.
fn vowel(f0: f64, f1: f64, f2: f64, secs: f64, sr: f64) -> Vec<f64> {
    let n = (secs * sr) as usize;
    let mut src = vec![0.0; n];
    let period = (sr / f0) as usize;
    for i in (0..n).step_by(period) {
        src[i] = 1.0;
    }
    let mut y = src;
    for (f, bw) in [(f1, 80.0), (f2, 120.0)] {
        let r = (-std::f64::consts::PI * bw / sr).exp();
        let c = 2.0 * r * (2.0 * std::f64::consts::PI * f / sr).cos();
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in y.iter_mut() {
            let o = *v + c * y1 - r * r * y2;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let m = y.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    y.iter().map(|v| 0.5 * v / m).collect()
}

#[test]
fn ticks_are_80ms_and_f0_is_found() {
    let sr = 16000.0;
    let x = vowel(200.0, 700.0, 1200.0, 1.6, sr);
    let (a, _) = acoustic::analyse(&x, sr);
    assert_eq!(a.tick_ms, 80);
    assert_eq!(a.n_ticks, (1.6 / TICK_S).ceil() as usize);
    let f0 = a.tracks["f0_hz"]
        .ticks
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert!(f0.len() > 10);
    let med = f0[f0.len() / 2];
    assert!((med - 200.0).abs() < 5.0, "f0 {med}");
    assert_eq!(a.tracks["f0_hz"].section, Section::Throat);
    assert_eq!(a.tracks["f1_hz"].section, Section::Mouth);
}

#[test]
fn same_voice_scores_higher_than_different_voice() {
    let sr = 16000.0;
    let mk = |f0, f1, f2| {
        let (mut a, _) = acoustic::analyse(&vowel(f0, f1, f2, 1.6, sr), sr);
        a.finish();
        a
    };
    let r = mk(200.0, 700.0, 1200.0);
    let same = mk(202.0, 710.0, 1210.0);
    let other = mk(110.0, 300.0, 2300.0);
    let s1 = score::compare(&r, &same).total;
    let s2 = score::compare(&r, &other).total;
    assert!(s1 > s2, "same {s1} other {s2}");
    assert!(s1 > 0.5);
}
