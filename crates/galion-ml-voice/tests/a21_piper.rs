// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! FP-03: the A21 slot speaks through the real Piper pack when it is on disk (CPU).
//! Skips (passes with a note) when weights/ is not fetched (scripts/a21/fetch-a21.sh).
#![cfg(feature = "speech")]
use galion_ml_voice::speech::{weights, SpeechBackend, SpeechStack, SPEECH_RATE};

#[test]
fn a21_piper_speaks_deterministic_words_on_cpu() {
    let st = weights::WeightStatus::discover();
    let Ok(_) = weights::A21WeightTts::try_new(&st) else {
        eprintln!("SKIP: A21 pack not active: {}", st.summary());
        return;
    };
    assert_eq!(st.id(), "present-verified", "{}", st.summary());
    // lat240 (owner approval t613s2992): galion-piper-rt is the default runtime when
    // installed (scripts/a21/build-piper-rt.sh); GALION_PIPER_LEGACY=1 keeps the old CLI.
    let dec = weights::decoder_for("onnx").unwrap();
    if std::env::var(weights::RUNTIME_ENV).is_err() && weights::piper_rt_runtime().is_some() {
        let want = if weights::legacy_forced() { "piper 2023.11.14-2" } else { "galion-piper-rt" };
        assert!(dec.contains(want), "{dec}");
    }
    eprintln!("A21 runtime: {dec}");
    // This test is about the A21 slot. Pin it: with clone weights fetched on the
    // host, `auto` would (correctly) pick the Chatterbox clone worker instead.
    // Single test in this binary, so setting the env here does not race.
    std::env::set_var("GALION_SPEECH", "a21");
    let mut s = SpeechStack::detect();
    assert!(matches!(s.backend, SpeechBackend::A21Weights { .. }), "{}", s.backend.label());
    assert_eq!(s.backend.speech_token(), "a21");
    let t = std::time::Instant::now();
    let a = s.speak("you said: hello galion").unwrap();
    let ms = t.elapsed().as_millis();
    let b = s.speak("you said: hello galion").unwrap();
    assert_eq!(a.sample_rate, SPEECH_RATE);
    assert_eq!(a.samples, b.samples, "deterministic (noise 0)");
    let secs = a.samples.len() as f32 / SPEECH_RATE as f32;
    assert!((0.8..4.0).contains(&secs), "{secs}s");
    // Not a tone: zero-crossing rate varies strongly across 20 ms windows (voiced vs fricatives).
    let w = SPEECH_RATE as usize / 50;
    let zcr: Vec<f32> = a.samples.chunks(w).filter(|c| c.iter().map(|x| x * x).sum::<f32>() / (c.len() as f32) > 1e-5)
        .map(|c| c.windows(2).filter(|p| (p[0] >= 0.0) != (p[1] >= 0.0)).count() as f32 / c.len() as f32).collect();
    let (lo, hi) = zcr.iter().fold((1f32, 0f32), |(l, h), &z| (l.min(z), h.max(z)));
    assert!(hi > 3.0 * lo.max(1e-3), "zcr range {lo}..{hi} looks tonal");
    eprintln!("A21 ok: {secs:.2}s audio in {ms} ms on CPU; zcr {lo:.3}..{hi:.3}");
}
