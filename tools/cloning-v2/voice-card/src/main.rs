// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! galion-voice-card — Stage 0 pre-clone analysis (432 Hz standard).
//!
//!   galion-voice-card analyze --wav REF.wav --voice-id ID --out-dir DIR
//!       [--name "Display name"] [--consent-ref TEXT] [--config stage0.toml] [--repo-root DIR]
//!
//! Any reference clip in, one voice card out: `<ID>.voice.toml` (machine-readable),
//! `<ID>.voice.md` (short, human) and `raw/<ID>.*.json` (each brick's own output).
//! The voice is a pluggable input: there is no per-voice code path.
//!
//! Glue only. Every number comes from an existing brick:
//!   refcheck (Emotion V2 capture gate)   LUFS, true peak, noise floor, DC, RT60, tilt, clipping, format
//!   galion-artic                         pYIN F0, Burg formants F1-F3, vowel space, syllable rate, pauses,
//!                                        fricative / sibilant / burst / VOT, HF tilt
//!   galion-hygiene                       bandwidth f90/f99 and energy above 4/8 kHz, LRA, clicks, clipping
//!   galion-emotion12 (emotion2vec+, ort) 14-emotion baseline posterior, whole clip and 3 s windows
//!   scorers/emotion proxies.rs (in-process, same source file) jitter, shimmer, LTAS tilt,
//!                                        alpha ratio, Hammarberg, YIN aperiodicity
//! A missing brick never fails the run: its fields go to [[unmeasured]] with the reason.
//! Never downloads, never trains, never writes audio anywhere except DIR/raw/<ID>-windows.

#[path = "../../scorers/emotion/src/audio.rs"]
#[allow(dead_code)]
mod audio;
#[path = "../../scorers/emotion/src/proxies.rs"]
#[allow(dead_code)]
mod proxies;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const HELP: &str = "galion-voice-card analyze --wav REF.wav --voice-id ID --out-dir DIR [--name NAME] [--consent-ref TEXT] [--config stage0.toml] [--repo-root DIR]";

// ---------------------------------------------------------------- card builder
struct Card {
    root: Map<String, Value>,
    unmeasured: Vec<(String, String)>,
}

fn r3(v: f64) -> Value {
    json!((v * 1000.0).round() / 1000.0)
}

impl Card {
    fn sec(&mut self, s: &str) -> &mut Map<String, Value> {
        self.root.entry(s.to_string()).or_insert_with(|| json!({})).as_object_mut().unwrap()
    }
    fn num(&mut self, s: &str, k: &str, v: Option<f64>, why: &str) {
        match v.filter(|x| x.is_finite()) {
            Some(x) => {
                self.sec(s).insert(k.into(), r3(x));
            }
            None => self.unmeasured.push((format!("{s}.{k}"), why.to_string())),
        }
    }
    fn int(&mut self, s: &str, k: &str, v: Option<f64>, why: &str) {
        match v.filter(|x| x.is_finite()) {
            Some(x) => {
                self.sec(s).insert(k.into(), json!(x.round() as i64));
            }
            None => self.unmeasured.push((format!("{s}.{k}"), why.to_string())),
        }
    }
    fn put(&mut self, s: &str, k: &str, v: Value) {
        self.sec(s).insert(k.into(), v);
    }
    fn missing(&mut self, field: &str, why: &str) {
        self.unmeasured.push((field.into(), why.into()));
    }
}

fn at(v: &Option<Value>, p: &str) -> Option<f64> {
    v.as_ref()?.pointer(p)?.as_f64()
}

// ---------------------------------------------------------------- bricks
fn brick(cfg: &toml::Value, root: &Path, key: &str, env: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env) {
        return PathBuf::from(p);
    }
    let rel = cfg.get("bricks").and_then(|b| b.get(key)).and_then(|v| v.as_str()).unwrap_or("");
    root.join(rel)
}

/// Run a brick that writes JSON to `out`. Exit 0 or 1 (refcheck FAIL) both count as "measured".
fn run_json(bin: &Path, args: &[String], out: &Path) -> std::result::Result<Value, String> {
    if !bin.is_file() {
        return Err(format!("brick not built: {}", bin.display()));
    }
    let st = Command::new(bin).args(args).output().map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    let code = st.status.code().unwrap_or(-1);
    if code != 0 && code != 1 {
        let err = String::from_utf8_lossy(&st.stderr);
        return Err(format!("{} exit {code}: {}", bin.display(), err.trim().chars().rev().take(300).collect::<String>().chars().rev().collect::<String>()));
    }
    let body = std::fs::read_to_string(out).map_err(|e| format!("{} wrote no JSON: {e}", bin.display()))?;
    serde_json::from_str(&body).map_err(|e| format!("bad JSON from {}: {e}", bin.display()))
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------- small helpers
fn pct(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    let mut x = v.to_vec();
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = p / 100.0 * (x.len() - 1) as f32;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    x[lo] + (x[hi] - x[lo]) * (pos - lo as f32)
}

/// Nearest equal-tempered note with A4 = `a4` Hz (432 Hz standard) and the offset in cents.
fn note_a432(f: f64, a4: f64) -> (String, f64, f64) {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let st = 12.0 * (f / a4).log2();
    let n = st.round() as i64;
    let idx = (n + 9).rem_euclid(12) as usize;
    let octave = 4 + (n + 9).div_euclid(12);
    let cents = 100.0 * (st - n as f64);
    (format!("{}{}", NAMES[idx], octave), cents, st)
}

struct Channels {
    n: u16,
    sr: u32,
    bits: u16,
    corr: Option<f64>,
    level_diff_db: Option<f64>,
    max_abs_diff: Option<f64>,
}

fn channels(path: &Path) -> Result<Channels> {
    let mut r = hound::WavReader::open(path)?;
    let sp = r.spec();
    let ch = sp.channels as usize;
    let raw: Vec<f64> = match sp.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|x| x.map(|v| v as f64)).collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let sc = 1.0 / (1i64 << (sp.bits_per_sample - 1)) as f64;
            r.samples::<i32>().map(|x| x.map(|v| v as f64 * sc)).collect::<Result<_, _>>()?
        }
    };
    let mut out = Channels { n: sp.channels, sr: sp.sample_rate, bits: sp.bits_per_sample, corr: None, level_diff_db: None, max_abs_diff: None };
    if ch >= 2 {
        let (mut sll, mut srr, mut slr, mut md) = (0.0, 0.0, 0.0, 0.0f64);
        for f in raw.chunks(ch) {
            let (l, r) = (f[0], f[1]);
            sll += l * l;
            srr += r * r;
            slr += l * r;
            md = md.max((l - r).abs());
        }
        out.corr = Some(slr / (sll.sqrt() * srr.sqrt()).max(1e-20));
        out.level_diff_db = Some(10.0 * ((sll + 1e-20) / (srr + 1e-20)).log10());
        out.max_abs_diff = Some(md);
    }
    Ok(out)
}

