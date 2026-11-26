// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The duplex loop seats the persona development config names.
//!
//! A fixture persona drives one scripted turn. The shipped development file
//! keeps the style numbers the virtual-clock gate already speaks with.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use horbm::{CardHit, CardLookup, Persona as HorbmPersona};
use horbm_duplex::{
    open_persona, ConfiguredPersona, CoreAffect, Duplex, Hypothesis, HypothesisKind, LoopError,
    Parts, Persona, RecogError, Recognizer, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE,
};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine};

const DEV_PERSONA_ENV: &str = "GALION_HORBM_DEV_PERSONA";
const PERSONA_DIR_ENV: &str = "GALION_HORBM_PERSONA_DIR";
const KETTLE: &str = "The kettle is on the hob.";

const NORTHWIND: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "northwind"
voice = "northwind"

[traits]
attention = 0.62
curiosity = 0.58
honesty = 1.0
steadiness = 0.7
warmth = 0.5

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "invented-past", "unsourced-fact"]

[style]
brevity = 0.64
warmth = 0.48
max_sentences = 2

[baseline]
valence = 0.12
arousal = 0.42
dominance = 0.55
approach = 0.05
pitch_centre_hz = 180.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

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

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "horbm-duplex-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp persona dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: KETTLE.into(),
            })
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        1
    }
}

struct Scripted {
    lines: VecDeque<Vec<String>>,
    words: Vec<String>,
    shown: usize,
    frames: u32,
    open: bool,
}

impl Scripted {
    fn new(lines: &[&str]) -> Self {
        Self {
            lines: lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect(),
            words: Vec::new(),
            shown: 0,
            frames: 0,
            open: false,
        }
    }

    fn snapshot(&self, kind: HypothesisKind) -> Hypothesis {
        Hypothesis {
            kind,
            text: self.words[..self.shown].join(" "),
            start_ms: 0,
            end_ms: self.audio_ms(),
        }
    }
}

impl Recognizer for Scripted {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        if !self.open {
            self.words = self.lines.pop_front().unwrap_or_default();
            self.shown = 0;
            self.frames = 0;
            self.open = true;
        }
        self.frames = self.frames.saturating_add(1);
        if self.shown < self.words.len() {
            self.shown += 1;
            return Ok(vec![self.snapshot(HypothesisKind::Partial)]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.words.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "stand-in"
    }
}

struct ToneMouth {
    span: u32,
    left: u32,
    pending_first: bool,
    spoken: Vec<String>,
}

impl ToneMouth {
    fn new(span: u32) -> Self {
        Self {
            span,
            left: 0,
            pending_first: false,
            spoken: Vec::new(),
        }
    }

    fn spoken(&self) -> &[String] {
        &self.spoken
    }
}

impl Mouth for ToneMouth {
    fn id(&self) -> &str {
        "tone"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.spoken.push(line.text.clone());
        if line.text.trim().is_empty() {
            self.left = 0;
            self.pending_first = false;
            return;
        }
        self.left = self.span;
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let mut samples = vec![0i16; FRAME_SAMPLES];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = if index % 2 == 0 { 400 } else { -400 };
        }
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        self.left = 0;
        self.pending_first = false;
    }

    fn speaking(&self) -> bool {
        self.left > 0
    }
}

fn tone() -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        *sample = if index % 2 == 0 { 800 } else { -800 };
    }
    pcm
}

fn silence() -> Vec<i16> {
    vec![0i16; FRAME_SAMPLES]
}

#[test]
fn northwind_fixture_runs_a_duplex_turn() {
    let parsed = HorbmPersona::parse(NORTHWIND).expect("northwind fixture");
    assert_eq!(parsed.id(), "northwind");
    assert_eq!(parsed.voice(), "northwind");
    let dir = TempDir::new("northwind");
    let path = dir.0.join("northwind.toml");
    std::fs::write(&path, NORTHWIND).expect("write fixture");
    let _hold = hold(Some(parsed.id()), Some(&dir.0));
    let loaded = HorbmPersona::load_development().expect("configured northwind");
    assert_eq!(loaded, parsed);
    let affect = CoreAffect::from_baseline(parsed.baseline());
    assert_eq!(affect.baseline().persona_id(), parsed.id());
    assert_eq!(affect.baseline().voice_id(), parsed.voice());
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Scripted::new(&["where is the kettle"]),
            emotion: affect,
            breath: horbm_duplex::SpeakingBreath,
            persona: ConfiguredPersona::from_persona(parsed),
            mouth: ToneMouth::new(24),
        },
        Kettle,
    )
    .expect("northwind duplex");
    duplex.set_aec(false);
    let heard = tone();
    let quiet = silence();
    for _ in 0..5 {
        duplex.tick(&heard).expect("word");
    }
    duplex.tick(&quiet).expect("eager");
    duplex.tick(&quiet).expect("commit");
    assert_eq!(duplex.mouth().spoken(), &[KETTLE.to_string()]);
    assert_eq!(duplex.persona().id(), "northwind");
    assert_eq!(duplex.persona().voice(), "northwind");
}

#[test]
fn shipped_dev_style_matches_the_old_consts() {
    let _hold = hold(None, None);
    let persona = HorbmPersona::load_development().expect("shipped development persona");
    let style = persona.style();
    assert!((style.brevity - 0.55).abs() < 1.0e-5, "{}", style.brevity);
    assert!((style.warmth - 0.8).abs() < 1.0e-5, "{}", style.warmth);
    assert_eq!(style.max_sentences, 2);
    let seated = horbm_duplex::ConfiguredPersona::from_persona(persona);
    assert!((seated.style().brevity - style.brevity).abs() < 1.0e-5);
    assert!((seated.style().warmth - style.warmth).abs() < 1.0e-5);
    assert_eq!(seated.style().max_sentences, style.max_sentences);
}

#[test]
fn parked_persona_is_refused_by_config_not_by_name() {
    let _hold = hold(None, None);
    let configured = HorbmPersona::load_development().expect("shipped development persona");
    assert!(open_persona(configured.id()).is_ok());
    let parked_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas/aria.toml");
    let parked = HorbmPersona::load(&parked_path).expect("parked persona file");
    assert_ne!(parked.id(), configured.id());
    let by_file = open_persona(parked.id()).expect_err("parked id");
    let by_other = open_persona("northwind").expect_err("fixture id");
    assert_eq!(
        by_file,
        LoopError::Persona("the id is not the configured persona")
    );
    assert_eq!(by_file, by_other);
    let text = by_file.to_string();
    assert!(
        !text.to_ascii_lowercase().contains(parked.id()),
        "refusal named the parked id: {text}"
    );
}
