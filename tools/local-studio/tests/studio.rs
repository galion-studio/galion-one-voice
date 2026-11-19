// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.

use galion_local_studio::audio::{self, sha256_bytes};
use galion_local_studio::config::StudioConfig;
use galion_local_studio::device::{choose, DeviceRequest};
use galion_local_studio::master::integrated_lufs;
use galion_local_studio::pipeline::{locate_reference, render_script, LoudnessEngine, RunInput};
use galion_local_studio::render::{CommandRenderer, SyntheticRenderer};
use galion_local_studio::score::{ProcessScorer, ScriptedScorer, TakeScores};
use std::path::{Path, PathBuf};

struct Tmp {
    path: PathBuf,
}

impl Tmp {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "galion-studio-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn config_wav_only() -> StudioConfig {
    let mut cfg = StudioConfig::parse(include_str!("../examples/studio.toml")).unwrap();
    cfg.output.formats = vec!["wav".into()];
    cfg.master.engine = galion_local_studio::config::LoudnessPref::InProcess;
    cfg
}

fn write_reference(dir: &Path) -> PathBuf {
    let path = dir.join("reference.wav");
    let tone = audio::sine(8_000, 24_000, 432.0, 0.1);
    audio::write_wav_mono(&path, &tone, 24_000).unwrap();
    path
}

fn scores(wer: f64, ecapa: f64, emotion: f64) -> TakeScores {
    TakeScores {
        wer: Some(wer),
        ecapa: Some(ecapa),
        emotion: Some(emotion),
        transcript: "measured".into(),
        detail: String::new(),
    }
}

#[test]
fn synthetic_tone_pipeline_picks_the_best_take_and_masters() {
    let tmp = Tmp::new("pipe");
    let reference = write_reference(&tmp.path);
    let out = tmp.path.join("out");
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let cfg = config_wav_only();
    let script =
        "[calm] Hello from the studio.\n[warm pause_ms=200] The notes are in your inbox.\n";
    let scripted = vec![
        vec![
            scores(0.10, 0.90, 0.90),
            scores(0.00, 0.80, 0.91),
            scores(0.00, 0.70, 0.95),
        ],
        vec![
            scores(0.00, 0.80, 0.84),
            scores(0.00, 0.86, 0.99),
            scores(0.20, 0.90, 0.90),
        ],
    ];
    let mut renderer = SyntheticRenderer;
    let mut scorer = ScriptedScorer { scores: scripted };
    let device = choose(DeviceRequest::Auto, false).unwrap();
    let output = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script,
            out_dir: &out,
            repo_root: Some(&repo),
            owner_flag: true,
            device,
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap();
    let captions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output.captions_path).unwrap()).unwrap();
    assert_eq!(captions["schema"], "galion-local-studio-captions/1");
    assert_eq!(captions["sentences"][0]["take"], 1);
    assert_eq!(captions["sentences"][1]["take"], 1);
    assert_eq!(captions["sentences"][0]["script"], "Hello from the studio.");
    assert_eq!(captions["sentences"][0]["emotion"], "calm");
    assert!(
        captions["sentences"][0]["start"].as_f64().unwrap()
            < captions["sentences"][0]["end"].as_f64().unwrap()
    );
    assert!(
        captions["sentences"][0]["end"].as_f64().unwrap()
            <= captions["sentences"][1]["start"].as_f64().unwrap()
    );
    assert_eq!(captions["bed"]["tuning_hz"], 432.0);
    assert_eq!(captions["bed"]["under_voice_db"], 18.0);
    assert_eq!(captions["bed"]["source"], "generated");
    let master_bytes = std::fs::read(&output.master_wav).unwrap();
    assert_eq!(captions["sha256"], sha256_bytes(&master_bytes));
    let take_hash = captions["sentences"][0]["sha256"].as_str().unwrap();
    let take_bytes = std::fs::read(out.join("takes/line-00-take-01.wav")).unwrap();
    assert_eq!(take_hash, sha256_bytes(&take_bytes));
    let (samples, sr) = audio::read_wav_mono(&output.master_wav).unwrap();
    let lufs = integrated_lufs(&samples, sr).unwrap().unwrap();
    assert!((lufs + 16.0).abs() < 0.35, "{lufs}");
    assert!(galion_local_studio::master::true_peak_dbtp(&samples) <= -0.95);
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output.report_path).unwrap()).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["audio_kind"], "synthetic-tone");
    assert_eq!(report["renderer"], "synthetic");
}

