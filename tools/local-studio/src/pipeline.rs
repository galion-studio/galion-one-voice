// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Render each line N times, keep the take that passes the gates, stitch,
//! mix the generated bed, and master.

use crate::audio::{self, StitchPiece};
use crate::bed::render_bed;
use crate::command::tail;
use crate::config::StudioConfig;
use crate::device::DeviceChoice;
use crate::master::{self, Mastered};
use crate::render::{AudioKind, RenderJob, TakeRenderer};
use crate::score::{judge, GateResult, ScoreJob, TakeScorer, TakeScores};
use crate::script::{self, Line};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_TAKE_SECS: f64 = 120.0;
const MAX_PROGRAM_SECS: f64 = 15.0 * 60.0;

#[derive(Debug)]
pub struct RunFailure {
    pub message: String,
    pub report_path: Option<PathBuf>,
}

impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.report_path {
            Some(path) => write!(f, "{}\nreport: {}", self.message, path.display()),
            None => f.write_str(&self.message),
        }
    }
}

pub enum LoudnessEngine {
    InProcess,
    Ffmpeg(PathBuf),
}

pub struct RunInput<'a> {
    pub reference: &'a Path,
    pub script: &'a str,
    pub out_dir: &'a Path,
    pub repo_root: Option<&'a Path>,
    pub owner_flag: bool,
    pub device: DeviceChoice,
    pub loudness: LoudnessEngine,
    pub ffmpeg: Option<PathBuf>,
}

#[derive(Debug)]
pub struct RunOutput {
    pub report_path: PathBuf,
    pub captions_path: PathBuf,
    pub master_wav: PathBuf,
    pub outputs: Vec<PathBuf>,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    ok: bool,
    error: Option<String>,
    renderer: String,
    audio_kind: String,
    os: String,
    device: DeviceChoice,
    reference_sha256: String,
    output_inside_repo: bool,
    gates: GateEcho,
    bed: BedEcho,
    lines: Vec<LineReport>,
    master: Option<MasterEcho>,
    outputs: Vec<String>,
}

#[derive(Serialize)]
struct GateEcho {
    wer_max: f64,
    ecapa_min: f64,
    emotion_min: f64,
    ecapa_comparison: &'static str,
}

#[derive(Serialize)]
struct BedEcho {
    enabled: bool,
    tuning_hz: f64,
    under_voice_db: f64,
    source: &'static str,
}

#[derive(Serialize)]
struct LineReport {
    index: usize,
    emotion: String,
    text: String,
    chosen_take: Option<usize>,
    passed: bool,
    takes: Vec<TakeReport>,
}

#[derive(Serialize)]
struct TakeReport {
    take: usize,
    seed: u64,
    wav: String,
    sha256: Option<String>,
    device_used: String,
    audio_kind: String,
    note: String,
    error: Option<String>,
    scores: TakeScores,
    gate: GateResult,
}

#[derive(Serialize)]
struct MasterEcho {
    engine: String,
    limited_by: String,
    lufs: f64,
    true_peak_dbtp: f64,
    sample_rate: u32,
}

#[derive(Serialize)]
struct Captions {
    schema: &'static str,
    sha256: String,
    reference_sha256: String,
    sample_rate: u32,
    loudness_lufs: f64,
    true_peak_dbtp: f64,
    bed: BedEcho,
    sentences: Vec<Sentence>,
}

#[derive(Serialize)]
struct Sentence {
    index: usize,
    start: f64,
    end: f64,
    script: String,
    emotion: String,
    take: usize,
    scores: ScoreEcho,
    gates: GateEchoBools,
    transcript: String,
    sha256: String,
}

#[derive(Serialize)]
struct ScoreEcho {
    wer: f64,
    ecapa: f64,
    emotion: f64,
}

#[derive(Serialize)]
struct GateEchoBools {
    wer: bool,
    ecapa: bool,
    emotion: bool,
    passed: bool,
}

struct Attempt {
    take_index: usize,
    seed: u64,
    wav_rel: String,
    sha256: Option<String>,
    samples: Option<Vec<f32>>,
    scores: TakeScores,
    gate: GateResult,
    device_used: String,
    note: String,
    error: Option<String>,
    kind: AudioKind,
}

struct Winner {
    index: usize,
    emotion: String,
    text: String,
    take_index: usize,
    samples: Vec<f32>,
    scores: TakeScores,
    gate: GateResult,
    sha256: String,
    gap_after: usize,
    kind: AudioKind,
}

