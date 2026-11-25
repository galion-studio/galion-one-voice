// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Boot reads the persona once. An invalid file is refused.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use horbm::{
    Brain, CardHit, CardLookup, InnerLife, MoveClaim, MoveVerdict, Persona, PersonaError,
    VoiceBaseline, UNKNOWN_LINE, UNKNOWN_REPLY,
};

const DEV_PERSONA_ENV: &str = "GALION_HORBM_DEV_PERSONA";
const PERSONA_DIR_ENV: &str = "GALION_HORBM_PERSONA_DIR";

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvVar {
    key: &'static str,
    prev: Option<std::ffi::OsString>,
}

impl EnvVar {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let prev = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        Self { key, prev }
    }
}

impl Drop for EnvVar {
    fn drop(&mut self) {
        match &self.prev {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

/// The lock is the last field, so it drops after the variables are restored.
struct HeldEnv {
    _id: EnvVar,
    _dir: EnvVar,
    _lock: MutexGuard<'static, ()>,
}

fn hold(id: Option<&str>, dir: Option<&Path>) -> HeldEnv {
    let lock = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let id = EnvVar::set(DEV_PERSONA_ENV, id);
    let dir = EnvVar::set(PERSONA_DIR_ENV, dir.and_then(|path| path.to_str()));
    HeldEnv {
        _id: id,
        _dir: dir,
        _lock: lock,
    }
}

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "horbm-boot-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn samantha() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas/samantha.toml")
}

fn boot_err(
    root: &std::path::Path,
    knowledge: &std::path::Path,
    persona: &std::path::Path,
) -> PersonaError {
    match Brain::boot(root, knowledge, &root.join("cases"), Some(persona)) {
        Err(err) => err,
        Ok(_) => panic!("boot accepted an invalid persona"),
    }
}

fn boot_unset(root: &std::path::Path, knowledge: &std::path::Path) -> PersonaError {
    match Brain::boot(root, knowledge, &root.join("cases"), None) {
        Err(err) => err,
        Ok(_) => panic!("boot accepted a missing configuration"),
    }
}

fn cards_unset() -> PersonaError {
    match Brain::try_with_cards(NoCards, None) {
        Err(err) => err,
        Ok(_) => panic!("cards accepted a missing configuration"),
    }
}

fn open_unset(root: &std::path::Path, knowledge: &std::path::Path) -> PersonaError {
    match Brain::try_open(root, knowledge, &root.join("cases")) {
        Err(err) => err,
        Ok(_) => panic!("open accepted a missing configuration"),
    }
}

fn bare() -> MoveClaim<'static> {
    MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card: false,
        sympathy_script: false,
    }
}

