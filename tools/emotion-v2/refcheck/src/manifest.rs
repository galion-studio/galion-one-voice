// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! sha256 manifest of capture originals. The tool reads audio; it does not rewrite it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use crate::check::{self, Role};
use crate::spec::{self, Spec, EMOTIONS};
use crate::wav;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub emotion: Option<String>,
    pub role: String,
    pub verdict: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub tool: String,
    pub standard: String,
    pub persona: String,
    pub consent: String,
    pub originals: String,
    pub files: Vec<SessionFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub tool: &'static str,
    pub verdict: String,
    pub files: usize,
    pub mismatches: Vec<String>,
    pub writable: Vec<String>,
    pub gaps: Vec<String>,
}

pub fn session_gaps(files: &[SessionFile], spec: &Spec) -> Vec<String> {
    let mut gaps = Vec::new();
    for name in &spec.emotions.names {
        let slug = spec::slug(name);
        let n = files
            .iter()
            .filter(|f| {
                f.role == "emotion"
                    && f.emotion.as_deref() == Some(slug.as_str())
                    && f.verdict.as_deref() == Some("PASS")
            })
            .count();
        if n < spec.takes.min_per_emotion as usize {
            gaps.push(format!("{slug} {n}/{}", spec.takes.min_per_emotion));
        }
    }
    let cal = files
        .iter()
        .filter(|f| f.role == "calibration" && f.verdict.as_deref() == Some("PASS"))
        .count();
    if cal < spec.calibration.min_passing as usize {
        gaps.push(format!(
            "calibration {cal}/{}",
            spec.calibration.min_passing
        ));
    }
    gaps
}

pub fn build_manifest(dir: &Path, spec: Option<&Spec>) -> Result<Manifest, String> {
    if !dir.is_dir() {
        return Err(format!("manifest: {} is not a directory", dir.display()));
    }
    let mut paths = Vec::new();
    collect(dir, &mut paths)?;
    paths.sort();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let rel = relative(dir, &path)?;
        let bytes = read_only(&path)?;
        let emotion = emotion_of(&rel);
        let role = role_of(&rel);
        let verdict = match spec {
            Some(spec) => {
                let pcm = wav::parse_wav(&bytes)?;
                let report = check::check_pcm(&pcm, spec, role);
                Some(report.verdict)
            }
            None => None,
        };
        files.push(SessionFile {
            path: rel,
            sha256: sha256_hex(&bytes),
            bytes: bytes.len() as u64,
            emotion,
            role: role.as_str().into(),
            verdict,
        });
    }
    let (standard, persona, consent) = match spec {
        Some(s) => (s.standard.clone(), s.persona.clone(), s.consent.clone()),
        None => (
            spec::STANDARD.into(),
            spec::PERSONA.into(),
            spec::CONSENT.into(),
        ),
    };
    Ok(Manifest {
        tool: "refcheck".into(),
        standard,
        persona,
        consent,
        originals: "read-only".into(),
        files,
    })
}

