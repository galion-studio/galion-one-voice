// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Results file. JSON and Markdown are rendered from the same [`Report`],
//! so the two views cannot disagree. An unmeasured report has no percentile
//! fields.

use crate::clock::BAR_US;
use crate::machine::Machine;
use crate::pipeline::{summarize, ReportParts};
use crate::piper::PiperInfo;
use crate::stats::rank;
use crate::traits::{Pad, StageNames};
use std::path::Path;

pub const SCHEMA: &str = "galion-s12-latency/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockKind {
    Measured,
    Derived,
}

impl ClockKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClockKind::Measured => "measured",
            ClockKind::Derived => "derived",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdClocks {
    pub spawn_to_first_pcm_us: u64,
    pub warmup_write_to_first_pcm_us: u64,
    pub warmup_phrase: String,
}

#[derive(Clone, Debug)]
pub struct StageStat {
    pub key: &'static str,
    pub title: &'static str,
    pub clock: ClockKind,
    pub n: usize,
    pub min_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

impl StageStat {
    pub fn p50_within_bar(&self) -> bool {
        self.p50_us <= BAR_US
    }
    pub fn p95_within_bar(&self) -> bool {
        self.p95_us <= BAR_US
    }
    pub fn p99_within_bar(&self) -> bool {
        self.p99_us <= BAR_US
    }
}

/// One rep. All fields are microseconds. `eos_to_heartbeat_play_us` is
/// derived from the measured arrivals. The others are wall clocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rep {
    pub vad_us: u64,
    pub eos_to_asr_us: u64,
    pub asr_to_reply_us: u64,
    pub reply_to_tts_us: u64,
    /// Reply ready until one 80 ms slice of PCM has been produced.
    pub reply_to_first_slice_us: u64,
    pub tts_to_audio_out_us: u64,
    pub eos_to_audio_out_us: u64,
    pub eos_to_heartbeat_play_us: u64,
    pub mood_ticks: u32,
    pub sink_peak_milli: u32,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub measured: bool,
    pub reason: Option<String>,
    pub backend: String,
    pub profile: String,
    pub machine: Machine,
    pub names: Option<StageNames>,
    pub reps: Vec<Rep>,
    pub summary: Vec<StageStat>,
    pub warmup_discarded: usize,
    pub tts_pinned_voice: bool,
    pub voice: Option<String>,
    pub cold: Option<ColdClocks>,
    pub piper: Option<PiperInfo>,
    pub notes: Vec<String>,
    pub last_mood: Option<Pad>,
    pub phrase: String,
    pub mood_rule: String,
}

impl Report {
    pub fn unmeasured(backend: &str, reason: &str, machine: Machine) -> Self {
        Self {
            measured: false,
            reason: Some(reason.to_string()),
            backend: backend.to_string(),
            profile: profile_name().to_string(),
            machine,
            names: None,
            reps: Vec::new(),
            summary: Vec::new(),
            warmup_discarded: 0,
            tts_pinned_voice: false,
            voice: None,
            cold: None,
            piper: None,
            notes: vec!["No percentiles. This run did not measure the stage clocks.".into()],
            last_mood: None,
            phrase: String::new(),
            mood_rule: "none".into(),
        }
    }

    pub fn measured(parts: ReportParts) -> Self {
        let summary = summarize(&parts.reps);
        Self {
            measured: true,
            reason: None,
            backend: parts.backend,
            profile: profile_name().to_string(),
            machine: parts.machine,
            names: Some(parts.names),
            warmup_discarded: parts.warmup,
            tts_pinned_voice: parts.claims_tts_measured,
            voice: parts.voice,
            cold: parts.cold,
            piper: parts.piper,
            notes: parts.notes,
            last_mood: parts.last_mood,
            phrase: parts.phrase,
            mood_rule: "toward-rest-0.1".into(),
            reps: parts.reps,
            summary,
        }
    }

    /// Build a measured report from reps the caller already collected.
    /// Tests use this with a fixed machine. The summary is computed here.
    pub fn from_reps(backend: &str, machine: Machine, names: StageNames, reps: Vec<Rep>) -> Self {
        let n_warm = 0;
        let summary = summarize(&reps);
        Self {
            measured: true,
            reason: None,
            backend: backend.to_string(),
            profile: profile_name().to_string(),
            machine,
            names: Some(names),
            reps,
            summary,
            warmup_discarded: n_warm,
            tts_pinned_voice: false,
            voice: None,
            cold: None,
            piper: None,
            notes: vec!["test report".into()],
            last_mood: Some(Pad::RESTING),
            phrase: crate::PHRASE.to_string(),
            mood_rule: "toward-rest-0.1".into(),
        }
    }

