// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! CPU score wrapper for Cloning V2.
//!
//! This crate does not score audio itself. It shells out to an existing
//! scorer when that binary and its model are both on disk. Otherwise the
//! JSON field is the string `unmeasured`.
//!
//! `emotion_match` is not computed here. The only accepted scorer is an
//! executable whose text, or a sibling marker, contains the formula name
//! `0.70*category_blend+0.30*emb_sim`. The arithmetic is never evaluated.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Identity string for the persona emotion_match scorer. Not an expression.
pub const EMOTION_MATCH_FORMULA: &str = "0.70*category_blend+0.30*emb_sim";

pub const DEVICE: &str = "cpu";
pub const SIMD: &str = "not-required";

#[cfg(feature = "igpu")]
pub const IGPU: &str = "not-wired";
#[cfg(not(feature = "igpu"))]
pub const IGPU: &str = "not-wired";

const EMOTION_NAMES: &[&str] = &["emotion_match", "emotion-match", "galion-emotion-match"];

/// Default thread cap matches the existing scorers (`--jobs` / `--threads` ≤ 2).
pub fn clamp_threads(n: usize) -> usize {
    n.clamp(1, 2)
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
    pub wer_bin: PathBuf,
    pub vosk_model: PathBuf,
    pub vosk_lib: PathBuf,
    pub ecapa_bin: PathBuf,
    pub ecapa_onnx: PathBuf,
    pub hf_home: PathBuf,
    pub mos_bin: PathBuf,
    pub squim_dirs: Vec<PathBuf>,
    pub emotion_dirs: Vec<PathBuf>,
    pub threads: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreRequest {
    pub wavs: Vec<PathBuf>,
    pub reference: Option<PathBuf>,
    pub ref_text: Option<String>,
    pub text_dir: Option<PathBuf>,
    pub nmr: Option<PathBuf>,
    /// Presence only. Do not spawn scorers.
    pub probe: bool,
}

pub fn discover(root: &Path) -> Layout {
    let scorers = root.join("cloning-v2/rollout-blendfix/scorers");
    let squim_env = std::env::var("GALION_SQUIM_MODELS").ok().map(PathBuf::from);
    let mut squim_dirs = Vec::new();
    if let Some(dir) = squim_env {
        squim_dirs.push(dir);
    }
    squim_dirs.push(scorers.join("quality/models"));
    squim_dirs.push(scorers.join("quality/galion-quality/models"));

    Layout {
        wer_bin: scorers.join("quality/bin/galion-wer"),
        vosk_model: env_path("GALION_VOSK_MODEL")
            .unwrap_or_else(|| root.join("weights/asr/vosk-model-small-en-us-0.15")),
        vosk_lib: env_path("GALION_VOSK_LIB")
            .unwrap_or_else(|| root.join("weights/asr/libvosk/libvosk.so")),
        ecapa_bin: scorers.join("ecapa/bin/galion-ecapa"),
        ecapa_onnx: env_path("GALION_ECAPA_MODEL")
            .unwrap_or_else(|| scorers.join("ecapa/models/ecapa.onnx")),
        hf_home: env_path("HF_HOME").unwrap_or_else(|| root.join("cache/hf")),
        mos_bin: scorers.join("quality/bin/galion-mos"),
        squim_dirs,
        emotion_dirs: vec![scorers.clone(), root.join("cloning-v2/scorers")],
        threads: clamp_threads(
            std::env::var("GALION_SCORE_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
        ),
        root: root.to_path_buf(),
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

pub fn report(layout: &Layout, req: &ScoreRequest) -> Value {
    let threads = clamp_threads(layout.threads);
    let wavs_ready = !req.wavs.is_empty() && req.wavs.iter().all(|p| p.is_file());
    let may_run = wavs_ready && !req.probe;

    let wer_bin_ok = executable_file(&layout.wer_bin);
    let vosk_ok = vosk_ready(&layout.vosk_model);
    let lib_ok = nonempty_file(&layout.vosk_lib);
    let wer_inputs = may_run && wer_inputs_ready(req);
    let (wer, wer_reason) = if !wer_bin_ok || !vosk_ok || !lib_ok {
        (unmeasured(), "scorer-or-model-missing")
    } else if !wer_inputs {
        (unmeasured(), "no-input")
    } else {
        match spawn_wer(layout, req, threads) {
            Ok(v) => (wer_from(&v), "wrapped"),
            Err(_) => (unmeasured(), "spawn-failed"),
        }
    };

    let ecapa_bin_ok = executable_file(&layout.ecapa_bin);
    let onnx_ok = nonempty_file(&layout.ecapa_onnx);
    let hf = find_hf_ecapa(&layout.hf_home);
    let ecapa_inputs = may_run && req.reference.as_ref().is_some_and(|p| p.is_file());
    // galion-ecapa is the CPU ONNX scorer. The speechbrain HF snapshot is
    // recorded below and is not sent to the CUDA python path.
    let (ecapa, ecapa_reason) = if !ecapa_bin_ok || !onnx_ok {
        (unmeasured(), "scorer-or-model-missing")
    } else if !ecapa_inputs {
        (unmeasured(), "no-input")
    } else {
        match spawn_ecapa(layout, req, threads) {
            Ok(v) => (ecapa_from(&v), "wrapped"),
            Err(_) => (unmeasured(), "spawn-failed"),
        }
    };

    let mos_bin_ok = executable_file(&layout.mos_bin);
    let subjective = find_onnx(&layout.squim_dirs, "squim_subjective.onnx");
    let objective = find_onnx(&layout.squim_dirs, "squim_objective.onnx");
    let nmr_ok = req.nmr.as_ref().is_some_and(|p| p.is_file());
    let can_subjective = subjective.is_some() && nmr_ok;
    let can_objective = objective.is_some();
    let (mos, squim, mos_reason) = if !mos_bin_ok || (!can_subjective && !can_objective) {
        (unmeasured(), unmeasured(), "scorer-or-model-missing")
    } else if !may_run {
        (unmeasured(), unmeasured(), "no-input")
    } else {
        match spawn_mos(layout, req, threads, can_subjective, can_objective) {
            Ok(v) => {
                let mos = if can_subjective {
                    mos_from(&v)
                } else {
                    unmeasured()
                };
                let squim = if can_objective {
                    squim_from(&v)
                } else {
                    unmeasured()
                };
                (mos, squim, "wrapped")
            }
            Err(_) => (unmeasured(), unmeasured(), "spawn-failed"),
        }
    };

    let emotion_bin = find_emotion_match(&layout.emotion_dirs);
    let (emotion_match, emotion_reason) = if emotion_bin.is_none() {
        (unmeasured(), "not-invented")
    } else if !may_run {
        (unmeasured(), "no-input")
    } else {
        match spawn_emotion(emotion_bin.as_ref().unwrap(), req, threads) {
            Ok(v) => (emotion_from(&v), "wrapped"),
            Err(_) => (unmeasured(), "spawn-failed"),
        }
    };

    json!({
        "tool": "galion-score-wrap",
        "version": env!("CARGO_PKG_VERSION"),
        "device": DEVICE,
        "cuda": false,
        "igpu": IGPU,
        "simd": SIMD,
        "threads": threads,
        "wer": wer,
        "ecapa": ecapa,
        "mos": mos,
        "squim": squim,
        "emotion_match": emotion_match,
        "roles": {
            "galion-score-wrap": "built",
            "galion-wer": "wrapped",
            "galion-mos": "wrapped",
            "galion-ecapa": "wrapped",
            "speechbrain-ecapa": "presence",
            "emotion_match": "not-invented"
        },
        "reasons": {
            "wer": wer_reason,
            "ecapa": ecapa_reason,
            "mos": mos_reason,
            "squim": mos_reason,
            "emotion_match": emotion_reason
        },
        "presence": {
            "galion_wer": file_presence(&layout.wer_bin, wer_bin_ok),
            "vosk": dir_presence(&layout.vosk_model, vosk_ok),
            "vosk_lib": file_presence(&layout.vosk_lib, lib_ok),
            "galion_ecapa": file_presence(&layout.ecapa_bin, ecapa_bin_ok),
            "ecapa_onnx": file_presence(&layout.ecapa_onnx, onnx_ok),
            "speechbrain_ecapa": hf_presence(hf.as_deref()),
            "galion_mos": file_presence(&layout.mos_bin, mos_bin_ok),
            "squim_subjective_onnx": path_opt_presence(subjective.as_deref()),
            "squim_objective_onnx": path_opt_presence(objective.as_deref()),
            "emotion_match": emotion_presence(emotion_bin.as_deref())
        }
    })
}

pub fn run(args: &[String]) -> i32 {
    match parse_args(args) {
        Err(msg) => {
            eprintln!("{msg}");
            2
        }
        Ok(Parsed::Help) => {
            println!("{USAGE}");
            0
        }
        Ok(Parsed::Job {
            layout,
            request,
            out,
        }) => {
            let body = report(&layout, &request);
            let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".into());
            println!("{text}");
            if let Some(path) = out {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::File::create(&path) {
                    Ok(mut f) => {
                        if let Err(err) = writeln!(f, "{text}") {
                            eprintln!("write {}: {err}", path.display());
                            return 2;
                        }
                    }
                    Err(err) => {
                        eprintln!("write {}: {err}", path.display());
                        return 2;
                    }
                }
            }
            0
        }
    }
}

const USAGE: &str = "\
galion-score-wrap — CPU wrapper around existing cloning scorers

Prints JSON. A missing scorer or model is the string \"unmeasured\".
emotion_match is never computed in this process.

  galion-score-wrap [--root DIR] [--probe]
                    [--wav FILE ...] [--ref FILE] [--ref-text TEXT | --text-dir DIR]
                    [--nmr FILE] [--threads N] [--out FILE]

Default device is CPU. Threads are clamped to 1..=2 (default 2).
The igpu feature does not select a GPU. CUDA is not a feature of this tool.
";

enum Parsed {
    Help,
    Job {
        layout: Layout,
        request: ScoreRequest,
        out: Option<PathBuf>,
    },
}

fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut root: Option<PathBuf> = None;
    let mut wavs = Vec::new();
    let mut reference = None;
    let mut ref_text = None;
    let mut text_dir = None;
    let mut nmr = None;
    let mut threads = None;
    let mut out = None;
    let mut probe = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            return Ok(Parsed::Help);
        }
        if arg == "--probe" {
            probe = true;
            i += 1;
            continue;
        }
        let (key, inline) = match arg.strip_prefix("--") {
            Some(rest) => match rest.split_once('=') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (rest.to_string(), None),
            },
            None => return Err(format!("unknown argument {arg}\n{USAGE}")),
        };
        let value = if let Some(v) = inline {
            v
        } else {
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("missing value for --{key}\n{USAGE}"))?
        };
        match key.as_str() {
            "root" => root = Some(PathBuf::from(value)),
            "wav" => wavs.push(PathBuf::from(value)),
            "ref" => reference = Some(PathBuf::from(value)),
            "ref-text" => ref_text = Some(value),
            "text-dir" => text_dir = Some(PathBuf::from(value)),
            "nmr" => nmr = Some(PathBuf::from(value)),
            "threads" => {
                let n: usize = value
                    .parse()
                    .map_err(|_| format!("--threads wants an integer, got {value}"))?;
                threads = Some(clamp_threads(n));
            }
            "out" => out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag --{other}\n{USAGE}")),
        }
        i += 1;
    }
    let root = root.unwrap_or_else(default_root);
    let mut layout = discover(&root);
    if let Some(n) = threads {
        layout.threads = n;
    }
    Ok(Parsed::Job {
        layout,
        request: ScoreRequest {
            wavs,
            reference,
            ref_text,
            text_dir,
            nmr,
            probe,
        },
        out,
    })
}

