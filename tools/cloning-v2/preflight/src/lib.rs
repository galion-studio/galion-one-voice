// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Rust-first preflight for Cloning V2.
//!
//! Piper training is refused. The only accepted engine is cbx (Chatterbox
//! Turbo ONNX). The check exits non-zero when that engine is not selected,
//! when the cbx binary is absent, or when the fp32 weight snapshot is absent.
//!
//! The check only looks at paths. It does not download weights, does not open
//! a network socket, does not execute cbx, and does not read or write audio.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Exit status when the arguments cannot be parsed.
pub const EXIT_USAGE: i32 = 2;
/// Exit status when the request is Piper training.
pub const EXIT_PIPER: i32 = 3;
/// Exit status when the cbx binary is absent.
pub const EXIT_CBX_ABSENT: i32 = 4;
/// Exit status when the weight snapshot is absent.
pub const EXIT_WEIGHTS_ABSENT: i32 = 5;

/// fp32 Chatterbox Turbo ONNX snapshot files.
///
/// Matches `pipeline/cbx-v2-timbre/src/hf.rs` for `ModelVariant::Fp32`:
/// `tokenizer.json` plus each `onnx/{stem}.onnx` and `onnx/{stem}.onnx_data`
/// pair (`conditional_decoder`, `speech_encoder`, `embed_tokens`, `language_model`).
pub const REQUIRED_WEIGHTS: &[&str] = &[
    "tokenizer.json",
    "onnx/conditional_decoder.onnx",
    "onnx/conditional_decoder.onnx_data",
    "onnx/speech_encoder.onnx",
    "onnx/speech_encoder.onnx_data",
    "onnx/embed_tokens.onnx",
    "onnx/embed_tokens.onnx_data",
    "onnx/language_model.onnx",
    "onnx/language_model.onnx_data",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Cbx,
    Piper,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub engine: Engine,
    pub cbx: PathBuf,
    pub weights: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub ok: bool,
    pub exit_code: i32,
    pub reason: &'static str,
    pub missing: Vec<String>,
    pub cbx: PathBuf,
    pub weights: PathBuf,
}

impl Check {
    pub fn message(&self) -> String {
        match self.reason {
            "piper-training-refused" => {
                "refusing Piper training: cloning-v2 does not train Piper voices".to_string()
            }
            "cbx-absent" => format!("cbx absent: {}", self.cbx.display()),
            "weights-absent" => {
                if self.missing.is_empty() {
                    format!("weights absent: {}", self.weights.display())
                } else {
                    format!(
                        "weights absent: {} missing {}",
                        self.weights.display(),
                        self.missing.join(", ")
                    )
                }
            }
            "ok" => format!(
                "cbx present: {}; weights present: {}",
                self.cbx.display(),
                self.weights.display()
            ),
            other => format!("preflight {other}"),
        }
    }

    pub fn to_json(&self) -> String {
        let mut missing = String::from("[");
        for (i, item) in self.missing.iter().enumerate() {
            if i > 0 {
                missing.push(',');
            }
            let _ = write!(missing, "\"{}\"", escape_json(item));
        }
        missing.push(']');
        format!(
            "{{\"ok\":{},\"exit_code\":{},\"reason\":\"{}\",\"engine\":\"{}\",\"cbx\":\"{}\",\"weights\":\"{}\",\"missing\":{}}}",
            if self.ok { "true" } else { "false" },
            self.exit_code,
            self.reason,
            if self.reason == "piper-training-refused" {
                "piper"
            } else {
                "cbx"
            },
            escape_json(&self.cbx.display().to_string()),
            escape_json(&self.weights.display().to_string()),
            missing,
        )
    }
}

/// Classify an `--engine` value. Piper and any piper-train alias are refused.
pub fn classify_engine(raw: &str) -> Result<Engine, ()> {
    let folded = raw.trim().to_ascii_lowercase().replace('_', "-");
    if folded == "cbx" || folded == "chatterbox" {
        Ok(Engine::Cbx)
    } else if folded == "piper"
        || folded == "piper-train"
        || folded == "train-piper"
        || folded.contains("piper")
    {
        Ok(Engine::Piper)
    } else {
        Err(())
    }
}

