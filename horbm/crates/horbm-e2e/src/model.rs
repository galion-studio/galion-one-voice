// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pinned Vosk small-en download.
//!
//! The zip is checked against `vosk-model.manifest` and unpacked into a cache
//! outside this repository. A mismatch deletes the partial file. The model is
//! never written into the tree. The libvosk zip pinned beside it is Linux
//! x86_64 only; other targets try `GALION_VOSK_LIB` and otherwise leave the
//! ear on the stub.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

const MANIFEST: &str = include_str!("../vosk-model.manifest");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsrSkip {
    CurlMissing,
    DownloadFailed,
    Sha256Mismatch,
    UnpackFailed,
    ModelUnavailable,
    LibUnavailable,
    CacheInsideRepo,
}

impl AsrSkip {
    pub fn token(self) -> &'static str {
        match self {
            Self::CurlMissing => "curl_missing",
            Self::DownloadFailed => "download_failed",
            Self::Sha256Mismatch => "sha256_mismatch",
            Self::UnpackFailed => "unpack_failed",
            Self::ModelUnavailable => "model_unavailable",
            Self::LibUnavailable => "lib_unavailable",
            Self::CacheInsideRepo => "cache_inside_repo",
        }
    }
}

struct Pins {
    model_name: String,
    model_sha256: String,
    model_url: String,
    lib_sha256: String,
    lib_url: String,
    lib_file: String,
}

pub struct ReadyModel {
    pub model_dir: PathBuf,
    pub lib: Option<PathBuf>,
    pub model_name: String,
}

pub fn ensure_model() -> Result<ReadyModel, AsrSkip> {
    let pins = pins()?;
    let cache = default_cache_dir();
    if !cache_outside_repo(&cache) {
        return Err(AsrSkip::CacheInsideRepo);
    }
    fs::create_dir_all(&cache).map_err(|_| AsrSkip::ModelUnavailable)?;
    let _lock = FetchLock::acquire(&cache)?;
    let model_dir = ensure_model_dir(&cache, &pins)?;
    let lib = ensure_lib(&cache, &pins)?;
    Ok(ReadyModel {
        model_dir,
        lib,
        model_name: pins.model_name,
    })
}

pub fn default_cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("GALION_VOSK_CACHE") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("galion").join("vosk");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home)
                .join(".cache")
                .join("galion")
                .join("vosk");
        }
    }
    std::env::temp_dir().join("galion-vosk-cache")
}

pub fn cache_outside_repo(path: &Path) -> bool {
    let path = absolute(path);
    let repo = absolute(&repo_root());
    !path.starts_with(&repo)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn absolute(path: &Path) -> PathBuf {
    if let Ok(canon) = path.canonicalize() {
        return canon;
    }
    let raw = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    normalize(&raw)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn pins() -> Result<Pins, AsrSkip> {
    let map = parse_manifest(MANIFEST);
    let pins = Pins {
        model_name: field(&map, "model_name")?,
        model_sha256: field(&map, "model_sha256")?,
        model_url: field(&map, "model_url")?,
        lib_sha256: field(&map, "lib_sha256")?,
        lib_url: field(&map, "lib_url")?,
        lib_file: field(&map, "lib_file")?,
    };
    if !sha_ok(&pins.model_sha256) || !sha_ok(&pins.lib_sha256) {
        return Err(AsrSkip::ModelUnavailable);
    }
    if !https_url(&pins.model_url) || !https_url(&pins.lib_url) {
        return Err(AsrSkip::ModelUnavailable);
    }
    if pins.model_name.contains(['/', '\\']) || pins.lib_file.contains(['/', '\\']) {
        return Err(AsrSkip::ModelUnavailable);
    }
    Ok(pins)
}

fn field(map: &[(String, String)], key: &str) -> Result<String, AsrSkip> {
    map.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
        .ok_or(AsrSkip::ModelUnavailable)
}

fn sha_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn https_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    !rest.is_empty() && !url.contains([' ', '\n', '\r', '\t', '"', '\'']) && !url.starts_with('-')
}

pub fn parse_manifest(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        out.push((key.to_string(), value.to_string()));
    }
    out
}

