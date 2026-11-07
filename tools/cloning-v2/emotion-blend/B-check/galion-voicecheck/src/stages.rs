//! Pluggable fix stages + watcher.
//!
//! Stage contract (see ../INTERFACE.md):
//!   reference stage (fix a):  <cmd...> --voice V --base BASE.wav --out-dir D --analysis PREFIX_ANALYSIS.json
//!                             must write D/reference.wav (exit 0). Optional D/stage.json metadata.
//!   prosody stage   (fix b):  <cmd...> --in-dir RENDERS --out-dir D --base BASE.wav --analysis ANALYSIS.json
//!                             must write D/<id>.wav for every RENDERS/<id>.wav (exit 0).
//! Discovery order per stage: per-voice config `stages.<kind>` (array cmd) → env GALION_VC_STAGE_<KIND>
//!   → emotion-blend/A-global and emotion-blend/C-sam: voicecheck-stage.json {"reference":{"cmd":[..]},"prosody":{"cmd":[..]}}
//!     or executables voicecheck-<kind> / bin/voicecheck-<kind>  → builtin.
use crate::analysis::{load, run_logged};
use crate::{dsp, gate};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum Provider { Builtin, External { cmd: Vec<String>, source: String }, BlendRef { bin: String, scorer: String } }

pub struct Providers { pub reference: Provider, pub prosody: Provider, pub searched: Vec<String> }
impl Providers {
    pub fn describe(&self) -> Value {
        let d = |p: &Provider| match p { Provider::Builtin => json!({"provider": "builtin"}), Provider::External { cmd, source } => json!({"provider": "external", "cmd": cmd, "source": source}),
            Provider::BlendRef { bin, .. } => json!({"provider": "A-global galion-blend-ref (crop-aware <=6 s diverse montage)", "cmd": bin}) };
        json!({"reference_fix_a": d(&self.reference), "prosody_fix_b": d(&self.prosody), "searched": self.searched})
    }
}

fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file() && std::fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

fn find(kind: &str, cfg: &Value, searched: &mut Vec<String>) -> Provider {
    if let Some(a) = cfg["stages"][kind].as_array() {
        let cmd: Vec<String> = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        if !cmd.is_empty() { return Provider::External { cmd, source: "per-voice config".into() }; }
    }
    let env = format!("GALION_VC_STAGE_{}", kind.to_uppercase());
    if let Ok(v) = std::env::var(&env) { if !v.trim().is_empty() { return Provider::External { cmd: v.split_whitespace().map(String::from).collect(), source: env }; } }
    let eb = crate::root().join("emotion-blend");
    for w in ["A-global", "C-sam"] {
        let d = eb.join(w);
        let m = d.join("voicecheck-stage.json");
        searched.push(m.to_string_lossy().into());
        if let Ok(j) = load(&m) {
            if let Some(a) = j[kind]["cmd"].as_array() {
                let cmd: Vec<String> = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                if !cmd.is_empty() { return Provider::External { cmd, source: m.to_string_lossy().into() }; }
            }
        }
        for c in [d.join(format!("voicecheck-{}", kind)), d.join("bin").join(format!("voicecheck-{}", kind))] {
            searched.push(c.to_string_lossy().into());
            if is_exec(&c) { return Provider::External { cmd: vec![c.to_string_lossy().into()], source: c.to_string_lossy().into() }; }
        }
    }
    if kind == "reference" {
        // A-global's shared crop-aware montage builder (pipeline/EMOTION-BLEND.md)
        let b = crate::root().join("pipeline/emotion-blend-ref/target/release/galion-blend-ref");
        searched.push(b.to_string_lossy().into());
        let sc = crate::root().join("scorers/emotion/target/release/galion-emotion");
        if is_exec(&b) && is_exec(&sc) { return Provider::BlendRef { bin: b.to_string_lossy().into(), scorer: sc.to_string_lossy().into() }; }
    }
    Provider::Builtin
}

pub fn discover(cfg: &Value) -> Providers {
    let mut searched = vec![];
    let reference = find("reference", cfg, &mut searched);
    let prosody = find("prosody", cfg, &mut searched);
    searched.dedup();
    Providers { reference, prosody, searched }
}

