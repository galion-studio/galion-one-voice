// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Offline gap list from a turn log.
//!
//! Reads each named log and writes one JSON object per distinct heard line.
//! Withheld text, an empty line, and a gap research already answered are
//! left out. A malformed line is counted on stderr and does not stop the
//! export. Paths come from arguments only. Research is not called.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    logs: Vec<PathBuf>,
    out: PathBuf,
}

struct GapGroup {
    reasons: BTreeSet<String>,
    count: u64,
    first_ts: String,
    last_ts: String,
}

enum Fail {
    Usage,
    Input(String),
    Output(String),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            eprintln!("horbm-gaps: {fail}");
            ExitCode::from(fail.code())
        }
    }
}

fn run() -> Result<(), Fail> {
    let args = parse_args()?;
    let mut groups = BTreeMap::new();
    let mut malformed = 0u64;
    for path in &args.logs {
        ingest(path, &mut groups, &mut malformed)?;
    }
    if malformed > 0 {
        eprintln!("horbm-gaps: malformed {malformed}");
    }
    let body = render(&sorted(groups))?;
    std::fs::write(&args.out, body)
        .map_err(|err| Fail::Output(format!("cannot write {}: {err}", args.out.display())))?;
    Ok(())
}

fn parse_args() -> Result<Args, Fail> {
    let mut logs = Vec::new();
    let mut out = None;
    let mut raw = env::args_os().skip(1);
    while let Some(arg) = raw.next() {
        if arg == "--" {
            logs.extend(raw.map(PathBuf::from));
            break;
        }
        if arg == "--out" {
            let Some(path) = raw.next() else {
                return Err(Fail::Usage);
            };
            if out.is_some() {
                return Err(Fail::Usage);
            }
            out = Some(PathBuf::from(path));
            continue;
        }
        if is_flag(&arg) {
            return Err(Fail::Usage);
        }
        logs.push(PathBuf::from(arg));
    }
    let Some(out) = out else {
        return Err(Fail::Usage);
    };
    if logs.is_empty() {
        return Err(Fail::Usage);
    }
    Ok(Args { logs, out })
}

fn is_flag(arg: &OsStr) -> bool {
    matches!(arg.to_str(), Some(text) if text.starts_with('-'))
}

fn ingest(
    path: &Path,
    groups: &mut BTreeMap<String, GapGroup>,
    malformed: &mut u64,
) -> Result<(), Fail> {
    let bytes = std::fs::read(path)
        .map_err(|err| Fail::Input(format!("cannot read {}: {err}", path.display())))?;
    for line in bytes.split(|byte| *byte == b'\n') {
        take_line(line, groups, malformed);
    }
    Ok(())
}

fn take_line(mut line: &[u8], groups: &mut BTreeMap<String, GapGroup>, malformed: &mut u64) {
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    if line.is_empty() {
        return;
    }
    let Ok(text) = std::str::from_utf8(line) else {
        *malformed += 1;
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    accept_line(text, groups, malformed);
}

fn accept_line(text: &str, groups: &mut BTreeMap<String, GapGroup>, malformed: &mut u64) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        *malformed += 1;
        return;
    };
    let Some(obj) = value.as_object() else {
        return;
    };
    if field(obj, "kind") != Some("gap") {
        return;
    }
    let (Some(heard), Some(reason), Some(ts)) =
        (field(obj, "heard"), field(obj, "reason"), field(obj, "ts"))
    else {
        *malformed += 1;
        return;
    };
    // The host writes the literal "withheld" after public_text. An answered
    // gap already has a reply, so it is not a card to write.
    if heard == "withheld" || reason == "research_answered" {
        return;
    }
    let heard = normalise_heard(heard);
    if heard.is_empty() {
        return;
    }
    record(groups, heard, reason, ts);
}

fn field<'a>(obj: &'a serde_json::Map<String, serde_json::Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(serde_json::Value::as_str)
}

/// Trim, lowercase ASCII letters, and collapse whitespace to one space.
fn normalise_heard(heard: &str) -> String {
    let mut out = String::with_capacity(heard.len());
    let mut spaced = false;
    for ch in heard.trim().chars() {
        if ch.is_whitespace() {
            spaced = !out.is_empty();
            continue;
        }
        if spaced {
            out.push(' ');
            spaced = false;
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn record(groups: &mut BTreeMap<String, GapGroup>, heard: String, reason: &str, ts: &str) {
    let group = groups.entry(heard).or_insert_with(|| GapGroup {
        reasons: BTreeSet::new(),
        count: 0,
        first_ts: ts.to_string(),
        last_ts: ts.to_string(),
    });
    group.reasons.insert(reason.to_string());
    group.count += 1;
    // Host timestamps are zero-padded UTC, so text order is time order.
    if ts < group.first_ts.as_str() {
        group.first_ts = ts.to_string();
    }
    if ts > group.last_ts.as_str() {
        group.last_ts = ts.to_string();
    }
}

fn sorted(groups: BTreeMap<String, GapGroup>) -> Vec<(String, GapGroup)> {
    let mut rows: Vec<_> = groups.into_iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .count
            .cmp(&left.1.count)
            .then_with(|| left.0.cmp(&right.0))
    });
    rows
}

fn render(rows: &[(String, GapGroup)]) -> Result<String, Fail> {
    let mut body = String::new();
    for (heard, group) in rows {
        body.push_str(&format_row(heard, group)?);
        body.push('\n');
    }
    Ok(body)
}

fn format_row(heard: &str, group: &GapGroup) -> Result<String, Fail> {
    let heard = serde_json::to_string(heard).map_err(json_fail)?;
    let reasons = serde_json::to_string(&group.reasons).map_err(json_fail)?;
    let first = serde_json::to_string(&group.first_ts).map_err(json_fail)?;
    let last = serde_json::to_string(&group.last_ts).map_err(json_fail)?;
    let count = group.count;
    Ok(format!(
        "{{\"heard\":{heard},\"reasons\":{reasons},\"count\":{count},\"first_ts\":{first},\"last_ts\":{last}}}"
    ))
}

fn json_fail(err: serde_json::Error) -> Fail {
    Fail::Output(err.to_string())
}

impl Fail {
    fn code(&self) -> u8 {
        match self {
            Fail::Output(_) => 1,
            Fail::Usage | Fail::Input(_) => 2,
        }
    }
}

impl std::fmt::Display for Fail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Usage => formatter.write_str("usage: horbm-gaps <log> [<log>...] --out <path>"),
            Fail::Input(message) | Fail::Output(message) => formatter.write_str(message),
        }
    }
}