pub fn render_script(
    config: &StudioConfig,
    input: &RunInput<'_>,
    renderer: &mut dyn TakeRenderer,
    scorer: &mut dyn TakeScorer,
) -> Result<RunOutput, RunFailure> {
    let reference = match prepare(config, input) {
        Ok(path) => path,
        Err(message) => {
            return Err(RunFailure {
                message,
                report_path: None,
            })
        }
    };
    if let Err(err) = std::fs::create_dir_all(input.out_dir) {
        return Err(RunFailure {
            message: format!("{}: {err}", input.out_dir.display()),
            report_path: None,
        });
    }
    let reference_sha256 = match audio::sha256_file(&reference) {
        Ok(hash) => hash,
        Err(message) => {
            return Err(RunFailure {
                message,
                report_path: None,
            })
        }
    };
    let mut report = Report {
        schema: "galion-local-studio-report/1",
        ok: false,
        error: None,
        renderer: renderer.name().to_string(),
        audio_kind: "pending".into(),
        os: std::env::consts::OS.to_string(),
        device: input.device.clone(),
        reference_sha256: reference_sha256.clone(),
        output_inside_repo: inside_repo(input.out_dir, input.repo_root),
        gates: GateEcho {
            wer_max: config.gates.wer_max,
            ecapa_min: config.gates.ecapa_min,
            emotion_min: config.gates.emotion_min,
            ecapa_comparison: "greater-than",
        },
        bed: bed_echo(config),
        lines: Vec::new(),
        master: None,
        outputs: Vec::new(),
    };
    let outcome = assemble(config, input, &reference, renderer, scorer, &mut report);
    if let Err(message) = &outcome {
        if report.error.is_none() {
            report.error = Some(message.clone());
        }
    }
    let report_path = match write_json(&input.out_dir.join("report.json"), &report) {
        Ok(path) => path,
        Err(message) => {
            return Err(RunFailure {
                message,
                report_path: None,
            })
        }
    };
    match outcome {
        Ok((captions_path, master_wav, outputs)) => Ok(RunOutput {
            report_path,
            captions_path,
            master_wav,
            outputs,
        }),
        Err(message) => Err(RunFailure {
            message,
            report_path: Some(report_path),
        }),
    }
}

fn prepare(config: &StudioConfig, input: &RunInput<'_>) -> Result<PathBuf, String> {
    if !input.owner_flag || !config.consent.owner_reference {
        return Err(
            "pass --owner-reference and set consent.owner_reference = true. The reference clip is the owner's own voice, supplied at runtime"
                .into(),
        );
    }
    let reference = locate_reference(input.reference, input.repo_root)?;
    let lines = script::parse(input.script)?;
    if lines.is_empty() {
        return Err("script has no spoken lines".into());
    }
    let _ = lines;
    if needs_encode(&config.output.formats) && input.ffmpeg.is_none() {
        return Err(
            "opus and mp3 need ffmpeg on PATH, or set output.formats to wav until ffmpeg is installed"
                .into(),
        );
    }
    Ok(reference)
}

pub fn locate_reference(reference: &Path, repo_root: Option<&Path>) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
    let given = if reference.is_absolute() {
        reference.to_path_buf()
    } else {
        cwd.join(reference)
    };
    if let Some(repo) = repo_root {
        if starts_with_repo(&given, repo) {
            return Err("reference clip must live outside the repository".into());
        }
    }
    if !given.is_file() {
        return Err(format!("reference clip is not a file: {}", given.display()));
    }
    let len = given
        .metadata()
        .map_err(|err| format!("{}: {err}", given.display()))?
        .len();
    if len == 0 {
        return Err("reference clip is empty".into());
    }
    if len > 200 * 1024 * 1024 {
        return Err("reference clip is larger than 200 MB".into());
    }
    let canon = given
        .canonicalize()
        .map_err(|err| format!("{}: {err}", given.display()))?;
    if let Some(repo) = repo_root {
        if starts_with_repo(&canon, repo) {
            return Err("reference clip must live outside the repository".into());
        }
    }
    Ok(canon)
}

fn starts_with_repo(path: &Path, repo: &Path) -> bool {
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    path.starts_with(&repo)
}

fn inside_repo(path: &Path, repo: Option<&Path>) -> bool {
    match repo {
        Some(repo) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let abs = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            starts_with_repo(&abs, repo)
                || abs
                    .canonicalize()
                    .is_ok_and(|canon| starts_with_repo(&canon, repo))
        }
        None => false,
    }
}

fn needs_encode(formats: &[String]) -> bool {
    formats.iter().any(|fmt| fmt == "opus" || fmt == "mp3")
}

