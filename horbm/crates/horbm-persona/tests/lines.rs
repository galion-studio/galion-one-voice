// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Configured short lines. An unset line stays none.

use std::path::PathBuf;

use horbm_persona::{BootConfig, Persona, PersonaError, PersonaLine, LINE_BYTES};

const FORGOT: &str = "Okay, I've let that go.";
const NOTHING: &str = "I don't have anything saved about you yet.";
const BAD_LINE: PersonaError =
    PersonaError::Invalid("persona line must be 1 to 80 printable ascii bytes with no quote");

fn northwind(lines: &str) -> String {
    format!(
        r#"
schema = "galion-horbm-persona/1"
version = 1
id = "northwind"
voice = "northwind"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy"]

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
{lines}"#
    )
}

fn personas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("personas")
}

#[test]
fn northwind_with_lines_reads_them() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<Persona>();
    let text = northwind(&format!(
        "[lines]\nforgot = \"{FORGOT}\"\nnothing_remembered = \"{NOTHING}\"\n"
    ));
    let persona = Persona::parse(&text).unwrap();
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.voice(), "northwind");
    assert_eq!(persona.line(PersonaLine::Forgot), Some(FORGOT));
    assert_eq!(persona.line(PersonaLine::NothingRemembered), Some(NOTHING));
}

#[test]
fn without_lines_is_none() {
    let persona = Persona::parse(&northwind("")).unwrap();
    assert_eq!(persona.id(), "northwind");
    assert_eq!(persona.line(PersonaLine::Forgot), None);
    assert_eq!(persona.line(PersonaLine::NothingRemembered), None);
    let empty = Persona::parse(&northwind("[lines]\n")).unwrap();
    assert_eq!(empty.line(PersonaLine::Forgot), None);
    assert_eq!(empty.line(PersonaLine::NothingRemembered), None);
    let standin = Persona::standin();
    assert_eq!(standin.line(PersonaLine::Forgot), None);
    assert_eq!(standin.line(PersonaLine::NothingRemembered), None);
}

#[test]
fn overlong_or_non_ascii_line_refused() {
    // `g` is printable and not hex, so the secret-marker scan does not eat the line.
    let eighty = "g".repeat(LINE_BYTES);
    let ok = Persona::parse(&northwind(&format!("[lines]\nforgot = \"{eighty}\"\n"))).unwrap();
    assert_eq!(ok.line(PersonaLine::Forgot), Some(eighty.as_str()));
    assert_eq!(ok.line(PersonaLine::NothingRemembered), None);

    let long = "g".repeat(LINE_BYTES + 1);
    let cases = [
        format!("[lines]\nforgot = \"{long}\"\n"),
        format!("[lines]\nnothing_remembered = \"{long}\"\n"),
        "[lines]\nforgot = \"\"\n".to_string(),
        "[lines]\nforgot = \"caf\u{00e9}\"\n".to_string(),
        "[lines]\nnothing_remembered = \"caf\u{00e9}\"\n".to_string(),
        "[lines]\nforgot = \"say \\\"hi\\\"\"\n".to_string(),
        "[lines]\nforgot = \"one\\ntwo\"\n".to_string(),
        "[lines]\nforgot = \"a\\tb\"\n".to_string(),
        "[lines]\nforgot = \"a\\rb\"\n".to_string(),
    ];
    for lines in cases {
        assert_eq!(
            Persona::parse(&northwind(&lines)).unwrap_err(),
            BAD_LINE,
            "{lines:?}"
        );
    }
}

#[test]
fn unknown_key_in_lines_refused() {
    let text = northwind("[lines]\nforgot = \"Gone.\"\nlater = \"no\"\n");
    assert_eq!(
        Persona::parse(&text).unwrap_err(),
        PersonaError::Invalid("persona file does not match the schema")
    );
}

#[test]
fn roundtrip_keeps_lines() {
    let both = Persona::parse(&northwind(&format!(
        "[lines]\nforgot = \"{FORGOT}\"\nnothing_remembered = \"{NOTHING}\"\n"
    )))
    .unwrap();
    let text = both.to_toml();
    assert!(text.contains("\n[lines]\n"));
    assert!(text.contains(&format!("forgot = \"{FORGOT}\"\n")));
    assert!(text.contains(&format!("nothing_remembered = \"{NOTHING}\"\n")));
    let again = Persona::parse(&text).unwrap();
    assert_eq!(both, again);
    assert_eq!(text, again.to_toml());
    assert_eq!(again.line(PersonaLine::Forgot), Some(FORGOT));
    assert_eq!(again.line(PersonaLine::NothingRemembered), Some(NOTHING));

    let one = Persona::parse(&northwind(
        "[lines]\nnothing_remembered = \"Nothing yet.\"\n",
    ))
    .unwrap();
    assert_eq!(one.line(PersonaLine::Forgot), None);
    assert_eq!(
        one.line(PersonaLine::NothingRemembered),
        Some("Nothing yet.")
    );
    let one_text = one.to_toml();
    assert!(one_text.contains("\n[lines]\nnothing_remembered = \"Nothing yet.\"\n"));
    assert!(!one_text.contains("forgot ="));
    let one_again = Persona::parse(&one_text).unwrap();
    assert_eq!(one, one_again);
    assert_eq!(one_text, one_again.to_toml());

    let slash = Persona::parse(&northwind("[lines]\nforgot = \"a\\\\b\"\n")).unwrap();
    assert_eq!(slash.line(PersonaLine::Forgot), Some("a\\b"));
    let slash_text = slash.to_toml();
    assert!(slash_text.contains("forgot = \"a\\\\b\"\n"));
    assert_eq!(slash, Persona::parse(&slash_text).unwrap());

    let bare = Persona::parse(&northwind("")).unwrap();
    assert!(!bare.to_toml().contains("[lines]"));
    assert_eq!(
        bare.to_toml(),
        Persona::parse(&bare.to_toml()).unwrap().to_toml()
    );
    assert!(!Persona::standin().to_toml().contains("[lines]"));
}

#[test]
fn aria_loads_with_no_lines() {
    let cfg = BootConfig::shipped().unwrap();
    let mut others = 0;
    for entry in std::fs::read_dir(personas()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let persona =
            Persona::load(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        if persona.is_configured_boot(&cfg) {
            continue;
        }
        assert_eq!(persona.line(PersonaLine::Forgot), None);
        assert_eq!(persona.line(PersonaLine::NothingRemembered), None);
        let text = persona.to_toml();
        assert!(!text.contains("[lines]"));
        assert!(text.ends_with("expect = \"pause\"\n"));
        assert_eq!(persona, Persona::parse(&text).unwrap());
        others += 1;
    }
    assert_eq!(others, 1);
}

#[test]
fn dev_persona_has_both_lines() {
    let cfg = BootConfig::shipped().unwrap();
    let persona = Persona::load_configured(&personas(), &cfg).unwrap();
    assert_eq!(persona.line(PersonaLine::Forgot), Some(FORGOT));
    assert_eq!(persona.line(PersonaLine::NothingRemembered), Some(NOTHING));
    let text = persona.to_toml();
    let again = Persona::parse(&text).unwrap();
    assert_eq!(persona, again);
    assert_eq!(text, again.to_toml());
    let bytes = std::mem::size_of::<Persona>();
    eprintln!("size_of::<Persona>() = {bytes}");
}
