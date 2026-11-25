// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Load, round-trip, and the character that stays while the moment moves.

use std::path::PathBuf;

use galion_emotion_core::{
    EmotionCore, InnerLife, IntentMsg, VoiceBaseline, AROUSAL, FRAME_MS, MASK_FATIGUE, VALENCE,
};
use horbm_persona::{
    BootConfig, LegalMovePort, MoveClaim, MoveVerdict, Persona, PersonaError, UNKNOWN_LINE,
};

const VALID: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "sample"
voice = "sample"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("personas")
}

fn load_example(name: &str) -> Persona {
    Persona::load(&examples().join(name)).unwrap_or_else(|err| panic!("load {name}: {err}"))
}

fn fact(has_card: bool) -> MoveClaim<'static> {
    MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card,
        sympathy_script: false,
    }
}

#[test]
fn development_default_is_samantha() {
    let shipped = Persona::samantha().unwrap();
    let file = load_example("samantha.toml");
    assert_eq!(shipped, file);
    assert_eq!(shipped.id(), "samantha");
    let aria = load_example("aria.toml");
    assert_eq!(aria.id(), "aria");
    assert_ne!(shipped.id(), aria.id());
    let text = std::fs::read_to_string(examples().join("aria.toml")).unwrap();
    assert!(text.contains("Inactive"));
    assert!(text.contains("Later"));
}

fn persona_with_id(id: &str) -> Persona {
    let text = VALID.replace("id = \"sample\"", &format!("id = \"{id}\""));
    Persona::parse(&text).unwrap_or_else(|err| panic!("parse {id}: {err}"))
}

#[test]
fn a_configured_id_boots_and_a_different_id_does_not() {
    let cfg = BootConfig::from_id("cedar").unwrap();
    let cedar = persona_with_id("cedar");
    let maple = persona_with_id("maple");
    assert_eq!(cedar.id(), "cedar");
    assert!(cedar.is_configured_boot(&cfg));
    assert!(!maple.is_configured_boot(&cfg));
    assert!(!Persona::parse(VALID).unwrap().is_configured_boot(&cfg));
    let aria = Persona::load(&examples().join("aria.toml")).unwrap();
    assert!(aria.is_configured_boot(&BootConfig::from_id(aria.id()).unwrap()));
    assert!(!aria.is_configured_boot(&cfg));
    let standin = Persona::standin();
    assert!(standin.is_configured_boot(&BootConfig::from_id(standin.id()).unwrap()));
    assert!(!standin.is_configured_boot(&cfg));
}