#[test]
fn invalid_persona_refused_at_boot() {
    let _env = hold(None, None);
    let root = scratch("bad");
    std::fs::create_dir(&root).unwrap();
    let bad = root.join("bad.toml");
    std::fs::write(&bad, "schema = \"nope\"\nversion = 1\n").unwrap();
    let knowledge = root.join("knowledge");
    let err = boot_err(&root, &knowledge, &bad);
    assert_eq!(
        err,
        PersonaError::Invalid("persona file does not match the schema")
    );
    assert!(!knowledge.exists());

    let text = std::fs::read_to_string(samantha())
        .unwrap()
        .replace("never_fabricate = true", "never_fabricate = false");
    let lied = root.join("lied.toml");
    std::fs::write(&lied, text).unwrap();
    let err = boot_err(&root, &knowledge, &lied);
    assert_eq!(err, PersonaError::Invalid("never_fabricate must be true"));
    assert!(!knowledge.exists());

    let missing = root.join("missing.toml");
    let err = boot_err(&root, &knowledge, &missing);
    assert_eq!(err, PersonaError::Unreadable);
    assert!(!knowledge.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_without_a_file_loads_samantha() {
    let _env = hold(None, None);
    let root = scratch("none");
    std::fs::create_dir(&root).unwrap();
    let knowledge = root.join("knowledge");
    let brain = Brain::boot(&root, &knowledge, &root.join("cases"), None).unwrap();
    assert!(!knowledge.exists());
    assert_eq!(brain.persona().id(), "samantha");
    assert_eq!(brain.emotion().baseline().persona_id(), "samantha");
    assert_ne!(
        brain.emotion().baseline().identity_bits(),
        VoiceBaseline::standin().identity_bits()
    );
    assert_eq!(brain.inner().care(0).unwrap().token_str(), "heard");
    assert_eq!(brain.admit_move(bare()), MoveVerdict::Forbid);
    assert_eq!(brain.persona().unknown_line(), UNKNOWN_REPLY);
    assert_eq!(UNKNOWN_LINE, UNKNOWN_REPLY);
    let open = Brain::open(&root, &knowledge, &root.join("cases"));
    assert_eq!(open.persona().id(), "samantha");
    let cards = Brain::with_cards(NoCards, None);
    assert_eq!(cards.persona().id(), "samantha");
    assert_ne!(cards.persona().id(), "aria");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_loads_samantha_and_keeps_her_while_the_moment_swings() {
    let _env = hold(None, None);
    let root = scratch("sam");
    std::fs::create_dir(&root).unwrap();
    let knowledge = root.join("knowledge");
    let mut brain = Brain::boot(&root, &knowledge, &root.join("cases"), Some(&samantha())).unwrap();
    assert!(!knowledge.exists());
    assert_eq!(brain.persona().id(), "samantha");
    assert_eq!(brain.emotion().baseline().persona_id(), "samantha");
    assert_eq!(brain.persona().unknown_line(), UNKNOWN_REPLY);
    assert_eq!(brain.inner().care(0).unwrap().token_str(), "heard");
    let before = *brain.persona();
    let style = brain.persona().style();
    let bits = brain.emotion().baseline().identity_bits();
    let nudged = brain.inner_nudge("what?", None).unwrap();
    assert!(nudged.surprise > 0.0);
    assert_eq!(*brain.persona(), before);
    assert_eq!(brain.persona().style(), style);
    assert_eq!(brain.emotion().baseline().identity_bits(), bits);
    assert_ne!(brain.emotion().moment().pos[1].to_bits(), bits.pos[1]);
    assert_eq!(brain.inner().learned_count(), 0);
    assert_eq!(brain.admit_move(bare()), MoveVerdict::Forbid);
    let card = MoveClaim {
        tag: b"fact",
        claims_fact: true,
        has_card: true,
        sympathy_script: false,
    };
    assert_eq!(brain.admit_move(card), MoveVerdict::Allow);

    let text = std::fs::read_to_string(samantha())
        .unwrap()
        .replace("care = \"heard\"", "care = \"focus\"");
    let file = root.join("focus.toml");
    std::fs::write(&file, text).unwrap();
    let focused = Brain::boot(&root, &knowledge, &root.join("cases"), Some(&file)).unwrap();
    assert_eq!(focused.persona().id(), "samantha");
    assert_eq!(focused.inner().care(0).unwrap().token_str(), "focus");
    assert_eq!(focused.inner().want(0).unwrap().token_str(), "answer");
    assert_ne!(
        focused.inner().care(0).unwrap().token_str(),
        InnerLife::seeded().care(0).unwrap().token_str()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_installs_the_persona_the_env_names() {
    let root = scratch("env");
    let personas = root.join("personas");
    std::fs::create_dir_all(&personas).unwrap();
    let shipped_text = std::fs::read_to_string(samantha()).unwrap();
    let text = shipped_text
        .replace("id = \"samantha\"", "id = \"cedar\"")
        .replace("voice = \"samantha\"", "voice = \"cedar\"")
        .replace("valence = 0.28", "valence = 0.0");
    assert_ne!(text, shipped_text);
    std::fs::write(personas.join("cedar.toml"), text).unwrap();
    let _env = hold(Some("cedar"), Some(&personas));
    let knowledge = root.join("knowledge");
    let brain = Brain::boot(&root, &knowledge, &root.join("cases"), None).unwrap();
    assert!(!knowledge.exists());
    assert_eq!(brain.persona().id(), "cedar");
    let shipped = Persona::load(&samantha()).unwrap();
    assert_eq!(shipped.id(), "samantha");
    assert_ne!(brain.persona().id(), shipped.id());
    assert_ne!(
        brain.emotion().baseline().identity_bits(),
        shipped.baseline().identity_bits()
    );
    assert_ne!(
        brain.persona().valence().to_bits(),
        shipped.valence().to_bits()
    );
    let cards = Brain::try_with_cards(NoCards, None).expect("configured persona");
    assert_eq!(cards.persona().id(), "cedar");
    let explicit = Brain::boot(&root, &knowledge, &root.join("cases"), Some(&samantha())).unwrap();
    assert_eq!(explicit.persona().id(), shipped.id());
    assert_eq!(
        explicit.emotion().baseline().identity_bits(),
        shipped.baseline().identity_bits()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn boot_refuses_when_nothing_is_configured() {
    let root = scratch("unset");
    std::fs::create_dir(&root).unwrap();
    let _env = hold(None, Some(&root));
    let knowledge = root.join("knowledge");
    let unset = PersonaError::Invalid("development persona is not configured");
    assert_eq!(boot_unset(&root, &knowledge), unset);
    assert_eq!(cards_unset(), unset);
    assert_eq!(open_unset(&root, &knowledge), unset);
    assert_eq!(
        Brain::with_cards(NoCards, None).persona().id(),
        Persona::standin().id()
    );
    assert_eq!(
        Brain::open(&root, &knowledge, &root.join("cases"))
            .persona()
            .id(),
        Persona::standin().id()
    );
    assert!(!knowledge.exists());

    std::fs::write(
        root.join("development.toml"),
        "development_persona = \"\"\n",
    )
    .unwrap();
    assert_eq!(boot_unset(&root, &knowledge), unset);

    std::fs::write(
        root.join("development.toml"),
        "development_persona = \"   \"\n",
    )
    .unwrap();
    assert_eq!(boot_unset(&root, &knowledge), unset);

    std::fs::write(
        root.join("development.toml"),
        "development_persona = \"cedar\"\n",
    )
    .unwrap();
    assert_eq!(boot_unset(&root, &knowledge), PersonaError::Unreadable);

    let mismatched = std::fs::read_to_string(samantha())
        .unwrap()
        .replace("id = \"samantha\"", "id = \"maple\"")
        .replace("voice = \"samantha\"", "voice = \"maple\"");
    std::fs::write(root.join("cedar.toml"), mismatched).unwrap();
    assert_eq!(
        boot_unset(&root, &knowledge),
        PersonaError::Invalid("persona file id is not the configured id")
    );
    assert!(!knowledge.exists());
    let _ = std::fs::remove_dir_all(&root);
}