pub fn builtin_reference(base: &Path, out_dir: &Path, target_s: f64) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let (x, sr) = dsp::read_mono(base)?;
    let (y, meta) = dsp::select_expressive(&x, sr, target_s, 6.0);
    if y.len() < sr as usize * 3 { bail!("expressive selection too short ({} s)", y.len() as f64 / sr as f64); }
    let out = out_dir.join("reference.wav");
    dsp::write_mono16(&out, &y, sr)?;
    std::fs::write(out_dir.join("stage.json"), serde_json::to_string_pretty(&json!({"stage": "reference", "provider": "builtin", "base": base, "meta": meta}))?)?;
    Ok(out)
}

pub fn run_reference(p: &Provider, voice: &str, base: &Path, out_dir: &Path, analysis: &Path) -> Result<PathBuf> {
    match p {
        Provider::Builtin => builtin_reference(base, out_dir, 10.0),
        Provider::BlendRef { bin, scorer } => {
            std::fs::create_dir_all(out_dir)?;
            let r = out_dir.join("reference.wav");
            if !r.exists() {
                run_logged(&["nice".into(), "-n".into(), "10".into(), bin.clone(), "--input".into(), base.to_string_lossy().into(), "--output".into(), r.to_string_lossy().into(),
                    "--meta".into(), out_dir.join("reference.json").to_string_lossy().into(), "--scorer".into(), scorer.clone()], &out_dir.join("stage.log"))?;
            }
            if !r.exists() { bail!("galion-blend-ref did not write {}", r.display()); }
            let _ = voice;
            Ok(r)
        }
        Provider::External { cmd, .. } => {
            std::fs::create_dir_all(out_dir)?;
            let mut c = cmd.clone();
            c.extend(["--voice".into(), voice.into(), "--base".into(), base.to_string_lossy().into(), "--out-dir".into(), out_dir.to_string_lossy().into(), "--analysis".into(), analysis.to_string_lossy().into()]);
            let mut full = vec!["nice".to_string(), "-n".into(), "10".into()]; full.extend(c);
            run_logged(&full, &out_dir.join("stage.log"))?;
            let r = out_dir.join("reference.wav");
            if !r.exists() { bail!("external reference stage did not write {}", r.display()); }
            Ok(r)
        }
    }
}

pub fn builtin_prosody(in_dir: &Path, out_dir: &Path, analysis: Option<&Value>, ids: &[String]) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let params = dsp::ProsodyParams::default();
    let (bdr, bpr) = match analysis {
        Some(a) => (a["base"]["energy_dr_db"].as_f64().unwrap_or(f64::NAN), a["base"]["pause_ratio"].as_f64().unwrap_or(f64::NAN)),
        None => (f64::NAN, f64::NAN),
    };
    let mut metas = vec![];
    for id in ids {
        let inp = in_dir.join(format!("{}.wav", id));
        let (x, sr) = dsp::read_mono(&inp)?;
        let line = analysis.and_then(|a| a["variant"]["lines"].as_array().and_then(|l| l.iter().find(|r| r["id"].as_str() == Some(id)).cloned()));
        let (rdr, rpr) = match &line { Some(l) => (l["energy_dr_db"].as_f64().unwrap_or(f64::NAN), l["pause_ratio"].as_f64().unwrap_or(f64::NAN)), None => (f64::NAN, f64::NAN) };
        let (y, m) = dsp::prosody_layer(&x, sr, &params, rdr, bdr, rpr, bpr);
        dsp::write_mono16(&out_dir.join(format!("{}.wav", id)), &y, sr)?;
        metas.push(json!({"id": id, "meta": m}));
    }
    std::fs::write(out_dir.join("stage.json"), serde_json::to_string_pretty(&json!({"stage": "prosody", "provider": "builtin", "params": format!("{:?}", params), "lines": metas}))?)?;
    Ok(())
}