    pub fn to_json(&self) -> String {
        let mut s = String::new();
        s.push_str("{\n");
        field(&mut s, 1, "schema", &json_str(SCHEMA), false);
        field(&mut s, 1, "spdx", &json_str("MIT"), false);
        field(
            &mut s,
            1,
            "copyright",
            &json_str("Copyright (c) 2026 Galion Labs"),
            false,
        );
        field(&mut s, 1, "measured", bool_s(self.measured), false);
        field(&mut s, 1, "product_claim", "false", false);
        field(&mut s, 1, "cpu_only", "true", false);
        field(&mut s, 1, "paid", "false", false);
        field(&mut s, 1, "backend", &json_str(&self.backend), false);
        field(&mut s, 1, "profile", &json_str(&self.profile), false);
        field(&mut s, 1, "frame_ms", "80", false);
        field(&mut s, 1, "frame_samples", "1920", false);
        field(&mut s, 1, "sample_rate_hz", "24000", false);
        field(&mut s, 1, "bar_us", "80000", false);
        field(
            &mut s,
            1,
            "percentile",
            &json_str("nearest-rank-ceil"),
            false,
        );
        field(
            &mut s,
            1,
            "percentile_source",
            &json_str("crates/galion-heartbeat/tests/frame_budget.rs rank"),
            false,
        );
        let n = self.reps.len();
        field(&mut s, 1, "n", &n.to_string(), false);
        field(
            &mut s,
            1,
            "warmup_discarded",
            &self.warmup_discarded.to_string(),
            false,
        );
        if n > 0 {
            field(&mut s, 1, "p99_index", &rank(n, 99, 100).to_string(), false);
            field(
                &mut s,
                1,
                "p99_is_max",
                bool_s(rank(n, 99, 100) == n - 1),
                false,
            );
        }
        field(&mut s, 1, "reason", &opt_str(self.reason.as_deref()), false);
        field(&mut s, 1, "phrase", &json_str(&self.phrase), false);
        field(&mut s, 1, "mood_rule", &json_str(&self.mood_rule), false);
        field(
            &mut s,
            1,
            "tts_pinned_voice",
            bool_s(self.tts_pinned_voice),
            false,
        );
        field(&mut s, 1, "voice", &opt_str(self.voice.as_deref()), false);
        s.push_str("  \"machine\": ");
        s.push_str(&machine_json(&self.machine));
        s.push_str(",\n");
        s.push_str("  \"stages\": ");
        s.push_str(&names_json(self.names.as_ref()));
        s.push_str(",\n");
        s.push_str("  \"cold\": ");
        s.push_str(&cold_json(self.cold.as_ref()));
        s.push_str(",\n");
        s.push_str("  \"piper\": ");
        s.push_str(&piper_json(self.piper.as_ref()));
        s.push_str(",\n");
        s.push_str("  \"last_mood\": ");
        s.push_str(&mood_json(self.last_mood));
        s.push_str(",\n");
        s.push_str("  \"notes\": ");
        s.push_str(&string_array(&self.notes));
        if self.measured {
            s.push_str(",\n  \"summary\": ");
            s.push_str(&summary_json(&self.summary));
            s.push_str(",\n  \"reps\": ");
            s.push_str(&reps_json(&self.reps));
            s.push('\n');
        } else {
            s.push('\n');
        }
        s.push_str("}\n");
        s
    }

    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("<!-- SPDX-License-Identifier: MIT -->\n");
        s.push_str("<!-- Copyright (c) 2026 Galion Labs -->\n\n");
        s.push_str("# S12 latency harness\n\n");
        s.push_str(&format!(
            "Backend `{backend}`. Profile `{profile}`. Schema `{schema}`.\n\n",
            backend = self.backend,
            profile = self.profile,
            schema = SCHEMA
        ));
        s.push_str("The pipeline product claim is false. This file is a measurement of the stages named below.\n\n");
        s.push_str("## Machine\n\n");
        s.push_str(&format!(
            "- CPU: {cpu} ({mhz} MHz)\n",
            cpu = self.machine.cpu,
            mhz = self.machine.cpu_mhz
        ));
        s.push_str(&format!("- nproc: {n}\n", n = self.machine.nproc));
        s.push_str(&format!(
            "- OS: {os} {arch}, kernel {k}\n",
            os = self.machine.os,
            arch = self.machine.arch,
            k = self.machine.kernel
        ));
        s.push_str(&format!("- Hostname: {h}\n", h = self.machine.hostname));
        s.push_str(&format!("- Load average: {l}\n", l = self.machine.loadavg));
        s.push_str(&format!("- Date (UTC): {d}\n", d = self.machine.date_utc));
        s.push_str(&format!("- Commit: `{c}`\n", c = self.machine.commit));
        s.push_str(&format!(
            "- Commit dirty: {d}\n",
            d = match self.machine.commit_dirty {
                Some(true) => "true",
                Some(false) => "false",
                None => "unknown",
            }
        ));
        s.push_str(&format!("- rustc: {r}\n\n", r = self.machine.rustc));
        if let Some(reason) = &self.reason {
            s.push_str(&format!("Not measured. {reason}\n\n"));
        }
        if !self.measured {
            s.push_str("No percentile table. This run did not time the stages.\n");
            return s;
        }
        s.push_str(&format!(
            "n = {n}. Warmup reps discarded: {w}.\n\n",
            n = self.reps.len(),
            w = self.warmup_discarded
        ));
        s.push_str("Percentile index is `ceil(n * p / 100) - 1` on the sorted samples, the same `rank` as `crates/galion-heartbeat/tests/frame_budget.rs`. ");
        if rank(self.reps.len().max(1), 99, 100) == self.reps.len().saturating_sub(1)
            && !self.reps.is_empty()
        {
            s.push_str("At this n, p99 is the maximum sample.\n\n");
        } else {
            s.push_str("At this n, p99 is not the maximum sample.\n\n");
        }
        s.push_str("The bar is 80 ms on each stage. `derived` rows are the 80 ms heartbeat grid from `scripts/eval/piper_stream_ttfa.py` (origin = first PCM byte). They are computed from the measured arrival. They are not a second wall clock and they are not a sleep. Each stamp is truncated to microseconds on its own, so the stage deltas can miss the direct end-to-end clock by a few microseconds.\n\n");
        s.push_str("| stage | clock | min ms | p50 ms | p95 ms | p99 ms | max ms | p50 ≤ 80 ms | p95 ≤ 80 ms | p99 ≤ 80 ms |\n");
        s.push_str("|---|---|---:|---:|---:|---:|---:|---|---|---|\n");
        for row in &self.summary {
            s.push_str(&format!(
                "| {title} | {clock} | {min} | {p50} | {p95} | {p99} | {max} | {a} | {b} | {c} |\n",
                title = row.title,
                clock = row.clock.as_str(),
                min = ms_text(row.min_us),
                p50 = ms_text(row.p50_us),
                p95 = ms_text(row.p95_us),
                p99 = ms_text(row.p99_us),
                max = ms_text(row.max_us),
                a = yes(row.p50_within_bar()),
                b = yes(row.p95_within_bar()),
                c = yes(row.p99_within_bar()),
            ));
        }
        s.push('\n');
        if let Some(names) = &self.names {
            s.push_str("## Stage engines\n\n");
            s.push_str(&format!("- VAD: `{n}`\n", n = names.vad));
            s.push_str(&format!("- ASR: `{n}`\n", n = names.asr));
            s.push_str(&format!("- Reply: `{n}`\n", n = names.reply));
            s.push_str(&format!("- TTS: `{n}`\n", n = names.tts));
            s.push_str(&format!("- Audio out: `{n}`\n\n", n = names.audio_out));
        }
        if let Some(cold) = &self.cold {
            s.push_str("## Cold Piper clocks\n\n");
            s.push_str("These two numbers are the warmup utterance. They are not in the percentile table.\n\n");
            s.push_str(&format!("- Phrase: {p}\n", p = cold.warmup_phrase));
            s.push_str(&format!(
                "- Spawn to first PCM: {ms} ms\n",
                ms = ms_text(cold.spawn_to_first_pcm_us)
            ));
            s.push_str(&format!(
                "- Warmup write to first PCM: {ms} ms\n\n",
                ms = ms_text(cold.warmup_write_to_first_pcm_us)
            ));
        }
        if let Some(p) = &self.piper {
            s.push_str("## Piper\n\n");
            s.push_str(&format!("- Runtime: `{r}`\n", r = p.runtime));
            s.push_str(&format!("- Model: `{m}`\n", m = p.model));
            s.push_str(&format!("- Sample rate: {r} Hz\n", r = p.sample_rate));
            s.push_str(&format!(
                "- Model sha256: `{h}` (pinned: {pin})\n",
                h = p.sha256,
                pin = yes(p.sha256_pinned)
            ));
            s.push_str(&format!(
                "- Config sha256: `{h}` (pinned: {pin})\n",
                h = p.json_sha256,
                pin = yes(p.json_sha256_pinned)
            ));
            s.push_str(&format!("- Prosody: {pros}\n\n", pros = p.prosody));
        }
        if let Some(mood) = self.last_mood {
            s.push_str(&format!(
                "Last reply mood: arousal {a:.4}, valence {v:.4}, dominance {d:.4}. Rule `{rule}`.\n\n",
                a = mood.arousal,
                v = mood.valence,
                d = mood.dominance,
                rule = self.mood_rule,
            ));
        }
        s.push_str("## Notes\n\n");
        for note in &self.notes {
            s.push_str(&format!("- {note}\n"));
        }
        s.push_str("\nRaw per-rep microseconds are in the JSON file under `reps`.\n");
        s
    }

    pub fn write_pair(
        &self,
        dir: &Path,
        stem: &str,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        let json_path = dir.join(format!("{stem}.json"));
        let md_path = dir.join(format!("{stem}.md"));
        let json = self.to_json();
        let md = self.to_markdown();
        atomic_write(&json_path, json.as_bytes())?;
        atomic_write(&md_path, md.as_bytes())?;
        Ok((json_path, md_path))
    }
}