fn assemble(
    config: &StudioConfig,
    input: &RunInput<'_>,
    reference: &Path,
    renderer: &mut dyn TakeRenderer,
    scorer: &mut dyn TakeScorer,
    report: &mut Report,
) -> Result<(PathBuf, PathBuf, Vec<PathBuf>), String> {
    let lines = script::parse(input.script)?;
    let sr = config.render.sample_rate;
    let mut winners = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let (attempts, winner) =
            render_line(config, input, reference, renderer, scorer, index, line)?;
        let passed = winner
            .as_ref()
            .map(|item| item.gate.passed)
            .unwrap_or(false);
        if let Some(item) = &winner {
            if report.audio_kind == "pending" {
                report.audio_kind = kind_name(item.kind).into();
            }
        }
        report.lines.push(LineReport {
            index,
            emotion: line.emotion.clone(),
            text: line.text.clone(),
            chosen_take: winner.as_ref().map(|item| item.take_index),
            passed,
            takes: attempts.iter().map(take_report).collect(),
        });
        match winner {
            Some(item) if item.gate.passed => winners.push(item),
            _ => {
                let message = format!(
                    "line {} has no take inside the gates (WER <= {}, ECAPA > {}, emotion >= {})",
                    index + 1,
                    config.gates.wer_max,
                    config.gates.ecapa_min,
                    config.gates.emotion_min
                );
                report.error = Some(message.clone());
                return Err(message);
            }
        }
    }
    if winners.len() != lines.len() {
        return Err("a line is missing a passing take".into());
    }
    let pieces: Vec<StitchPiece<'_>> = winners
        .iter()
        .map(|winner| StitchPiece {
            samples: &winner.samples,
            gap_after: winner.gap_after,
        })
        .collect();
    let stitched = audio::stitch(
        audio::ms_samples(sr, config.timing.lead_in_ms),
        &pieces,
        audio::ms_samples(sr, config.timing.tail_ms),
    );
    if stitched.samples.len() as f64 > f64::from(sr) * MAX_PROGRAM_SECS {
        return Err("stitched program is longer than 15 minutes".into());
    }
    let mixed = if config.bed.enabled {
        let bed = render_bed(stitched.samples.len(), sr, config.bed.tuning_hz);
        audio::mix_under(
            &stitched.samples,
            &bed,
            &stitched.speech,
            config.bed.under_voice_db,
        )
    } else {
        stitched.samples
    };
    let mastered = match &input.loudness {
        LoudnessEngine::InProcess => master::apply(
            &mixed,
            sr,
            config.master.target_lufs,
            config.master.true_peak_dbtp,
        )?,
        LoudnessEngine::Ffmpeg(bin) => master::apply_ffmpeg(
            bin,
            &mixed,
            sr,
            config.master.target_lufs,
            config.master.true_peak_dbtp,
        )?,
    };
    if mastered.true_peak_dbtp > config.master.true_peak_dbtp + 0.05 {
        return Err(format!(
            "true peak {:.2} dBTP is above the ceiling",
            mastered.true_peak_dbtp
        ));
    }
    let master_path = input.out_dir.join("master.wav");
    audio::write_wav_mono(&master_path, &mastered.samples, sr)?;
    let master_hash = audio::sha256_file(&master_path)?;
    let captions = captions_doc(
        config,
        &report.reference_sha256,
        &master_hash,
        sr,
        &mastered,
        &winners,
        &stitched.speech,
    );
    let captions_path = write_json(&input.out_dir.join("captions.json"), &captions)?;
    let mut outputs = vec![master_path.clone(), captions_path.clone()];
    for fmt in &config.output.formats {
        if fmt == "wav" {
            continue;
        }
        let dest = input.out_dir.join(format!("master.{fmt}"));
        let bin = input
            .ffmpeg
            .as_ref()
            .ok_or_else(|| "ffmpeg is missing".to_string())?;
        encode(bin, &master_path, &dest, fmt)?;
        outputs.push(dest);
    }
    report.ok = true;
    report.master = Some(MasterEcho {
        engine: mastered.engine,
        limited_by: mastered.limited_by,
        lufs: round3(mastered.lufs),
        true_peak_dbtp: round3(mastered.true_peak_dbtp),
        sample_rate: sr,
    });
    report.outputs = outputs
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    Ok((captions_path, master_path, outputs))
}