#[test]
fn a_fresh_persona_boots_when_configured() {
    let fresh = persona_with_id("northwind");
    assert_ne!(fresh.id(), "cedar");
    let cfg = BootConfig::resolve(Some(fresh.id()), None).unwrap();
    assert!(fresh.is_configured_boot(&cfg));
    assert!(!persona_with_id("cedar").is_configured_boot(&cfg));
    let mut files = 0;
    for entry in std::fs::read_dir(examples()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let persona =
            Persona::load(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        let own = BootConfig::from_id(persona.id()).unwrap();
        assert!(persona.is_configured_boot(&own), "{}", persona.id());
        assert!(!persona.is_configured_boot(&cfg), "{}", persona.id());
        files += 1;
    }
    assert_eq!(files, 2);
}

#[test]
fn unset_boot_config_refuses() {
    let unset = PersonaError::Invalid("development persona is not configured");
    assert_eq!(BootConfig::from_id("").unwrap_err(), unset);
    assert_eq!(BootConfig::from_id("   ").unwrap_err(), unset);
    assert_eq!(BootConfig::resolve(None, None).unwrap_err(), unset);
    assert_eq!(BootConfig::resolve(Some(""), None).unwrap_err(), unset);
    assert_eq!(
        BootConfig::resolve(Some("  "), Some("")).unwrap_err(),
        unset
    );
    assert_eq!(BootConfig::from_toml("").unwrap_err(), unset);
    assert_eq!(
        BootConfig::from_toml("development_persona = \"\"").unwrap_err(),
        unset
    );
    assert_eq!(BootConfig::from_toml("# note\n").unwrap_err(), unset);
    assert_eq!(
        BootConfig::from_id("Cedar").unwrap_err(),
        PersonaError::Invalid("development persona id is not a short token")
    );
    assert_eq!(
        BootConfig::from_toml("note = \"x\"\n").unwrap_err(),
        PersonaError::Invalid("development persona file does not match the schema")
    );
    let file = "development_persona = \"maple\"\n";
    assert!(BootConfig::resolve(Some("NOPE"), Some(file)).is_err());
    assert_eq!(BootConfig::resolve(None, Some(file)).unwrap().id(), "maple");
    assert_eq!(
        BootConfig::resolve(Some("  cedar  "), Some(file))
            .unwrap()
            .id(),
        "cedar"
    );
    assert_eq!(
        BootConfig::resolve(Some(""), Some(file)).unwrap().id(),
        "maple"
    );
}

#[test]
fn shipped_boot_file_names_a_persona_that_boots() {
    let cfg = BootConfig::shipped().unwrap();
    let path = examples().join(format!("{}.toml", cfg.id()));
    let persona = Persona::load(&path).expect("shipped boot id has a persona file");
    assert_eq!(persona.id(), cfg.id());
    assert!(persona.is_configured_boot(&cfg));
    let other = persona_with_id("cedar");
    assert!(!other.is_configured_boot(&cfg));
    match std::env::var(BootConfig::ENV) {
        Ok(value) if !value.trim().is_empty() => {
            assert_eq!(
                BootConfig::from_process().unwrap(),
                BootConfig::from_id(&value).unwrap()
            );
        }
        _ => assert_eq!(BootConfig::from_process().unwrap(), cfg),
    }
}

#[test]
fn load_roundtrip() {
    for name in ["samantha.toml", "aria.toml"] {
        let persona = load_example(name);
        let text = persona.to_toml();
        let again = Persona::parse(&text).unwrap();
        assert_eq!(persona, again, "{name}");
        assert_eq!(text, again.to_toml(), "{name}");
        assert_eq!(persona.unknown_line(), UNKNOWN_LINE);
    }
    let standin = Persona::standin();
    let again = Persona::parse(&standin.to_toml()).unwrap();
    assert_eq!(standin, again);
    assert_eq!(standin.to_toml(), again.to_toml());
}

#[test]
fn persona_stays_constant_while_the_moment_swings() {
    let persona = load_example("samantha.toml");
    let before = persona;
    let style = persona.style();
    let warmth = persona.trait_value("warmth").unwrap().to_bits();
    let mut core = EmotionCore::from_baseline(persona.baseline());
    let bits = core.baseline().identity_bits();
    assert_eq!(core.baseline().persona_id(), "samantha");
    assert_eq!(
        core.baseline().valence().to_bits(),
        persona.valence().to_bits()
    );
    assert_eq!(
        core.baseline().pitch_centre_hz.to_bits(),
        persona.pitch_centre_hz().to_bits()
    );
    for i in 0..40u32 {
        let mut msg = IntentMsg::hold(i * FRAME_MS);
        msg.fatigue = 0.95;
        msg.factor_mask = MASK_FATIGUE;
        msg.strength = 1.0;
        msg.set_pull(VALENCE, -0.6).unwrap();
        core.step(&msg).unwrap();
    }
    assert_eq!(persona, before);
    assert_eq!(persona.style(), style);
    assert_eq!(persona.style().max_sentences, 2);
    assert_eq!(persona.trait_value("warmth").unwrap().to_bits(), warmth);
    assert_eq!(core.baseline().identity_bits(), bits);
    assert!(core.moment().pos[AROUSAL] < core.baseline().arousal());
    assert_ne!(
        core.moment().pos[VALENCE].to_bits(),
        core.baseline().valence().to_bits()
    );
    assert_eq!(persona.care(), "heard");
    assert_eq!(persona.inner_life().care(0).unwrap().token_str(), "heard");
}

#[test]
fn invalid_persona_refused() {
    assert!(Persona::parse(VALID).is_ok());
    let cases = [
        ("version = 1", "version = 2", "version must be 1"),
        (
            "schema = \"galion-horbm-persona/1\"",
            "schema = \"galion-horbm-persona/9\"",
            "schema must be galion-horbm-persona/1",
        ),
        (
            "never_fabricate = true",
            "never_fabricate = false",
            "never_fabricate must be true",
        ),
        (
            "unknown = \"I don't know.\"",
            "unknown = \"maybe\"",
            "unknown line must be I don't know.",
        ),
        (
            "warmth = 0.61",
            "warmth = 1.5",
            "trait value must be a finite number from 0 to 1",
        ),
        (
            "warmth = 0.61",
            "Warmth = 0.61",
            "trait name is not a short token",
        ),
        (
            "warmth = 0.61",
            "warmth = \"high\"",
            "persona file does not match the schema",
        ),
        ("id = \"sample\"", "id = \"\"", "id is not a short token"),
        (
            "id = \"sample\"",
            "id = \"this-name-is-too-long\"",
            "id is not a short token",
        ),
        (
            "arousal = 0.31",
            "arousal = 1.4",
            "baseline value is outside its range",
        ),
        (
            "max_sentences = 2",
            "max_sentences = 0",
            "max_sentences must be 1 to 4",
        ),
        (
            "care = \"heard\"",
            "care = \"Heard\"",
            "inner token must be 1 to 16 lowercase letters",
        ),
        (
            "care = \"heard\"",
            "care = \"the person\"",
            "inner token must be 1 to 16 lowercase letters",
        ),
        (
            "forbid = [\"fake-sympathy\", \"unsourced-fact\"]",
            "forbid = [\"fake-sympathy\", \"fake-sympathy\"]",
            "duplicate boundary token",
        ),
    ];
    for (from, to, reason) in cases {
        let text = VALID.replace(from, to);
        assert_ne!(text, VALID, "{reason}");
        match Persona::parse(&text) {
            Err(PersonaError::Invalid(got)) => assert_eq!(got, reason, "{from} -> {to}"),
            other => panic!("expected invalid {reason}, got {other:?}"),
        }
    }
    let nine = VALID.replace(
        "warmth = 0.61\n",
        "a = 0.1\nb = 0.1\nc = 0.1\nd = 0.1\ne = 0.1\nf = 0.1\ng = 0.1\nh = 0.1\ni = 0.1\n",
    );
    assert_eq!(
        Persona::parse(&nine).unwrap_err(),
        PersonaError::Invalid("traits must be 1 to 8 finite values from 0 to 1")
    );
    assert_eq!(
        Persona::parse(&format!("note = \"x\"\n{VALID}")).unwrap_err(),
        PersonaError::Invalid("persona file does not match the schema")
    );
    let at = format!("# a{}b.example\n", '@');
    let ip = format!("# see {}.{}.{}.{}\n", 10, 1, 2, 3);
    let audio = format!("# clip{}\n", ".wav");
    for marker in [at.as_str(), ip.as_str(), audio.as_str()] {
        assert_eq!(
            Persona::parse(&format!("{marker}{VALID}")).unwrap_err(),
            PersonaError::Invalid("persona text contains a secret-shaped marker"),
            "{marker}"
        );
    }
    let missing =
        std::env::temp_dir().join(format!("horbm-persona-missing-{}", std::process::id()));
    assert_eq!(
        Persona::load(&missing).unwrap_err(),
        PersonaError::Unreadable
    );
}

#[test]
fn constraint_port_forbids_a_fact_without_a_source() {
    let bare = fact(false);
    let card = fact(true);
    let script = MoveClaim {
        tag: b"ask",
        claims_fact: false,
        has_card: false,
        sympathy_script: true,
    };
    let past = MoveClaim {
        tag: b"invented-past",
        claims_fact: false,
        has_card: false,
        sympathy_script: false,
    };
    let ask = MoveClaim {
        tag: b"ask",
        claims_fact: false,
        has_card: false,
        sympathy_script: false,
    };
    for name in ["samantha.toml", "aria.toml"] {
        let persona = load_example(name);
        let port: &dyn LegalMovePort = &persona;
        assert_eq!(port.verdict(bare), MoveVerdict::Forbid, "{name}");
        assert_eq!(persona.verdict(bare), MoveVerdict::Forbid, "{name}");
        assert_eq!(persona.verdict(card), MoveVerdict::Allow, "{name}");
        assert_eq!(persona.verdict(script), MoveVerdict::Forbid, "{name}");
        assert_eq!(persona.verdict(past), MoveVerdict::Forbid, "{name}");
        assert_eq!(persona.verdict(ask), MoveVerdict::Allow, "{name}");
        assert_eq!(persona.verdict(bare), persona.verdict(bare));
    }
    assert_eq!(Persona::standin().verdict(bare), MoveVerdict::Forbid);
    let open = Persona::parse(&VALID.replace(
        "forbid = [\"fake-sympathy\", \"unsourced-fact\"]",
        "forbid = [\"fake-sympathy\"]",
    ))
    .unwrap();
    assert_eq!(open.verdict(bare), MoveVerdict::Allow);
    assert_eq!(open.verdict(script), MoveVerdict::Forbid);
    assert!(!galion_emotion_core::learning_allowed());
}

#[test]
fn same_in_same_out() {
    let path = examples().join("samantha.toml");
    assert_eq!(snapshot(&path), snapshot(&path));
    let aria = examples().join("aria.toml");
    assert_eq!(snapshot(&aria), snapshot(&aria));
}

fn snapshot(
    path: &std::path::Path,
) -> (
    Persona,
    MoveVerdict,
    MoveVerdict,
    horbm_persona::StyleHook,
    String,
    [u32; 4],
) {
    let persona = Persona::load(path).unwrap();
    let forbid = persona.verdict(fact(false));
    let allow = persona.verdict(fact(true));
    let mut core = EmotionCore::from_baseline(persona.baseline());
    for i in 0..4u32 {
        let mut msg = IntentMsg::hold(i * FRAME_MS);
        msg.fatigue = 0.9;
        msg.factor_mask = MASK_FATIGUE;
        core.step(&msg).unwrap();
    }
    let pos = [
        core.moment().pos[0].to_bits(),
        core.moment().pos[1].to_bits(),
        core.moment().pos[2].to_bits(),
        core.moment().pos[3].to_bits(),
    ];
    let text = persona.to_toml();
    (persona, forbid, allow, persona.style(), text, pos)
}

#[test]
fn example_files_carry_no_personal_data() {
    let personal = [
        "koenig",
        "johansson",
        "bettany",
        "drake",
        "maciej",
        "grajczyk",
        "librivox",
        "scarlett",
        "@",
        "http://",
        "https://",
        ".wav",
        ".mp3",
        ".flac",
        ".ogg",
        ".pcm",
        "sha256",
    ];
    let mut names = Vec::new();
    for entry in std::fs::read_dir(examples()).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(
            !matches!(ext, "wav" | "mp3" | "flac" | "ogg" | "pcm" | "wave"),
            "{}",
            path.display()
        );
        names.push(path.file_name().unwrap().to_string_lossy().into_owned());
        let text = std::fs::read_to_string(&path).unwrap();
        let lower = text.to_ascii_lowercase();
        for needle in personal {
            assert!(
                !lower.contains(needle),
                "{} contains {needle}",
                path.display()
            );
        }
        assert!(
            lower.contains("not a measured person"),
            "{}",
            path.display()
        );
        assert!(Persona::load(&path).is_ok());
    }
    names.sort();
    assert_eq!(
        names,
        vec!["aria.toml".to_string(), "samantha.toml".to_string()]
    );
    let sam = std::fs::read_to_string(examples().join("samantha.toml")).unwrap();
    let aria = std::fs::read_to_string(examples().join("aria.toml")).unwrap();
    assert_eq!(
        comments(&sam),
        vec![
            "# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.",
            "# Development default. Boot loads this file.",
            "# Test voice. Warm, curious, playful, honest, and emotionally attentive.",
            "# Not a recording. Not a measured person. No clip and no donor.",
        ]
    );
    assert_eq!(
        comments(&aria),
        vec![
            "# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.",
            "# Later. Inactive during development. Boot loads Samantha and does not load this file.",
            "# Test voice. Warmth and curiosity, with calm precision.",
            "# Playful, honest, emotionally attentive, steady at work.",
            "# Not a recording. Not a measured person. No clip and no donor.",
        ]
    );
}