fn profile_name() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

pub fn ms_text(us: u64) -> String {
    let whole = us / 1000;
    let frac = us % 1000;
    format!("{whole}.{frac:03}")
}

fn yes(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

fn bool_s(v: bool) -> &'static str {
    if v {
        "true"
    } else {
        "false"
    }
}

fn field(out: &mut String, indent: usize, key: &str, value: &str, last: bool) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    out.push_str(value);
    if !last {
        out.push(',');
    }
    out.push('\n');
}

pub fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn opt_str(v: Option<&str>) -> String {
    match v {
        Some(s) => json_str(s),
        None => "null".into(),
    }
}

fn machine_json(m: &Machine) -> String {
    format!(
        "{{\n    \"os\": {os},\n    \"arch\": {arch},\n    \"kernel\": {kernel},\n    \"cpu\": {cpu},\n    \"cpu_mhz\": {mhz},\n    \"nproc\": {n},\n    \"hostname\": {host},\n    \"loadavg\": {load},\n    \"date_utc\": {date},\n    \"commit\": {commit},\n    \"commit_dirty\": {dirty},\n    \"rustc\": {rustc}\n  }}",
        os = json_str(&m.os),
        arch = json_str(&m.arch),
        kernel = json_str(&m.kernel),
        cpu = json_str(&m.cpu),
        mhz = json_str(&m.cpu_mhz),
        n = m.nproc,
        host = json_str(&m.hostname),
        load = json_str(&m.loadavg),
        date = json_str(&m.date_utc),
        commit = json_str(&m.commit),
        dirty = match m.commit_dirty {
            Some(v) => bool_s(v),
            None => "null",
        },
        rustc = json_str(&m.rustc),
    )
}