#[test]
fn a_line_with_no_passing_take_writes_no_master() {
    let tmp = Tmp::new("fail");
    let reference = write_reference(&tmp.path);
    let out = tmp.path.join("out");
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let cfg = config_wav_only();
    let mut renderer = SyntheticRenderer;
    let mut scorer = ScriptedScorer {
        scores: vec![vec![scores(0.2, 0.4, 0.4), scores(0.2, 0.4, 0.4)]],
    };
    let mut cfg = cfg;
    cfg.render.takes = 2;
    let err = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script: "[calm] This line misses the gate.\n",
            out_dir: &out,
            repo_root: Some(&repo),
            owner_flag: true,
            device: choose(DeviceRequest::Cpu, true).unwrap(),
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap_err();
    assert!(err.report_path.unwrap().is_file());
    assert!(!out.join("master.wav").exists());
    assert!(err.message.contains("gates"));
}

#[test]
fn reference_inside_the_repo_is_refused() {
    let tmp = Tmp::new("repo");
    let reference = write_reference(&tmp.path);
    let err = locate_reference(&reference, Some(&tmp.path)).unwrap_err();
    assert!(err.contains("outside the repository"));
}

#[test]
fn owner_flag_is_required() {
    let tmp = Tmp::new("owner");
    let reference = write_reference(&tmp.path);
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let cfg = config_wav_only();
    let mut renderer = SyntheticRenderer;
    let mut scorer = ScriptedScorer {
        scores: vec![vec![scores(0.0, 0.8, 0.9)]],
    };
    let err = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script: "[calm] Hello.\n",
            out_dir: &tmp.path.join("out"),
            repo_root: Some(&repo),
            owner_flag: false,
            device: choose(DeviceRequest::Auto, false).unwrap(),
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap_err();
    assert!(err.message.contains("owner-reference"));
    assert!(err.report_path.is_none());
}

#[test]
fn process_scorer_uses_whisper_text_for_wer() {
    let tmp = Tmp::new("proc");
    let reference = write_reference(&tmp.path);
    let out = tmp.path.join("out");
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let script_path = tmp.path.join("score.sh");
    std::fs::write(
        &script_path,
        r#"#!/bin/sh
role=$1
take=$2
case "$role" in
  whisper)
    case "$take" in
      0) printf '%s\n' '{"text":"the meeting is on thursday"}' ;;
      1) printf '%s\n' '{"text":"the meeting is on friday","wer":0}' ;;
      *) printf '%s\n' '{"text":"totally unrelated words here now"}' ;;
    esac
    ;;
  ecapa)
    case "$take" in
      0) printf '%s\n' '{"mean":0.80}' ;;
      1) printf '%s\n' '{"mean":0.60}' ;;
      *) printf '%s\n' '{"mean":0.90}' ;;
    esac
    ;;
  emotion)
    case "$take" in
      0) printf '%s\n' '{"emotion":0.90}' ;;
      1) printf '%s\n' '{"emotion":0.90}' ;;
      *) printf '%s\n' '{"emotion":0.50}' ;;
    esac
    ;;
esac
"#,
    )
    .unwrap();
    let sh = script_path.display().to_string();
    let mut cfg = config_wav_only();
    cfg.render.takes = 3;
    let mut renderer = SyntheticRenderer;
    let mut scorer = ProcessScorer {
        whisper: vec![
            "/bin/sh".into(),
            sh.clone(),
            "whisper".into(),
            "{take}".into(),
        ],
        ecapa: vec![
            "/bin/sh".into(),
            sh.clone(),
            "ecapa".into(),
            "{take}".into(),
        ],
        emotion: vec!["/bin/sh".into(), sh, "emotion".into(), "{take}".into()],
        model: "small".into(),
        label_map: Default::default(),
    };
    let output = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script: "[calm] the meeting is on thursday\n",
            out_dir: &out,
            repo_root: Some(&repo),
            owner_flag: true,
            device: choose(DeviceRequest::Auto, false).unwrap(),
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap();
    let captions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.captions_path).unwrap()).unwrap();
    assert_eq!(captions["sentences"][0]["take"], 0);
    assert_eq!(captions["sentences"][0]["scores"]["wer"], 0.0);
    assert!(
        captions["sentences"][0]["scores"]["ecapa"]
            .as_f64()
            .unwrap()
            > 0.7057
    );
}

