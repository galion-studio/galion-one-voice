// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Clip-store gate for the V2-S14 worker.
//!
//! `send_reference` is on only when owner clearance t273 is recorded and
//! the persona voice file says so. The path, when there is one, is a WAV
//! under `$GALION_CLIP_STORE` outside the git work tree. No counsel name
//! is filled in here. `LOCKS.toml` is not read and not written.

use super::sha256;
use std::io::Read;
use std::path::{Path, PathBuf};

const SAMANTHA_TOML: &str =
    include_str!("../../../../docs/galion-voice-v2/personas/voice/samantha.toml");
const JARVIS_TOML: &str =
    include_str!("../../../../docs/galion-voice-v2/personas/voice/jarvis.toml");

/// `samantha` or `jarvis`. Any other id is not a clip persona.
pub fn exact_persona(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().as_str() {
        "samantha" => Some("samantha"),
        "jarvis" => Some("jarvis"),
        _ => None,
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn clearance_path() -> PathBuf {
    if let Ok(p) = std::env::var("GALION_CLIP_CLEARANCE") {
        if !p.trim().is_empty() {
            return PathBuf::from(p.trim());
        }
    }
    repo_root().join("docs/galion-voice-v2/symphony/clearance-t273.rec")
}

/// True when `[worker] send_reference = true` in the committed voice file.
pub fn toml_send_reference(persona: &str) -> bool {
    let text = match exact_persona(persona) {
        Some("jarvis") => JARVIS_TOML,
        Some("samantha") => SAMANTHA_TOML,
        _ => return false,
    };
    let mut in_worker = false;
    for line in text.lines() {
        let s = line.trim();
        if s.starts_with('[') && s.ends_with(']') {
            in_worker = s == "[worker]";
            continue;
        }
        if in_worker {
            if let Some(rest) = s.strip_prefix("send_reference") {
                let rest = rest.trim().trim_start_matches('=').trim();
                return rest == "true";
            }
        }
    }
    false
}

struct Record {
    decision: String,
    owner: String,
    recorded: String,
    personas: Vec<String>,
    scope: String,
    public_demo: String,
    counsel: String,
    counsel_present: bool,
    private_cleared: String,
    wav_only: String,
    lock_edited: String,
    send_reference: String,
}

fn parse_record(text: &str) -> Option<Record> {
    let mut decision = None;
    let mut owner = None;
    let mut recorded = None;
    let mut personas = None;
    let mut scope = None;
    let mut public_demo = None;
    let mut counsel = None;
    let mut private_cleared = None;
    let mut wav_only = None;
    let mut lock_edited = None;
    let mut send_reference = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        let v = v.trim();
        match k.trim() {
            "decision" => decision = Some(v.to_string()),
            "owner" => owner = Some(v.to_string()),
            "recorded" => recorded = Some(v.to_string()),
            "personas" => {
                personas = Some(
                    v.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect(),
                );
            }
            "scope" => scope = Some(v.to_string()),
            "public_demo" => public_demo = Some(v.to_string()),
            "counsel" => counsel = Some(v.to_string()),
            "private_cleared" => private_cleared = Some(v.to_string()),
            "wav_only" => wav_only = Some(v.to_string()),
            "lock_edited" => lock_edited = Some(v.to_string()),
            "send_reference" => send_reference = Some(v.to_string()),
            _ => {}
        }
    }
    let counsel = counsel?;
    Some(Record {
        decision: decision?,
        owner: owner?,
        recorded: recorded?,
        personas: personas?,
        scope: scope?,
        public_demo: public_demo?,
        counsel_present: true,
        counsel,
        private_cleared: private_cleared?,
        wav_only: wav_only?,
        lock_edited: lock_edited?,
        send_reference: send_reference?,
    })
}

fn record_allows(text: &str, persona: &str) -> bool {
    let Some(rec) = parse_record(text) else {
        return false;
    };
    rec.counsel_present
        && rec.decision == "t273"
        && rec.owner == "Maciej Grajczyk"
        && !rec.recorded.is_empty()
        && rec.scope == "local-zero-shot-conditioning"
        && rec.public_demo == "no"
        && rec.private_cleared == "yes"
        && rec.wav_only == "yes"
        && rec.lock_edited == "no"
        && rec.send_reference == "yes"
        && rec.personas.iter().any(|p| p == persona)
}

/// `off` when the toggle is `0` or `off`. Otherwise the hub layer stays on.
/// The WAV is not rewritten.
pub fn jarvis_texture_from(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim) {
        Some(v) if v == "0" || v.eq_ignore_ascii_case("off") => "off",
        _ => "hub",
    }
}

pub fn jarvis_texture() -> &'static str {
    jarvis_texture_from(std::env::var("GALION_JARVIS_TEXTURE").ok().as_deref())
}

/// Ledger line for this persona says PRIVATE CLEARED under owner decision t273.
pub fn ledger_private_cleared(text: &str, persona: &str) -> bool {
    let needle = match persona {
        "samantha" => "Samantha · PRIVATE CLEARED",
        "jarvis" => "Jarvis · PRIVATE CLEARED",
        _ => return false,
    };
    text.lines().any(|line| {
        line.contains(needle) && line.contains("t273") && line.contains("Maciej Grajczyk")
    })
}