/// Read-only presence check. Piper is refused before any path is inspected.
pub fn evaluate(request: &Request) -> Check {
    if request.engine == Engine::Piper {
        return Check {
            ok: false,
            exit_code: EXIT_PIPER,
            reason: "piper-training-refused",
            missing: Vec::new(),
            cbx: request.cbx.clone(),
            weights: request.weights.clone(),
        };
    }

    if !is_present_file(&request.cbx) {
        return Check {
            ok: false,
            exit_code: EXIT_CBX_ABSENT,
            reason: "cbx-absent",
            missing: Vec::new(),
            cbx: request.cbx.clone(),
            weights: request.weights.clone(),
        };
    }

    let missing = missing_weights(&request.weights);
    if !missing.is_empty() {
        return Check {
            ok: false,
            exit_code: EXIT_WEIGHTS_ABSENT,
            reason: "weights-absent",
            missing,
            cbx: request.cbx.clone(),
            weights: request.weights.clone(),
        };
    }

    Check {
        ok: true,
        exit_code: 0,
        reason: "ok",
        missing: Vec::new(),
        cbx: request.cbx.clone(),
        weights: request.weights.clone(),
    }
}

/// A weight file counts as present only when it is a non-empty regular file.
pub fn missing_weights(weights: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    if !weights.is_dir() {
        missing.push(".".to_string());
        for rel in REQUIRED_WEIGHTS {
            missing.push((*rel).to_string());
        }
        return missing;
    }
    for rel in REQUIRED_WEIGHTS {
        if !is_present_file(&weights.join(rel)) {
            missing.push((*rel).to_string());
        }
    }
    missing
}

fn is_present_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.len() > 0
}

/// Parse CLI args and run the check. Prints one JSON object on stdout.
///
/// Piper training is refused even when `--cbx` and `--weights` are omitted.
pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{}", usage());
        return 0;
    }

    let parsed = match parse_args(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("{}", usage());
            println!(
                "{{\"ok\":false,\"exit_code\":{EXIT_USAGE},\"reason\":\"usage\",\"engine\":null,\"cbx\":null,\"weights\":null,\"missing\":[]}}"
            );
            return EXIT_USAGE;
        }
    };

    let check = evaluate(&parsed);
    println!("{}", check.to_json());
    if !check.ok {
        eprintln!("{}", check.message());
    }
    check.exit_code
}

fn parse_args(args: &[String]) -> Result<Request, String> {
    let mut engine = Engine::Cbx;
    let mut engine_set = false;
    let mut cbx: Option<PathBuf> = None;
    let mut weights: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(value) = arg.strip_prefix("--engine=") {
            engine = classify_engine(value).map_err(|_| format!("unknown engine '{value}'"))?;
            engine_set = true;
            i += 1;
            continue;
        }
        if arg == "--engine" {
            let value = args
                .get(i + 1)
                .ok_or_else(|| "missing value for --engine".to_string())?;
            engine = classify_engine(value).map_err(|_| format!("unknown engine '{value}'"))?;
            engine_set = true;
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--cbx=") {
            cbx = Some(PathBuf::from(value));
            i += 1;
            continue;
        }
        if arg == "--cbx" {
            let value = args
                .get(i + 1)
                .ok_or_else(|| "missing value for --cbx".to_string())?;
            cbx = Some(PathBuf::from(value));
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--weights=") {
            weights = Some(PathBuf::from(value));
            i += 1;
            continue;
        }
        if arg == "--weights" {
            let value = args
                .get(i + 1)
                .ok_or_else(|| "missing value for --weights".to_string())?;
            weights = Some(PathBuf::from(value));
            i += 2;
            continue;
        }
        if classify_engine(arg).ok() == Some(Engine::Piper) {
            engine = Engine::Piper;
            engine_set = true;
            i += 1;
            continue;
        }
        return Err(format!("unknown argument '{arg}'"));
    }

    if engine == Engine::Piper {
        return Ok(Request {
            engine,
            cbx: cbx.unwrap_or_default(),
            weights: weights.unwrap_or_default(),
        });
    }

    if !engine_set {
        engine = Engine::Cbx;
    }
    let cbx = cbx.ok_or_else(|| "missing --cbx".to_string())?;
    let weights = weights.ok_or_else(|| "missing --weights".to_string())?;
    Ok(Request {
        engine,
        cbx,
        weights,
    })
}