#[test]
fn emotion_stdout_is_not_read_from_the_ecapa_file() {
    let tmp = Tmp::new("split-score");
    let reference = write_reference(&tmp.path);
    let out = tmp.path.join("out");
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let script_path = tmp.path.join("score.sh");
    std::fs::write(
        &script_path,
        r#"#!/bin/sh
role=$1
case "$role" in
  whisper)
    printf '%s\n' '{"text":"hello there"}'
    ;;
  ecapa)
    printf '%s\n' '{"mean":0.90,"emotion":0.99}' > "$2"
    ;;
  emotion)
    printf '%s\n' '{"emotion":0.84}'
    ;;
esac
"#,
    )
    .unwrap();
    let sh = script_path.display().to_string();
    let mut cfg = config_wav_only();
    cfg.render.takes = 1;
    let mut renderer = SyntheticRenderer;
    let mut scorer = ProcessScorer {
        whisper: vec!["/bin/sh".into(), sh.clone(), "whisper".into()],
        ecapa: vec![
            "/bin/sh".into(),
            sh.clone(),
            "ecapa".into(),
            "{json}".into(),
        ],
        emotion: vec!["/bin/sh".into(), sh, "emotion".into()],
        model: "small".into(),
        label_map: Default::default(),
    };
    let output = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script: "[calm] hello there\n",
            out_dir: &out,
            repo_root: Some(&repo),
            owner_flag: true,
            device: choose(DeviceRequest::Auto, false).unwrap(),
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap();
    let captions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.captions_path).unwrap()).unwrap();
    let emotion = captions["sentences"][0]["scores"]["emotion"]
        .as_f64()
        .unwrap();
    assert!((emotion - 0.84).abs() < 1e-6, "{emotion}");
    assert!((emotion - 0.99).abs() > 0.05, "{emotion}");
}

#[test]
fn command_renderer_reads_the_wav_it_was_given() {
    let tmp = Tmp::new("cmd");
    let reference = write_reference(&tmp.path);
    let fixture = tmp.path.join("fixture.wav");
    audio::write_wav_mono(&fixture, &audio::sine(16_000, 24_000, 432.0, 0.15), 24_000).unwrap();
    let copy_sh = tmp.path.join("copy.sh");
    std::fs::write(&copy_sh, "#!/bin/sh\ncp \"$1\" \"$2\"\n").unwrap();
    let out = tmp.path.join("out");
    let repo = tmp.path.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let mut cfg = config_wav_only();
    cfg.render.takes = 1;
    let mut renderer = CommandRenderer {
        template: vec![
            "/bin/sh".into(),
            copy_sh.display().to_string(),
            fixture.display().to_string(),
            "{out}".into(),
        ],
        retry_cpu: false,
    };
    let mut scorer = ScriptedScorer {
        scores: vec![vec![scores(0.0, 0.8, 0.9)]],
    };
    let output = render_script(
        &cfg,
        &RunInput {
            reference: &reference,
            script: "[calm] Copied tone.\n",
            out_dir: &out,
            repo_root: Some(&repo),
            owner_flag: true,
            device: choose(DeviceRequest::Auto, false).unwrap(),
            loudness: LoudnessEngine::InProcess,
            ffmpeg: None,
        },
        &mut renderer,
        &mut scorer,
    )
    .unwrap();
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.report_path).unwrap()).unwrap();
    assert_eq!(report["audio_kind"], "runtime");
    assert_eq!(report["lines"][0]["takes"][0]["device_used"], "cpu");
}

#[test]
fn sources_do_not_embed_an_ip_address_or_a_secret() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect(&root.join("src"), &mut files);
    collect(&root.join("examples"), &mut files);
    files.push(root.join("README.md"));
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(!has_ipv4(&text), "ip in {}", path.display());
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("begin openssh"), "{}", path.display());
        assert!(!lower.contains("hf_token="), "{}", path.display());
        assert!(!lower.contains("api_key="), "{}", path.display());
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some("target") {
            continue;
        }
        if path.is_dir() {
            collect(&path, out);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "md" | "toml" | "txt" | "sh")
        ) {
            out.push(path);
        }
    }
}

fn has_ipv4(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            if ipv4_at(&text[i..]) {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn ipv4_at(text: &str) -> bool {
    let mut rest = text;
    for part in 0..4 {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if !(1..=3).contains(&digits) {
            return false;
        }
        let value: u32 = rest[..digits].parse().unwrap_or(999);
        if value > 255 {
            return false;
        }
        rest = &rest[digits..];
        if part < 3 {
            if !rest.starts_with('.') {
                return false;
            }
            rest = &rest[1..];
        }
    }
    true
}
