//! refprep — Cloning V2 shared reference-preparation stage.
//! One code path + one default config for every voice. Per-voice differences come only from
//! measurements made on that voice's recording (noise floor, f99, ECAPA phrase consistency).
//! Inputs are opened read-only; refprep refuses to write over its input.
mod dsp;
mod resample;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "refprep", about = "Cloning V2 shared reference preparation")]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    /// Original recording -> conditioning reference WAV + metadata JSON
    Prep {
        #[arg(long)] input: PathBuf,
        #[arg(long)] output: PathBuf,
        #[arg(long)] meta: PathBuf,
        #[arg(long)] config: Option<PathBuf>,
        /// scratch dir for phrase WAVs (ECAPA consistency)
        #[arg(long)] work_dir: Option<PathBuf>,
    },
    /// Print the built-in default config (TOML)
    DefaultConfig,
    /// Measure LUFS / true-peak / f99 / clicks of WAVs
    Measure { files: Vec<PathBuf> },
}

#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Gate { enabled: bool, floor_percentile: f32, open_above_floor_db: f32, max_atten_db: f32, min_snr_db: f32,
    spectral: bool, n_std: f64, prop_decrease: f64, smooth_hz: f64, smooth_ms: f64 }
impl Default for Gate { fn default() -> Self { Gate { enabled: true, floor_percentile: 0.10, open_above_floor_db: 8.0, max_atten_db: 12.0, min_snr_db: 45.0,
    spectral: true, n_std: 1.8, prop_decrease: 0.55, smooth_hz: 500.0, smooth_ms: 50.0 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Declick { enabled: bool, threshold: f32 }
impl Default for Declick { fn default() -> Self { Declick { enabled: true, threshold: 0.35 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Trim { pad_s: f32 }
impl Default for Trim { fn default() -> Self { Trim { pad_s: 0.10 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Select { mode: String, min_seg_s: f32, max_seg_s: f32, pause_s: f32, max_gap_s: f32, mad_k: f64, abs_floor_drop: f64,
    min_keep_s: f32, xfade_ms: f32, ecapa_bin: String, ecapa_threads: u32 }
impl Default for Select { fn default() -> Self { Select { mode: "ecapa_consistent".into(), min_seg_s: 1.5, max_seg_s: 6.0, pause_s: 0.20,
    max_gap_s: 0.30, mad_k: 2.0, abs_floor_drop: 0.10, min_keep_s: 10.0, xfade_ms: 15.0,
    ecapa_bin: "/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/ecapa/target/release/galion-ecapa".into(), ecapa_threads: 2 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct HfShelf { enabled: bool, trigger_f99_hz: f64, freq_hz: f64, gain_db: f64, q: f64, exciter_mix: f64 }
impl Default for HfShelf { fn default() -> Self { HfShelf { enabled: true, trigger_f99_hz: 3500.0, freq_hz: 2800.0, gain_db: 2.5, q: 0.707, exciter_mix: 0.08 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Loud { target_lufs: f64, true_peak_dbtp: f64 }
impl Default for Loud { fn default() -> Self { Loud { target_lufs: -23.0, true_peak_dbtp: -1.0 } } }
#[derive(Serialize, Deserialize, Clone)] #[serde(default)]
struct Config { sample_rate: u32, gate: Gate, declick: Declick, trim: Trim, select: Select, hf_shelf: HfShelf, loudness: Loud }
impl Default for Config { fn default() -> Self { Config { sample_rate: 24000, gate: Gate::default(), declick: Declick::default(), trim: Trim::default(),
    select: Select::default(), hf_shelf: HfShelf::default(), loudness: Loud::default() } } }

fn read_mono(path: &Path) -> Result<(Vec<f32>, u32, u16)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let sp = r.spec(); let ch = sp.channels as usize;
    let inter: Vec<f32> = match (sp.sample_format, sp.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => r.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?,
        (hound::SampleFormat::Int, 24) => r.samples::<i32>().map(|s| s.map(|v| v as f32 / 8388608.0)).collect::<Result<_, _>>()?,
        (hound::SampleFormat::Int, 32) => r.samples::<i32>().map(|s| s.map(|v| v as f32 / 2147483648.0)).collect::<Result<_, _>>()?,
        (hound::SampleFormat::Float, 32) => r.samples::<f32>().collect::<Result<_, _>>()?,
        (f, b) => bail!("unsupported wav {:?}/{}", f, b),
    };
    Ok((inter.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect(), sp.sample_rate, sp.channels))
}
fn write_s16(path: &Path, x: &[f32], sr: u32) -> Result<()> {
    let sp = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, sp)?;
    for &s in x { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16)?; }
    w.finalize()?; Ok(())
}
fn measure(x: &[f32], sr: u32) -> serde_json::Value {
    let (f90, f99, gt4) = dsp::bandwidth(x, sr);
    json!({"duration_s": x.len() as f64 / sr as f64, "integrated_lufs": (dsp::integrated_lufs(x, sr) * 100.0).round() / 100.0,
        "true_peak_dbtp": (dsp::db(dsp::true_peak(x)) * 100.0).round() / 100.0, "f90_hz": f90, "f99_hz": f99,
        "pct_energy_gt_4k": (gt4 * 1e5).round() / 1e3, "clicks": dsp::count_clicks(x, 0.35)})
}

/// Downward-expander gate. Skipped (data-driven) when the recording is already clean.
fn gate(x: &[f32], sr: u32, g: &Gate) -> (Vec<f32>, serde_json::Value) {
    let (n, h) = ((sr as f32 * 0.020) as usize, (sr as f32 * 0.010) as usize);
    let rms = dsp::frame_rms(x, n, h);
    let floor = dsp::quantile(&rms, g.floor_percentile).max(1e-7);
    let p90 = dsp::quantile(&rms, 0.90).max(1e-7);
    let snr = 20.0 * (p90 / floor).log10();
    let floor_db = 20.0 * floor.log10();
    if !g.enabled || snr >= g.min_snr_db || rms.is_empty() {
        return (x.to_vec(), json!({"applied": false, "noise_floor_dbfs": floor_db, "frame_snr_db": snr,
            "reason": if !g.enabled { "disabled" } else { "snr>=min_snr_db" }}));
    }
    if g.spectral {
        let y = dsp::spectral_gate(x, sr, 2048, 512, g.n_std, g.prop_decrease, g.smooth_hz, g.smooth_ms);
        return (y, json!({"applied": true, "kind": "stationary_spectral", "noise_floor_dbfs": floor_db, "frame_snr_db": snr,
            "n_std": g.n_std, "prop_decrease": g.prop_decrease}));
    }
    let open = floor_db + g.open_above_floor_db;
    let raw: Vec<f32> = rms.iter().map(|r| {
        let l = 20.0 * r.max(1e-9).log10();
        let a = if l >= open { 0.0 } else if l <= floor_db { -g.max_atten_db } else { -g.max_atten_db * (open - l) / (open - floor_db) };
        10f32.powf(a / 20.0) }).collect();
    // fast open (1 frame), slow release (~80 ms)
    let rel = (-1.0f32 / 8.0).exp();
    let mut sm = raw.clone();
    for i in 1..sm.len() { sm[i] = if raw[i] > sm[i - 1] { raw[i] } else { rel * sm[i - 1] + (1.0 - rel) * raw[i] }; }
    // look-ahead: open slightly before onsets
    let la = sm.clone(); for i in 0..sm.len() { sm[i] = la[i..(i + 3).min(la.len())].iter().cloned().fold(0.0, f32::max); }
    let mut out = x.to_vec(); let mut atten_sum = 0f64;
    for (i, o) in out.iter_mut().enumerate() {
        let fpos = (i as f32 - n as f32 / 2.0) / h as f32;
        let k = fpos.floor().clamp(0.0, (sm.len() - 1) as f32) as usize; let k1 = (k + 1).min(sm.len() - 1);
        let t = (fpos - k as f32).clamp(0.0, 1.0);
        let gg = sm[k] * (1.0 - t) + sm[k1] * t; *o *= gg; atten_sum += gg as f64;
    }
    (out, json!({"applied": true, "noise_floor_dbfs": floor_db, "frame_snr_db": snr, "open_dbfs": open,
        "mean_gain_db": 20.0 * (atten_sum / x.len() as f64).log10()}))
}

fn speech_mask(x: &[f32], sr: u32) -> (Vec<bool>, usize) {
    let (n, h) = ((sr as f32 * 0.025) as usize, (sr as f32 * 0.010) as usize);
    let rms = dsp::frame_rms(x, n, h);
    let thr = (dsp::quantile(&rms, 0.15) * 3.0).max(1e-5);
    (rms.iter().map(|r| *r > thr).collect(), h)
}
fn trim(x: &[f32], sr: u32, pad_s: f32) -> (Vec<f32>, f64, f64) {
    let (m, h) = speech_mask(x, sr);
    let idx: Vec<usize> = m.iter().enumerate().filter(|(_, s)| **s).map(|(i, _)| i).collect();
    if idx.is_empty() { return (x.to_vec(), 0.0, x.len() as f64 / sr as f64); }
    let pad = (pad_s * sr as f32) as usize;
    let a = (idx[0] * h).saturating_sub(pad); let b = (idx[idx.len() - 1] * h + (sr as usize / 40) + pad).min(x.len());
    (x[a..b].to_vec(), a as f64 / sr as f64, b as f64 / sr as f64)
}
/// Phrase segments (sample ranges) split at pauses >= pause_s, merged into [min_seg, max_seg].
fn phrases(x: &[f32], sr: u32, s: &Select) -> Vec<(usize, usize)> {
    let (m, h) = speech_mask(x, sr);
    let min_pause = (s.pause_s * sr as f32 / h as f32).round() as usize;
    let mut runs = Vec::new(); let mut i = 0;
    while i < m.len() {
        if m[i] { let st = i; let mut j = i; let mut sil = 0;
            while j < m.len() && (m[j] || sil < min_pause) { if m[j] { sil = 0 } else { sil += 1 } j += 1; }
            let end = j - sil; runs.push((st * h, (end * h + sr as usize / 40).min(x.len()))); i = j;
        } else { i += 1; }
    }
    let (mn, mx) = ((s.min_seg_s * sr as f32) as usize, (s.max_seg_s * sr as f32) as usize);
    let mut segs: Vec<(usize, usize)> = Vec::new();
    for r in runs {
        if let Some(last) = segs.last_mut() { if (last.1 - last.0) < mn || r.1 - last.0 <= mx { last.1 = r.1; continue; } }
        segs.push(r);
    }
    if segs.len() > 1 { let l = segs.len(); if segs[l - 1].1 - segs[l - 1].0 < mn { segs[l - 2].1 = segs[l - 1].1; segs.pop(); } }
    segs
}

fn ecapa_scores(bin: &str, threads: u32, refw: &Path, segs: &[PathBuf]) -> Result<Vec<f64>> {
    let out = refw.with_file_name("ecapa_segments.json");
    let st = Command::new("nice").args(["-n", "10", bin, "--ref"]).arg(refw).args(["--threads", &threads.to_string(), "--no-ceiling", "--raw", "--out"])
        .arg(&out).args(segs).output().context("run galion-ecapa")?;
    if !st.status.success() { bail!("galion-ecapa failed: {}", String::from_utf8_lossy(&st.stderr)); }
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&out)?)?;
    segs.iter().map(|p| v["scores"][p.display().to_string()].as_f64().with_context(|| format!("no score for {}", p.display()))).collect()
}

fn select(x: &[f32], sr: u32, s: &Select, work: &Path) -> Result<(Vec<f32>, serde_json::Value)> {
    let xf = ((s.xfade_ms / 1000.0) * sr as f32) as usize;
    match s.mode.as_str() {
        "full" => Ok((x.to_vec(), json!({"mode": "full"}))),
        "multiclip_windows" => {
            let n = x.len(); let win = ((0.55 * n as f32) as usize).max(4 * sr as usize).min(n);
            let mut st = vec![0, ((0.2 * n as f32) as usize).min(n - win), n - win]; st.dedup();
            let parts: Vec<Vec<f32>> = st.iter().map(|&a| x[a..a + win].to_vec()).collect();
            Ok((dsp::crossfade_concat(&parts, xf), json!({"mode": "multiclip_windows", "starts_s": st.iter().map(|&a| a as f64 / sr as f64).collect::<Vec<_>>(), "win_s": win as f64 / sr as f64})))
        }
        "ecapa_consistent" => {
            std::fs::create_dir_all(work)?;
            let segs = phrases(x, sr, s);
            let refw = work.join("full_cleaned.wav"); write_s16(&refw, x, sr)?;
            let paths: Vec<PathBuf> = (0..segs.len()).map(|i| work.join(format!("seg_{i:02}.wav"))).collect();
            for (p, (a, b)) in paths.iter().zip(&segs) { write_s16(p, &x[*a..*b], sr)?; }
            let t0 = Instant::now();
            let cos = if segs.len() >= 3 { ecapa_scores(&s.ecapa_bin, s.ecapa_threads, &refw, &paths)? } else { vec![1.0; segs.len()] };
            let ecapa_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let mut srt = cos.clone(); srt.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = if srt.is_empty() { 0.0 } else { srt[srt.len() / 2] };
            let mut dev: Vec<f64> = cos.iter().map(|c| (c - med).abs()).collect(); dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mad = if dev.is_empty() { 0.0 } else { 1.4826 * dev[dev.len() / 2] };
            let thr = (med - s.mad_k * mad).min(med - s.abs_floor_drop);
            let mut keep: Vec<bool> = cos.iter().map(|c| *c >= thr).collect();
            // never keep less than min_keep_s of speech: restore best dropped first
            let dur = |k: &Vec<bool>| segs.iter().zip(k).filter(|(_, k)| **k).map(|((a, b), _)| (b - a) as f32 / sr as f32).sum::<f32>();
            let mut order: Vec<usize> = (0..segs.len()).collect(); order.sort_by(|&a, &b| cos[b].partial_cmp(&cos[a]).unwrap());
            for &i in &order { if dur(&keep) >= s.min_keep_s { break; } keep[i] = true; }
            let half_gap = (s.max_gap_s / 2.0 * sr as f32) as usize;
            let mut parts = Vec::new();
            for (i, (a, b)) in segs.iter().enumerate() { if !keep[i] { continue; }
                let lo = if i == 0 { 0 } else { a.saturating_sub(half_gap).max(segs[i - 1].1) };
                let hi = if i + 1 == segs.len() { x.len() } else { (b + half_gap).min(segs[i + 1].0) };
                parts.push(x[lo..hi].to_vec()); }
            let y = dsp::crossfade_concat(&parts, xf);
            let segmeta: Vec<_> = segs.iter().zip(&cos).zip(&keep).map(|(((a, b), c), k)| json!({"start_s": *a as f64 / sr as f64,
                "end_s": *b as f64 / sr as f64, "ecapa_cos_vs_full": c, "kept": k})).collect();
            Ok((y, json!({"mode": "ecapa_consistent", "n_phrases": segs.len(), "n_kept": keep.iter().filter(|k| **k).count(),
                "median_cos": med, "mad_scaled": mad, "drop_threshold": thr, "ecapa_ms": ecapa_ms, "phrases": segmeta})))
        }
        m => bail!("unknown select.mode {m}"),
    }
}

fn prep(input: &Path, output: &Path, meta: &Path, cfg: &Config, cfg_path: Option<&Path>, work: &Path) -> Result<serde_json::Value> {
    let t0 = Instant::now();
    let ci = std::fs::canonicalize(input)?;
    if let Ok(co) = std::fs::canonicalize(output) { if co == ci { bail!("refusing to overwrite input"); } }
    let sr = cfg.sample_rate;
    let (raw, sr0, ch0) = read_mono(input)?;
    let x = resample::resample(&raw, sr0, sr);
    let m_in = measure(&x, sr);
    // 1. DC / rumble HP 60 Hz
    let xd: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    let x: Vec<f32> = dsp::filt(&dsp::highpass(sr as f64, 60.0, 0.707), &xd).iter().map(|&v| v as f32).collect();
    // 2. declick
    let (x, nclick) = if cfg.declick.enabled { dsp::declick(&x, cfg.declick.threshold) } else { (x, 0) };
    // 3. denoise gate (data-driven skip)
    let (x, gmeta) = gate(&x, sr, &cfg.gate);
    // 4. trim
    let (x, ta, tb) = trim(&x, sr, cfg.trim.pad_s);
    // 5. segment selection
    let (x, smeta) = select(&x, sr, &cfg.select, work)?;
    // 6. HF shelf only if band-limited (data-driven)
    let f99_in = m_in["f99_hz"].as_f64().unwrap_or(0.0);
    let hf_apply = cfg.hf_shelf.enabled && f99_in < cfg.hf_shelf.trigger_f99_hz;
    let x = if hf_apply { dsp::hf_shelf_exciter(&x, sr, cfg.hf_shelf.freq_hz, cfg.hf_shelf.gain_db, cfg.hf_shelf.q, cfg.hf_shelf.exciter_mix) } else { x };
    // 7. loudness -> target LUFS, then true-peak ceiling
    let l0 = dsp::integrated_lufs(&x, sr);
    let mut g = 10f64.powf((cfg.loudness.target_lufs - l0) / 20.0);
    let tp = dsp::true_peak(&x) * g; let ceil = 10f64.powf(cfg.loudness.true_peak_dbtp / 20.0);
    let tp_limited = tp > ceil; if tp_limited { g *= ceil / tp; }
    let y: Vec<f32> = x.iter().map(|v| (*v as f64 * g) as f32).collect();
    write_s16(output, &y, sr)?;
    let m_out = measure(&y, sr);
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
    let v = json!({"tool": "refprep 0.1.0", "input": ci.display().to_string(), "input_sr": sr0, "input_channels": ch0,
        "output": output.display().to_string(), "config_path": cfg_path.map(|p| p.display().to_string()), "config": cfg,
        "in": m_in, "out": m_out,
        "steps": {"highpass_hz": 60.0, "declick_fixed": nclick, "gate": gmeta, "trim_s": [ta, tb], "select": smeta,
            "hf_shelf": {"applied": hf_apply, "input_f99_hz": f99_in, "trigger_f99_hz": cfg.hf_shelf.trigger_f99_hz},
            "loudness": {"pre_lufs": l0, "gain_db": 20.0 * g.log10(), "true_peak_limited": tp_limited}},
        "elapsed_ms": elapsed});
    std::fs::write(meta, serde_json::to_string_pretty(&v)?)?;
    Ok(v)
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::DefaultConfig => print!("{}", toml::to_string_pretty(&Config::default())?),
        Cmd::Measure { files } => for f in files { let (x, sr, _) = read_mono(&f)?; println!("{} {}", f.display(), measure(&x, sr)); },
        Cmd::Prep { input, output, meta, config, work_dir } => {
            let cfg: Config = match &config { Some(p) => toml::from_str(&std::fs::read_to_string(p)?)?, None => Config::default() };
            let work = work_dir.unwrap_or_else(|| output.with_extension("work"));
            let v = prep(&input, &output, &meta, &cfg, config.as_deref(), &work)?;
            println!("{}", serde_json::to_string_pretty(&json!({"output": v["output"], "in": v["in"], "out": v["out"], "elapsed_ms": v["elapsed_ms"]}))?);
        }
    }
    Ok(())
}