fn default_root() -> PathBuf {
    env_path("GALION_ROOT").unwrap_or_else(|| PathBuf::from("/home/galion/workspace/galion"))
}

fn unmeasured() -> Value {
    Value::String("unmeasured".into())
}

fn nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.len() > 0 && meta.permissions().mode() & 0o111 != 0
}

fn vosk_ready(model: &Path) -> bool {
    nonempty_file(&model.join("am/final.mdl")) && nonempty_file(&model.join("conf/model.conf"))
}

fn wer_inputs_ready(req: &ScoreRequest) -> bool {
    if let Some(text) = &req.ref_text {
        return !text.trim().is_empty();
    }
    let Some(dir) = &req.text_dir else {
        return false;
    };
    req.wavs.iter().all(|wav| {
        let stem = wav.file_stem().unwrap_or_default();
        nonempty_file(&dir.join(format!("{}.txt", stem.to_string_lossy())))
    })
}

fn find_onnx(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| nonempty_file(p))
}

fn find_hf_ecapa(hf_home: &Path) -> Option<PathBuf> {
    let hub = hf_home.join("hub/models--speechbrain--spkrec-ecapa-voxceleb/snapshots");
    if let Ok(rd) = fs::read_dir(&hub) {
        let mut snaps: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        snaps.sort();
        for snap in snaps {
            if nonempty_file(&snap.join("embedding_model.ckpt"))
                && nonempty_file(&snap.join("hyperparams.yaml"))
            {
                return Some(snap);
            }
        }
    }
    let alt = hf_home.join("speechbrain/spkrec-ecapa-voxceleb");
    if nonempty_file(&alt.join("embedding_model.ckpt"))
        && nonempty_file(&alt.join("hyperparams.yaml"))
    {
        return Some(alt);
    }
    None
}