fn ledger_file_ok(persona: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(repo_root().join("LEDGER.md")) else {
        return false;
    };
    ledger_private_cleared(&text, persona)
}

/// Owner record t273 allows this persona, and the voice file unlocks the send.
pub fn clearance_recorded(persona: &str) -> bool {
    let Some(persona) = exact_persona(persona) else {
        return false;
    };
    if !toml_send_reference(persona) {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(clearance_path()) else {
        return false;
    };
    record_allows(&text, persona) && ledger_file_ok(persona)
}

fn sidecar_path(wav: &Path) -> PathBuf {
    let mut s = wav.as_os_str().to_owned();
    s.push(".sha256");
    PathBuf::from(s)
}

fn read_sha(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let token = text.split_whitespace().next()?.trim();
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_string())
    } else {
        None
    }
}

fn is_wav_file(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("wav") {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 12];
    f.read_exact(&mut head).is_ok() && &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE"
}

/// WAV under an outside store, or nothing. Does not invent a hash.
pub fn decide(
    persona: &str,
    clearance_text: &str,
    send_flag: bool,
    store: &Path,
    repo: &Path,
) -> Option<PathBuf> {
    let persona = exact_persona(persona)?;
    if !send_flag || !record_allows(clearance_text, persona) || !ledger_file_ok(persona) {
        return None;
    }
    if !store.is_absolute() {
        return None;
    }
    let store_c = std::fs::canonicalize(store).ok()?;
    let repo_c = std::fs::canonicalize(repo).ok()?;
    if store_c.starts_with(&repo_c) {
        return None;
    }
    let wav = store_c.join(persona).join("reference.wav");
    let wav_c = std::fs::canonicalize(&wav).ok()?;
    let persona_dir = store_c.join(persona);
    if !wav_c.starts_with(&persona_dir) || wav_c.starts_with(&repo_c) {
        return None;
    }
    if !is_wav_file(&wav_c) {
        return None;
    }
    let expected = read_sha(&sidecar_path(&wav_c))?;
    let got = sha256::file_hex(&wav_c).ok()?;
    if !got.eq_ignore_ascii_case(&expected) {
        return None;
    }
    Some(wav_c)
}

