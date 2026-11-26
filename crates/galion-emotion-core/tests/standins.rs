// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Closed eval over the fixed stand-in files.

use std::path::PathBuf;

use galion_emotion_core::standins::memory::{DenyMemory, PersonMemory};
use galion_emotion_core::{load_standins, FATIGUE_REST, FRAME_MS};

fn standins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("standins")
}

#[test]
fn standin_script_runs_on_the_clock_and_leaves_the_baseline() {
    let dir = standins();
    for name in ["persona.txt", "memory.txt", "inner-life.txt", "intent.txt"] {
        let path = dir.join(name);
        assert!(path.is_file(), "missing {name}");
    }
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(
            !matches!(ext, "wav" | "mp3" | "flac" | "ogg" | "pcm"),
            "audio in stand-ins: {}",
            path.display()
        );
    }

    let world = load_standins(&dir).unwrap();
    assert_eq!(world.baseline.persona_id(), "standin-speaker");
    assert_eq!(world.baseline.voice_id(), "steady-rest");
    assert!((world.baseline.pitch_centre_hz - 216.0).abs() < 1e-4);
    assert_eq!(world.memory.len(), 2);
    assert!(world.inner.care.contains("heard"));
    assert!(world.inner.want.contains("question"));
    assert!(world.inner.expect.contains("pause"));
    assert_eq!(world.script.len(), 40);
    for pair in world.script.windows(2) {
        assert_eq!(pair[1].t_ms - pair[0].t_ms, FRAME_MS);
    }
    assert_eq!(world.script[0].t_ms, 0);
    assert_eq!(world.script[39].verb_str(), "settle");
    assert!(world.script[39].fatigue > FATIGUE_REST);

    let bits = world.baseline.identity_bits();
    let core = world.replay().unwrap();
    assert_eq!(core.baseline().identity_bits(), bits);
    assert_eq!(core.frames(), 40);
    assert_eq!(core.moment().t_ms, 39 * FRAME_MS);
    assert!(core.inside().fatigue > 0.8);
    assert!(core.moment().pos[galion_emotion_core::AROUSAL] < core.baseline().arousal());
    let cue = core.conductor_cue();
    assert_eq!(cue.fatigue, core.inside().fatigue);
    assert!(!cue.may_share && !cue.may_remember);
}

#[test]
fn deny_memory_forget_matching_is_empty() {
    let mut memory: Box<dyn PersonMemory> = Box::new(DenyMemory::new());
    memory.set_remember(true);
    assert!(!memory.remember_granted());
    assert!(memory.forget_matching("person-alpha", "sister").is_empty());
}

#[test]
fn deny_memory_list_person_is_empty() {
    let mut memory: Box<dyn PersonMemory> = Box::new(DenyMemory::new());
    memory.set_remember(true);
    assert!(!memory.remember_granted());
    assert!(memory.list_person("person-alpha", 8).is_empty());
}