pub fn verify_manifest(
    manifest_path: &Path,
    spec: &Spec,
    require_session: bool,
    allow_writable: bool,
) -> Result<VerifyReport, String> {
    let text = fs::read_to_string(manifest_path).map_err(|e| format!("manifest: {e}"))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| format!("manifest: {e}"))?;
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut mismatches = Vec::new();
    let mut writable = Vec::new();
    if manifest.standard != spec.standard
        || manifest.persona != spec.persona
        || manifest.consent != spec.consent
    {
        mismatches.push("manifest identity does not match the spec".into());
    }
    if manifest.originals != "read-only" {
        mismatches.push("manifest originals marker is not read-only".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    // Session gaps use the verdict measured now, so a hand-edited PASS does not seal.
    let mut scored = manifest.files.clone();
    for (file, scored_file) in manifest.files.iter().zip(scored.iter_mut()) {
        if !seen.insert(file.path.clone()) {
            mismatches.push(format!("{} listed twice", file.path));
            continue;
        }
        let path = resolve(root, &file.path)?;
        if !allow_writable && is_writable(&path)? {
            writable.push(file.path.clone());
        }
        let bytes = read_only(&path)?;
        if bytes.len() as u64 != file.bytes {
            mismatches.push(format!(
                "{} length {} != {}",
                file.path,
                bytes.len(),
                file.bytes
            ));
        }
        let hash = sha256_hex(&bytes);
        if hash != file.sha256.to_ascii_lowercase() {
            mismatches.push(format!("{} sha256 changed", file.path));
        }
        if require_session {
            let role = role_of(&file.path);
            let emotion = emotion_of(&file.path);
            if file.role != role.as_str() || file.emotion != emotion {
                mismatches.push(format!(
                    "{} role or emotion does not match its path",
                    file.path
                ));
            }
            let pcm = wav::parse_wav(&bytes)?;
            let verdict = check::check_pcm(&pcm, spec, role).verdict;
            if file.verdict.as_deref() != Some(verdict.as_str()) {
                let stored = file.verdict.as_deref().unwrap_or("missing");
                mismatches.push(format!(
                    "{} verdict {stored} != checked {verdict}",
                    file.path
                ));
            }
            scored_file.role = role.as_str().into();
            scored_file.emotion = emotion;
            scored_file.verdict = Some(verdict);
        }
    }
    let gaps = if require_session {
        session_gaps(&scored, spec)
    } else {
        Vec::new()
    };
    let pass = mismatches.is_empty() && writable.is_empty() && gaps.is_empty();
    Ok(VerifyReport {
        tool: "refcheck",
        verdict: if pass { "PASS" } else { "FAIL" }.into(),
        files: manifest.files.len(),
        mismatches,
        writable,
        gaps,
    })
}

fn emotion_of(rel: &str) -> Option<String> {
    for comp in Path::new(rel).components() {
        if let std::path::Component::Normal(s) = comp {
            let c = s.to_string_lossy().to_ascii_lowercase();
            if EMOTIONS.iter().any(|name| spec::slug(name) == c) {
                return Some(c);
            }
        }
    }
    None
}

fn role_of(rel: &str) -> Role {
    for comp in Path::new(rel).components() {
        if let std::path::Component::Normal(s) = comp {
            if s.to_string_lossy().eq_ignore_ascii_case("calibration") {
                return Role::Calibration;
            }
        }
    }
    Role::Emotion
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("manifest: {}: {e}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("manifest: {e}"))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("manifest: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "manifest: symlink is not an original ({})",
                path.display()
            ));
        }
        if meta.is_dir() {
            collect(&path, out)?;
        } else if name.eq_ignore_ascii_case("MANIFEST.json") {
            continue;
        } else if name.to_ascii_lowercase().ends_with(".wav") {
            out.push(path);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| format!("manifest: {} is outside {}", path.display(), root.display()))?;
    let s = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    if s.is_empty() || s.split('/').any(|p| p == ".." || p.is_empty()) {
        return Err(format!("manifest: bad relative path {s}"));
    }
    Ok(s)
}

fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.starts_with('/') || rel.split(['/', '\\']).any(|p| p == ".." || p.is_empty()) {
        return Err(format!("manifest: {rel} escapes the session"));
    }
    let joined = root.join(rel);
    let root_c = root.canonicalize().map_err(|e| format!("manifest: {e}"))?;
    let file_c = joined
        .canonicalize()
        .map_err(|e| format!("manifest: {rel}: {e}"))?;
    if !file_c.starts_with(&root_c) {
        return Err(format!("manifest: {rel} escapes the session"));
    }
    Ok(file_c)
}

fn read_only(path: &Path) -> Result<Vec<u8>, String> {
    let _f = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .map_err(|e| format!("manifest: cannot read {}: {e}", path.display()))?;
    fs::read(path).map_err(|e| format!("manifest: cannot read {}: {e}", path.display()))
}

fn is_writable(path: &Path) -> Result<bool, String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)
        .map_err(|e| format!("manifest: {e}"))?
        .permissions()
        .mode();
    Ok(mode & 0o222 != 0)
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let dig = Sha256::digest(bytes);
    let mut s = String::with_capacity(dig.len() * 2);
    for b in dig {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

pub fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("json: {e}"))
}