fn ensure_model_dir(cache: &Path, pins: &Pins) -> Result<PathBuf, AsrSkip> {
    let dir = cache.join(&pins.model_name);
    let stamp = cache.join("model.sha256");
    if model_ready(&dir) && stamp_matches(&stamp, &pins.model_sha256) {
        return Ok(dir);
    }
    let _ = fs::remove_dir_all(&dir);
    let zip = cache.join(format!("{}.zip.part", pins.model_name));
    fetch_zip(&pins.model_url, &zip, &pins.model_sha256)?;
    unpack(&zip, cache)?;
    let _ = fs::remove_file(&zip);
    if !model_ready(&dir) || !tree_inside(cache) {
        let _ = fs::remove_dir_all(&dir);
        return Err(AsrSkip::ModelUnavailable);
    }
    write_stamp(&stamp, &pins.model_sha256)?;
    eprintln!("vosk model: pinned cache ready");
    Ok(dir)
}

fn model_ready(dir: &Path) -> bool {
    dir.join("am/final.mdl").is_file() && dir.join("conf/model.conf").is_file()
}

fn ensure_lib(cache: &Path, pins: &Pins) -> Result<Option<PathBuf>, AsrSkip> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        match ensure_linux_lib(cache, pins) {
            Ok(path) => return Ok(Some(path)),
            Err(AsrSkip::LibUnavailable)
            | Err(AsrSkip::DownloadFailed)
            | Err(AsrSkip::CurlMissing) => {}
            Err(AsrSkip::Sha256Mismatch) => return Err(AsrSkip::Sha256Mismatch),
            Err(other) => return Err(other),
        }
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (cache, pins);
    }
    if let Some(path) = env_lib() {
        return Ok(Some(path));
    }
    Ok(None)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn ensure_linux_lib(cache: &Path, pins: &Pins) -> Result<PathBuf, AsrSkip> {
    let root = cache.join("libvosk");
    let stamp = cache.join("lib.sha256");
    if let Some(path) = find_named(&root, &pins.lib_file) {
        if stamp_matches(&stamp, &pins.lib_sha256) {
            return Ok(path);
        }
    }
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).map_err(|_| AsrSkip::LibUnavailable)?;
    let zip = cache.join("libvosk.zip.part");
    fetch_zip(&pins.lib_url, &zip, &pins.lib_sha256)?;
    unpack(&zip, &root)?;
    let _ = fs::remove_file(&zip);
    if !tree_inside(&root) {
        let _ = fs::remove_dir_all(&root);
        return Err(AsrSkip::LibUnavailable);
    }
    let Some(path) = find_named(&root, &pins.lib_file) else {
        let _ = fs::remove_dir_all(&root);
        return Err(AsrSkip::LibUnavailable);
    };
    write_stamp(&stamp, &pins.lib_sha256)?;
    eprintln!("vosk lib: pinned linux x86_64 cache ready");
    Ok(path)
}

fn env_lib() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("GALION_VOSK_LIB")?);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn fetch_zip(url: &str, dest: &Path, sha: &str) -> Result<(), AsrSkip> {
    if dest.is_file() {
        let _ = fs::remove_file(dest);
    }
    if !https_url(url) {
        return Err(AsrSkip::DownloadFailed);
    }
    let mut command = Command::new("curl");
    command.args([
        "-fL",
        "--retry",
        "3",
        "--retry-delay",
        "2",
        "--max-time",
        "180",
        "-sS",
        "-A",
        "galion-horbm-e2e",
        "-o",
    ]);
    command.arg(dest);
    command.arg(url);
    let status = match command.status() {
        Ok(status) => status,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(AsrSkip::CurlMissing),
        Err(_) => return Err(AsrSkip::DownloadFailed),
    };
    if !status.success() {
        let _ = fs::remove_file(dest);
        return Err(AsrSkip::DownloadFailed);
    }
    let got = sha256_file(dest)?;
    if !got.eq_ignore_ascii_case(sha) {
        let _ = fs::remove_file(dest);
        return Err(AsrSkip::Sha256Mismatch);
    }
    Ok(())
}

