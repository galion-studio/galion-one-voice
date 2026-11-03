// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! One test, so the process environment is not shared with a sibling test.
use galion_openmind::{NerveEmitter, VoiceNote};
use std::time::{Duration, Instant};

#[test]
fn a_non_loopback_url_is_refused_and_the_note_stays_local() {
    std::env::remove_var("NERVE_URL");
    std::env::set_var("GALION_NERVE_URL", "http://10.1.2.3:8670");
    std::env::set_var("GALION_NERVE_LOG", "/no/such/dir/galion-nerve.jsonl");
    let em = NerveEmitter::from_env();
    std::env::remove_var("GALION_NERVE_URL");
    std::env::remove_var("GALION_NERVE_LOG");
    let stats = em.stats();
    assert_eq!(
        stats.forward_refused, 1,
        "non-loopback URL must not be contacted"
    );
    assert_eq!(stats.log_skipped, 1);
    em.note(VoiceNote::frame_miss(2)).unwrap();
    let t0 = Instant::now();
    loop {
        if em.stats().published >= 1 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    let line = em.lines().pop().unwrap();
    assert!(line.contains("voice.frame_miss"));
    assert_eq!(em.stats().forwarded, 0);
    assert_eq!(em.stats().forward_failed, 0);
}