fn write_wav16(path: &Path, x: &[f32], sr: u32) -> Result<()> {
    let spec = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec)?;
    for v in x {
        w.write_sample((v.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

fn band<'a>(v: f64, lo: f64, hi: f64, labels: [&'a str; 3]) -> &'a str {
    if v < lo {
        labels[0]
    } else if v > hi {
        labels[2]
    } else {
        labels[1]
    }
}

fn cf(cfg: &toml::Value, sec: &str, k: &str, d: f64) -> f64 {
    cfg.get(sec).and_then(|t| t.get(k)).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).unwrap_or(d)
}

fn find_root() -> PathBuf {
    if let Ok(p) = std::env::var("GALION_REPO_ROOT") {
        return p.into();
    }
    let mut d = std::env::current_dir().unwrap_or_default();
    loop {
        if d.join("LOCKS.toml").is_file() {
            return d;
        }
        if !d.pop() {
            return std::env::current_dir().unwrap_or_default();
        }
    }
}

// ---------------------------------------------------------------- main
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("analyze") {
        println!("{HELP}");
        return Ok(());
    }
    let mut fl: BTreeMap<String, String> = BTreeMap::new();
    let mut it = args[1..].iter();
    while let Some(k) = it.next() {
        let k = k.strip_prefix("--").with_context(|| format!("unexpected {k}\n{HELP}"))?;
        fl.insert(k.into(), it.next().with_context(|| format!("--{k} needs a value"))?.clone());
    }
    let wav = PathBuf::from(fl.get("wav").context("--wav")?);
    let id = fl.get("voice-id").context("--voice-id")?.clone();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("--voice-id must be [A-Za-z0-9_-]+");
    }
    let out_dir = PathBuf::from(fl.get("out-dir").context("--out-dir")?);
    let root = fl.get("repo-root").map(PathBuf::from).unwrap_or_else(find_root);
    let cfg_path = fl.get("config").map(PathBuf::from).unwrap_or_else(|| root.join("tools/cloning-v2/voice-card/stage0.toml"));
    let cfg: toml::Value = toml::from_str(&std::fs::read_to_string(&cfg_path).with_context(|| format!("config {}", cfg_path.display()))?)?;
    let a4 = cfg.get("tuning_a4_hz").and_then(|v| v.as_float()).unwrap_or(432.0);
    let raw_dir = out_dir.join("raw");
    std::fs::create_dir_all(&raw_dir)?;
    let rawf = |name: &str| raw_dir.join(format!("{id}.{name}.json"));

    let mut c = Card { root: Map::new(), unmeasured: vec![] };
    c.root.insert("schema".into(), json!("galion-voice-card/1"));
    c.root.insert("standard".into(), json!(cfg.get("standard").and_then(|v| v.as_str()).unwrap_or("galion-voice-v2-432")));
    c.root.insert("tool".into(), json!(format!("galion-voice-card {}", env!("CARGO_PKG_VERSION"))));
    c.root.insert("generated".into(), json!(std::env::var("VOICE_CARD_TIMESTAMP").unwrap_or_else(|_| "unset (set VOICE_CARD_TIMESTAMP)".into())));

    // ---- identity
    let bytes = std::fs::read(&wav).with_context(|| format!("read {}", wav.display()))?;
    let sha = format!("{:x}", Sha256::digest(&bytes));
    drop(bytes);
    let chs = channels(&wav)?;
    let a = audio::load_wav(&s(&wav))?;
    let dur = a.samples.len() as f64 / a.sr as f64;
    c.put("identity", "voice_id", json!(id));
    c.put("identity", "display_name", json!(fl.get("name").cloned().unwrap_or_else(|| id.clone())));
    c.put("identity", "source_file", json!(wav.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()));
    c.put("identity", "source_sha256", json!(sha));
    c.num("identity", "duration_s", Some(dur), "");
    c.put("identity", "consent_ref", json!(fl.get("consent-ref").cloned().unwrap_or_else(|| "none given".into())));
    c.put("identity", "consent_checked_here", json!(false));
    c.put("identity", "consent_note", json!("Stage 0 only measures. galion-consent still gates every clone; training needs a signed consent_scope training-data-generation."));

    // ---- bricks
    let spec = std::env::var("GALION_REFCHECK_SPEC").map(PathBuf::from).unwrap_or_else(|_| root.join(cfg["bricks"]["refcheck_spec"].as_str().unwrap_or("")));
    let refcheck = run_json(&brick(&cfg, &root, "refcheck", "GALION_REFCHECK_BIN"),
        &["check".into(), "--wav".into(), s(&wav), "--spec".into(), s(&spec), "--out".into(), s(&rawf("refcheck"))], &rawf("refcheck"));
    let artic = run_json(&brick(&cfg, &root, "artic", "GALION_ARTIC_BIN"),
        &["profile".into(), "--out".into(), s(&rawf("artic")), s(&wav)], &rawf("artic"));
    let hyg = run_json(&brick(&cfg, &root, "hygiene", "GALION_HYGIENE_BIN"),
        &["--out".into(), s(&rawf("hygiene")), s(&wav)], &rawf("hygiene"));
    let why = |r: &std::result::Result<Value, String>| r.as_ref().err().cloned().unwrap_or_default();
    let (w_rc, w_ar, w_hy) = (why(&refcheck), why(&artic), why(&hyg));
    let rc: Option<Value> = refcheck.ok();
    let ar: Option<Value> = artic.ok().and_then(|v| v.pointer("/files/0").cloned());
    let hy: Option<Value> = hyg.ok().and_then(|v| v.pointer("/files/0").cloned());
    let rc_check = |name: &str| -> Option<Value> {
        rc.as_ref()?.get("checks")?.as_array()?.iter().find(|c| c["name"] == name).cloned()
    };
    let rc_val = |name: &str| rc_check(name).and_then(|c| c["value"].as_f64());

    // in-process proxies (same source file as scorers/emotion)
    let x16 = audio::resample(&a.samples, a.sr, proxies::SR as u32);
    let px = proxies::compute(&x16, &a.samples, a.sr as usize);
    let fdb = proxies::frame_db(&x16);
    let maxdb = fdb.iter().cloned().fold(f32::MIN, f32::max);
    let act_thr = (maxdb - 35.0).max(-55.0);
    let (f0y, aper) = proxies::yin_f0(&x16, &fdb, act_thr);
    let hnr: Vec<f32> = f0y.iter().zip(&aper).filter(|(f, _)| **f > 0.0)
        .map(|(_, a)| { let a = a.clamp(1e-4, 0.9999); 10.0 * ((1.0 - a) / a).log10() }).collect();
    let active: Vec<f32> = fdb.iter().cloned().filter(|d| *d > act_thr).collect();

    // ---- pitch
    let f0med = at(&ar, "/f0/pyin/median_hz");
    c.num("pitch", "median_hz", f0med, &w_ar);
    c.num("pitch", "mean_hz", at(&ar, "/f0/pyin/mean_hz"), &w_ar);
    c.num("pitch", "p5_hz", at(&ar, "/f0/pyin/p5_hz"), &w_ar);
    c.num("pitch", "p95_hz", at(&ar, "/f0/pyin/p95_hz"), &w_ar);
    c.num("pitch", "range_p5_p95_st", at(&ar, "/f0/pyin/range_p5_p95_st"), &w_ar);
    c.num("pitch", "variability_sd_st", at(&ar, "/f0/pyin/sd_st"), &w_ar);
    c.num("pitch", "voiced_frac_of_speech", at(&ar, "/f0/pyin/voiced_frac_of_speech"), &w_ar);
    c.num("pitch", "yin_crosscheck_median_hz", at(&ar, "/f0/yin/median_hz"), &w_ar);
    if let Some(f) = f0med {
        let (note, cents, st) = note_a432(f, a4);
        c.num("pitch", "tuning_a4_hz", Some(a4), "");
        c.put("pitch", "nearest_note_a432", json!(note));
        c.num("pitch", "cents_from_note", Some(cents), "");
        c.num("pitch", "semitones_from_432", Some(st), "");
    }
    c.put("pitch", "method", json!("galion-artic pYIN, 10 ms, 65-600 Hz; YIN cross-check"));

    // ---- timbre
    for (k, p) in [("f1_median_hz", "/vowels/f1/median"), ("f2_median_hz", "/vowels/f2/median"), ("f3_median_hz", "/vowels/f3/median")] {
        // per-vowel-segment medians (galion-artic syllable nuclei)
        let v = at(&ar, p);
        c.num("timbre", k, v, &w_ar);
    }
    c.num("timbre", "vowel_space_trimmed_bark2", at(&ar, "/vowel_space/hull_area_trimmed_bark2"), &w_ar);
    c.num("timbre", "f1_range_p5_p95_hz", at(&ar, "/vowel_space/f1_range_p5_p95_hz"), &w_ar);
    c.num("timbre", "f2_range_p5_p95_hz", at(&ar, "/vowel_space/f2_range_p5_p95_hz"), &w_ar);
    c.num("timbre", "vowel_dispersion_bark", at(&ar, "/vowel_space/dispersion_bark"), &w_ar);
    c.num("timbre", "spectral_tilt_db_oct", rc_val("spectral_tilt"), &w_rc);
    c.num("timbre", "ltas_voiced_tilt_db_oct", Some(px.spectral_tilt_db_oct as f64), "");
    c.num("timbre", "alpha_ratio_db", Some(px.alpha_ratio_db as f64), "");
    c.num("timbre", "hammarberg_db", Some(px.hammarberg_db as f64), "");
    c.num("timbre", "hf_tilt_4k_db", at(&ar, "/consonants/hf_tilt_4k_db"), &w_ar);
    c.num("timbre", "bandwidth_f90_hz", at(&hy, "/bandwidth/f90_hz"), &w_hy);
    c.num("timbre", "bandwidth_f99_hz", at(&hy, "/bandwidth/f99_hz"), &w_hy);
    c.num("timbre", "energy_frac_above_4k", at(&hy, "/bandwidth/energy_frac_gt_4k"), &w_hy);
    c.num("timbre", "energy_frac_above_8k", at(&hy, "/bandwidth/energy_frac_gt_8k"), &w_hy);

    // ---- tempo
    c.num("tempo", "speech_rate_syl_s", at(&ar, "/rate/speech_rate_syl_s"), &w_ar);
    c.num("tempo", "articulation_rate_syl_s", at(&ar, "/rate/articulation_rate_syl_s"), &w_ar);
    c.int("tempo", "syllables", at(&ar, "/rate/n_syllables"), &w_ar);
    c.num("tempo", "pauses_per_min", at(&ar, "/pauses/per_min"), &w_ar);
    c.num("tempo", "pause_median_s", at(&ar, "/pauses/dur/median"), &w_ar);
    c.num("tempo", "pause_ratio", at(&ar, "/pauses/pause_ratio"), &w_ar);
    c.int("tempo", "pause_count_ge300ms", at(&ar, "/pauses/n"), &w_ar);
    c.missing("tempo.words_per_s", "needs a transcript; no ASR brick is wired into stage 0 yet (galion-asr-vosk is the candidate)");

    // ---- voicing / breath
    c.num("voicing", "jitter_local_pct", Some(px.jitter_local_pct as f64), "");
    c.num("voicing", "shimmer_local_pct", Some(px.shimmer_local_pct as f64), "");
    c.num("voicing", "shimmer_local_db", Some(px.shimmer_local_db as f64), "");
    c.int("voicing", "glottal_cycles", Some(px.glottal_cycles as f64), "");
    c.num("voicing", "hnr_proxy_db_median", Some(pct(&hnr, 50.0) as f64), "no voiced frames");
    c.put("voicing", "hnr_method", json!("proxy: 10*log10((1-a)/a) from YIN CMND aperiodicity a on voiced frames; not Praat HNR"));
    c.missing("voicing.breath_events", "no breath detector brick yet; band-limited sources also hide breath noise");

    // ---- loudness / dynamics
    c.num("loudness", "integrated_lufs_mono", rc_val("integrated_lufs"), &w_rc);
    c.num("loudness", "integrated_lufs_all_channels", at(&hy, "/loudness/integrated_lufs"), &w_hy);
    c.put("loudness", "lufs_note", json!("mono = refcheck BS.1770-4 on the downmix; all_channels = galion-hygiene summing every channel (dual-mono reads about 3 dB louder)"));
    c.num("loudness", "true_peak_dbtp", rc_val("true_peak"), &w_rc);
    c.num("loudness", "rms_dbfs", at(&hy, "/rms_dbfs"), &w_hy);
    c.num("loudness", "lra_lu", at(&hy, "/loudness/lra_lu"), &w_hy);
    c.num("loudness", "dynamic_range_db_p95_p5", Some(px.energy_dr_db as f64), "");
    c.num("loudness", "energy_sd_db", Some(px.energy_sd_db as f64), "");

    // ---- articulation
    c.num("articulation", "fricative_centroid_hz", at(&ar, "/consonants/fricative/centroid_hz/median"), &w_ar);
    c.num("articulation", "sibilant_centroid_hz", at(&ar, "/consonants/fricative/sibilant_centroid_hz"), &w_ar);
    c.num("articulation", "fricative_sd_hz", at(&ar, "/consonants/fricative/sd_hz/median"), &w_ar);
    c.num("articulation", "fricative_skew", at(&ar, "/consonants/fricative/skewness/median"), &w_ar);
    c.int("articulation", "fricative_frames", at(&ar, "/consonants/fricative/n_frames"), &w_ar);
    c.num("articulation", "burst_centroid_hz", at(&ar, "/consonants/plosive/burst_centroid_hz/median"), &w_ar);
    c.num("articulation", "burst_rel_db", at(&ar, "/consonants/plosive/burst_rel_db/median"), &w_ar);
    c.num("articulation", "bursts_per_s", at(&ar, "/consonants/plosive/per_s"), &w_ar);
    c.num("articulation", "vot_median_ms", at(&ar, "/consonants/plosive/vot_ms/median"), &w_ar);
    c.num("articulation", "long_lag_frac", at(&ar, "/consonants/plosive/long_lag_frac"), &w_ar);

    // ---- environment
    let nf = rc_val("noise_floor");
    c.num("environment", "noise_floor_dbfs", nf, &w_rc);
    let p50a = pct(&active, 50.0) as f64;
    c.num("environment", "snr_db_est", nf.map(|n| p50a - n), &w_rc);
    c.put("environment", "snr_method", json!("median active 25 ms frame level minus refcheck noise floor (quietest 400 ms)"));
    match rc_check("reverb") {
        Some(r) if r["value"].is_f64() => c.num("environment", "rt60_s", r["value"].as_f64(), ""),
        Some(r) => c.missing("environment.rt60_s", &format!("refcheck: {}", r["detail"].as_str().unwrap_or("not measurable"))),
        None => c.missing("environment.rt60_s", &w_rc),
    }
    c.num("environment", "dc_offset_dbfs", rc_val("dc_offset"), &w_rc);
    c.int("environment", "clipped_samples", at(&hy, "/clipped_samples"), &w_hy);
    c.int("environment", "clicks", at(&hy, "/clicks"), &w_hy);
    c.int("environment", "sample_rate_hz", Some(chs.sr as f64), "");
    c.int("environment", "bit_depth", Some(chs.bits as f64), "");
    c.int("environment", "channels", Some(chs.n as f64), "");
    let dual_min = cf(&cfg, "bands", "dual_mono_corr_min", 0.999);
    let dual_mono = chs.corr.map(|x| x >= dual_min);
    if chs.n >= 2 {
        c.num("environment", "lr_correlation", chs.corr, "");
        c.num("environment", "lr_level_diff_db", chs.level_diff_db, "");
        c.num("environment", "lr_max_abs_diff", chs.max_abs_diff, "");
        c.put("environment", "dual_mono", json!(dual_mono.unwrap_or(false)));
    }
    if let Some(r) = &rc {
        c.put("environment", "refcheck_verdict", r["verdict"].clone());
        let failed: Vec<Value> = r["checks"].as_array().map(|a| a.iter().filter(|c| c["pass"] == false).map(|c| c["name"].clone()).collect()).unwrap_or_default();
        c.put("environment", "refcheck_failed", json!(failed));
    }

    // ---- emotion (emotion2vec+ via galion-emotion12)
    let e12 = brick(&cfg, &root, "emotion12", "GALION_EMOTION12_BIN");
    let emo_file = std::env::var("EMOTION_V2_GATE_EMOTIONS").map(PathBuf::from).unwrap_or_else(|_| root.join(cfg["bricks"]["emotion12_emotions"].as_str().unwrap_or("")));
    let win_s = cf(&cfg, "emotion_windows", "window_s", 3.0);
    let hop_s = cf(&cfg, "emotion_windows", "hop_s", 1.5);
    let wdir = raw_dir.join(format!("{id}-windows"));
    std::fs::create_dir_all(&wdir)?;
    let mut files = vec![s(&wav)];
    let (wn, hn) = ((win_s * 16000.0) as usize, (hop_s * 16000.0) as usize);
    let mut starts = vec![];
    let mut st0 = 0usize;
    while st0 + wn <= x16.len() {
        let p = wdir.join(format!("w{:03}.wav", starts.len()));
        write_wav16(&p, &x16[st0..st0 + wn], 16000)?;
        files.push(s(&p));
        starts.push(st0 as f64 / 16000.0);
        st0 += hn;
    }
    let mut eargs = vec!["score".to_string(), "--device".into(), std::env::var("EMOTION_V2_DEVICE").unwrap_or_else(|_| "cpu".into()),
        "--emotions".into(), s(&emo_file), "--out".into(), s(&rawf("emotion12"))];
    eargs.extend(files.iter().cloned());
    let emo = if std::env::var("ORT_DYLIB_PATH").is_err() { Err("ORT_DYLIB_PATH unset (onnxruntime for galion-emotion12)".to_string()) } else { run_json(&e12, &eargs, &rawf("emotion12")) };
    match emo {
        Ok(e) => {
            let labels: Vec<String> = e["labels"].as_array().unwrap().iter().map(|v| v.as_str().unwrap_or("").into()).collect();
            let mlabels: Vec<String> = e["model_labels"].as_array().unwrap().iter().map(|v| v.as_str().unwrap_or("").into()).collect();
            let rows = e["files"].as_array().cloned().unwrap_or_default();
            c.put("emotion", "scorer", json!(format!("{} ({}) via galion-emotion12", e["model"].as_str().unwrap_or(""), e["device"].as_str().unwrap_or(""))));
            c.int("emotion", "emotions", Some(labels.len() as f64), "");
            if let Some(w) = rows.first() {
                c.put("emotion", "whole_clip_top", w["top"].clone());
                let mut post = Map::new();
                for (l, p) in labels.iter().zip(w["posterior"].as_array().unwrap()) { post.insert(l.clone(), r3(p.as_f64().unwrap_or(0.0))); }
                c.put("emotion", "whole_clip_posterior", Value::Object(post));
                let mut mp = Map::new();
                for (l, p) in mlabels.iter().zip(w["model_probs"].as_array().unwrap()) { mp.insert(l.clone(), json!((p.as_f64().unwrap_or(0.0) * 1e4).round() / 1e4)); }
                c.put("emotion", "whole_clip_model_probs", Value::Object(mp));
            }
            let wins = &rows[1.min(rows.len())..];
            if !wins.is_empty() {
                let mut tops: BTreeMap<String, usize> = BTreeMap::new();
                let mut mean = vec![0.0; labels.len()];
                let mut timeline = vec![];
                for (i, w) in wins.iter().enumerate() {
                    let t = w["top"].as_str().unwrap_or("?").to_string();
                    *tops.entry(t.clone()).or_default() += 1;
                    let p = w["posterior"].as_array().unwrap();
                    for (k, v) in p.iter().enumerate() { mean[k] += v.as_f64().unwrap_or(0.0) / wins.len() as f64; }
                    // model-side confidence (before the even split of shared classes)
                    let mx = w["model_probs"].as_array().map(|a| a.iter().filter_map(|v| v.as_f64()).fold(0.0, f64::max)).unwrap_or(f64::NAN);
                    timeline.push(json!({"start_s": r3(starts[i]), "top": t, "model_p": r3(mx)}));
                }
                let mut mm = Map::new();
                for (l, v) in labels.iter().zip(mean) { mm.insert(l.clone(), r3(v)); }
                let mut wt = Map::new();
                c.num("emotion", "window_s", Some(win_s), "");
                c.num("emotion", "hop_s", Some(hop_s), "");
                c.int("emotion", "windows", Some(wins.len() as f64), "");
                for (k, v) in tops { wt.insert(k, json!(v)); }
                c.put("emotion", "window_top_counts", Value::Object(wt));
                c.put("emotion", "window_mean_posterior", Value::Object(mm));
                c.put("emotion", "timeline", Value::Array(timeline));
            }
            c.put("emotion", "posterior_note", json!("14-emotion posterior = emotion2vec+ 9-class output projected by the gate-emotions file; a class shared by several emotions is split evenly (config, not measured), so e.g. neutral/calm are equal by construction."));
        }
        Err(why) => {
            for k in ["whole_clip_posterior", "window_mean_posterior", "whole_clip_top"] { c.missing(&format!("emotion.{k}"), &format!("pending scorer: {why}")); }
        }
    }
    c.missing("emotion.gate_scores", "pending: the per-emotion 0.83 gate score compares a render with that emotion's reference clip of the same voice; a single reference only gives the baseline posterior above");

    // ---- delivery (plain words from config bands; numbers above stay raw)
    let mut lines: Vec<String> = vec![];
    let mut limits: Vec<String> = vec![];
    if let (Some(f), Some(sd), Some(rg)) = (f0med, at(&ar, "/f0/pyin/sd_st"), at(&ar, "/f0/pyin/range_p5_p95_st")) {
        let (note, cents, _) = note_a432(f, a4);
        let tone = band(sd, cf(&cfg, "bands", "pitch_flat_below_st", 2.0), cf(&cfg, "bands", "pitch_lively_above_st", 3.5), ["flat", "moderate", "lively"]);
        lines.push(format!("Pitch centres on {f:.0} Hz ({note} {cents:+.0} cents at A4={a4:.0} Hz) with {tone} intonation: {sd:.1} st SD, {rg:.1} st p5-p95 range."));
    }
    if let (Some(ar_), Some(pr), Some(pm)) = (at(&ar, "/rate/articulation_rate_syl_s"), at(&ar, "/pauses/pause_ratio"), at(&ar, "/pauses/per_min")) {
        let pace = band(ar_, cf(&cfg, "bands", "articulation_slow_below", 4.0), cf(&cfg, "bands", "articulation_fast_above", 5.5), ["slow", "moderate", "fast"]);
        let pz = if pr >= cf(&cfg, "bands", "pause_ratio_high", 0.25) { "pause-rich" } else { "fluent" };
        lines.push(format!("Pace is {pace} ({ar_:.2} syllables/s while speaking) and {pz}: {pm:.1} pauses/min, {:.0}% of the time silent.", pr * 100.0));
    }
    if let Some(lra) = at(&hy, "/loudness/lra_lu") {
        let d = if lra < cf(&cfg, "bands", "lra_compressed_below_lu", 3.0) { "steady, even loudness" } else { "varied loudness" };
        lines.push(format!("Dynamics: {d} (LRA {lra:.1} LU, frame range {:.1} dB).", px.energy_dr_db));
    }
    if let Some(t) = c.root.get("emotion").and_then(|e| e.get("whole_clip_top")).and_then(|v| v.as_str()) {
        let wc = c.root["emotion"].get("window_top_counts").map(|v| v.to_string()).unwrap_or_default();
        lines.push(format!("Emotion baseline (emotion2vec+): {t} on the whole clip; 3 s windows {wc}."));
    }
    if let Some(f99) = at(&hy, "/bandwidth/f99_hz") {
        if f99 < cf(&cfg, "bands", "hf_limited_f99_below_hz", 4000.0) {
            let s = format!("Band-limited: 99% of energy below {f99:.0} Hz ({:.2}% above 4 kHz), so breath, sibilance and air are mostly missing.", at(&hy, "/bandwidth/energy_frac_gt_4k").unwrap_or(f64::NAN) * 100.0);
            lines.push(s.clone());
            limits.push(s);
        }
    }
    if let Some(n) = nf {
        let tgt = cf(&cfg, "bands", "noise_floor_target_dbfs", -60.0);
        lines.push(format!("Room: noise floor {n:.1} dBFS, SNR about {:.0} dB{}.", p50a - n,
            match c.root.get("environment").and_then(|e| e.get("rt60_s")) { Some(v) => format!(", RT60 {v} s"), None => ", RT60 not measurable".into() }));
        if n > tgt { limits.push(format!("Noise floor {n:.1} dBFS is above the {tgt:.0} dBFS capture target.")); }
    }
    if dual_mono == Some(true) { limits.push(format!("Stereo file is dual-mono (L/R correlation {:.4}); no stereo information, downmix to mono loses nothing.", chs.corr.unwrap())); }
    if let Some(r) = &rc {
        for chk in r["checks"].as_array().into_iter().flatten().filter(|c| c["pass"] == false) {
            limits.push(format!("refcheck {}: {} (limit {}{})", chk["name"].as_str().unwrap_or(""),
                chk["value"].as_f64().map(|v| format!("{v:.2}")).unwrap_or("n/a".into()), chk["limit"].as_str().unwrap_or(""),
                chk["unit"].as_str().map(|u| format!(" {u}")).unwrap_or_default()));
        }
    }
    c.put("delivery", "summary", json!(lines.join(" ")));
    c.put("quality_limits", "items", json!(limits));

    // ---- recommended prep (what refprep's shared config would do with these measurements)
    let rp_path = std::env::var("GALION_REFPREP_CONFIG").map(PathBuf::from).unwrap_or_else(|_| root.join(cfg["bricks"]["refprep_config"].as_str().unwrap_or("")));
    let mut prep: Vec<String> = vec![];
    match std::fs::read_to_string(&rp_path).ok().and_then(|t| toml::from_str::<toml::Value>(&t).ok()) {
        Some(rp) => {
            c.put("prep", "refprep_config", json!(rp_path.strip_prefix(&root).map(s).unwrap_or(s(&rp_path))));
            let trig = cf(&rp, "hf_shelf", "trigger_f99_hz", 3500.0);
            if let Some(f99) = at(&hy, "/bandwidth/f99_hz") {
                let on = f99 < trig;
                c.put("prep", "hf_shelf_triggers", json!(on));
                if on {
                    prep.push(format!("refprep HF shelf ON (f99 {f99:.0} Hz < trigger {trig:.0} Hz): +{:.1} dB at {:.0} Hz, exciter mix {:.2}. Restored highs are estimated, not recovered.",
                        cf(&rp, "hf_shelf", "gain_db", 0.0), cf(&rp, "hf_shelf", "freq_hz", 0.0), cf(&rp, "hf_shelf", "exciter_mix", 0.0)));
                } else {
                    prep.push(format!("refprep HF shelf off (f99 {f99:.0} Hz >= trigger {trig:.0} Hz): no HF restoration needed."));
                }
            }
            let minsnr = cf(&rp, "gate", "min_snr_db", 45.0);
            let p90p10 = (pct(&fdb, 90.0) - pct(&fdb, 10.0)) as f64;
            c.num("prep", "frame_snr_p90_p10_db", Some(p90p10), "");
            prep.push(format!("refprep denoise gate {} (frame p90/p10 {p90p10:.1} dB vs skip-at {minsnr:.0} dB).", if p90p10 >= minsnr { "skipped, already clean" } else { "ON" }));
            prep.push(format!("Loudness to {:.0} LUFS, true peak <= {:.1} dBTP; resample to {} Hz; select mode {}.",
                cf(&rp, "loudness", "target_lufs", -23.0), cf(&rp, "loudness", "true_peak_dbtp", -1.0),
                rp.get("sample_rate").and_then(|v| v.as_integer()).unwrap_or(0), rp.get("select").and_then(|t| t.get("mode")).and_then(|v| v.as_str()).unwrap_or("?")));
        }
        None => c.missing("prep.refprep_config", &format!("refprep config not found at {}", rp_path.display())),
    }
    if dual_mono == Some(true) { prep.push("Downmix to mono first (dual-mono).".into()); }
    if let Some(r) = &rc {
        let failed = |n: &str| r["checks"].as_array().map(|a| a.iter().any(|c| c["name"] == n && c["pass"] == false)).unwrap_or(false);
        if failed("duration") { prep.push("Cut into 3-12 s takes at word gaps for Emotion V2 references.".into()); }
        if failed("sample_rate") || failed("bit_depth") { prep.push("Capture spec (48 kHz / 24-bit mono) is not met by this file; a re-record is the real fix, upsampling adds nothing.".into()); }
    }
    if limits.iter().any(|l| l.starts_with("Band-limited")) { prep.push("Best fix: re-record with full bandwidth and a phonetically balanced multi-clip set (capture kit).".into()); }
    c.put("prep", "steps", json!(prep));

    // ---- clone targets
    let t = |k: &str, d: f64| cf(&cfg, "targets", k, d);
    if let Some(f) = f0med { let tol = t("f0_tol_st", 0.5); c.put("clone_targets", "f0_median_hz", json!({"target": r3(f), "min": r3(f * 2f64.powf(-tol / 12.0)), "max": r3(f * 2f64.powf(tol / 12.0))})); }
    for (k, p) in [("articulation_rate_syl_s", "/rate/articulation_rate_syl_s"), ("pause_ratio", "/pauses/pause_ratio")] {
        if let Some(v) = at(&ar, p) { let tol = t("rate_tol_frac", 0.10); c.put("clone_targets", k, json!({"target": r3(v), "min": r3(v * (1.0 - tol)), "max": r3(v * (1.0 + tol))})); }
    }
    for k in ["f1_median_hz", "f2_median_hz", "f3_median_hz"] {
        if let Some(v) = c.root.get("timbre").and_then(|x| x.get(k)).and_then(|v| v.as_f64()) { let tol = t("formant_tol_frac", 0.08); c.put("clone_targets", k, json!({"target": r3(v), "min": r3(v * (1.0 - tol)), "max": r3(v * (1.0 + tol))})); }
    }
    if let Some(v) = at(&ar, "/consonants/fricative/centroid_hz/median") { let tol = t("fricative_tol_hz", 500.0); c.put("clone_targets", "fricative_centroid_hz", json!({"target": r3(v), "min": r3(v - tol), "max": r3(v + tol)})); }
    if let Some(v) = rc_val("integrated_lufs") { let tol = t("lufs_tol", 1.0); c.put("clone_targets", "integrated_lufs_mono", json!({"target": r3(v), "min": r3(v - tol), "max": r3(v + tol)})); }
    if let Some(top) = c.root.get("emotion").and_then(|e| e.get("whole_clip_top")).cloned() { c.put("clone_targets", "emotion_baseline_top", top); }
    c.put("clone_targets", "gate", json!({"wer_max": cf(&cfg, "gate", "wer_max", 0.018), "ecapa_min": cf(&cfg, "gate", "ecapa_min", 0.7057), "emotion_min_per_emotion": cf(&cfg, "gate", "emotion_min", 0.83)}));

    // ---- unmeasured + bricks used
    let un: Vec<Value> = c.unmeasured.iter().map(|(f, w)| json!({"field": f, "reason": w})).collect();
    c.root.insert("bricks".into(), json!({
        "refcheck": if w_rc.is_empty() { "ok".to_string() } else { w_rc.clone() },
        "galion_artic": if w_ar.is_empty() { "ok".to_string() } else { w_ar.clone() },
        "galion_hygiene": if w_hy.is_empty() { "ok".to_string() } else { w_hy.clone() },
        "galion_emotion12": if c.root.contains_key("emotion") && c.root["emotion"].get("scorer").is_some() { "ok".to_string() } else { "pending".into() },
        "proxies_rs": "ok (in-process, tools/cloning-v2/scorers/emotion/src/proxies.rs)",
    }));
    c.root.insert("unmeasured".into(), Value::Array(un));

    let card = Value::Object(c.root);
    let toml_s = toml::to_string(&card).context("toml")?;
    let head = "# Galion voice card (stage 0, pre-clone). Generated by galion-voice-card; do not hand-edit numbers.\n# Universal schema: same fields for every voice. Unmeasured fields are listed in [[unmeasured]] with the reason.\n";
    std::fs::write(out_dir.join(format!("{id}.voice.toml")), format!("{head}{toml_s}"))?;
    std::fs::write(out_dir.join(format!("{id}.voice.md")), markdown(&card))?;
    println!("{}", out_dir.join(format!("{id}.voice.toml")).display());
    Ok(())
}

