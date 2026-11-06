//! Runs the Galion Rust scorers on a set of 12 renders (and on the base recording)
//! and builds line-by-line rows + summary vs the base.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Tools {
    pub root: PathBuf,
}
impl Tools {
    pub fn new(root: &Path) -> Self { Tools { root: root.to_path_buf() } }
    pub fn emotion(&self) -> PathBuf { self.root.join("scorers/emotion/target/release/galion-emotion") }
    pub fn ecapa(&self) -> PathBuf { self.root.join("scorers/ecapa/target/release/galion-ecapa") }
    pub fn wer(&self) -> PathBuf { self.root.join("scorers/quality/bin/galion-wer") }
    pub fn mos(&self) -> PathBuf { self.root.join("scorers/quality/bin/galion-mos") }
    pub fn hygiene(&self) -> PathBuf { self.root.join("scorers/quality/bin/galion-hygiene") }
    pub fn artic(&self) -> PathBuf { self.root.join("scorers/articulation/bin/galion-artic") }
    pub fn speak(&self) -> PathBuf { self.root.join("pipeline/bin/cbx-speak-voice") }
    pub fn cbx(&self) -> PathBuf { self.root.join("pipeline/cbx-v2-timbre/target/release/cbx") }
}

/// Run `nice -n 10 <args>` with logs appended to `log`.
pub fn run_logged(args: &[String], log: &Path) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
    writeln!(f, "$ {}", args.join(" "))?;
    let out = Command::new(&args[0]).args(&args[1..]).output().with_context(|| format!("spawn {}", args[0]))?;
    f.write_all(&out.stdout)?;
    f.write_all(&out.stderr)?;
    if !out.status.success() { bail!("command failed ({}): {}  (see {})", out.status, args.join(" "), log.display()); }
    Ok(())
}

fn s(p: &Path) -> String { p.to_string_lossy().to_string() }
fn niced(bin: PathBuf) -> Vec<String> { vec!["nice".into(), "-n".into(), "10".into(), s(&bin)] }

pub fn load(p: &Path) -> Result<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?).with_context(|| format!("parse {}", p.display()))
}

fn f(v: &Value, path: &[&str]) -> Option<f64> {
    let mut c = v;
    for k in path { c = c.get(*k)?; }
    c.as_f64()
}

/// Score all wavs. Each scorer output is cached in `out_dir` (re-used if present).
pub fn run_scorers(t: &Tools, wavs: &[PathBuf], base: &Path, text_dir: &Path, nmr: &Path, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let log = out_dir.join("scorers.log");
    let w: Vec<String> = wavs.iter().map(|p| s(p)).collect();
    let o = |n: &str| out_dir.join(n);
    let jobs: Vec<(&str, Vec<String>)> = vec![
        ("emo.json", [niced(t.emotion()), vec!["score".into(), "--threads".into(), "2".into(), "--compact".into(), "--ref".into(), s(base), "--out".into(), s(&o("emo.json"))], w.clone()].concat()),
        ("ecapa.json", [niced(t.ecapa()), vec!["--ref".into(), s(base)], w.clone(), vec!["--threads".into(), "2".into(), "--no-ceiling".into(), "--out".into(), s(&o("ecapa.json"))]].concat()),
        ("wer.json", [niced(t.wer()), vec!["--jobs".into(), "2".into(), "--text-dir".into(), s(text_dir), "--out".into(), s(&o("wer.json"))], w.clone()].concat()),
        ("mos.json", [niced(t.mos()), vec!["--nmr".into(), s(nmr), "--threads".into(), "2".into(), "--out".into(), s(&o("mos.json"))], w.clone()].concat()),
        ("hygiene.json", [niced(t.hygiene()), vec!["--out".into(), s(&o("hygiene.json"))], w.clone()].concat()),
        ("artic.json", [niced(t.artic()), vec!["profile".into(), "-j".into(), "2".into(), "--compact".into(), "--out".into(), s(&o("artic.json"))], w.clone()].concat()),
    ];
    for (name, cmd) in jobs {
        if o(name).exists() { continue; }
        eprintln!("  [score] {} ({} files)", name, wavs.len());
        run_logged(&cmd, &log)?;
        if name == "ecapa.json" && !o(name).exists() { bail!("ecapa produced no output"); }
    }
    Ok(())
}

