//! galion-fidelity — reference conditioning and the offline fidelity report.
//!
//! Does not synthesize (Piper stays the TTS), does not load Qwen, and does not
//! evaluate consent. Clone-time consent remains `galion-consent` / `galion_consent::evaluate`, once.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use galion_fidelity::audio::{self, TARGET_SR};
use galion_fidelity::score::{self, EvalInput};
use galion_fidelity::select::{self, Config};
use galion_fidelity::{self_check, short_case, CONSENT_EVALUATIONS};

fn usage() -> ! {
    eprintln!(
        "galion-fidelity — cloning-v2 reference fidelity (Rust)\n\
         \n\
         galion-fidelity self-check\n\
         galion-fidelity prepare --input REF.wav [--input REF2.wav ...] --out-wav OUT.wav --out-json OUT.json\n\
         galion-fidelity timbre --clone H.wav --ref R.wav --out OUT.wav [--strength 0.35|0.55]\n\
         galion-fidelity eval --ref R.wav --hyp H.wav --out REPORT.json \\\n\
             [--ref-text S] [--hyp-text S] [--ecapa-json E.json] [--persona-json P.json] \\\n\
             [--apply-timbre] [--timbre-strength F]\n\
         galion-fidelity synth-eval --out-dir DIR\n\
         \n\
         Outputs must be outside the git tree. No audio or weights are committed.\n\
         ECAPA / WER / emotion_match stay empty until transcripts, galion-ecapa JSON,\n\
         and persona-scorer JSON are passed. The DSP embedding cosine is not ECAPA.\n\
         Consent evaluations performed by this binary: {CONSENT_EVALUATIONS}."
    );
    std::process::exit(2);
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn flags(args: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            if let Some(v) = args.get(i + 1) {
                out.push(v.clone());
            }
        }
        i += 1;
    }
    out
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn require(value: Option<String>, name: &str) -> String {
    value.unwrap_or_else(|| {
        eprintln!("missing {name}");
        usage();
    })
}

fn read_json(path: &str) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn write_json(path: &PathBuf, value: &serde_json::Value) -> Result<(), String> {
    audio::ensure_output_allowed(path)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        usage();
    }
    let cmd = args[0].as_str();
    let rest = &args[1..];
    let result = match cmd {
        "self-check" => cmd_self_check(),
        "prepare" => cmd_prepare(rest),
        "timbre" => cmd_timbre(rest),
        "eval" => cmd_eval(rest),
        "synth-eval" => cmd_synth(rest),
        other => {
            eprintln!("unknown command {other}");
            usage();
        }
    };
    if let Err(err) = result {
        eprintln!("galion-fidelity: {err}");
        std::process::exit(1);
    }
}