fn find_emotion_match(dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        for name in EMOTION_NAMES {
            let direct = dir.join(name);
            if emotion_scorer(&direct) {
                return Some(direct);
            }
        }
        let Ok(rd) = fs::read_dir(dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if !path.is_dir() {
                continue;
            }
            for name in EMOTION_NAMES {
                let nested = path.join(name);
                if emotion_scorer(&nested) {
                    return Some(nested);
                }
                let binned = path.join("bin").join(name);
                if emotion_scorer(&binned) {
                    return Some(binned);
                }
            }
        }
    }
    None
}

/// A scorer counts only when it is executable and carries the formula name.
/// The formula is not evaluated.
fn emotion_scorer(path: &Path) -> bool {
    if !executable_file(path) {
        return false;
    }
    if text_has_formula(path) {
        return true;
    }
    let Some(dir) = path.parent() else {
        return false;
    };
    for name in ["FORMULA", "formula.txt", "formula.name"] {
        if text_has_formula(&dir.join(name)) {
            return true;
        }
    }
    false
}

fn text_has_formula(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 || meta.len() > 1024 * 1024 {
        return false;
    }
    fs::read_to_string(path)
        .map(|text| text.contains(EMOTION_MATCH_FORMULA))
        .unwrap_or(false)
}

fn file_presence(path: &Path, present: bool) -> Value {
    json!({
        "path": path.display().to_string(),
        "present": present,
        "bytes": bytes_field(path, present),
    })
}