/// Path to put on the worker line, or `None` when the gate is closed.
pub fn reference_to_send(persona: &str) -> Option<PathBuf> {
    let store = std::env::var("GALION_CLIP_STORE").ok()?;
    let store = PathBuf::from(store.trim());
    let text = std::fs::read_to_string(clearance_path()).ok()?;
    decide(
        persona,
        &text,
        toml_send_reference(persona),
        &store,
        &repo_root(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn record() -> String {
        std::fs::read_to_string(
            repo_root().join("docs/galion-voice-v2/symphony/clearance-t273.rec"),
        )
        .expect("clearance record")
    }

    fn write_wav(path: &Path, header_ok: bool) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        if header_ok {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"RIFF");
            bytes.extend_from_slice(&36u32.to_le_bytes());
            bytes.extend_from_slice(b"WAVE");
            bytes.extend_from_slice(b"fmt ");
            bytes.extend_from_slice(&16u32.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&16_000u32.to_le_bytes());
            bytes.extend_from_slice(&32_000u32.to_le_bytes());
            bytes.extend_from_slice(&2u16.to_le_bytes());
            bytes.extend_from_slice(&16u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&0u32.to_le_bytes());
            f.write_all(&bytes).unwrap();
        } else {
            f.write_all(b"ID3\x03mp3-not-a-wav").unwrap();
        }
    }

    fn sidecart(wav: &Path) {
        let hex = sha256::file_hex(wav).unwrap();
        let side = sidecar_path(wav);
        std::fs::write(&side, format!("{hex}  reference.wav\n")).unwrap();
    }

    #[test]
    fn committed_clearance_names_no_counsel_and_unlocks_the_flag() {
        let text = record();
        let rec = parse_record(&text).expect("parse");
        assert_eq!(rec.decision, "t273");
        assert_eq!(rec.owner, "Maciej Grajczyk");
        assert_eq!(rec.recorded, "2026-11-03");
        assert!(
            rec.counsel.is_empty(),
            "counsel must stay empty in this commit"
        );
        assert!(rec.counsel_present);
        assert_eq!(rec.private_cleared, "yes");
        let ledger = std::fs::read_to_string(repo_root().join("LEDGER.md")).unwrap();
        assert!(ledger_private_cleared(&ledger, "samantha"));
        assert!(ledger_private_cleared(&ledger, "jarvis"));
        assert!(!ledger_private_cleared("no marker", "samantha"));
        assert_eq!(jarvis_texture_from(None), "hub");
        assert_eq!(jarvis_texture_from(Some("0")), "off");
        assert_eq!(jarvis_texture_from(Some("off")), "off");
        assert_eq!(jarvis_texture_from(Some("1")), "hub");
        assert_eq!(rec.public_demo, "no");
        assert_eq!(rec.lock_edited, "no");
        assert!(toml_send_reference("samantha"));
        assert!(toml_send_reference("jarvis"));
        assert!(!toml_send_reference("other"));
        assert!(record_allows(&text, "samantha"));
        assert!(record_allows(&text, "jarvis"));
        assert!(!record_allows(&text, "other"));
        let doc = std::fs::read_to_string(
            repo_root().join("docs/galion-voice-v2/symphony/CLIP-STORE.md"),
        )
        .unwrap();
        assert!(doc.contains("t273"));
        assert!(doc.contains("GALION_CLIP_STORE"));
        assert!(doc.contains("send_reference"));
        assert!(doc.contains("Maciej Grajczyk"));
        assert!(doc.contains("No counsel name is written here"));
        assert!(doc.contains("PRIVATE CLEARED"));
        assert!(doc.contains("GALION_JARVIS_TEXTURE"));
        assert!(doc.contains("BLOCKED"));
        let locks = std::fs::read_to_string(repo_root().join("LOCKS.toml")).unwrap();
        assert!(locks.contains("reference_clips = false"));
        assert!(!locks.contains("reference_clips = true"));
    }

    #[test]
    fn outside_wav_is_sent_and_everything_else_is_not() {
        let text = record();
        let repo = repo_root();
        let root = std::env::temp_dir().join(format!("galion-clip-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let wav = root.join("samantha").join("reference.wav");
        write_wav(&wav, true);
        sidecart(&wav);
        let sent = decide("samantha", &text, true, &root, &repo).expect("wav");
        assert_eq!(sent.file_name().unwrap(), "reference.wav");
        assert!(decide("jarvis", &text, true, &root, &repo).is_none());
        assert!(decide("samantha", &text, false, &root, &repo).is_none());
        let closed = text.replace("send_reference=yes", "send_reference=no");
        assert!(decide("samantha", &closed, true, &root, &repo).is_none());

        std::fs::write(
            sidecar_path(&wav),
            "0000000000000000000000000000000000000000000000000000000000000000  reference.wav\n",
        )
        .unwrap();
        assert!(decide("samantha", &text, true, &root, &repo).is_none());
        sidecart(&wav);

        let mp3 = root.join("jarvis").join("reference.wav");
        write_wav(&mp3, false);
        sidecart(&mp3);
        assert!(decide("jarvis", &text, true, &root, &repo).is_none());

        let inside = repo
            .join("target")
            .join(format!("clip-store-{}", std::process::id()));
        let inside_wav = inside.join("samantha").join("reference.wav");
        write_wav(&inside_wav, true);
        sidecart(&inside_wav);
        assert!(decide("samantha", &text, true, &inside, &repo).is_none());

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&inside);
    }

    #[test]
    fn stage_script_keeps_mp3_and_in_tree_stores_out() {
        let script = repo_root().join("scripts/clone/stage-clip-store.sh");
        let refused = std::process::Command::new("bash")
            .arg(&script)
            .env_remove("GALION_CLIP_STORE")
            .output()
            .expect("stage");
        assert_eq!(refused.status.code(), Some(2));
        let err = String::from_utf8_lossy(&refused.stderr);
        assert!(err.contains("GALION_CLIP_STORE"), "{err}");

        let inside = repo_root()
            .join("target")
            .join(format!("stage-inside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&inside);
        let inside_run = std::process::Command::new("bash")
            .arg(&script)
            .env("GALION_CLIP_STORE", &inside)
            .output()
            .expect("stage inside");
        assert_eq!(
            inside_run.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&inside_run.stderr)
        );
        let _ = std::fs::remove_dir_all(&inside);

        let refs = std::env::temp_dir().join(format!("galion-clip-refs-{}", std::process::id()));
        let store = std::env::temp_dir().join(format!("galion-clip-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&refs);
        let _ = std::fs::remove_dir_all(&store);
        std::fs::create_dir_all(&refs).unwrap();
        std::fs::write(refs.join("samantha.mp3"), b"not-a-wav").unwrap();
        let mp3 = std::process::Command::new("bash")
            .arg(&script)
            .env("GALION_CLIP_STORE", &store)
            .env("GALION_CLIP_REFS", &refs)
            .output()
            .expect("stage mp3");
        assert_eq!(
            mp3.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&mp3.stderr)
        );
        assert!(String::from_utf8_lossy(&mp3.stderr)
            .to_ascii_lowercase()
            .contains("wav"));
        assert!(!store.join("samantha/reference.wav").exists());

        std::fs::remove_file(refs.join("samantha.mp3")).unwrap();
        let absent = std::process::Command::new("bash")
            .arg(&script)
            .env("GALION_CLIP_STORE", &store)
            .env("GALION_CLIP_REFS", &refs)
            .output()
            .expect("stage absent");
        assert_eq!(
            absent.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&absent.stderr)
        );
        let out = String::from_utf8_lossy(&absent.stdout);
        assert!(
            out.contains("absent samantha") && out.contains("absent jarvis"),
            "{out}"
        );
        let _ = std::fs::remove_dir_all(&refs);
        let _ = std::fs::remove_dir_all(&store);
    }
}