fn cmd_self_check() -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(&self_check()).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn cmd_prepare(args: &[String]) -> Result<(), String> {
    let inputs = flags(args, "--input");
    if inputs.is_empty() {
        eprintln!("prepare needs at least one --input");
        usage();
    }
    let out_wav = PathBuf::from(require(flag(args, "--out-wav"), "--out-wav"));
    let out_json = PathBuf::from(require(flag(args, "--out-json"), "--out-json"));
    audio::ensure_output_allowed(&out_wav)?;
    audio::ensure_output_allowed(&out_json)?;
    let mut chunks = Vec::new();
    for path in &inputs {
        let (samples, sr) = audio::read_wav_mono(std::path::Path::new(path))?;
        chunks.push((samples, sr));
    }
    let mut cfg = Config::default();
    if has(args, "--timbre") {
        eprintln!(
            "--timbre is ignored by prepare; envelope match is the separate `timbre` command"
        );
    }
    if let Some(ideal) = flag(args, "--ideal-s") {
        cfg.ideal_s = ideal
            .parse()
            .map_err(|_| format!("bad --ideal-s {ideal}"))?;
    }
    let prepared = select::prepare(&chunks, &cfg)?;
    audio::write_wav_s16(&out_wav, &prepared.samples, prepared.meta.sample_rate)?;
    write_json(
        &out_json,
        &serde_json::to_value(&prepared.meta).map_err(|e| e.to_string())?,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&prepared.meta).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn cmd_timbre(args: &[String]) -> Result<(), String> {
    let clone_path = require(flag(args, "--clone"), "--clone");
    let ref_path = require(flag(args, "--ref"), "--ref");
    let out = PathBuf::from(require(flag(args, "--out"), "--out"));
    audio::ensure_output_allowed(&out)?;
    let strength: f32 = match flag(args, "--strength") {
        Some(s) => s.parse().map_err(|_| format!("bad --strength {s}"))?,
        None => 0.55,
    };
    let (clone, clone_sr) = audio::read_wav_mono(std::path::Path::new(&clone_path))?;
    let (reference, reference_sr) = audio::read_wav_mono(std::path::Path::new(&ref_path))?;
    let clone = audio::resample_linear(&clone, clone_sr, TARGET_SR);
    let reference = audio::resample_linear(&reference, reference_sr, TARGET_SR);
    let matched =
        galion_fidelity::envelope::correct_envelope(&clone, &reference, TARGET_SR, strength);
    audio::write_wav_s16(&out, &matched, TARGET_SR)?;
    println!(
        "{}",
        serde_json::json!({
            "op": "timbre",
            "strength": strength,
            "sample_rate": TARGET_SR,
            "out": out.display().to_string(),
            "note": "opt-in spectral envelope post layer; strength 0 would have been a copy",
        })
    );
    Ok(())
}

fn cmd_eval(args: &[String]) -> Result<(), String> {
    let ref_path = require(flag(args, "--ref"), "--ref");
    let hyp_path = require(flag(args, "--hyp"), "--hyp");
    let out = PathBuf::from(require(flag(args, "--out"), "--out"));
    audio::ensure_output_allowed(&out)?;
    let (reference, reference_sr) = audio::read_wav_mono(std::path::Path::new(&ref_path))?;
    let (hypothesis, hypothesis_sr) = audio::read_wav_mono(std::path::Path::new(&hyp_path))?;
    let ecapa = match flag(args, "--ecapa-json") {
        Some(path) => Some(read_json(&path)?),
        None => None,
    };
    let persona = match flag(args, "--persona-json") {
        Some(path) => Some(read_json(&path)?),
        None => None,
    };
    let strength = flag(args, "--timbre-strength").and_then(|s| s.parse().ok());
    let report = score::evaluate(EvalInput {
        reference: &reference,
        reference_sr,
        hypothesis: &hypothesis,
        hypothesis_sr,
        reference_text: flag(args, "--ref-text").as_deref(),
        hypothesis_text: flag(args, "--hyp-text").as_deref(),
        ecapa_json: ecapa.as_ref(),
        persona_json: persona.as_ref(),
        apply_timbre: has(args, "--apply-timbre"),
        timbre_strength: strength,
    });
    let value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
    write_json(&out, &value)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn cmd_synth(args: &[String]) -> Result<(), String> {
    let dir = PathBuf::from(require(flag(args, "--out-dir"), "--out-dir"));
    let reference_path = dir.join("short-ref.wav");
    let hypothesis_path = dir.join("short-hyp.wav");
    let plan_path = dir.join("short-plan.json");
    let eval_path = dir.join("short-eval.json");
    let summary_path = dir.join("summary.json");
    for path in [
        &reference_path,
        &hypothesis_path,
        &plan_path,
        &eval_path,
        &summary_path,
    ] {
        audio::ensure_output_allowed(path)?;
    }
    let prepared = short_case();
    let reference = galion_fidelity::clean_vowel();
    audio::write_wav_s16(&reference_path, &reference, TARGET_SR)?;
    audio::write_wav_s16(
        &hypothesis_path,
        &prepared.samples,
        prepared.meta.sample_rate,
    )?;
    write_json(
        &plan_path,
        &serde_json::to_value(&prepared.meta).map_err(|e| e.to_string())?,
    )?;
    let report = score::evaluate(EvalInput {
        reference: &reference,
        reference_sr: TARGET_SR,
        hypothesis: &prepared.samples,
        hypothesis_sr: prepared.meta.sample_rate,
        reference_text: None,
        hypothesis_text: None,
        ecapa_json: None,
        persona_json: None,
        apply_timbre: false,
        timbre_strength: None,
    });
    let eval_value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
    write_json(&eval_path, &eval_value)?;
    let check = self_check();
    let summary = serde_json::json!({
        "schema": "galion-fidelity-synth-eval/1",
        "note": "Synthetic harmonic fixtures, not a speaker and not a rollout measurement. ECAPA, WER, and emotion_match are not measured here.",
        "short_plan": check["audio"]["short"],
        "long_plan": check["audio"]["long"],
        "cap_plan": check["audio"]["cap"],
        "eval_vs_clean_vowel": eval_value,
        "envelope": {
            "clean": check["audio"]["envelope_ratio_clean"],
            "damaged": check["audio"]["envelope_ratio_damaged"],
            "repaired": check["audio"]["envelope_ratio_repaired"],
        },
        "gates": {
            "wer_max_percent": 1.8,
            "ecapa_min_exclusive": 0.7057,
            "emotion_match_min": 0.83,
            "measured": false,
        }
    });
    write_json(&summary_path, &summary)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    );
    Ok(())
}