fn dir_presence(path: &Path, present: bool) -> Value {
    json!({
        "path": path.display().to_string(),
        "present": present,
        "bytes": if present { json_u64(tree_bytes(path)) } else { unmeasured() },
    })
}

fn path_opt_presence(path: Option<&Path>) -> Value {
    match path {
        Some(path) => file_presence(path, true),
        None => json!({"path": Value::Null, "present": false, "bytes": "unmeasured"}),
    }
}

fn hf_presence(path: Option<&Path>) -> Value {
    match path {
        Some(path) => json!({
            "path": path.display().to_string(),
            "present": true,
            "bytes": json_u64(file_len(&path.join("embedding_model.ckpt"))),
            "snapshot_bytes": json_u64(tree_bytes(path)),
        }),
        None => json!({
            "path": Value::Null,
            "present": false,
            "bytes": "unmeasured",
            "snapshot_bytes": "unmeasured",
        }),
    }
}

fn emotion_presence(path: Option<&Path>) -> Value {
    match path {
        Some(path) => json!({
            "path": path.display().to_string(),
            "present": true,
            "formula": EMOTION_MATCH_FORMULA,
            "bytes": bytes_field(path, true),
        }),
        None => json!({
            "path": Value::Null,
            "present": false,
            "formula": EMOTION_MATCH_FORMULA,
            "bytes": "unmeasured",
        }),
    }
}

fn bytes_field(path: &Path, present: bool) -> Value {
    if present {
        json_u64(file_len(path))
    } else {
        unmeasured()
    }
}

fn json_u64(n: Option<u64>) -> Value {
    match n {
        Some(n) => json!(n),
        None => unmeasured(),
    }
}

fn file_len(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

fn tree_bytes(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    if path.is_file() {
        return file_len(path);
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    let mut seen = 0u32;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            seen += 1;
            if seen > 10_000 {
                return None;
            }
            let child = ent.path();
            if child.is_dir() {
                stack.push(child);
            } else if child.is_file() {
                if let Some(n) = file_len(&child) {
                    total = total.saturating_add(n);
                }
            }
        }
    }
    Some(total)
}