fn comments(text: &str) -> Vec<&str> {
    text.lines().filter(|line| line.starts_with('#')).collect()
}

#[test]
fn aria_keeps_warmth_and_adds_precision() {
    let sam = load_example("samantha.toml");
    let aria = load_example("aria.toml");
    assert_eq!(sam.id(), "samantha");
    assert_eq!(aria.id(), "aria");
    assert!(sam.trait_value("precision").is_none());
    assert!(sam.trait_value("steadiness").is_none());
    assert!(aria.trait_value("precision").unwrap() > 0.8);
    assert!(aria.trait_value("steadiness").unwrap() > 0.8);
    assert!(sam.trait_value("warmth").unwrap() > aria.trait_value("warmth").unwrap());
    assert!(aria.trait_value("warmth").unwrap() > 0.7);
    assert!(sam.trait_value("curiosity").unwrap() > 0.7);
    assert!(aria.trait_value("curiosity").unwrap() > 0.7);
    assert!(aria.trait_value("play").unwrap() > 0.0);
    assert!(sam.trait_value("play").unwrap() > aria.trait_value("play").unwrap());
    assert!(aria.style().brevity > sam.style().brevity);
    assert!(aria.arousal() < sam.arousal());
    assert!(sam.valence() > aria.valence());
    assert!(aria.valence() > Persona::standin().valence());
    assert!(aria.style().warmth > 0.6);
    assert_eq!(sam.style().max_sentences, 2);
    assert_eq!(aria.style().max_sentences, 2);
    assert_eq!(sam.pitch_centre_hz().to_bits(), 216.0f32.to_bits());
    assert_eq!(aria.pitch_centre_hz().to_bits(), 216.0f32.to_bits());
    assert_eq!(sam.expect_token(), "pause");
    assert_eq!(aria.expect_token(), "pause");
    let mut prev = "";
    for i in 0..aria.trait_len() {
        let (key, _) = aria.trait_at(i).unwrap();
        assert!(key > prev, "{key} before {prev}");
        prev = key;
    }
}

