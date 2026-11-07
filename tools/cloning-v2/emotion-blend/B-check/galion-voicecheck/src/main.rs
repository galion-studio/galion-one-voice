//! galion-voicecheck — automatic per-voice check for the Cloning V2 system.
mod analysis;
mod dsp;
mod emo_backend;
mod emotion_match;
mod gate;
mod report;
mod stages;

use analysis::{build_variant, load, run_logged, run_scorers, Tools};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LOCK: &str = "/tmp/cbx-render.lock";

pub fn root() -> PathBuf {
    std::env::var("GALION_CLONING_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/workspace/swarm/voice-v2-sprint/cloning-v2"))
}
pub fn bcheck() -> PathBuf { root().join("emotion-blend/B-check") }
pub fn voices_dir() -> PathBuf {
    std::env::var("CBX_VOICES_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(format!("{}/.cache/huggingface/cbx/voices", std::env::var("HOME").unwrap_or("/home/box".into()))))
}
fn clip_store() -> PathBuf { root().parent().unwrap().join("clip-store-own") }

const USAGE: &str = r#"galion-voicecheck — Cloning V2 automatic per-voice check

USAGE
  galion-voicecheck <voice> --base <recording.wav> [options]          (= `check`)
  galion-voicecheck check <voice> --base <recording.wav> [options]
  galion-voicecheck add --name <own-x> --voice-wav <rec.wav> [check options]   hook: cbx voice add + check
  galion-voicecheck watch [--interval 60] [--once] [--initial-scan] [--dry-run]
  galion-voicecheck layer --in-dir R --out-dir D --base B [--analysis A.json]  builtin prosody stage (b)
  galion-voicecheck expressive-ref --base B --out-dir D [--target-s 10]       builtin reference stage (a)
  galion-voicecheck stages            show which fix providers would be used

CHECK OPTIONS
  --base PATH            base (owner) recording; default from config/voices/<voice>.json or clip-store-own/<p>/reference.wav
  --dry-run              never render, never write the voices dir; needs --renders for pre-fix lines
  --renders DIR          use existing pre-fix renders DIR/tNN.wav instead of rendering
  --fix-renders a=DIR    use existing renders for fix a (or c=DIR) instead of rendering
  --fixes a,b,c          which fix attempts to run (default a,b,c; max 3)
  --threshold X          ECAPA bar (default 0.7057 or config `ecapa_threshold`)
  --wer-max X            WER bar (default 0.018 or config `wer_max`)
  --require-beat-prefix  ECAPA bar = max(threshold, voice's own pre-fix ECAPA)
  --persona P            persona slug for the winner name own-<P>-v2-blend-<fix>
  --mos-nmr PATH         non-matching reference for galion-mos
  --texts TSV            test set (default emotion-fix/test12.tsv)
  --no-save              run gate but do not save the winner
  --tag NAME             run directory tag (default timestamp)
  --emotion-window S-E   force a joyful window START-END seconds on the base (also used as a scored span)
  --emo-min X            emotion_match gate (default 0.83 or config emo_min); skipped if no moments configured
  --emotion-backend NAME classifier backend (default galion-emotion; pluggable)
  --emotion-presence X   min category strength to count as a moment (default 0.28)
  --skip-emotion         disable emotion-match gate even if config has moments
"#;

pub struct Args { pub pos: Vec<String>, pub kv: BTreeMap<String, String>, pub flags: Vec<String> }
const FLAGS: &[&str] = &["--dry-run", "--no-save", "--require-beat-prefix", "--once", "--initial-scan", "--skip-emotion", "--help", "-h"];
pub fn parse(argv: &[String]) -> Args {
    let mut a = Args { pos: vec![], kv: BTreeMap::new(), flags: vec![] };
    let mut i = 0;
    while i < argv.len() {
        let x = &argv[i];
        if FLAGS.contains(&x.as_str()) { a.flags.push(x.clone()); }
        else if let Some((k, v)) = x.split_once('=').filter(|_| x.starts_with("--")) { a.kv.insert(k.to_string(), v.to_string()); }
        else if x.starts_with("--") && i + 1 < argv.len() { a.kv.insert(x.clone(), argv[i + 1].clone()); i += 1; }
        else { a.pos.push(x.clone()); }
        i += 1;
    }
    a
}
impl Args {
    fn flag(&self, f: &str) -> bool { self.flags.iter().any(|x| x == f) }
    fn get(&self, k: &str) -> Option<&String> { self.kv.get(k) }
}

fn now(fmt: &str) -> String {
    Command::new("date").arg(format!("+{}", fmt)).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

pub fn read_texts(tsv: &Path) -> Result<(Vec<String>, BTreeMap<String, String>)> {
    let mut ids = vec![]; let mut m = BTreeMap::new();
    for l in std::fs::read_to_string(tsv)?.lines() {
        if l.trim().is_empty() || l.starts_with('#') { continue; }
        if let Some((id, t)) = l.split_once('\t') { ids.push(id.to_string()); m.insert(id.to_string(), t.to_string()); }
    }
    Ok((ids, m))
}

fn voice_config(voice: &str) -> Value {
    let p = bcheck().join("config/voices").join(format!("{}.json", voice));
    load(&p).unwrap_or(json!({}))
}
fn voice_defaults(voice: &str) -> Value { load(&root().join("pipeline/voice-defaults").join(format!("{}.json", voice))).unwrap_or(json!({})) }

pub fn resolve_base(voice: &str, cfg: &Value) -> Option<PathBuf> {
    if let Some(b) = cfg["base"].as_str() { return Some(PathBuf::from(b)); }
    let p = clip_store().join(gate::persona_of(voice)).join("reference.wav");
    if p.exists() { Some(p) } else { None }
}

fn mem_avail_mb() -> u64 {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|s| s.lines().find(|l| l.starts_with("MemAvailable")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())).unwrap_or(0) / 1024
}

/// Render the test set, one line at a time, each under flock(/tmp/cbx-render.lock).
/// cbx-speak-voice applies nice -n 10, --intra-threads 2 and per-voice defaults (timbre-match …).
fn render_set(t: &Tools, voice: &str, cand: Option<&str>, ids: &[String], texts: &BTreeMap<String, String>, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let log = out.join("render.log");
    for id in ids {
        let o = out.join(format!("{}.wav", id));
        if o.exists() { continue; }
        while mem_avail_mb() < 3800 { eprintln!("  [render] waiting for memory ({} MB avail)", mem_avail_mb()); std::thread::sleep(std::time::Duration::from_secs(15)); }
        let mut cmd: Vec<String> = vec!["flock".into(), LOCK.into(), "env".into(), "CBX_INTRA_THREADS=2".into(), t.speak().to_string_lossy().into()];
        match cand {
            None => { cmd.extend(["--voice".into(), voice.into()]); }
            Some(c) => {
                // candidate profile (new conditioning); carry over the SOURCE voice's own defaults explicitly
                cmd.extend(["--voice".into(), c.into()]);
                let d = voice_defaults(voice);
                if let Some(tm) = d["timbre_match"].as_f64().filter(|v| *v > 0.0) {
                    cmd.extend(["--timbre-match".into(), tm.to_string()]);
                    if let Some(tr) = d["timbre_ref"].as_str() { cmd.extend(["--timbre-ref".into(), tr.into()]); }
                }
                cmd.extend(["--dtype".into(), d["dtype"].as_str().unwrap_or("fp32").into()]);
            }
        }
        cmd.extend(["--text".into(), texts[id].clone(), "--out-wav".into(), o.to_string_lossy().into(), "--temperature".into(), "0".into(), "--seed".into(), "0".into()]);
        eprintln!("  [render] {} {}", voice, id);
        run_logged(&cmd, &log)?;
    }
    Ok(())
}

fn wav_list(dir: &Path, ids: &[String]) -> Result<Vec<PathBuf>> {
    let mut v = vec![];
    for id in ids {
        let p = dir.join(format!("{}.wav", id));
        if !p.exists() { bail!("missing render {}", p.display()); }
        v.push(std::fs::canonicalize(&p)?);
    }
    Ok(v)
}

struct Ctx<'a> { t: &'a Tools, ids: &'a [String], texts: &'a BTreeMap<String, String>, base: &'a Path, base_info: &'a Value, text_dir: &'a Path, nmr: &'a Path }

fn score_variant(c: &Ctx, name: &str, wav_dir: &Path, vdir: &Path) -> Result<Value> {
    let wavs = wav_list(wav_dir, c.ids)?;
    eprintln!("[{}] scoring {}", name, wav_dir.display());
    run_scorers(c.t, &wavs, c.base, c.text_dir, c.nmr, &vdir.join("scores"))?;
    let mut v = build_variant(name, c.ids, c.texts, &wavs, &vdir.join("scores"), c.base_info)?;
    v["wav_dir"] = json!(wav_dir.to_string_lossy());
    std::fs::write(vdir.join("analysis.json"), serde_json::to_string_pretty(&json!({"base": c.base_info, "variant": v}))?)?;
    Ok(v)
}

fn metrics_of(v: &Value) -> Option<gate::Metrics> {
    Some(gate::Metrics {
        wer: v["summary"]["wer"].as_f64()?,
        ecapa: v["summary"]["ecapa_mean"].as_f64()?,
        emotion_match: v["summary"]["emotion_match"].as_f64(),
    })
}


fn attach_emotion_match(variant: &Value, moments: &[emotion_match::Moment], ids: &[String], wav_dir: &Path, presence: f64, vdir: &Path) -> Result<Value> {
    let emo_path = vdir.join("scores/emo.json");
    let emo = analysis::load(&emo_path)?;
    let mut wavs = Vec::new();
    for id in ids {
        wavs.push(wav_dir.join(format!("{}.wav", id)));
    }
    let lines = emo_backend::lines_from_emo_json(&emo, ids, &wavs);
    let mut r = emotion_match::score_against_moments(&lines, moments, presence);
    r.ceiling = None;
    let j = emotion_match::result_to_json(&r);
    std::fs::write(vdir.join("emotion_match.json"), serde_json::to_string_pretty(&j)?)?;
    let mut v = variant.clone();
    v["summary"]["emotion_match"] = json!(r.emotion_match);
    v["emotion_match"] = j;
    Ok(v)
}

pub fn check(a: &Args) -> Result<PathBuf> {
    let voice = a.pos.first().context("need <voice>")?.trim_end_matches(".cbxvoice").to_string();
    let cfg = voice_config(&voice);
    let dry = a.flag("--dry-run");
    let base = a.get("--base").map(PathBuf::from).or_else(|| resolve_base(&voice, &cfg)).context("no --base and no config/clip-store base for this voice")?;
    let base = std::fs::canonicalize(&base).with_context(|| format!("base {}", base.display()))?;
    let skip_emo = a.flag("--skip-emotion");
    let emo_cfg_on = cfg["emotion_enabled"].as_bool().unwrap_or(false)
        || cfg.get("emotion_window").is_some()
        || cfg.get("emotion_moments").is_some()
        || a.get("--emotion-window").is_some()
        || a.get("--emo-min").is_some();
    let emotion_enabled = emo_cfg_on && !skip_emo;
    let emo_min = a.get("--emo-min").and_then(|s| s.parse().ok()).or(cfg["emo_min"].as_f64()).unwrap_or(emotion_match::DEFAULT_EMO_MIN);
    let emo_presence = a.get("--emotion-presence").and_then(|s| s.parse().ok()).or(cfg["emotion_presence"].as_f64()).unwrap_or(emotion_match::DEFAULT_PRESENCE);
    let emo_window_s = cfg["emotion_window_s"].as_f64().unwrap_or(emotion_match::DEFAULT_WINDOW_S);
    let emo_backend_name = a.get("--emotion-backend").cloned().or(cfg["emotion_backend"].as_str().map(String::from)).unwrap_or_else(|| "galion-emotion".into());
    let forced_window = a.get("--emotion-window").and_then(|s| emotion_match::parse_window_spec(s))
        .or_else(|| {
            cfg["emotion_window"].as_str().and_then(emotion_match::parse_window_spec)
                .or_else(|| {
                    let arr = cfg["emotion_window"].as_array()?;
                    if arr.len() == 2 { Some((arr[0].as_f64()?, arr[1].as_f64()?)) } else { None }
                })
        });
    let gcfg = gate::GateCfg {
        wer_max: a.get("--wer-max").and_then(|s| s.parse().ok()).or(cfg["wer_max"].as_f64()).unwrap_or(gate::DEFAULT_WER_MAX),
        ecapa_threshold: a.get("--threshold").and_then(|s| s.parse().ok()).or(cfg["ecapa_threshold"].as_f64()).unwrap_or(gate::DEFAULT_ECAPA_THRESHOLD),
        require_beat_prefix: a.flag("--require-beat-prefix") || cfg["require_beat_prefix"].as_bool().unwrap_or(false),
        emotion_enabled,
        emo_min,
    };
    let persona = a.get("--persona").cloned().or(cfg["persona"].as_str().map(String::from)).unwrap_or_else(|| gate::persona_of(&voice));
    let nmr = a.get("--mos-nmr").map(PathBuf::from).or(cfg["mos_nmr"].as_str().map(PathBuf::from)).unwrap_or_else(|| {
        let s = base.to_string_lossy();
        clip_store().join(if s.contains("/samantha/") { "jarvis" } else { "samantha" }).join("reference.wav")
    });
    let tsv = a.get("--texts").map(PathBuf::from).unwrap_or_else(|| root().join("emotion-fix/test12.tsv"));
    let (ids, texts) = read_texts(&tsv)?;
    let fixes: Vec<String> = a.get("--fixes").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or(vec!["a".into(), "b".into(), "c".into()]);
    if fixes.len() > 3 { bail!("at most three fix attempts"); }
    let mut fix_renders: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (k, v) in &a.kv { if k == "--fix-renders" { for part in v.split(',') { if let Some((f, d)) = part.split_once('=') { fix_renders.insert(f.into(), PathBuf::from(d)); } } } }
    let stamp = a.get("--tag").cloned().unwrap_or_else(|| format!("{}{}", now("%Y%m%d-%H%M%S"), if dry { "-dry" } else { "" }));
    if dry && a.get("--renders").is_none() { bail!("--dry-run needs --renders DIR (no new renders in dry mode)"); }
    let run = bcheck().join("runs").join(&voice).join(&stamp);
    std::fs::create_dir_all(&run)?;
    let text_dir = run.join("text");
    std::fs::create_dir_all(&text_dir)?;
    for id in &ids { std::fs::write(text_dir.join(format!("{}.txt", id)), &texts[id])?; }
    let t = Tools::new(&root());
    let started = now("%Y-%m-%dT%H:%M:%S%z");
    eprintln!("galion-voicecheck {} base={} run={} dry={}", voice, base.display(), run.display(), dry);

    // 1) pre-fix renders
    let prefix_dir = match a.get("--renders") {
        Some(d) => PathBuf::from(d),
        None => {
            if dry { bail!("--dry-run needs --renders DIR (no new renders in dry mode)"); }
            if !voices_dir().join(format!("{}.cbxvoice", voice)).exists() { bail!("voice {} not found in {}", voice, voices_dir().display()); }
            let d = run.join("prefix/wav"); render_set(&t, &voice, None, &ids, &texts, &d)?; d
        }
    };
    // 2) analysis vs base
    eprintln!("[base] analysing {}", base.display());
    let base_info = analysis::analyse_base(&t, &base, &nmr, &run.join("base"))?;
    let c = Ctx { t: &t, ids: &ids, texts: &texts, base: &base, base_info: &base_info, text_dir: &text_dir, nmr: &nmr };
    let mut prefix = score_variant(&c, "prefix", &prefix_dir, &run.join("prefix"))?;

    // Emotion moments from the base (read-only cut into temp files). Skipped when gate disabled.
    let emo_backend = emo_backend::resolve(&emo_backend_name, &root())?;
    let mut base_moments_json = Value::Null;
    let mut moments: Vec<emotion_match::Moment> = Vec::new();
    // Always classify moments for the report/leaderboard unless --skip-emotion.
    // The gate itself is controlled solely by gcfg.emotion_enabled.
    if !skip_emo {
        eprintln!("[emotion] backend={} window_s={:.2} presence={:.2} gate={}", emo_backend.id(), emo_window_s, emo_presence, emotion_enabled);
        match emo_backend::classify_base_windows(emo_backend.as_ref(), &base, &run.join("emotion"), emo_window_s, forced_window) {
            Ok(ws) => {
                let base_windows = ws;
                moments = emotion_match::find_moments(&base_windows, emo_presence);
                let ceiling = emotion_match::ceiling_vs_rest(&moments, &base_windows);
                // Profile the base against its own windows (self-match ceiling).
                let self_lines: Vec<(String, emotion_match::EmoVec)> = base_windows.iter().map(|w| (w.id.clone(), w.clone())).collect();
                let mut tmp = emotion_match::score_against_moments(&self_lines, &moments, emo_presence);
                tmp.ceiling = ceiling;
                base_moments_json = emotion_match::result_to_json(&tmp);
                base_moments_json["backend"] = json!(emo_backend.id());
                base_moments_json["n_windows"] = json!(base_windows.len());
                base_moments_json["forced_window"] = json!(forced_window);
                std::fs::write(run.join("emotion/moments.json"), serde_json::to_string_pretty(&base_moments_json)?)?;
                eprintln!("[emotion] {} moments across {} base windows; self-ceiling={:?}", moments.len(), base_windows.len(), ceiling);
            }
            Err(e) => {
                eprintln!("[emotion] base window classify failed: {:#}", e);
                if emotion_enabled { return Err(e); }
            }
        }
    }
    if !moments.is_empty() {
        prefix = attach_emotion_match(&prefix, &moments, &ids, &prefix_dir, emo_presence, &run.join("prefix"))?;
    }
    let pre_m = metrics_of(&prefix);

    // 3) fixes
    let provs = stages::discover(&cfg);
    let mut variants: BTreeMap<String, Value> = BTreeMap::new();
    let mut status: BTreeMap<String, Value> = BTreeMap::new();
    let mut fix_a_wav: Option<PathBuf> = fix_renders.get("a").cloned();
    let mut fix_a_ref: Option<PathBuf> = None;
    let mut fix_a_cand: Option<String> = None;
    for fx in &fixes {
        let vdir = run.join(format!("fix-{}", fx));
        std::fs::create_dir_all(&vdir)?;
        let res: Result<Option<PathBuf>> = (|| {
            match fx.as_str() {
                "a" => {
                    let r = stages::run_reference(&provs.reference, &voice, &base, &vdir, &run.join("prefix/analysis.json"))?;
                    fix_a_ref = Some(r.clone());
                    if let Some(d) = fix_renders.get("a") { status.insert("a".into(), json!({"status": "scored_external_renders", "renders": d, "reference_wav_built_not_used_for_these_renders": r})); return Ok(Some(d.clone())); }
                    if dry { status.insert("a".into(), json!({"status": "reference_built_encode_and_render_skipped_dry", "reference_wav": r})); return Ok(None); }
                    let cand = gate::winner_name(&persona, &format!("cand-a-{}", stamp.replace('-', "")), &|n: &str| voices_dir().join(format!("{}.cbxvoice", n)).exists());
                    encode_candidate(&t, &cand, &base, &r, &vdir)?;
                    fix_a_cand = Some(cand.clone());
                    status.insert("a".into(), json!({"status": "scored", "candidate_profile": cand, "reference_wav": r}));
                    let d = vdir.join("wav"); render_set(&t, &voice, Some(&cand), &ids, &texts, &d)?; fix_a_wav = Some(d.clone()); Ok(Some(d))
                }
                "b" => {
                    if let Some(d) = fix_renders.get("b") { status.insert("b".into(), json!({"status": "scored_external_renders", "renders": d})); return Ok(Some(d.clone())); }
                    let d = vdir.join("wav");
                    stages::run_prosody(&provs.prosody, &prefix_dir, &d, &base, &run.join("prefix/analysis.json"), &ids)?; Ok(Some(d))
                }
                "c" => {
                    if let Some(d) = fix_renders.get("c") { status.insert("c".into(), json!({"status": "scored_external_renders", "renders": d})); return Ok(Some(d.clone())); }
                    let Some(adir) = fix_a_wav.clone() else {
                        status.insert("c".into(), json!({"status": if dry { "skipped_dry_needs_fix_a_render" } else { "skipped_fix_a_not_rendered" }})); return Ok(None);
                    };
                    // analysis of a's renders is needed by the prosody layer (per-line targets)
                    let aan = run.join("fix-a/analysis.json");
                    if !aan.exists() { std::fs::create_dir_all(run.join("fix-a"))?; score_variant(&c, "a", &adir, &run.join("fix-a"))?; }
                    let d = vdir.join("wav");
                    stages::run_prosody(&provs.prosody, &adir, &d, &base, &aan, &ids)?; Ok(Some(d))
                }
                other => bail!("unknown fix {}", other),
            }
        })();
        match res {
            Ok(Some(d)) => {
                let mut v = score_variant(&c, fx, &d, &vdir)?;
                if !moments.is_empty() {
                    v = attach_emotion_match(&v, &moments, &ids, &d, emo_presence, &vdir)?;
                }
                status.entry(fx.clone()).or_insert(json!({"status": "scored"}));
                variants.insert(fx.clone(), v);
            }
            Ok(None) => {}
            Err(e) => { eprintln!("[fix {}] error: {:#}", fx, e); status.insert(fx.clone(), json!({"status": "error", "error": format!("{:#}", e)})); }
        }
    }

    // 4) gate
    let pre_dec = gate::evaluate(&gcfg, pre_m.as_ref(), "prefix", pre_m.as_ref());
    let decisions: Vec<gate::Decision> = fixes.iter().map(|f| gate::evaluate(&gcfg, pre_m.as_ref(), f, variants.get(f).and_then(metrics_of).as_ref())).collect();
    let win = gate::pick_winner(&decisions);
    let mut winner = Value::Null;
    if let Some(i) = win {
        let fx = decisions[i].fix.clone();
        let exists = |n: &str| voices_dir().join(format!("{}.cbxvoice", n)).exists();
        let name = gate::winner_name(&persona, &fx, &exists);
        let mut w = json!({"fix": fx, "name": name, "ecapa": decisions[i].ecapa, "wer": decisions[i].wer, "status": "pending_owner_approval", "saved": false});
        if dry || a.flag("--no-save") {
            w["status"] = json!(if dry { "dry_run_would_save" } else { "not_saved_no_save_flag" });
        } else {
            let _ = &fix_a_ref;
            match save_winner(&voice, &name, &fx, fix_a_cand.as_deref()) {
                Ok(info) => { w["saved"] = json!(true); w["save"] = info; }
                Err(e) => { w["status"] = json!("save_failed"); w["error"] = json!(format!("{:#}", e)); }
            }
        }
        winner = w;
    }
    let rep = json!({
        "schema": "galion-voicecheck/1", "voice": voice, "persona": persona, "base": base, "dry_run": dry,
        "started": started, "finished": now("%Y-%m-%dT%H:%M:%S%z"), "timezone": "Europe/Zagreb",
        "run_dir": run, "texts": tsv, "render": {"wrapper": t.speak(), "temperature": 0, "seed": 0, "lock": LOCK, "nice": 10, "threads": 2, "prefix_wav_dir": prefix_dir},
        "gate": {"wer_max": gcfg.wer_max, "ecapa_threshold": gcfg.ecapa_threshold, "require_beat_prefix": gcfg.require_beat_prefix,
            "emotion_enabled": gcfg.emotion_enabled, "emo_min": gcfg.emo_min, "emotion_presence": emo_presence,
            "emotion_backend": emo_backend_name, "emotion_window_s": emo_window_s, "forced_emotion_window": forced_window,
            "wer_metric": "galion-wer summary.mean_raw (Vosk small en-us)", "ecapa_metric": "galion-ecapa mean cosine vs base",
            "emotion_metric": "blended emotion_match vs base moments (see emotion_match::formula)"},
        "providers": provs.describe(),
        "emotion_profile": base_moments_json,
        "base_analysis": base_info,
        "prefix": prefix, "prefix_gate": report::dec_json(&pre_dec),
        "fixes": fixes.iter().map(|f| json!({"fix": f, "status": status.get(f).cloned().unwrap_or(json!({"status": "not_run"})), "variant": variants.get(f).cloned().unwrap_or(Value::Null)})).collect::<Vec<_>>(),
        "decisions": decisions.iter().map(report::dec_json).collect::<Vec<_>>(),
        "winner": winner, "fix_a_candidate_profile": fix_a_cand,
        "production_voice_untouched": true,
    });
    std::fs::write(run.join("report.json"), serde_json::to_string_pretty(&rep)?)?;
    std::fs::write(run.join("report.md"), report::markdown(&rep))?;
    let rdir = bcheck().join("reports").join(&voice);
    std::fs::create_dir_all(&rdir)?;
    std::fs::copy(run.join("report.json"), rdir.join(format!("{}.json", stamp)))?;
    std::fs::copy(run.join("report.md"), rdir.join(format!("{}.md", stamp)))?;
    std::fs::write(rdir.join("LATEST"), format!("{}\n", stamp))?;
    if !winner.is_null() && winner["saved"].as_bool() == Some(true) {
        let ap = bcheck().join("approvals");
        std::fs::create_dir_all(&ap)?;
        std::fs::write(ap.join(format!("PENDING-{}.json", winner["name"].as_str().unwrap())), serde_json::to_string_pretty(&json!({
            "status": "pending_owner_approval", "candidate_voice": winner["name"], "source_voice": voice, "fix": winner["fix"],
            "ecapa": winner["ecapa"], "wer": winner["wer"], "prefix_ecapa": pre_m.map(|m| m.ecapa), "prefix_wer": pre_m.map(|m| m.wer),
            "report": rdir.join(format!("{}.md", stamp)), "note": "Production voice NOT changed. Owner must approve before any promotion."
        }))?)?;
    }
    eprintln!("report: {}", run.join("report.md").display());
    println!("{}", run.join("report.json").display());
    Ok(run)
}

/// Encode a candidate profile for fix (a): identity tensors from the base, conditioning from the
/// expressive reference (A-global's cbx-blend --conditioning-wav). Falls back to plain cbx voice add.
fn encode_candidate(t: &Tools, cand: &str, base: &Path, reference: &Path, vdir: &Path) -> Result<()> {
    if gate::is_protected(cand) { bail!("refusing protected name {}", cand); }
    if voices_dir().join(format!("{}.cbxvoice", cand)).exists() { bail!("candidate {} exists", cand); }
    let blend = root().join("pipeline/cbx-v2-timbre/target/release/cbx-blend");
    let mut cmd: Vec<String> = vec!["flock".into(), LOCK.into(), "nice".into(), "-n".into(), "10".into()];
    if blend.exists() {
        cmd.extend([blend.to_string_lossy().into(), "--intra-threads".into(), "2".into(), "--inter-threads".into(), "2".into(), "voice".into(), "add".into(), "--dtype".into(), "fp32".into(),
            "--name".into(), cand.into(), "--voice-wav".into(), base.to_string_lossy().into(), "--conditioning-wav".into(), reference.to_string_lossy().into()]);
    } else {
        cmd.extend([t.cbx().to_string_lossy().into(), "--intra-threads".into(), "2".into(), "voice".into(), "add".into(), "--name".into(), cand.into(), "--voice-wav".into(), reference.to_string_lossy().into()]);
    }
    eprintln!("  [encode] candidate {}", cand);
    run_logged(&cmd, &vdir.join("encode.log"))
}

fn copy_new(src: &Path, dst: &Path) -> Result<()> {
    use std::io::Write;
    let bytes = std::fs::read(src)?;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(dst).with_context(|| format!("create_new {}", dst.display()))?;
    f.write_all(&bytes)?;
    Ok(())
}

/// Save the winner under a NEW name (create_new byte copy); never touches the production profile.
fn save_winner(voice: &str, name: &str, fx: &str, cand: Option<&str>) -> Result<Value> {
    if gate::is_protected(name) { bail!("refusing protected name {}", name); }
    let dst = voices_dir().join(format!("{}.cbxvoice", name));
    if dst.exists() { bail!("{} exists; never overwriting", dst.display()); }
    let src = match fx {
        "a" | "c" => voices_dir().join(format!("{}.cbxvoice", cand.context("no fix-a candidate profile (fix-a renders were supplied externally?)")?)),
        "b" => voices_dir().join(format!("{}.cbxvoice", voice)),
        _ => bail!("unknown fix"),
    };
    copy_new(&src, &dst)?;
    let mut info = json!({"path": dst, "method": format!("byte copy of {}", src.display())});
    if fx == "b" || fx == "c" {
        let mut d = voice_defaults(voice);
        if !d.is_object() || d.as_object().unwrap().is_empty() { d = json!({"dtype": "fp32", "temperature": 0, "seed": 0}); }
        d["voice"] = json!(name);
        d["prosody_layer"] = json!({"enabled": true, "cmd": "galion-voicecheck layer --in-dir <renders> --out-dir <out> --analysis <analysis.json>", "params": "builtin defaults (strength 0.5, expand<=1.2, gain<=3 dB, +<=150 ms/pause, <=10% total)"});
        d["notes"] = json!(format!("PROPOSED by galion-voicecheck from {} fix {}; pending owner approval", voice, fx));
        let ap = bcheck().join("approvals");
        std::fs::create_dir_all(&ap)?;
        let pp = ap.join(format!("{}.voice-defaults.proposed.json", name));
        std::fs::write(&pp, serde_json::to_string_pretty(&d)?)?;
        info["proposed_defaults"] = json!(pp);
    }
    Ok(info)
}

fn cmd_add(a: &Args) -> Result<()> {
    let name = a.get("--name").context("--name")?.clone();
    let wav = a.get("--voice-wav").context("--voice-wav")?.clone();
    if gate::is_protected(&name) { bail!("refusing to (re)create protected voice {}", name); }
    let dst = voices_dir().join(format!("{}.cbxvoice", name));
    if dst.exists() { bail!("{} already exists; never overwriting a voice", dst.display()); }
    let t = Tools::new(&root());
    let log = bcheck().join("logs/add.log");
    std::fs::create_dir_all(log.parent().unwrap())?;
    run_logged(&["flock".into(), LOCK.into(), "nice".into(), "-n".into(), "10".into(), t.cbx().to_string_lossy().into(), "--intra-threads".into(), "2".into(),
        "voice".into(), "add".into(), "--name".into(), name.clone(), "--voice-wav".into(), wav.clone()], &log)?;
    // mark as seen so the watcher does not double-check it
    stages::mark_seen(&format!("{}.cbxvoice", name), "checked_by_add_hook")?;
    let mut argv: Vec<String> = vec![name];
    if a.get("--base").is_none() { argv.extend(["--base".into(), wav]); }
    for (k, v) in &a.kv { if k != "--name" && k != "--voice-wav" { argv.extend([k.clone(), v.clone()]); } }
    argv.extend(a.flags.iter().cloned());
    check(&parse(&argv))?;
    Ok(())
}

fn cmd_layer(a: &Args) -> Result<()> {
    let ind = PathBuf::from(a.get("--in-dir").context("--in-dir")?);
    let outd = PathBuf::from(a.get("--out-dir").context("--out-dir")?);
    let an = a.get("--analysis").map(|p| load(Path::new(p))).transpose()?;
    let mut ids: Vec<String> = std::fs::read_dir(&ind)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".wav")).map(|n| n.trim_end_matches(".wav").to_string()).collect();
    ids.sort();
    stages::builtin_prosody(&ind, &outd, an.as_ref(), &ids)
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let a = parse(&argv);
    if argv.is_empty() || a.flag("--help") || a.flag("-h") { print!("{}", USAGE); return; }
    let sub = a.pos[0].as_str();
    let rest = || { let mut b = parse(&argv); b.pos.remove(0); b };
    let r = match sub {
        "check" => check(&rest()).map(|_| ()),
        "add" => cmd_add(&rest()),
        "watch" => stages::watch(&rest()),
        "layer" => cmd_layer(&rest()),
        "expressive-ref" => { let b = rest(); (|| { let base = PathBuf::from(b.get("--base").context("--base")?); let out = PathBuf::from(b.get("--out-dir").context("--out-dir")?);
            let ts = b.get("--target-s").and_then(|s| s.parse().ok()).unwrap_or(10.0); stages::builtin_reference(&base, &out, ts).map(|p| println!("{}", p.display())) })() }
        "stages" => { println!("{}", serde_json::to_string_pretty(&stages::discover(&json!({})).describe()).unwrap()); Ok(()) }
        _ => check(&a).map(|_| ()),
    };
    if let Err(e) = r { eprintln!("error: {:#}", e); std::process::exit(1); }
}