fn spawn_wer(layout: &Layout, req: &ScoreRequest, threads: usize) -> Result<Value, String> {
    let out = temp_json();
    let mut args = vec![
        "--model".into(),
        layout.vosk_model.display().to_string(),
        "--lib".into(),
        layout.vosk_lib.display().to_string(),
        "--jobs".into(),
        threads.to_string(),
        "--out".into(),
        out.display().to_string(),
    ];
    if let Some(text) = &req.ref_text {
        args.push("--ref".into());
        args.push(text.clone());
    } else if let Some(dir) = &req.text_dir {
        args.push("--text-dir".into());
        args.push(dir.display().to_string());
    }
    for wav in &req.wavs {
        args.push(wav.display().to_string());
    }
    let result = capture(&layout.wer_bin, &args, threads, &out);
    let _ = fs::remove_file(&out);
    result
}

fn spawn_ecapa(layout: &Layout, req: &ScoreRequest, threads: usize) -> Result<Value, String> {
    let out = temp_json();
    let mut args = vec![
        "--ref".into(),
        req.reference.as_ref().unwrap().display().to_string(),
        "--model".into(),
        layout.ecapa_onnx.display().to_string(),
        "--threads".into(),
        threads.to_string(),
        "--out".into(),
        out.display().to_string(),
    ];
    for wav in &req.wavs {
        args.push(wav.display().to_string());
    }
    let result = capture(&layout.ecapa_bin, &args, threads, &out);
    let _ = fs::remove_file(&out);
    result
}

fn spawn_mos(
    layout: &Layout,
    req: &ScoreRequest,
    threads: usize,
    subjective: bool,
    objective: bool,
) -> Result<Value, String> {
    let out = temp_json();
    let models = layout
        .squim_dirs
        .iter()
        .find(|d| {
            nonempty_file(&d.join("squim_subjective.onnx"))
                || nonempty_file(&d.join("squim_objective.onnx"))
        })
        .cloned()
        .unwrap_or_else(|| layout.squim_dirs.first().cloned().unwrap_or_default());
    let mut args = vec![
        "--models".into(),
        models.display().to_string(),
        "--threads".into(),
        threads.to_string(),
        "--out".into(),
        out.display().to_string(),
    ];
    if !subjective {
        args.push("--no-subjective".into());
    } else if let Some(nmr) = &req.nmr {
        args.push("--nmr".into());
        args.push(nmr.display().to_string());
    }
    if !objective {
        args.push("--no-objective".into());
    }
    for wav in &req.wavs {
        args.push(wav.display().to_string());
    }
    let result = capture(&layout.mos_bin, &args, threads, &out);
    let _ = fs::remove_file(&out);
    result
}

fn spawn_emotion(bin: &Path, req: &ScoreRequest, threads: usize) -> Result<Value, String> {
    let out = temp_json();
    let mut args = vec![
        "--threads".into(),
        threads.to_string(),
        "--out".into(),
        out.display().to_string(),
    ];
    if let Some(reference) = &req.reference {
        args.push("--ref".into());
        args.push(reference.display().to_string());
    }
    for wav in &req.wavs {
        args.push(wav.display().to_string());
    }
    let result = capture(bin, &args, threads, &out);
    let _ = fs::remove_file(&out);
    result
}

fn capture(bin: &Path, args: &[String], threads: usize, out: &Path) -> Result<Value, String> {
    let mut cmd = command_for(bin);
    cmd.args(args)
        .env("CUDA_VISIBLE_DEVICES", "")
        .env("OMP_NUM_THREADS", threads.to_string())
        .env("OPENBLAS_NUM_THREADS", threads.to_string())
        .env("MKL_NUM_THREADS", threads.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!("exit {}", output.status));
    }
    if let Ok(text) = fs::read_to_string(out) {
        if let Ok(value) = serde_json::from_str::<Value>(text.trim()) {
            return Ok(value);
        }
    }
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).map_err(|err| err.to_string())
}

fn command_for(bin: &Path) -> Command {
    if Path::new("/usr/bin/nice").is_file() {
        let mut cmd = Command::new("nice");
        cmd.arg("-n").arg("10").arg(bin);
        cmd
    } else {
        Command::new(bin)
    }
}

fn temp_json() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("galion-score-wrap-{}-{n}.json", std::process::id()))
}

fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite())
}

fn wer_from(value: &Value) -> Value {
    finite(value.pointer("/summary/mean").unwrap_or(&Value::Null))
        .map(|n| json!(n))
        .unwrap_or_else(unmeasured)
}

fn ecapa_from(value: &Value) -> Value {
    finite(value.pointer("/mean").unwrap_or(&Value::Null))
        .map(|n| json!(n))
        .unwrap_or_else(unmeasured)
}