fn render_line(
    config: &StudioConfig,
    input: &RunInput<'_>,
    reference: &Path,
    renderer: &mut dyn TakeRenderer,
    scorer: &mut dyn TakeScorer,
    index: usize,
    line: &Line,
) -> Result<(Vec<Attempt>, Option<Winner>), String> {
    let sr = config.render.sample_rate;
    let mut attempts = Vec::new();
    for take in 0..config.render.takes {
        let seed = config
            .render
            .seed
            .wrapping_add((index as u64).wrapping_mul(1000))
            .wrapping_add(take as u64);
        let wav_rel = format!("takes/line-{index:02}-take-{take:02}.wav");
        let wav_path = input.out_dir.join(&wav_rel);
        let job = RenderJob {
            line_index: index,
            take_index: take,
            text: &line.text,
            emotion: &line.emotion,
            reference,
            out_wav: &wav_path,
            seed,
            temperature: config.render.temperature,
            device_ep: &input.device.ep,
            device_kind: input.device.used,
            sample_rate: sr,
        };
        let rendered = match renderer.render(&job) {
            Ok(rendered) => rendered,
            Err(crate::command::ToolError::Fatal(message)) => return Err(message),
            Err(crate::command::ToolError::Take(message)) => {
                attempts.push(failed_attempt(take, seed, wav_rel, message));
                continue;
            }
        };
        let mut samples = if rendered.sample_rate == sr {
            rendered.samples
        } else {
            audio::resample_linear(&rendered.samples, rendered.sample_rate, sr)
        };
        let duration = samples.len() as f64 / f64::from(sr);
        if duration > MAX_TAKE_SECS {
            attempts.push(failed_attempt(
                take,
                seed,
                wav_rel,
                format!("take is {duration:.1} s, above {MAX_TAKE_SECS:.0} s"),
            ));
            continue;
        }
        audio::fade_edges(&mut samples, sr, 8);
        if let Err(err) = audio::write_wav_mono(&wav_path, &samples, sr) {
            attempts.push(failed_attempt(take, seed, wav_rel, err));
            continue;
        }
        let hash = audio::sha256_file(&wav_path).ok();
        let score_job = ScoreJob {
            line_index: index,
            take_index: take,
            text: &line.text,
            emotion: &line.emotion,
            reference,
            wav: &wav_path,
            device_ep: &rendered.device_used,
        };
        let scores = match scorer.score(&score_job) {
            Ok(scores) => scores,
            Err(crate::command::ToolError::Fatal(message)) => return Err(message),
            Err(crate::command::ToolError::Take(message)) => TakeScores {
                detail: message.clone(),
                ..TakeScores::default()
            },
        };
        let gate = judge(&config.gates, &scores);
        attempts.push(Attempt {
            take_index: take,
            seed,
            wav_rel,
            sha256: hash,
            samples: Some(samples),
            scores,
            gate,
            device_used: rendered.device_used,
            note: rendered.note,
            error: None,
            kind: rendered.kind,
        });
    }
    if attempts.is_empty() {
        return Err(format!("line {} produced no takes", index + 1));
    }
    let best = pick_best(&attempts);
    let winner = attempts.get(best).and_then(|attempt| {
        let samples = attempt.samples.clone()?;
        Some(Winner {
            index,
            emotion: line.emotion.clone(),
            text: line.text.clone(),
            take_index: attempt.take_index,
            samples,
            scores: attempt.scores.clone(),
            gate: attempt.gate.clone(),
            sha256: attempt.sha256.clone().unwrap_or_default(),
            gap_after: audio::ms_samples(sr, line.pause_after_ms.unwrap_or(config.timing.gap_ms)),
            kind: attempt.kind,
        })
    });
    Ok((attempts, winner))
}

fn failed_attempt(take: usize, seed: u64, wav_rel: String, message: String) -> Attempt {
    Attempt {
        take_index: take,
        seed,
        wav_rel,
        sha256: None,
        samples: None,
        scores: TakeScores {
            detail: message.clone(),
            ..TakeScores::default()
        },
        gate: GateResult {
            wer: false,
            ecapa: false,
            emotion: false,
            passed: false,
            detail: message.clone(),
        },
        device_used: String::new(),
        note: String::new(),
        error: Some(message),
        kind: AudioKind::Runtime,
    }
}

fn pick_best(attempts: &[Attempt]) -> usize {
    let mut best = 0usize;
    for index in 1..attempts.len() {
        if better(&attempts[index], &attempts[best]) {
            best = index;
        }
    }
    best
}