#[test]
fn inner_seed_comes_from_the_persona_and_the_stand_in_is_the_fallback() {
    let seeded = Persona::parse(VALID).unwrap().inner_life();
    assert_eq!(seeded, InnerLife::seeded());
    let focus = Persona::parse(&VALID.replace("care = \"heard\"", "care = \"focus\"")).unwrap();
    assert_eq!(focus.inner_life().care(0).unwrap().token_str(), "focus");
    assert_eq!(focus.want(), "answer");
    assert_eq!(focus.inner_life().expect(0).unwrap().token_str(), "pause");
    assert_ne!(
        focus.inner_life().care(0).unwrap().token_str(),
        InnerLife::seeded().care(0).unwrap().token_str()
    );
    let standin = Persona::standin();
    assert_eq!(
        standin.baseline().identity_bits(),
        VoiceBaseline::standin().identity_bits()
    );
    assert_eq!(standin.inner_life(), InnerLife::seeded());
    assert_eq!(standin.id(), "standin-speaker");
    assert_eq!(standin.voice(), "steady-rest");
}

#[test]
fn traits_are_open_numbers() {
    let text = VALID.replace("warmth = 0.61", "warmth = 0.61\nprecision = 0.5");
    let persona = Persona::parse(&text).unwrap();
    assert_eq!(persona.trait_len(), 2);
    assert!(persona.trait_value("precision").unwrap() > 0.4);
    assert!(persona.trait_value("no-such").is_none());
}
