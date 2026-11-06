//! Invoke the unmodified universal scorer and cbx renderer.
//! Never writes into protected originals or existing clone / .cbxvoice names.

use crate::is_protected_voice_name;
use crate::scorer::{score_argv, ResolvedScorer};
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn cloning_v2_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn default_scorer_py() -> PathBuf {
    cloning_v2_root().join("scorers/universal/py/score_universal.py")
}

pub fn default_scorer_python() -> PathBuf {
    cloning_v2_root().join("scorers/universal/venv/bin/python3")
}

pub fn default_cbx() -> PathBuf {
    cloning_v2_root().join("emotion-fix/cbx-emo/target/release/cbx")
}

pub fn default_catalog() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("probe_catalog.json")
}

pub fn render_lock() -> PathBuf {
    cloning_v2_root().join("emotion-blend/render.lock")
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .context("sha256sum")?;
    if !out.status.success() {
        bail!("sha256sum failed for {}", path.display());
    }
    let s = String::from_utf8_lossy(&out.stdout);
    Ok(s.split_whitespace().next().unwrap_or("").to_string())
}

fn run_logged(cmd: &mut Command, log: &Path) -> Result<()> {
    let displayed = format!("{cmd:?}");
    eprintln!("[run] {displayed}");
    let output = cmd.output().with_context(|| format!("spawn {displayed}"))?;
    if let Some(parent) = log.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .with_context(|| format!("open log {}", log.display()))?;
    writeln!(f, "$ {displayed}")?;
    f.write_all(&output.stdout)?;
    f.write_all(&output.stderr)?;
    writeln!(f, "exit={}\n", output.status)?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("command failed: {displayed}\n{err}");
    }
    Ok(())
}

/// Invoke unmodified `score_universal.py` on the given wavs.
pub fn score_universal(
    python: &Path,
    scorer: &Path,
    wavs: &[PathBuf],
    out_json: &Path,
    log: &Path,
    resume: bool,
) -> Result<()> {
    if wavs.is_empty() {
        bail!("score_universal called with no wavs");
    }
    if let Some(parent) = out_json.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if out_json.exists() {
        if resume {
            eprintln!("[targets] resume: reuse {}", out_json.display());
            return Ok(());
        }
        bail!("refuse overwrite: {}", out_json.display());
    }
    let mut cmd = Command::new(python);
    cmd.arg(scorer);
    for w in wavs {
        cmd.arg(w);
    }
    cmd.arg("--out").arg(out_json).arg("--threads").arg("2");
    cmd.stdin(Stdio::null());
    run_logged(&mut cmd, log)
}

/// Invoke the resolved backend (universal by default, persona when opted in).
pub fn score_with(
    resolved: &ResolvedScorer,
    wavs: &[PathBuf],
    out_json: &Path,
    log: &Path,
    resume: bool,
) -> Result<()> {
    if wavs.is_empty() {
        bail!("score called with no wavs");
    }
    if let Some(parent) = out_json.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if out_json.exists() {
        if resume {
            eprintln!("[targets] resume: reuse {}", out_json.display());
            return Ok(());
        }
        bail!("refuse overwrite: {}", out_json.display());
    }
    let mut cmd = Command::new(&resolved.python);
    for a in score_argv(resolved, wavs, out_json) {
        cmd.arg(a);
    }
    cmd.stdin(Stdio::null());
    eprintln!("[targets] scorer={} script={}", resolved.kind.as_str(), resolved.script.display());
    run_logged(&mut cmd, log)
}

pub fn write_manifest(path: &Path, lines: &[(String, String)], resume: bool) -> Result<()> {
    if path.exists() {
        if resume {
            eprintln!("[targets] resume: reuse {}", path.display());
            return Ok(());
        }
        bail!("refuse overwrite: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    for (id, text) in lines {
        if id.contains('\t') || text.contains('\n') {
            bail!("refusing control characters in manifest line {id}");
        }
        writeln!(f, "{id}\t{text}")?;
    }
    Ok(())
}

fn flock_cbx(cbx: &Path, rest: &[&str]) -> Command {
    let lock = render_lock();
    let mut cmd = Command::new("flock");
    cmd.arg(&lock)
        .arg("flock")
        .arg("/tmp/cbx-render.lock")
        .arg("nice")
        .arg("-n")
        .arg("10")
        .arg(cbx)
        .arg("--intra-threads")
        .arg("2")
        .arg("--inter-threads")
        .arg("2")
        .arg("--ep")
        .arg("cpu");
    for a in rest {
        cmd.arg(a);
    }
    cmd.stdin(Stdio::null());
    cmd
}

pub fn install_probe_voice_in_cache(name: &str, src: &Path) -> Result<()> {
    if is_protected_voice_name(name) {
        bail!("refusing to install protected voice name {name} into cache");
    }
    if !src.is_file() {
        bail!("probe voice missing: {}", src.display());
    }
    let dest = default_voice_cache().join(format!("{name}.cbxvoice"));
    if dest.exists() {
        eprintln!("[targets] cache already has {}, not overwriting", dest.display());
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = std::fs::read(src)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest)
        .with_context(|| format!("create_new cache voice {}", dest.display()))?;
    f.write_all(&data)?;
    eprintln!("[targets] installed new cache voice {}", dest.display());
    Ok(())
}

pub fn voice_add(
    cbx: &Path,
    name: &str,
    original: &Path,
    out_file: &Path,
    log: &Path,
    resume: bool,
) -> Result<()> {
    if is_protected_voice_name(name) {
        bail!("refusing protected voice name {name}");
    }
    let cache = default_voice_cache().join(format!("{name}.cbxvoice"));
    if cache.exists() && !resume {
        bail!("refuse overwrite existing cached voice {}", cache.display());
    }
    if out_file.exists() {
        if resume {
            eprintln!("[targets] resume: reuse {}", out_file.display());
            return install_probe_voice_in_cache(name, out_file);
        }
        bail!("refuse overwrite existing probe voice {}", out_file.display());
    }
    if let Some(parent) = out_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let orig = original.to_string_lossy().to_string();
    let out = out_file.to_string_lossy().to_string();
    let mut cmd = flock_cbx(
        cbx,
        &[
            "voice",
            "add",
            "--dtype",
            "fp32",
            "--name",
            name,
            "--voice-wav",
            orig.as_str(),
            "--out-file",
            out.as_str(),
        ],
    );
    run_logged(&mut cmd, log)?;
    install_probe_voice_in_cache(name, out_file)
}

pub fn default_voice_cache() -> PathBuf {
    let home = std::env::var("HF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let h = std::env::var("HOME").unwrap_or_else(|_| "/home/box".into());
            PathBuf::from(h).join(".cache/huggingface")
        });
    home.join("cbx/voices")
}