fn better(a: &Attempt, b: &Attempt) -> bool {
    use std::cmp::Ordering::{Equal, Greater};
    let pass = (a.gate.passed as u8).cmp(&(b.gate.passed as u8));
    if pass != Equal {
        return pass == Greater;
    }
    let emotion = cmp_score(a.scores.emotion, b.scores.emotion);
    if emotion != Equal {
        return emotion == Greater;
    }
    let ecapa = cmp_score(a.scores.ecapa, b.scores.ecapa);
    if ecapa != Equal {
        return ecapa == Greater;
    }
    let wer = cmp_score(b.scores.wer, a.scores.wer);
    if wer != Equal {
        return wer == Greater;
    }
    a.take_index < b.take_index
}

fn cmp_score(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (finite(a), finite(b)) {
        (None, None) => std::cmp::Ordering::Equal,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => left.total_cmp(&right),
    }
}

fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|item| item.is_finite())
}

fn take_report(attempt: &Attempt) -> TakeReport {
    TakeReport {
        take: attempt.take_index,
        seed: attempt.seed,
        wav: attempt.wav_rel.clone(),
        sha256: attempt.sha256.clone(),
        device_used: attempt.device_used.clone(),
        audio_kind: kind_name(attempt.kind).into(),
        note: attempt.note.clone(),
        error: attempt.error.clone(),
        scores: round_scores(&attempt.scores),
        gate: attempt.gate.clone(),
    }
}

fn round_scores(scores: &TakeScores) -> TakeScores {
    TakeScores {
        wer: scores.wer.map(round6),
        ecapa: scores.ecapa.map(round6),
        emotion: scores.emotion.map(round6),
        transcript: scores.transcript.clone(),
        detail: scores.detail.clone(),
    }
}

fn captions_doc(
    config: &StudioConfig,
    reference_sha256: &str,
    master_hash: &str,
    sample_rate: u32,
    mastered: &Mastered,
    winners: &[Winner],
    speech: &[(usize, usize)],
) -> Captions {
    let sentences = winners
        .iter()
        .zip(speech)
        .map(|(winner, (start, end))| Sentence {
            index: winner.index,
            start: round3(*start as f64 / f64::from(sample_rate)),
            end: round3(*end as f64 / f64::from(sample_rate)),
            script: winner.text.clone(),
            emotion: winner.emotion.clone(),
            take: winner.take_index,
            scores: ScoreEcho {
                wer: round6(winner.scores.wer.unwrap_or(f64::NAN)),
                ecapa: round6(winner.scores.ecapa.unwrap_or(f64::NAN)),
                emotion: round6(winner.scores.emotion.unwrap_or(f64::NAN)),
            },
            gates: GateEchoBools {
                wer: winner.gate.wer,
                ecapa: winner.gate.ecapa,
                emotion: winner.gate.emotion,
                passed: winner.gate.passed,
            },
            transcript: winner.scores.transcript.clone(),
            sha256: winner.sha256.clone(),
        })
        .collect();
    Captions {
        schema: "galion-local-studio-captions/1",
        sha256: master_hash.to_string(),
        reference_sha256: reference_sha256.to_string(),
        sample_rate,
        loudness_lufs: round3(mastered.lufs),
        true_peak_dbtp: round3(mastered.true_peak_dbtp),
        bed: bed_echo(config),
        sentences,
    }
}

fn bed_echo(config: &StudioConfig) -> BedEcho {
    BedEcho {
        enabled: config.bed.enabled,
        tuning_hz: config.bed.tuning_hz,
        under_voice_db: config.bed.under_voice_db,
        source: "generated",
    }
}

fn kind_name(kind: AudioKind) -> &'static str {
    match kind {
        AudioKind::SyntheticTone => "synthetic-tone",
        AudioKind::Runtime => "runtime",
    }
}

fn round3(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("{}: {err}", parent.display()))?;
        }
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    text.push('\n');
    std::fs::write(path, text).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(path.to_path_buf())
}

fn encode(ffmpeg: &Path, wav: &Path, dest: &Path, format: &str) -> Result<(), String> {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y").arg("-hide_banner").arg("-i").arg(wav);
    match format {
        "opus" => {
            cmd.args(["-c:a", "libopus", "-b:a", "96k"]);
        }
        "mp3" => {
            cmd.args(["-c:a", "libmp3lame", "-q:a", "2"]);
        }
        other => return Err(format!("encoder format {other} is not opus or mp3")),
    }
    cmd.arg(dest);
    let output = cmd
        .output()
        .map_err(|err| format!("{}: {err}", ffmpeg.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg {format} failed: {}", tail(&stderr, 400)));
    }
    Ok(())
}