pub fn run_prosody(p: &Provider, in_dir: &Path, out_dir: &Path, base: &Path, analysis: &Path, ids: &[String]) -> Result<()> {
    match p {
        Provider::Builtin | Provider::BlendRef { .. } => { let a = load(analysis).ok(); builtin_prosody(in_dir, out_dir, a.as_ref(), ids) }
        Provider::External { cmd, .. } => {
            std::fs::create_dir_all(out_dir)?;
            let mut full = vec!["nice".to_string(), "-n".into(), "10".into()]; full.extend(cmd.clone());
            full.extend(["--in-dir".into(), in_dir.to_string_lossy().into(), "--out-dir".into(), out_dir.to_string_lossy().into(), "--base".into(), base.to_string_lossy().into(), "--analysis".into(), analysis.to_string_lossy().into()]);
            run_logged(&full, &out_dir.join("stage.log"))?;
            for id in ids { if !out_dir.join(format!("{}.wav", id)).exists() { bail!("external prosody stage missing {}.wav", id); } }
            Ok(())
        }
    }
}

// ---------------- watcher ----------------
fn state_path() -> PathBuf { std::env::var("GALION_VC_STATE").map(PathBuf::from).unwrap_or_else(|_| crate::bcheck().join("state/seen.json")) }
fn load_state() -> Value { load(&state_path()).unwrap_or(json!({})) }
fn save_state(s: &Value) -> Result<()> {
    let p = state_path(); std::fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}
pub fn mark_seen(file: &str, status: &str) -> Result<()> {
    let mut s = load_state();
    if !s.is_object() { s = json!({}); }
    s[file] = json!({"status": status});
    save_state(&s)
}

pub fn watch(a: &crate::Args) -> Result<()> {
    let interval: u64 = a.kv.get("--interval").and_then(|s| s.parse().ok()).unwrap_or(60);
    let once = a.flags.iter().any(|f| f == "--once");
    let initial = a.flags.iter().any(|f| f == "--initial-scan");
    let dry = a.flags.iter().any(|f| f == "--dry-run");
    let vd = crate::voices_dir();
    let mut st = load_state();
    if !st.is_object() || st.as_object().unwrap().is_empty() {
        st = json!({});
        if !initial {
            for e in std::fs::read_dir(&vd)?.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if gate::watch_eligible(&n) { st[&n] = json!({"status": "preexisting_at_watch_start"}); }
            }
            save_state(&st)?;
            eprintln!("watch: seeded {} pre-existing voices (not checked; use --initial-scan to check them)", st.as_object().unwrap().len());
        }
    }
    let exe = std::env::current_exe()?;
    loop {
        let mut names: Vec<String> = std::fs::read_dir(&vd)?.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| gate::watch_eligible(n)).collect();
        names.sort();
        for n in names {
            st = load_state();
            let prev = st[&n]["status"].as_str().unwrap_or("").to_string();
            if !prev.is_empty() && prev != "needs_base" { continue; }
            let p = vd.join(&n);
            let age = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()).unwrap_or(0);
            if age < 15 { continue; } // still being written
            let voice = n.trim_end_matches(".cbxvoice").to_string();
            let cfg = load(&crate::bcheck().join("config/voices").join(format!("{}.json", voice))).unwrap_or(json!({}));
            let Some(base) = crate::resolve_base(&voice, &cfg) else {
                if prev != "needs_base" {
                    eprintln!("watch: {} has no base recording (add config/voices/{}.json with \"base\")", voice, voice);
                    let d = crate::bcheck().join("reports").join(&voice); std::fs::create_dir_all(&d)?;
                    std::fs::write(d.join("NEEDS-BASE.md"), format!("# {}\n\nNo base recording found. Create `emotion-blend/B-check/config/voices/{}.json` with {{\"base\": \"/path/to/recording.wav\"}}; the watcher retries every pass.\n", voice, voice))?;
                    mark_seen(&n, "needs_base")?;
                }
                continue;
            };
            if dry { eprintln!("watch(dry): would check {} --base {}", voice, base.display()); continue; }
            mark_seen(&n, "checking")?;
            eprintln!("watch: checking new voice {} (base {})", voice, base.display());
            let ok = std::process::Command::new("nice").args(["-n", "10"]).arg(&exe).args(["check", &voice, "--base"]).arg(&base).status().context("spawn check")?.success();
            mark_seen(&n, if ok { "checked" } else { "check_failed" })?;
        }
        if once { break; }
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
    Ok(())
}