fn names_json(names: Option<&StageNames>) -> String {
    let Some(n) = names else {
        return "null".into();
    };
    format!(
        "{{\n    \"vad\": {vad},\n    \"asr\": {asr},\n    \"reply\": {reply},\n    \"tts\": {tts},\n    \"audio_out\": {out}\n  }}",
        vad = json_str(&n.vad),
        asr = json_str(&n.asr),
        reply = json_str(&n.reply),
        tts = json_str(&n.tts),
        out = json_str(&n.audio_out),
    )
}

fn cold_json(cold: Option<&ColdClocks>) -> String {
    let Some(c) = cold else {
        return "null".into();
    };
    format!(
        "{{\n    \"spawn_to_first_pcm_us\": {spawn},\n    \"warmup_write_to_first_pcm_us\": {warm},\n    \"warmup_phrase\": {phrase},\n    \"in_percentiles\": false\n  }}",
        spawn = c.spawn_to_first_pcm_us,
        warm = c.warmup_write_to_first_pcm_us,
        phrase = json_str(&c.warmup_phrase),
    )
}

fn piper_json(p: Option<&PiperInfo>) -> String {
    let Some(p) = p else {
        return "null".into();
    };
    let args = string_array(&p.args);
    format!(
        "{{\n    \"runtime\": {rt},\n    \"model\": {model},\n    \"sample_rate\": {rate},\n    \"sha256\": {sha},\n    \"sha256_pinned\": {pin},\n    \"json_sha256\": {jsha},\n    \"json_sha256_pinned\": {jpin},\n    \"length_scale\": {ls},\n    \"prosody\": {pros},\n    \"args\": {args}\n  }}",
        rt = json_str(&p.runtime),
        model = json_str(&p.model),
        rate = p.sample_rate,
        sha = json_str(&p.sha256),
        pin = bool_s(p.sha256_pinned),
        jsha = json_str(&p.json_sha256),
        jpin = bool_s(p.json_sha256_pinned),
        ls = json_str(&p.length_scale),
        pros = json_str(&p.prosody),
        args = args,
    )
}