fn mos_from(value: &Value) -> Value {
    finite(value.pointer("/summary/mos/mean").unwrap_or(&Value::Null))
        .map(|n| json!(n))
        .unwrap_or_else(unmeasured)
}

fn squim_from(value: &Value) -> Value {
    let mut map = serde_json::Map::new();
    for (key, pointer) in [
        ("stoi", "/summary/stoi/mean"),
        ("pesq", "/summary/pesq/mean"),
        ("si_sdr", "/summary/si_sdr/mean"),
    ] {
        if let Some(n) = value.pointer(pointer).and_then(finite) {
            map.insert(key.to_string(), json!(n));
        }
    }
    if map.is_empty() {
        unmeasured()
    } else {
        Value::Object(map)
    }
}

/// Pass through a child `emotion_match` number. Never read `category_blend`
/// or `emb_sim`, and never evaluate the formula name.
fn emotion_from(value: &Value) -> Value {
    for pointer in ["/emotion_match", "/summary/emotion_match"] {
        let Some(slot) = value.pointer(pointer) else {
            continue;
        };
        if slot.as_str() == Some("unmeasured") {
            return unmeasured();
        }
        if let Some(n) = finite(slot) {
            return json!(n);
        }
    }
    unmeasured()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new() -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!("galion-score-wrap-{nanos}-{n}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write_exec(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn touch(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn empty_layout(root: &Path) -> Layout {
        Layout {
            root: root.to_path_buf(),
            wer_bin: root.join("missing-wer"),
            vosk_model: root.join("missing-vosk"),
            vosk_lib: root.join("missing-libvosk.so"),
            ecapa_bin: root.join("missing-ecapa"),
            ecapa_onnx: root.join("missing-ecapa.onnx"),
            hf_home: root.join("missing-hf"),
            mos_bin: root.join("missing-mos"),
            squim_dirs: vec![root.join("missing-squim")],
            emotion_dirs: vec![root.join("missing-emotion")],
            threads: 2,
        }
    }

    fn scored_request(tree: &TempTree) -> ScoreRequest {
        let wav = tree.root.join("clip.wav");
        touch(&wav, b"not-audio");
        ScoreRequest {
            wavs: vec![wav],
            reference: None,
            ref_text: Some("hello".into()),
            text_dir: None,
            nmr: None,
            probe: false,
        }
    }

    #[test]
    fn missing_scorers_are_the_string_unmeasured() {
        let tree = TempTree::new();
        let layout = empty_layout(&tree.root);
        let report = report(&layout, &scored_request(&tree));
        for key in ["wer", "ecapa", "mos", "squim", "emotion_match"] {
            assert_eq!(report[key], json!("unmeasured"), "{key}");
            assert!(report[key].is_string(), "{key}");
        }
        assert_eq!(report["device"], json!("cpu"));
        assert_eq!(report["cuda"], json!(false));
        assert_eq!(report["igpu"], json!("not-wired"));
        assert_eq!(report["simd"], json!("not-required"));
        assert_eq!(report["threads"], json!(2));
        assert_eq!(report["roles"]["galion-score-wrap"], json!("built"));
        assert_eq!(report["roles"]["emotion_match"], json!("not-invented"));
        assert_eq!(report["reasons"]["emotion_match"], json!("not-invented"));
        assert_eq!(report["presence"]["vosk"]["bytes"], json!("unmeasured"));
        assert_eq!(
            report["presence"]["squim_subjective_onnx"]["bytes"],
            json!("unmeasured")
        );
        assert_eq!(
            report["presence"]["emotion_match"]["formula"],
            json!(EMOTION_MATCH_FORMULA)
        );
        assert_eq!(report["presence"]["emotion_match"]["present"], json!(false));
    }

    #[test]
    fn probe_does_not_spawn_even_when_a_stub_scorer_exists() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let marker = tree.root.join("spawned");
        let wer = tree.root.join("galion-wer");
        write_exec(
            &wer,
            &format!(
                "#!/bin/sh\ntouch {}\nprintf '%s\\n' '{{\"summary\":{{\"mean\":0.5}}}}'\n",
                marker.display()
            ),
        );
        layout.wer_bin = wer;
        layout.vosk_model = tree.root.join("vosk");
        touch(&layout.vosk_model.join("am/final.mdl"), b"mdl");
        touch(&layout.vosk_model.join("conf/model.conf"), b"conf");
        layout.vosk_lib = tree.root.join("libvosk.so");
        touch(&layout.vosk_lib, b"lib");
        let mut req = scored_request(&tree);
        req.probe = true;
        let report = report(&layout, &req);
        assert_eq!(report["wer"], json!("unmeasured"));
        assert!(!marker.exists(), "probe must not spawn");
        assert_eq!(report["presence"]["galion_wer"]["present"], json!(true));
        assert_eq!(report["presence"]["vosk"]["present"], json!(true));
    }

    #[test]
    fn wer_shells_out_only_when_vosk_is_present_and_passes_the_child_number() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let wer = tree.root.join("galion-wer");
        write_exec(
            &wer,
            "#!/bin/sh\nprintf '%s\\n' '{\"summary\":{\"mean\":0.25}}'\n",
        );
        layout.wer_bin = wer;
        layout.vosk_model = tree.root.join("vosk");
        touch(&layout.vosk_model.join("am/final.mdl"), b"mdl");
        touch(&layout.vosk_model.join("conf/model.conf"), b"conf");
        layout.vosk_lib = tree.root.join("libvosk.so");
        touch(&layout.vosk_lib, b"lib");
        let report = report(&layout, &scored_request(&tree));
        assert_eq!(report["wer"], json!(0.25));
        assert_eq!(report["ecapa"], json!("unmeasured"));
        assert_eq!(report["mos"], json!("unmeasured"));
        assert_eq!(report["squim"], json!("unmeasured"));
        assert_eq!(report["emotion_match"], json!("unmeasured"));
        assert_eq!(report["reasons"]["wer"], json!("wrapped"));
    }

    #[test]
    fn wer_without_the_vosk_model_does_not_spawn() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let marker = tree.root.join("spawned");
        let wer = tree.root.join("galion-wer");
        write_exec(
            &wer,
            &format!(
                "#!/bin/sh\ntouch {}\nprintf '%s\\n' '{{\"summary\":{{\"mean\":0.5}}}}'\n",
                marker.display()
            ),
        );
        layout.wer_bin = wer;
        layout.vosk_lib = tree.root.join("libvosk.so");
        touch(&layout.vosk_lib, b"lib");
        let report = report(&layout, &scored_request(&tree));
        assert_eq!(report["wer"], json!("unmeasured"));
        assert!(!marker.exists());
        assert_eq!(report["reasons"]["wer"], json!("scorer-or-model-missing"));
    }

    #[test]
    fn emotion_match_is_not_invented_from_parts() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let bin = tree.root.join("emotion").join("emotion_match");
        // Parts are present in the JSON. The formula name is not, so this
        // executable is not the persona scorer and must not be spawned.
        let marker = tree.root.join("emotion-spawned");
        write_exec(
            &bin,
            &format!(
                "#!/bin/sh\ntouch {}\nprintf '%s\\n' '{{\"category_blend\":0.9,\"emb_sim\":0.8,\"emotion_match\":0.99}}'\n",
                marker.display()
            ),
        );
        layout.emotion_dirs = vec![tree.root.join("emotion")];
        let report = report(&layout, &scored_request(&tree));
        assert_eq!(report["emotion_match"], json!("unmeasured"));
        assert!(!marker.exists());
        let blended = 0.70 * 0.9 + 0.30 * 0.8;
        assert_ne!(report["emotion_match"], json!(blended));
    }

    #[test]
    fn formula_marked_emotion_scorer_is_passed_through() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let bin = tree.root.join("emotion").join("emotion_match");
        write_exec(
            &bin,
            &format!(
                "#!/bin/sh\n# {EMOTION_MATCH_FORMULA}\nprintf '%s\\n' '{{\"category_blend\":0.9,\"emb_sim\":0.8,\"emotion_match\":0.42}}'\n"
            ),
        );
        layout.emotion_dirs = vec![tree.root.join("emotion")];
        let report = report(&layout, &scored_request(&tree));
        assert_eq!(report["emotion_match"], json!(0.42));
        assert_ne!(report["emotion_match"], json!(0.70 * 0.9 + 0.30 * 0.8));
        assert_eq!(report["reasons"]["emotion_match"], json!("wrapped"));
    }

    #[test]
    fn formula_marked_scorer_without_emotion_match_stays_unmeasured() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let bin = tree
            .root
            .join("emotion")
            .join("bin")
            .join("galion-emotion-match");
        write_exec(
            &bin,
            &format!(
                "#!/bin/sh\n# {EMOTION_MATCH_FORMULA}\nprintf '%s\\n' '{{\"category_blend\":1,\"emb_sim\":1}}'\n"
            ),
        );
        layout.emotion_dirs = vec![tree.root.clone()];
        let got = report(&layout, &scored_request(&tree));
        assert_eq!(got["emotion_match"], json!("unmeasured"));
        assert_ne!(got["emotion_match"], json!(1.0));
    }

    #[test]
    fn mos_and_squim_pass_child_means_and_skip_when_onnx_is_missing() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let mos = tree.root.join("galion-mos");
        write_exec(
            &mos,
            "#!/bin/sh\nprintf '%s\\n' '{\"summary\":{\"mos\":{\"mean\":3.5},\"stoi\":{\"mean\":0.9},\"pesq\":{\"mean\":2.5},\"si_sdr\":{\"mean\":18.0}}}'\n",
        );
        layout.mos_bin = mos;
        layout.squim_dirs = vec![tree.root.join("models")];
        touch(&layout.squim_dirs[0].join("squim_objective.onnx"), b"onnx");
        let mut req = scored_request(&tree);
        let objective_only = report(&layout, &req);
        assert_eq!(objective_only["mos"], json!("unmeasured"));
        assert_eq!(objective_only["squim"]["stoi"], json!(0.9));
        assert_eq!(objective_only["squim"]["pesq"], json!(2.5));
        assert_eq!(objective_only["squim"]["si_sdr"], json!(18.0));

        touch(&layout.squim_dirs[0].join("squim_subjective.onnx"), b"onnx");
        req.nmr = Some(tree.root.join("nmr.wav"));
        touch(req.nmr.as_ref().unwrap(), b"nmr");
        let both = report(&layout, &req);
        assert_eq!(both["mos"], json!(3.5));
        assert_eq!(both["squim"]["stoi"], json!(0.9));
    }

    #[test]
    fn ecapa_without_onnx_stays_unmeasured_even_if_an_hf_snapshot_exists() {
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        let marker = tree.root.join("ecapa-spawned");
        let bin = tree.root.join("galion-ecapa");
        write_exec(
            &bin,
            &format!(
                "#!/bin/sh\ntouch {}\nprintf '%s\\n' '{{\"mean\":0.7}}'\n",
                marker.display()
            ),
        );
        layout.ecapa_bin = bin;
        let snap = tree
            .root
            .join("hf/hub/models--speechbrain--spkrec-ecapa-voxceleb/snapshots/abc");
        touch(&snap.join("embedding_model.ckpt"), b"ckpt");
        touch(
            &snap.join("hyperparams.yaml"),
            b"pretrained_path: speechbrain/spkrec-ecapa-voxceleb\n",
        );
        layout.hf_home = tree.root.join("hf");
        let mut req = scored_request(&tree);
        req.reference = Some(tree.root.join("ref.wav"));
        touch(req.reference.as_ref().unwrap(), b"ref");
        let report = report(&layout, &req);
        assert_eq!(report["ecapa"], json!("unmeasured"));
        assert!(!marker.exists());
        assert_eq!(
            report["presence"]["speechbrain_ecapa"]["present"],
            json!(true)
        );
        assert_eq!(report["presence"]["ecapa_onnx"]["present"], json!(false));
        assert_eq!(
            report["presence"]["ecapa_onnx"]["bytes"],
            json!("unmeasured")
        );
    }

    #[test]
    fn threads_clamp_to_two() {
        assert_eq!(clamp_threads(0), 1);
        assert_eq!(clamp_threads(1), 1);
        assert_eq!(clamp_threads(2), 2);
        assert_eq!(clamp_threads(8), 2);
        let tree = TempTree::new();
        let mut layout = empty_layout(&tree.root);
        layout.threads = 99;
        let report = report(&layout, &ScoreRequest::default());
        assert_eq!(report["threads"], json!(2));
        assert_eq!(IGPU, "not-wired");
        assert_eq!(DEVICE, "cpu");
        assert_eq!(SIMD, "not-required");
    }

    #[test]
    fn cli_probe_on_an_empty_root_is_unmeasured() {
        let tree = TempTree::new();
        let code = run(&[
            "--root".into(),
            tree.root.display().to_string(),
            "--probe".into(),
            "--threads".into(),
            "8".into(),
        ]);
        assert_eq!(code, 0);
    }
}