pub fn usage() -> &'static str {
    "galion-cloning-preflight — refuse Piper training; require cbx and fp32 weights\n\n\
USAGE:\n  galion-cloning-preflight --cbx PATH --weights DIR [--engine cbx]\n  \
galion-cloning-preflight --engine piper\n\n\
EXIT:\n  0  cbx binary and fp32 weight snapshot are present\n  \
2  usage\n  \
3  Piper training refused\n  \
4  cbx absent\n  \
5  weights absent\n\n\
Does not download weights, execute cbx, or read or write audio."
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
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
            let root = std::env::temp_dir().join(format!("galion-cloning-preflight-{nanos}-{n}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(bytes).unwrap();
    }

    fn fixture(tree: &TempTree, weight_bytes: &[u8]) -> (PathBuf, PathBuf) {
        let cbx = tree.path().join("cbx");
        write_file(&cbx, b"cbx");
        let weights = tree.path().join("weights");
        fs::create_dir_all(&weights).unwrap();
        for rel in REQUIRED_WEIGHTS {
            write_file(&weights.join(rel), weight_bytes);
        }
        (cbx, weights)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut found = Vec::new();
        fn walk(dir: &Path, prefix: &str, found: &mut Vec<String>) {
            let mut entries: Vec<_> = fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            entries.sort();
            for path in entries {
                let name = path.file_name().unwrap().to_string_lossy();
                let rel = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{prefix}/{name}")
                };
                if path.is_dir() {
                    walk(&path, &rel, found);
                } else {
                    found.push(rel);
                }
            }
        }
        walk(dir, "", &mut found);
        found
    }

    #[test]
    fn piper_training_is_refused_even_when_files_exist() {
        let tree = TempTree::new();
        let (cbx, weights) = fixture(&tree, b"x");
        let before = names(tree.path());
        let check = evaluate(&Request {
            engine: Engine::Piper,
            cbx: cbx.clone(),
            weights: weights.clone(),
        });
        assert!(!check.ok);
        assert_eq!(check.exit_code, EXIT_PIPER);
        assert_eq!(check.reason, "piper-training-refused");
        assert!(check.message().contains("refusing Piper training"));
        assert_eq!(names(tree.path()), before);
    }

    #[test]
    fn piper_aliases_are_refused() {
        for raw in [
            "piper",
            "Piper",
            "PIPER",
            "piper-train",
            "piper_train",
            "train-piper",
        ] {
            assert_eq!(classify_engine(raw), Ok(Engine::Piper), "{raw}");
        }
        assert_eq!(classify_engine("cbx"), Ok(Engine::Cbx));
        assert_eq!(classify_engine("chatterbox"), Ok(Engine::Cbx));
        assert!(classify_engine("flite").is_err());
    }

    #[test]
    fn cli_refuses_piper_without_paths_and_without_creating_them() {
        let code = run(&["--engine".into(), "piper".into()]);
        assert_eq!(code, EXIT_PIPER);
        let code = run(&["piper-train".into()]);
        assert_eq!(code, EXIT_PIPER);
        let code = run(&["--engine=piper_train".into()]);
        assert_eq!(code, EXIT_PIPER);
    }

    #[test]
    fn missing_cbx_exits_nonzero() {
        let tree = TempTree::new();
        let weights = tree.path().join("weights");
        fs::create_dir_all(&weights).unwrap();
        for rel in REQUIRED_WEIGHTS {
            write_file(&weights.join(rel), b"x");
        }
        let cbx = tree.path().join("missing-cbx");
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx: cbx.clone(),
            weights,
        });
        assert!(!check.ok);
        assert_eq!(check.exit_code, EXIT_CBX_ABSENT);
        assert_eq!(check.reason, "cbx-absent");
        assert!(!cbx.exists());
    }

    #[test]
    fn empty_cbx_file_and_cbx_directory_are_absent() {
        let tree = TempTree::new();
        let (cbx, weights) = fixture(&tree, b"x");
        write_file(&cbx, b"");
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx: cbx.clone(),
            weights: weights.clone(),
        });
        assert_eq!(check.reason, "cbx-absent");
        assert_eq!(check.exit_code, EXIT_CBX_ABSENT);

        fs::remove_file(&cbx).unwrap();
        fs::create_dir(&cbx).unwrap();
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx,
            weights,
        });
        assert_eq!(check.reason, "cbx-absent");
    }

    #[test]
    fn missing_weights_directory_exits_nonzero() {
        let tree = TempTree::new();
        let cbx = tree.path().join("cbx");
        write_file(&cbx, b"cbx");
        let weights = tree.path().join("no-such-weights");
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx,
            weights: weights.clone(),
        });
        assert!(!check.ok);
        assert_eq!(check.exit_code, EXIT_WEIGHTS_ABSENT);
        assert_eq!(check.reason, "weights-absent");
        assert!(check.missing.iter().any(|m| m == "tokenizer.json"));
        assert!(check
            .missing
            .iter()
            .any(|m| m == "onnx/language_model.onnx_data"));
        assert!(!weights.exists());
    }

    #[test]
    fn one_missing_or_empty_weight_file_is_absent() {
        let tree = TempTree::new();
        let (cbx, weights) = fixture(&tree, b"x");
        let dropped = weights.join("onnx/speech_encoder.onnx");
        fs::remove_file(&dropped).unwrap();
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx: cbx.clone(),
            weights: weights.clone(),
        });
        assert_eq!(check.exit_code, EXIT_WEIGHTS_ABSENT);
        assert_eq!(check.missing, vec!["onnx/speech_encoder.onnx".to_string()]);

        write_file(&dropped, b"");
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx,
            weights,
        });
        assert_eq!(check.reason, "weights-absent");
        assert_eq!(check.missing, vec!["onnx/speech_encoder.onnx".to_string()]);
    }

    #[test]
    fn both_cbx_and_weights_absent_reports_cbx_first() {
        let tree = TempTree::new();
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx: tree.path().join("cbx"),
            weights: tree.path().join("weights"),
        });
        assert_eq!(check.reason, "cbx-absent");
        assert_eq!(check.exit_code, EXIT_CBX_ABSENT);
    }

    #[test]
    fn present_cbx_and_weights_pass_without_writing_audio() {
        let tree = TempTree::new();
        let (cbx, weights) = fixture(&tree, b"x");
        let before = names(tree.path());
        assert!(before.iter().all(|name| !name.ends_with(".wav")));
        let check = evaluate(&Request {
            engine: Engine::Cbx,
            cbx: cbx.clone(),
            weights: weights.clone(),
        });
        assert!(check.ok);
        assert_eq!(check.exit_code, 0);
        assert_eq!(check.reason, "ok");
        assert!(check.missing.is_empty());
        assert_eq!(names(tree.path()), before);
        let json = check.to_json();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"reason\":\"ok\""));
        assert!(!json.contains(".wav"));
    }

    #[test]
    fn cli_exit_codes_for_absent_cbx_and_weights() {
        let tree = TempTree::new();
        let weights = tree.path().join("weights");
        fs::create_dir_all(&weights).unwrap();
        for rel in REQUIRED_WEIGHTS {
            write_file(&weights.join(rel), b"x");
        }
        let missing_cbx = tree.path().join("no-cbx");
        let code = run(&[
            "--engine".into(),
            "cbx".into(),
            "--cbx".into(),
            missing_cbx.display().to_string(),
            "--weights".into(),
            weights.display().to_string(),
        ]);
        assert_eq!(code, EXIT_CBX_ABSENT);

        let cbx = tree.path().join("cbx");
        write_file(&cbx, b"cbx");
        let code = run(&[
            "--cbx".into(),
            cbx.display().to_string(),
            "--weights".into(),
            tree.path().join("missing-weights").display().to_string(),
        ]);
        assert_eq!(code, EXIT_WEIGHTS_ABSENT);

        let code = run(&[
            "--cbx".into(),
            cbx.display().to_string(),
            "--weights".into(),
            weights.display().to_string(),
        ]);
        assert_eq!(code, 0);
    }

    #[test]
    fn usage_is_nonzero_and_help_is_zero() {
        assert_eq!(run(&[]), EXIT_USAGE);
        assert_eq!(run(&["--cbx".into(), "x".into()]), EXIT_USAGE);
        assert_eq!(run(&["--engine".into(), "flite".into()]), EXIT_USAGE);
        assert_eq!(run(&["--help".into()]), 0);
    }

    #[test]
    fn required_weight_list_matches_fp32_onnx_pairs() {
        assert_eq!(REQUIRED_WEIGHTS[0], "tokenizer.json");
        let stems = [
            "conditional_decoder",
            "speech_encoder",
            "embed_tokens",
            "language_model",
        ];
        for stem in stems {
            let onnx = format!("onnx/{stem}.onnx");
            let data = format!("onnx/{stem}.onnx_data");
            assert!(REQUIRED_WEIGHTS.iter().any(|rel| *rel == onnx), "{onnx}");
            assert!(REQUIRED_WEIGHTS.iter().any(|rel| *rel == data), "{data}");
        }
        assert_eq!(REQUIRED_WEIGHTS.len(), 1 + stems.len() * 2);
    }

    #[test]
    fn json_escapes_quotes_and_backslashes() {
        let check = Check {
            ok: false,
            exit_code: EXIT_CBX_ABSENT,
            reason: "cbx-absent",
            missing: vec!["a\"b".into()],
            cbx: PathBuf::from("a\\b\"c"),
            weights: PathBuf::from("w"),
        };
        let json = check.to_json();
        assert!(json.contains("a\\\\b\\\"c"), "{json}");
        assert!(json.contains("a\\\"b"), "{json}");
        assert!(!json.contains(".wav"));
    }
}