/// Base recording analysis: emotion/AVD/proxies, ASR word count (for w/s), articulation, MOS, hygiene.
pub fn analyse_base(t: &Tools, base: &Path, nmr: &Path, out_dir: &Path) -> Result<Value> {
    std::fs::create_dir_all(out_dir)?;
    let log = out_dir.join("scorers.log");
    let o = |n: &str| out_dir.join(n);
    let b = s(base);
    if !o("emo.json").exists() { run_logged(&[niced(t.emotion()), vec!["score".into(), "--threads".into(), "2".into(), "--compact".into(), "--out".into(), s(&o("emo.json")), b.clone()]].concat(), &log)?; }
    if !o("asr.json").exists() { run_logged(&[niced(t.wer()), vec!["--jobs".into(), "1".into(), "--ref".into(), "x".into(), "--out".into(), s(&o("asr.json")), b.clone()]].concat(), &log)?; }
    if !o("artic.json").exists() { run_logged(&[niced(t.artic()), vec!["profile".into(), "-j".into(), "2".into(), "--compact".into(), "--out".into(), s(&o("artic.json")), b.clone()]].concat(), &log)?; }
    if !o("mos.json").exists() { run_logged(&[niced(t.mos()), vec!["--nmr".into(), s(nmr), "--threads".into(), "2".into(), "--out".into(), s(&o("mos.json")), b.clone()]].concat(), &log)?; }
    if !o("hygiene.json").exists() { run_logged(&[niced(t.hygiene()), vec!["--out".into(), s(&o("hygiene.json")), b.clone()]].concat(), &log)?; }
    let emo = load(&o("emo.json"))?;
    let e = &emo["files"][0];
    let asr = load(&o("asr.json"))?;
    let hyp = asr["files"][0]["hyp"].as_str().unwrap_or("").to_string();
    let hyp_words = hyp.split_whitespace().count();
    let span = f(e, &["proxies", "speech_span_s"]).unwrap_or(f64::NAN);
    let artic = load(&o("artic.json"))?;
    let mos = load(&o("mos.json"))?;
    let hyg = load(&o("hygiene.json"))?;
    let p = &e["proxies"];
    Ok(json!({
        "path": b,
        "duration_s": e["duration_s"],
        "f0_mean_hz": p["f0_mean_hz"], "f0_median_hz": p["f0_median_hz"], "f0_sd_st": p["f0_sd_st"],
        "f0_range_st": p["f0_range_st_p5_p95"], "f0_range_st_minmax": p["f0_range_st_minmax"],
        "energy_dr_db": p["energy_dr_db"], "energy_sd_db": p["energy_sd_db"], "rms_dbfs": p["rms_dbfs"],
        "pause_ratio": p["pause_ratio"], "speech_span_s": span,
        "asr_hyp": hyp, "asr_words": hyp_words,
        "wps": if span > 0.0 { hyp_words as f64 / span } else { f64::NAN },
        "wps_note": "base words/s = Vosk ASR word count of the base / speech span (base has no fixed transcript)",
        "ser": e["ser"]["probs"], "ser_top": e["ser"]["top"], "avd": e["avd"],
        "articulation_rate_syl_s": artic["files"][0]["rate"]["articulation_rate_syl_s"],
        "mos": mos["files"][0]["mos"],
        "clicks": hyg["files"][0]["clicks"], "clipped_runs": hyg["files"][0]["clipped_runs"],
    }))
}

fn by_path<'a>(arr: &'a Value, key: &str) -> BTreeMap<String, &'a Value> {
    let mut m = BTreeMap::new();
    if let Some(a) = arr.as_array() { for x in a { if let Some(p) = x[key].as_str() { m.insert(p.to_string(), x); } } }
    m
}
fn mean(v: &[f64]) -> f64 { let v: Vec<f64> = v.iter().cloned().filter(|x| x.is_finite()).collect(); if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 } }
fn nz(x: f64) -> Value { if x.is_finite() { json!((x * 1e5).round() / 1e5) } else { Value::Null } }