pub fn batch_speak(
    cbx: &Path,
    voice: &str,
    manifest: &Path,
    out_dir: &Path,
    pace: f64,
    log: &Path,
    expected_ids: &[String],
    resume: bool,
) -> Result<()> {
    if resume && !expected_ids.is_empty() {
        let all_there = expected_ids.iter().all(|id| out_dir.join(format!("{id}.wav")).is_file());
        if all_there {
            eprintln!("[targets] resume: reuse synthesized wavs in {}", out_dir.display());
            return Ok(());
        }
    }
    if is_protected_voice_name(voice) {
        bail!("refusing to speak with protected deployed voice name {voice}");
    }
    std::fs::create_dir_all(out_dir)?;
    let pace_s = format!("{pace}");
    let voice_s = voice.to_string();
    let man = manifest.to_string_lossy().to_string();
    let out = out_dir.to_string_lossy().to_string();
    let mut cmd = flock_cbx(
        cbx,
        &[
            "batch",
            "--dtype",
            "fp32",
            "--voice",
            voice_s.as_str(),
            "--manifest",
            man.as_str(),
            "--out-dir",
            out.as_str(),
            "--pace",
            pace_s.as_str(),
            "--temperature",
            "0",
            "--seed",
            "0",
            "--repetition-penalty",
            "1.2",
        ],
    );
    run_logged(&mut cmd, log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_protected_voice_add() {
        let err = voice_add(
            Path::new("/bin/true"),
            "own-samantha",
            Path::new("/tmp/orig.wav"),
            Path::new("/tmp/x.cbxvoice"),
            Path::new("/tmp/x.log"),
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("protected"), "{err}");
        let err = voice_add(
            Path::new("/bin/true"),
            "own-jarvis",
            Path::new("/tmp/orig.wav"),
            Path::new("/tmp/x.cbxvoice"),
            Path::new("/tmp/x.log"),
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("protected"));
    }

    #[test]
    fn cache_install_refuses_protected() {
        let err = install_probe_voice_in_cache("own-samantha", Path::new("/tmp/nope.cbxvoice"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("protected"), "{err}");
    }

    #[test]
    fn default_paths_point_inside_cloning_v2() {
        let root = cloning_v2_root();
        assert!(root.ends_with("cloning-v2"), "{}", root.display());
        assert!(default_scorer_py().ends_with("scorers/universal/py/score_universal.py"));
        assert!(default_catalog().ends_with("probe_catalog.json"));
    }

    #[test]
    fn persona_opt_in_invokes_stub_script() {
        let dir = std::env::temp_dir().join(format!("galion-persona-stub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("score_persona_stub.py");
        let captured = dir.join("argv.json");
        let wav = dir.join("t.wav");
        let out = dir.join("out.json");
        let log = dir.join("log.txt");
        std::fs::write(&wav, b"x").unwrap();
        std::fs::write(
            &stub,
            format!(
                "#!/usr/bin/env python3\nimport json,sys\nfrom pathlib import Path\nPath({:?}).write_text(json.dumps(sys.argv))\nout=sys.argv[sys.argv.index('--out')+1]\nPath(out).write_text('{{\"files\":[]}}')\n",
                captured
            ),
        )
        .unwrap();
        let resolved = crate::scorer::resolve_scorer(
            crate::scorer::ScorerKind::Persona,
            Some(stub.clone()),
            Some(PathBuf::from("python3")),
            Some(PathBuf::from("/tmp/demo-persona.toml")),
        );
        score_with(&resolved, &[wav], &out, &log, false).unwrap();
        let argv: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&captured).unwrap()).unwrap();
        assert!(argv.iter().any(|a| a.ends_with("score_persona_stub.py")), "{argv:?}");
        assert!(argv.contains(&"--persona".to_string()) || argv.contains(&"--persona-file".to_string()), "{argv:?}");
        assert!(argv.contains(&"/tmp/demo-persona.toml".to_string()), "{argv:?}");
        assert!(out.is_file());
    }
}