fn mood_json(pad: Option<Pad>) -> String {
    let Some(p) = pad else {
        return "null".into();
    };
    format!(
        "{{\"arousal\": {a:.4}, \"valence\": {v:.4}, \"dominance\": {d:.4}}}",
        a = p.arousal,
        v = p.valence,
        d = p.dominance,
    )
}

fn string_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".into();
    }
    let mut s = String::from("[\n");
    for (i, item) in items.iter().enumerate() {
        s.push_str("    ");
        s.push_str(&json_str(item));
        if i + 1 != items.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]");
    s
}

fn summary_json(rows: &[StageStat]) -> String {
    let mut s = String::from("[\n");
    for (i, row) in rows.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"key\": {key}, \"title\": {title}, \"clock\": {clock}, \"n\": {n}, \"min_us\": {min}, \"p50_us\": {p50}, \"p95_us\": {p95}, \"p99_us\": {p99}, \"max_us\": {max}, \"p50_ms\": {p50ms}, \"p95_ms\": {p95ms}, \"p99_ms\": {p99ms}, \"p50_within_bar\": {a}, \"p95_within_bar\": {b}, \"p99_within_bar\": {c}}}",
            key = json_str(row.key),
            title = json_str(row.title),
            clock = json_str(row.clock.as_str()),
            n = row.n,
            min = row.min_us,
            p50 = row.p50_us,
            p95 = row.p95_us,
            p99 = row.p99_us,
            max = row.max_us,
            p50ms = json_str(&ms_text(row.p50_us)),
            p95ms = json_str(&ms_text(row.p95_us)),
            p99ms = json_str(&ms_text(row.p99_us)),
            a = bool_s(row.p50_within_bar()),
            b = bool_s(row.p95_within_bar()),
            c = bool_s(row.p99_within_bar()),
        ));
        if i + 1 != rows.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]");
    s
}

fn reps_json(reps: &[Rep]) -> String {
    let mut s = String::from("[\n");
    for (i, r) in reps.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"vad_us\": {vad}, \"eos_to_asr_us\": {asr}, \"asr_to_reply_us\": {reply}, \"reply_to_tts_us\": {tts}, \"reply_to_first_slice_us\": {slice}, \"tts_to_audio_out_us\": {gap}, \"eos_to_audio_out_us\": {e2e}, \"eos_to_heartbeat_play_us\": {play}, \"heartbeat_play_derived\": true, \"mood_ticks\": {ticks}, \"sink_peak_milli\": {peak}}}",
            vad = r.vad_us,
            asr = r.eos_to_asr_us,
            reply = r.asr_to_reply_us,
            tts = r.reply_to_tts_us,
            slice = r.reply_to_first_slice_us,
            gap = r.tts_to_audio_out_us,
            e2e = r.eos_to_audio_out_us,
            play = r.eos_to_heartbeat_play_us,
            ticks = r.mood_ticks,
            peak = r.sink_peak_milli,
        ));
        if i + 1 != reps.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]");
    s
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))?;
    Ok(())
}