fn g(card: &Value, p: &str) -> String {
    match card.pointer(p) {
        Some(Value::Number(n)) => match n.as_f64() { Some(f) if f.fract() == 0.0 && f.abs() < 1e12 => format!("{}", f as i64), _ => n.to_string() },
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "unmeasured".into(),
    }
}

fn markdown(c: &Value) -> String {
    let mut m = String::new();
    let name = g(c, "/identity/display_name");
    m += &format!("# Voice card: {name}\n\n");
    m += &format!("Stage 0 pre-clone analysis of `{}` ({} s, {} Hz, {}-bit, {} ch). Standard {}.\n\n",
        g(c, "/identity/source_file"), g(c, "/identity/duration_s"), g(c, "/environment/sample_rate_hz"), g(c, "/environment/bit_depth"), g(c, "/environment/channels"), g(c, "/standard"));
    m += &format!("{}\n\n", g(c, "/delivery/summary"));
    m += "| Area | Key numbers |\n|---|---|\n";
    m += &format!("| Pitch | median {} Hz ({} {} cents, A4=432), p5-p95 {}-{} Hz ({} st), SD {} st |\n", g(c, "/pitch/median_hz"), g(c, "/pitch/nearest_note_a432"), g(c, "/pitch/cents_from_note"), g(c, "/pitch/p5_hz"), g(c, "/pitch/p95_hz"), g(c, "/pitch/range_p5_p95_st"), g(c, "/pitch/variability_sd_st"));
    m += &format!("| Timbre | F1/F2/F3 {}/{}/{} Hz, tilt {} dB/oct, f99 {} Hz, {:.2}% of energy above 4 kHz |\n", g(c, "/timbre/f1_median_hz"), g(c, "/timbre/f2_median_hz"), g(c, "/timbre/f3_median_hz"), g(c, "/timbre/spectral_tilt_db_oct"), g(c, "/timbre/bandwidth_f99_hz"), c.pointer("/timbre/energy_frac_above_4k").and_then(|v| v.as_f64()).map(|v| v * 100.0).unwrap_or(f64::NAN));
    m += &format!("| Tempo | {} syl/s articulation, {} syl/s overall, {} pauses/min, median pause {} s, pause ratio {} |\n", g(c, "/tempo/articulation_rate_syl_s"), g(c, "/tempo/speech_rate_syl_s"), g(c, "/tempo/pauses_per_min"), g(c, "/tempo/pause_median_s"), g(c, "/tempo/pause_ratio"));
    m += &format!("| Voicing | jitter {}%, shimmer {}%, HNR proxy {} dB |\n", g(c, "/voicing/jitter_local_pct"), g(c, "/voicing/shimmer_local_pct"), g(c, "/voicing/hnr_proxy_db_median"));
    m += &format!("| Loudness | {} LUFS (mono), true peak {} dBTP, LRA {} LU |\n", g(c, "/loudness/integrated_lufs_mono"), g(c, "/loudness/true_peak_dbtp"), g(c, "/loudness/lra_lu"));
    m += &format!("| Articulation | fricative centroid {} Hz, sibilant {} Hz, burst {} Hz, VOT {} ms |\n", g(c, "/articulation/fricative_centroid_hz"), g(c, "/articulation/sibilant_centroid_hz"), g(c, "/articulation/burst_centroid_hz"), g(c, "/articulation/vot_median_ms"));
    m += &format!("| Emotion | whole clip: {}; windows: {} |\n", g(c, "/emotion/whole_clip_top"), g(c, "/emotion/window_top_counts"));
    m += &format!("| Environment | noise floor {} dBFS, SNR ~{} dB, RT60 {}, dual-mono {}, clipped {}, refcheck {} |\n\n", g(c, "/environment/noise_floor_dbfs"), g(c, "/environment/snr_db_est"), g(c, "/environment/rt60_s"), if c.pointer("/environment/dual_mono").is_some() { g(c, "/environment/dual_mono") } else { "n/a (mono file)".into() }, g(c, "/environment/clipped_samples"), g(c, "/environment/refcheck_verdict"));
    let list = |p: &str| c.pointer(p).and_then(|v| v.as_array()).map(|a| a.iter().map(|x| format!("- {}\n", x.as_str().unwrap_or(""))).collect::<String>()).unwrap_or_default();
    m += "## Limits\n"; m += &list("/quality_limits/items"); m += "\n";
    m += "## Prep before cloning\n"; m += &list("/prep/steps"); m += "\n";
    m += "## Not measured\n";
    for u in c.pointer("/unmeasured").and_then(|v| v.as_array()).into_iter().flatten() {
        m += &format!("- `{}`: {}\n", u["field"].as_str().unwrap_or(""), u["reason"].as_str().unwrap_or(""));
    }
    m += &format!("\nConsent: {} (not checked by stage 0; galion-consent gates cloning).\n", g(c, "/identity/consent_ref"));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a432_reference_is_a4_zero_cents() {
        let (n, cents, st) = note_a432(432.0, 432.0);
        assert_eq!(n, "A4");
        assert!(cents.abs() < 1e-9 && st.abs() < 1e-9);
    }

    #[test]
    fn note_names_and_octaves() {
        assert_eq!(note_a432(216.0, 432.0).0, "A3");
        assert_eq!(note_a432(432.0 * 2f64.powf(3.0 / 12.0), 432.0).0, "C5");
        let (n, cents, _) = note_a432(176.236, 432.0);
        assert_eq!(n, "F3");
        assert!((cents - 47.77).abs() < 0.05, "{cents}");
    }

    #[test]
    fn unmeasured_is_recorded_not_invented() {
        let mut c = Card { root: Map::new(), unmeasured: vec![] };
        c.num("pitch", "median_hz", None, "brick not built");
        c.num("pitch", "mean_hz", Some(f64::NAN), "nan");
        c.num("pitch", "p5_hz", Some(150.0), "");
        assert_eq!(c.unmeasured.len(), 2);
        assert!(c.root["pitch"].get("median_hz").is_none());
        assert_eq!(c.root["pitch"]["p5_hz"], json!(150.0));
    }

    #[test]
    fn dual_mono_detected() {
        let dir = std::env::temp_dir().join(format!("vc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("dm.wav");
        let spec = hound::WavSpec { channels: 2, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(&p, spec).unwrap();
        for i in 0..1600 {
            let v = ((i as f32 * 0.07).sin() * 8000.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        let ch = channels(&p).unwrap();
        assert!(ch.corr.unwrap() > 0.9999);
        assert!(ch.level_diff_db.unwrap().abs() < 1e-6);
    }
}
