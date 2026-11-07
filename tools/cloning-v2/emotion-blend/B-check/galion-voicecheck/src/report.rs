use crate::gate::Decision;
use serde_json::{json, Value};

pub fn dec_json(d: &Decision) -> Value {
    json!({"fix": d.fix, "keep": d.keep, "wer": d.wer, "ecapa": d.ecapa, "emotion_match": d.emotion_match,
        "wer_max": d.wer_max, "ecapa_bar": d.ecapa_bar, "emo_min": d.emo_min, "emotion_enabled": d.emotion_enabled,
        "wer_ok": d.wer_ok, "ecapa_ok": d.ecapa_ok, "emotion_ok": d.emotion_ok, "reasons": d.reasons})
}

fn n(v: &Value, dp: usize) -> String { match v.as_f64() { Some(x) => format!("{:.*}", dp, x), None => "—".into() } }
fn pct(v: &Value) -> String { match v.as_f64() { Some(x) => format!("{:.2}%", x * 100.0), None => "—".into() } }
fn labels(v: &Value) -> String {
    v.as_object().map(|m| { let mut s: Vec<String> = m.iter().map(|(k, c)| format!("{} {}", k, c)).collect(); s.sort(); s.join(", ") }).unwrap_or("—".into())
}

fn summary_row(name: &str, v: &Value) -> String {
    let s = &v["summary"]; let m = &s["means"];
    format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{}/{} | {} |\n", name,
        n(&s["ecapa_mean"], 4), pct(&s["wer"]), n(&s["mos_mean"], 3), n(&m["f0_mean_hz"], 1), n(&m["f0_sd_st"], 2), n(&m["f0_range_st"], 2),
        n(&m["energy_dr_db"], 2), n(&m["wps"], 3), n(&m["pause_ratio"], 3), n(&s["emotion_distance_mean"], 4), labels(&s["label_counts"]),
        n(&m["arousal"], 3), n(&m["valence"], 3), n(&m["dominance"], 3), format!("{}/{}", s["clicks_total"], s["clipped_runs_total"]))
}

fn lines_table(v: &Value) -> String {
    let mut o = String::from("| Line | F0 mean Hz (Δbase) | F0 sd st | F0 range st | Energy DR dB (Δ) | w/s (×base) | Pause ratio (Δ) | P(neu/hap/ang/sad) | top | A/V/D | ECAPA | WER | MOS | artic syl/s |\n|---|---:|---:|---:|---:|---:|---:|---|---|---|---:|---:|---:|---:|\n");
    for r in v["lines"].as_array().into_iter().flatten() {
        let d = &r["vs_base"];
        o += &format!("| {} | {} ({:+.1}) | {} | {} | {} ({:+.1}) | {} (×{}) | {} ({:+.3}) | {}/{}/{}/{} | {} | {}/{}/{} | {} | {} | {} | {} |\n",
            r["id"].as_str().unwrap_or(""), n(&r["f0_mean_hz"], 1), d["f0_mean_hz"].as_f64().unwrap_or(f64::NAN), n(&r["f0_sd_st"], 2), n(&r["f0_range_st"], 2),
            n(&r["energy_dr_db"], 1), d["energy_dr_db"].as_f64().unwrap_or(f64::NAN), n(&r["wps"], 2), n(&d["wps_ratio"], 2), n(&r["pause_ratio"], 3), d["pause_ratio"].as_f64().unwrap_or(f64::NAN),
            n(&r["ser"]["neu"], 2), n(&r["ser"]["hap"], 2), n(&r["ser"]["ang"], 2), n(&r["ser"]["sad"], 2), r["ser_top"].as_str().unwrap_or(""),
            n(&r["avd"]["arousal"], 3), n(&r["avd"]["valence"], 3), n(&r["avd"]["dominance"], 3), n(&r["ecapa"], 4), pct(&r["wer"]), n(&r["mos"], 2), n(&r["articulation_rate_syl_s"], 2));
    }
    o
}