fn unpack(zip: &Path, dest: &Path) -> Result<(), AsrSkip> {
    fs::create_dir_all(dest).map_err(|_| AsrSkip::UnpackFailed)?;
    let status = Command::new("unzip")
        .arg("-q")
        .arg("-o")
        .arg(zip)
        .arg("-d")
        .arg(dest)
        .status()
        .map_err(|_| AsrSkip::UnpackFailed)?;
    if status.success() {
        Ok(())
    } else {
        Err(AsrSkip::UnpackFailed)
    }
}

fn tree_inside(root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            return false;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return false;
            };
            let path = entry.path();
            let Ok(canon) = path.canonicalize() else {
                return false;
            };
            if !canon.starts_with(&root) {
                return false;
            }
            if canon.is_dir() {
                stack.push(canon);
            }
        }
    }
    true
}

fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|item| item.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

fn stamp_matches(path: &Path, sha: &str) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    text.trim().eq_ignore_ascii_case(sha)
}

fn write_stamp(path: &Path, sha: &str) -> Result<(), AsrSkip> {
    fs::write(path, format!("{sha}\n")).map_err(|_| AsrSkip::ModelUnavailable)
}

fn sha256_file(path: &Path) -> Result<String, AsrSkip> {
    let mut file = File::open(path).map_err(|_| AsrSkip::ModelUnavailable)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|_| AsrSkip::ModelUnavailable)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

struct FetchLock {
    path: PathBuf,
}

impl FetchLock {
    fn acquire(cache: &Path) -> Result<Self, AsrSkip> {
        let path = cache.join(".fetch-lock");
        for _ in 0..100 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(&path) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    thread::sleep(Duration::from_millis(200));
                }
                Err(_) => return Err(AsrSkip::ModelUnavailable),
            }
        }
        Err(AsrSkip::ModelUnavailable)
    }
}

impl Drop for FetchLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_is_stale(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    modified
        .elapsed()
        .map(|age| age > Duration::from_secs(15 * 60))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const MODEL_SHA: &str = "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498";
    const LIB_SHA: &str = "bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2";

    #[test]
    fn the_manifest_pins_the_official_small_en_zip() {
        let pins = pins().expect("manifest");
        assert_eq!(pins.model_name, "vosk-model-small-en-us-0.15");
        assert_eq!(pins.model_sha256, MODEL_SHA);
        assert_eq!(pins.lib_sha256, LIB_SHA);
        assert!(pins.model_url.contains("vosk-model-small-en-us-0.15.zip"));
        assert!(parse_manifest(MANIFEST)
            .iter()
            .any(|(key, value)| key == "license" && value == "Apache-2.0"));
    }

    #[test]
    fn sha256_of_abc_matches_the_known_digest() {
        let mut hasher = Sha256::new();
        hasher.update(b"abc");
        assert_eq!(
            format!("{:x}", hasher.finalize()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_file_hash_mismatch_is_visible() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("horbm-e2e-sha-{nanos}"));
        fs::write(&path, b"abc").unwrap();
        let got = sha256_file(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!got.eq_ignore_ascii_case(MODEL_SHA));
    }

    #[test]
    fn the_default_cache_is_outside_the_repo() {
        assert!(cache_outside_repo(&default_cache_dir()));
        assert!(!cache_outside_repo(&repo_root().join("weights")));
        assert!(!cache_outside_repo(&repo_root()));
        assert!(cache_outside_repo(
            &std::env::temp_dir().join("galion-vosk-cache")
        ));
    }

    #[test]
    fn only_https_urls_are_fetched() {
        assert!(https_url(
            "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip"
        ));
        assert!(!https_url("http://alphacephei.com/vosk/models/x.zip"));
        assert!(!https_url("file:///tmp/model.zip"));
        assert!(!https_url("https://example.com/a b.zip"));
    }

    #[test]
    fn skip_tokens_say_why_the_ear_stayed_stub() {
        assert_eq!(AsrSkip::DownloadFailed.token(), "download_failed");
        assert_eq!(AsrSkip::Sha256Mismatch.token(), "sha256_mismatch");
        assert_eq!(AsrSkip::LibUnavailable.token(), "lib_unavailable");
        assert_eq!(AsrSkip::CacheInsideRepo.token(), "cache_inside_repo");
    }
}
