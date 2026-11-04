// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Scratch repo + clip store for the consent gate tests. Synthetic only:
//! a generated tone, fixture names, pinned sizes as sparse files. No real
//! voice, no real weights.
#![allow(dead_code)]

use galion_consent::{sha256, weight_files, Env};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQ: AtomicUsize = AtomicUsize::new(0);

pub const TODAY: (i32, u32, u32) = (2026, 11, 4);

pub struct Tree {
    pub root: PathBuf,
    pub repo: PathBuf,
    pub store: PathBuf,
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn wav_bytes(rate: u32, channels: u16, width: u16, seconds: f64) -> Vec<u8> {
    let frames = (rate as f64 * seconds).round() as u32;
    let block = channels as u32 * width as u32;
    let data_len = frames * block;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * block).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&(width * 8).to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let v = ((i as f64 * 0.05).sin() * 8000.0) as i16;
        for _ in 0..channels {
            if width == 2 {
                out.extend_from_slice(&v.to_le_bytes());
            } else {
                out.push(((v >> 8) as i8 as i16 + 128) as u8);
            }
        }
    }
    out
}

pub fn fields(persona: &str, sha: &str, seconds: f64) -> Vec<(String, String)> {
    [
        ("date", "2026-11-04 10:00 CET".to_string()),
        ("persona", persona.to_string()),
        ("work", "Galion gate fixture tone".to_string()),
        ("performer", "Fixture Performer".to_string()),
        ("grantor", "Fixture Performer".to_string()),
        ("instrument", "fixture release 001".to_string()),
        ("grantee", "Galion Studio".to_string()),
        ("scope", "local-zero-shot-conditioning".to_string()),
        ("public_demo", "no".to_string()),
        ("expiry", "2027-12-31".to_string()),
        ("operator", "Fixture Operator".to_string()),
        ("withdrawal", "written notice, removed within 24h".to_string()),
        ("counsel", "Fixture Counsel".to_string()),
        ("sha256", sha.to_string()),
        ("duration_s", format!("{seconds:.3}")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub fn render(fields: &[(String, String)]) -> String {
    fields.iter().map(|(k, v)| format!("{k}: {v}\n")).collect()
}

impl Tree {
    /// A ready tree: lock open, worker present, pinned sizes on disk, no clip.
    pub fn new(tag: &str) -> Self {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("galion-consent-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let repo = root.join("repo");
        let store = root.join("store");
        fs::create_dir_all(repo.join("scripts/clone")).unwrap();
        fs::create_dir_all(&store).unwrap();
        fs::write(repo.join("LOCKS.toml"), "[formats]\nreference_clips = true\n\n[formats.exceptions.v2s14_chatterbox]\nreference_clips = true # fixture\n").unwrap();
        fs::write(repo.join("scripts/clone/chatterbox_worker.py"), "# fixture\n").unwrap();
        let mut pins = String::from("# fixture pins\n");
        for (i, rel) in weight_files("nano").into_iter().chain(weight_files("turbo")).enumerate() {
            let size = 100 + i as u64;
            if !pins.contains(&format!("{rel} ")) {
                pins.push_str(&format!("{rel} {} {size} fixture fixture fixture\n", "0".repeat(64)));
                let path = repo.join("weights/clone").join(&rel);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::File::create(&path).unwrap().set_len(size).unwrap();
            }
        }
        fs::write(repo.join("scripts/clone/PINS.sha256"), pins).unwrap();
        Tree { root, repo, store }
    }

    pub fn env(&self) -> Env {
        let mut env = Env::at(&self.repo);
        env.store = Some(self.store.clone());
        env.today = Some(TODAY);
        env
    }

    pub fn folder(&self, persona: &str) -> PathBuf {
        self.store.join(persona)
    }

    /// Place a wav + sidecar + consent.txt as ingest-clip.sh would.
    pub fn place_with(&self, persona: &str, wav: &[u8], edit: impl FnOnce(&mut Vec<(String, String)>)) {
        let dir = self.folder(persona);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("reference.wav");
        fs::write(&path, wav).unwrap();
        let sha = sha256::file_hex(&path).unwrap();
        fs::write(dir.join("reference.wav.sha256"), format!("{sha}  reference.wav\n")).unwrap();
        let secs = galion_consent::wav_info(&path).map(|w| w.0).unwrap_or(10.0);
        let mut f = fields(persona, &sha, secs);
        edit(&mut f);
        fs::write(dir.join("consent.txt"), render(&f)).unwrap();
    }

    pub fn place(&self, persona: &str) {
        self.place_with(persona, &wav_bytes(16_000, 1, 2, 10.0), |_| {});
    }

    pub fn set(&self, persona: &str, key: &str, value: &str) {
        let path = self.folder(persona).join("consent.txt");
        let text = fs::read_to_string(&path).unwrap();
        let out: String = text
            .lines()
            .map(|l| if l.starts_with(&format!("{key}:")) { format!("{key}: {value}\n") } else { format!("{l}\n") })
            .collect();
        fs::write(path, out).unwrap();
    }
}

pub fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
}