pub fn markdown(r: &Value) -> String {
    let b = &r["base_analysis"];
    let g = &r["gate"];
    let mut o = format!("# galion-voicecheck — `{}`\n\n", r["voice"].as_str().unwrap_or(""));
    o += &format!("- Run: {} → {} (Europe/Zagreb){}\n", r["started"].as_str().unwrap_or(""), r["finished"].as_str().unwrap_or(""), if r["dry_run"].as_bool() == Some(true) { " — **DRY RUN** (no renders, voices dir untouched)" } else { "" });
    let emo_note = if g["emotion_enabled"].as_bool() == Some(true) {
        format!(" AND emotion_match ≥ {}", n(&g["emo_min"], 4))
    } else { " (emotion gate off)".into() };
    o += &format!("- Base recording: `{}`\n- Pre-fix renders: `{}`\n- Gate: WER ≤ {} AND ECAPA > {}{}{}\n", r["base"].as_str().unwrap_or(""), r["render"]["prefix_wav_dir"].as_str().unwrap_or(""),
        pct(&g["wer_max"]), n(&g["ecapa_threshold"], 4), if g["require_beat_prefix"].as_bool() == Some(true) { " (and > voice's own pre-fix ECAPA)" } else { "" }, emo_note);
    for f in r["fixes"].as_array().into_iter().flatten() {
        if f["status"]["status"].as_str() == Some("scored_external_renders") {
            o += &format!("- NOTE fix {}: scored on externally supplied renders `{}` (--fix-renders), not rendered by this run\n", f["fix"].as_str().unwrap_or(""), f["status"]["renders"].as_str().unwrap_or(""));
        }
    }
    o += &format!("- Fix providers: reference(a) = {}, prosody(b) = {}\n- Production voice untouched: **yes** (winner only ever saved under a new `own-<p>-v2-blend-<fix>` name, pending owner approval)\n\n",
        r["providers"]["reference_fix_a"]["provider"], r["providers"]["prosody_fix_b"]["provider"]);
    o += "## Base recording\n\n| F0 mean Hz | F0 sd st | F0 range st | Energy DR dB | w/s (ASR est.) | Pause ratio | P(neu/hap/ang/sad) | A/V/D | MOS | artic syl/s |\n|---:|---:|---:|---:|---:|---:|---|---|---:|---:|\n";
    o += &format!("| {} | {} | {} | {} | {} | {} | {}/{}/{}/{} | {}/{}/{} | {} | {} |\n\n", n(&b["f0_mean_hz"], 1), n(&b["f0_sd_st"], 2), n(&b["f0_range_st"], 2), n(&b["energy_dr_db"], 2), n(&b["wps"], 3), n(&b["pause_ratio"], 3),
        n(&b["ser"]["neu"], 2), n(&b["ser"]["hap"], 2), n(&b["ser"]["ang"], 2), n(&b["ser"]["sad"], 2), n(&b["avd"]["arousal"], 3), n(&b["avd"]["valence"], 3), n(&b["avd"]["dominance"], 3), n(&b["mos"], 3), n(&b["articulation_rate_syl_s"], 2));
    o += "## Summary (n=12)\n\n| Variant | ECAPA ↑ | WER ↓ | MOS ↑ | F0 mean Hz | F0 sd st | F0 range st | Energy DR dB | w/s | Pause ratio | Emo dist ↓ | SER labels | A/V/D | clicks/clipped |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---|\n";
    o += &summary_row("base recording (ref)", &json!({"summary": {"means": {"f0_mean_hz": b["f0_mean_hz"], "f0_sd_st": b["f0_sd_st"], "f0_range_st": b["f0_range_st"], "energy_dr_db": b["energy_dr_db"], "wps": b["wps"], "pause_ratio": b["pause_ratio"], "arousal": b["avd"]["arousal"], "valence": b["avd"]["valence"], "dominance": b["avd"]["dominance"]}, "mos_mean": b["mos"], "label_counts": {b["ser_top"].as_str().unwrap_or("?"): 1}, "clicks_total": b["clicks"], "clipped_runs_total": b["clipped_runs"]}}));
    o += &summary_row("pre-fix (voice as cloned)", &r["prefix"]);
    for f in r["fixes"].as_array().into_iter().flatten() {
        if !f["variant"].is_null() { o += &summary_row(&format!("fix {}", f["fix"].as_str().unwrap_or("")), &f["variant"]); }
    }
    o += "\n## Gate decisions\n\n| Variant | Status | WER | ECAPA | ECAPA bar | Emo match | Emo min | Keep | Reason |\n|---|---|---:|---:|---:|---:|---:|---|---|\n";
    let pg = &r["prefix_gate"];
    o += &format!("| pre-fix (info) | scored | {} | {} | {} | {} | {} | {} | {} |\n", pct(&pg["wer"]), n(&pg["ecapa"], 4), n(&pg["ecapa_bar"], 4), n(&pg["emotion_match"], 4), n(&pg["emo_min"], 4), if pg["keep"].as_bool() == Some(true) { "would pass" } else { "would fail" }, pg["reasons"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default());
    for (f, d) in r["fixes"].as_array().into_iter().flatten().zip(r["decisions"].as_array().into_iter().flatten()) {
        o += &format!("| fix {} | {} | {} | {} | {} | {} | {} | {} | {} |\n", f["fix"].as_str().unwrap_or(""), f["status"]["status"].as_str().unwrap_or(""), pct(&d["wer"]), n(&d["ecapa"], 4), n(&d["ecapa_bar"], 4), n(&d["emotion_match"], 4), n(&d["emo_min"], 4),
            if d["keep"].as_bool() == Some(true) { "**KEEP**" } else { "reject" }, d["reasons"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default());
    }
    // Emotion leaderboard (from prefix if present)
    if let Some(lb) = r["prefix"]["emotion_match"]["leaderboard"].as_array() {
        o += "\n## Emotion-match leaderboard (pre-fix vs base moments)\n\n| Rank | Category | Kind | Score | Best line | Best match | Mean match | Moment s | Strength |\n|---:|---|---|---:|---|---:|---:|---|---:|\n";
        for (i, c) in lb.iter().enumerate() {
            o += &format!("| {} | {} | {} | {} | {} | {} | {} | {}–{} | {} |\n",
                i + 1, c["category"].as_str().unwrap_or(""), c["kind"].as_str().unwrap_or(""),
                n(&c["score"], 4), c["best_line"].as_str().unwrap_or("—"), n(&c["best_match"], 4), n(&c["mean_match"], 4),
                n(&c["moment_start_s"], 2), n(&c["moment_end_s"], 2), n(&c["moment_strength"], 3));
        }
        if let Some(nm) = r["prefix"]["emotion_match"]["not_measurable"].as_array() {
            if !nm.is_empty() {
                o += "\nNot measurable with current backend: ";
                o += &nm.iter().filter_map(|x| x["category"].as_str()).collect::<Vec<_>>().join(", ");
                o += ".\n";
            }
        }
        o += &format!("\nBlended emotion_match (pre-fix): **{}** (formula in emotion_match.json).\n", n(&r["prefix"]["summary"]["emotion_match"], 4));
    }
    let w = &r["winner"];
    o += "\n## Winner\n\n";
    if w.is_null() { o += "No fix passed the gate → nothing saved; the voice stays as cloned.\n"; }
    else { o += &format!("Fix **{}** → `{}` — status **{}** (saved: {}). ECAPA {} / WER {}. Requires owner approval before any promotion.\n", w["fix"].as_str().unwrap_or(""), w["name"].as_str().unwrap_or(""), w["status"].as_str().unwrap_or(""), w["saved"], n(&w["ecapa"], 4), pct(&w["wer"])); }
    o += "\n## Line-by-line — pre-fix vs base\n\n";
    o += &lines_table(&r["prefix"]);
    for f in r["fixes"].as_array().into_iter().flatten() {
        if !f["variant"].is_null() { o += &format!("\n## Line-by-line — fix {}\n\n", f["fix"].as_str().unwrap_or("")); o += &lines_table(&f["variant"]); }
    }
    o
}