/// Build per-line rows and the variant summary from cached scorer outputs.
pub fn build_variant(name: &str, ids: &[String], texts: &BTreeMap<String, String>, wavs: &[PathBuf], scores: &Path, base: &Value) -> Result<Value> {
    let emo = load(&scores.join("emo.json"))?;
    let ec = load(&scores.join("ecapa.json"))?;
    let wer = load(&scores.join("wer.json"))?;
    let mos = load(&scores.join("mos.json"))?;
    let hyg = load(&scores.join("hygiene.json"))?;
    let art = load(&scores.join("artic.json"))?;
    let em = by_path(&emo["files"], "file");
    let wm = by_path(&wer["files"], "path");
    let mm = by_path(&mos["files"], "path");
    let hm = by_path(&hyg["files"], "path");
    let am = by_path(&art["files"], "file");
    let bf = |k: &str| base[k].as_f64().unwrap_or(f64::NAN);
    let mut rows = Vec::new();
    let mut cols: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for (id, w) in ids.iter().zip(wavs) {
        let p = w.to_string_lossy().to_string();
        let e = em.get(&p).copied().unwrap_or(&Value::Null);
        let pr = &e["proxies"];
        let words = texts.get(id).map(|t| t.split_whitespace().count()).unwrap_or(0) as f64;
        let span = f(pr, &["speech_span_s"]).unwrap_or(f64::NAN);
        let wps = if span > 0.0 { words / span } else { f64::NAN };
        let g = |v: Option<f64>| v.unwrap_or(f64::NAN);
        let r = json!({
            "id": id, "wav": p, "words": words, "duration_s": e["duration_s"],
            "f0_mean_hz": nz(g(f(pr, &["f0_mean_hz"]))), "f0_sd_st": nz(g(f(pr, &["f0_sd_st"]))), "f0_range_st": nz(g(f(pr, &["f0_range_st_p5_p95"]))),
            "energy_dr_db": nz(g(f(pr, &["energy_dr_db"]))), "energy_sd_db": nz(g(f(pr, &["energy_sd_db"]))), "rms_dbfs": nz(g(f(pr, &["rms_dbfs"]))),
            "wps": nz(wps), "pause_ratio": nz(g(f(pr, &["pause_ratio"]))),
            "ser": e["ser"]["probs"], "ser_top": e["ser"]["top"], "avd": e["avd"],
            "emotion_distance": e["vs_ref"]["emotion_distance"], "ser_js_div": e["vs_ref"]["ser_js_div"],
            "ecapa": ec["scores"][&p], "wer": wm.get(&p).map(|x| x["wer_raw"].clone()).unwrap_or(Value::Null),
            "asr_hyp": wm.get(&p).map(|x| x["hyp"].clone()).unwrap_or(Value::Null),
            "mos": mm.get(&p).map(|x| x["mos"].clone()).unwrap_or(Value::Null),
            "clicks": hm.get(&p).map(|x| x["clicks"].clone()).unwrap_or(Value::Null),
            "clipped_runs": hm.get(&p).map(|x| x["clipped_runs"].clone()).unwrap_or(Value::Null),
            "articulation_rate_syl_s": am.get(&p).map(|x| x["rate"]["articulation_rate_syl_s"].clone()).unwrap_or(Value::Null),
            "vs_base": {
                "f0_mean_hz": nz(g(f(pr, &["f0_mean_hz"])) - bf("f0_mean_hz")),
                "f0_sd_st": nz(g(f(pr, &["f0_sd_st"])) - bf("f0_sd_st")),
                "f0_range_st": nz(g(f(pr, &["f0_range_st_p5_p95"])) - bf("f0_range_st")),
                "energy_dr_db": nz(g(f(pr, &["energy_dr_db"])) - bf("energy_dr_db")),
                "wps_ratio": nz(wps / bf("wps")),
                "pause_ratio": nz(g(f(pr, &["pause_ratio"])) - bf("pause_ratio")),
                "arousal": nz(g(f(e, &["avd", "arousal"])) - f(base, &["avd", "arousal"]).unwrap_or(f64::NAN)),
                "valence": nz(g(f(e, &["avd", "valence"])) - f(base, &["avd", "valence"]).unwrap_or(f64::NAN)),
                "dominance": nz(g(f(e, &["avd", "dominance"])) - f(base, &["avd", "dominance"]).unwrap_or(f64::NAN)),
            }
        });
        for k in ["f0_mean_hz", "f0_sd_st", "f0_range_st", "energy_dr_db", "energy_sd_db", "wps", "pause_ratio", "articulation_rate_syl_s"] {
            cols.entry(k).or_default().push(r[k].as_f64().unwrap_or(f64::NAN));
        }
        for k in ["neu", "hap", "ang", "sad"] { cols.entry(match k { "neu" => "p_neu", "hap" => "p_hap", "ang" => "p_ang", _ => "p_sad" }).or_default().push(f(&r, &["ser", k]).unwrap_or(f64::NAN)); }
        for k in ["arousal", "valence", "dominance"] { cols.entry(k).or_default().push(f(&r, &["avd", k]).unwrap_or(f64::NAN)); }
        rows.push(r);
    }
    let mut means = serde_json::Map::new();
    for (k, v) in &cols { means.insert(k.to_string(), nz(mean(v))); }
    let clicks: i64 = rows.iter().map(|r| r["clicks"].as_i64().unwrap_or(0)).sum();
    let clipped: i64 = rows.iter().map(|r| r["clipped_runs"].as_i64().unwrap_or(0)).sum();
    let wer_val = f(&wer, &["summary", "mean_raw"]).or(f(&wer, &["summary", "mean"]));
    Ok(json!({
        "name": name, "n": rows.len(),
        "summary": {
            "ecapa_mean": ec["mean"], "ecapa_sd": ec["sd"], "ecapa_min": ec["min"],
            "wer": wer_val, "wer_rounded": wer["summary"]["mean"], "wer_max_line": wer["summary"]["max"],
            "mos_mean": mos["summary"]["mos"]["mean"],
            "emotion_distance_mean": emo["summary"]["mean_emotion_distance"],
            "ser_js_mean": emo["summary"]["mean_ser_js_div"],
            "label_counts": emo["summary"]["label_counts"], "avd_dist_mean": emo["summary"]["mean_avd_dist"],
            "clicks_total": clicks, "clipped_runs_total": clipped,
            "means": means,
        },
        "lines": rows,
    }))
}
