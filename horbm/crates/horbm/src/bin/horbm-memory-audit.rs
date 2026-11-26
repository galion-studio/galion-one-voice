// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local audit of one person-memory file and, when asked, one learn pack.
//!
//! Do not run this while 7881 has that database open.
//!
//! Every path is an argument. There is no default data root. This program
//! does not read the environment, does not use the network, and does not
//! write the turn log.
//!
//! `list` opens the memory file with `SQLITE_OPEN_READ_ONLY` and only after
//! the file exists. It does not create that file or its parent. `PRAGMA
//! user_version` and the named columns are compared with the `CREATE TABLE`
//! statements `horbm-memory` installs. That check, and the `items` and
//! `archive` readers, are `EpisodicStore::open_readonly`, `check_schema`,
//! `read_items`, and `read_archive`. This program maps those rows to JSON.
//! A schema difference exits 3. This program does not guess a newer or older
//! shape.
//!
//! `forget` archives one row through `EpisodicStore` with remember granted.
//! Exit 0 is `Archived`. Any other outcome exits 1. A missing file exits 2
//! and is not created.
//!
//! `withdraw-learn` removes the learn pack directories through `LearnPass`.
//!
//! `list` prints JSON lines, in this order:
//!
//! - `{"kind":"row","item_id","layer","said_by","date","source","pinned","marked","current","text"}`
//!   for `items` whose `layer` is `episode` or `diary`
//! - `{"kind":"tombstone","item_id","layer","said_by","date","source","text","reason"}`
//!   for `archive`
//! - `{"kind":"learned","item_id","said_by","date","source","text"}`
//!   when `--learn` is set, from `LearnPass::audit`
//!
//! `--person` keeps lines whose `said_by` is that id.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use galion_emotion_core::standins::memory::{DiaryArchiveRow, DiaryItemRow, DiaryReadError};
use horbm::{EpisodicStore, LearnConfig, LearnPass, PersonMemory, WriteOutcome};

const EXIT_OK: u8 = 0;
const EXIT_FAIL: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_SCHEMA: u8 = 3;

enum Stop {
    Usage(String),
    Missing(String),
    Schema(String),
    Fail(String),
}

impl Stop {
    fn code(self) -> u8 {
        let (code, message, show_usage) = match self {
            Stop::Usage(message) => (EXIT_USAGE, message, true),
            Stop::Missing(message) => (EXIT_USAGE, message, false),
            Stop::Schema(message) => (EXIT_SCHEMA, message, false),
            Stop::Fail(message) => (EXIT_FAIL, message, false),
        };
        eprintln!("horbm-memory-audit: {message}");
        if show_usage {
            usage();
        }
        code
    }
}

fn main() -> ExitCode {
    ExitCode::from(dispatch())
}

fn dispatch() -> u8 {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let Some(cmd) = args.first() else {
        usage();
        return EXIT_USAGE;
    };
    let Some(cmd) = cmd.to_str() else {
        usage();
        return EXIT_USAGE;
    };
    let rest = &args[1..];
    match cmd {
        "list" => cmd_list(rest),
        "forget" => cmd_forget(rest),
        "withdraw-learn" => cmd_withdraw(rest),
        "help" | "--help" | "-h" => {
            usage();
            EXIT_OK
        }
        other => {
            eprintln!("horbm-memory-audit: unknown command {other}");
            usage();
            EXIT_USAGE
        }
    }
}

fn usage() {
    eprintln!(
        "\
usage: horbm-memory-audit list --memory <db> [--learn <pack_dir>] [--person <id>]
       horbm-memory-audit forget --memory <db> --person <id> <item_id>
       horbm-memory-audit withdraw-learn --learn <pack_dir>"
    );
}

struct ListArgs {
    memory: PathBuf,
    learn: Option<PathBuf>,
    person: Option<String>,
}

struct ForgetArgs {
    memory: PathBuf,
    person: String,
    item_id: String,
}

fn cmd_list(args: &[OsString]) -> u8 {
    let parsed = match parse_list(args) {
        Ok(parsed) => parsed,
        Err(stop) => return stop.code(),
    };
    match list(&parsed) {
        Ok(()) => EXIT_OK,
        Err(stop) => stop.code(),
    }
}

fn list(args: &ListArgs) -> Result<(), Stop> {
    let conn = EpisodicStore::open_readonly(&args.memory).map_err(stop_of)?;
    EpisodicStore::check_schema(&conn).map_err(stop_of)?;
    let mut lines =
        json_items(&EpisodicStore::read_items(&conn, args.person.as_deref()).map_err(stop_of)?);
    lines.extend(json_archive(
        &EpisodicStore::read_archive(&conn, args.person.as_deref()).map_err(stop_of)?,
    ));
    if let Some(pack) = &args.learn {
        lines.extend(read_learned(pack, args.person.as_deref())?);
    }
    for line in lines {
        let text = serde_json::to_string(&line).map_err(|err| Stop::Fail(err.to_string()))?;
        println!("{text}");
    }
    Ok(())
}

fn stop_of(err: DiaryReadError) -> Stop {
    match err {
        DiaryReadError::Missing(message) => Stop::Missing(message),
        DiaryReadError::Schema(message) => Stop::Schema(message),
        DiaryReadError::Fail(message) => Stop::Fail(message),
    }
}

fn json_items(rows: &[DiaryItemRow]) -> Vec<serde_json::Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "kind": "row",
                "item_id": row.item_id,
                "layer": row.layer,
                "said_by": row.said_by,
                "date": row.date,
                "source": row.source,
                "pinned": row.pinned,
                "marked": row.marked,
                "current": row.current,
                "text": row.text,
            })
        })
        .collect()
}

fn json_archive(rows: &[DiaryArchiveRow]) -> Vec<serde_json::Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "kind": "tombstone",
                "item_id": row.item_id,
                "layer": row.layer,
                "said_by": row.said_by,
                "date": row.date,
                "source": row.source,
                "text": row.text,
                "reason": row.reason,
            })
        })
        .collect()
}

fn cmd_forget(args: &[OsString]) -> u8 {
    let parsed = match parse_forget(args) {
        Ok(parsed) => parsed,
        Err(stop) => return stop.code(),
    };
    if !parsed.memory.is_file() {
        return Stop::Missing(format!("{} is not a file", parsed.memory.display())).code();
    }
    let mut store = EpisodicStore::open(&parsed.memory, true);
    let outcome = store.forget(&parsed.person, &parsed.item_id);
    println!("{outcome:?}");
    drop(store);
    if matches!(outcome, WriteOutcome::Archived { .. }) {
        EXIT_OK
    } else {
        EXIT_FAIL
    }
}

fn cmd_withdraw(args: &[OsString]) -> u8 {
    let pack = match parse_withdraw(args) {
        Ok(pack) => pack,
        Err(stop) => return stop.code(),
    };
    match LearnPass::open(LearnConfig::granted(&pack)).withdraw() {
        Ok(()) => EXIT_OK,
        Err(err) => Stop::Fail(err.to_string()).code(),
    }
}

fn read_learned(pack: &Path, person: Option<&str>) -> Result<Vec<serde_json::Value>, Stop> {
    let audit = LearnPass::open(LearnConfig::fresh(pack))
        .audit()
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut out = Vec::new();
    for row in audit.rows {
        if person.is_some_and(|id| row.said_by != id) {
            continue;
        }
        out.push(serde_json::json!({
            "kind": "learned",
            "item_id": row.item_id,
            "said_by": row.said_by,
            "date": row.date,
            "source": row.source,
            "text": row.text,
        }));
    }
    Ok(out)
}

fn parse_list(args: &[OsString]) -> Result<ListArgs, Stop> {
    let mut memory = None;
    let mut learn = None;
    let mut person = None;
    let mut i = 0;
    while i < args.len() {
        let flag = arg_str(&args[i])?;
        match flag {
            "--memory" => {
                let path = next_path(args, &mut i, "--memory")?;
                assign_once(&mut memory, path, "--memory")?;
            }
            "--learn" => {
                let path = next_path(args, &mut i, "--learn")?;
                assign_once(&mut learn, path, "--learn")?;
            }
            "--person" => {
                let id = next_id(args, &mut i, "--person")?;
                assign_once(&mut person, id, "--person")?;
            }
            other => {
                return Err(Stop::Usage(format!("unknown argument {other}")));
            }
        }
        i += 1;
    }
    let memory = memory.ok_or_else(|| Stop::Usage("--memory is required".to_string()))?;
    Ok(ListArgs {
        memory,
        learn,
        person,
    })
}

fn parse_forget(args: &[OsString]) -> Result<ForgetArgs, Stop> {
    let mut memory = None;
    let mut person = None;
    let mut item_id = None;
    let mut i = 0;
    while i < args.len() {
        let flag = arg_str(&args[i])?;
        match flag {
            "--memory" => {
                let path = next_path(args, &mut i, "--memory")?;
                assign_once(&mut memory, path, "--memory")?;
            }
            "--person" => {
                let id = next_id(args, &mut i, "--person")?;
                assign_once(&mut person, id, "--person")?;
            }
            other if other.starts_with('-') => {
                return Err(Stop::Usage(format!("unknown argument {other}")));
            }
            other => {
                let id = other.trim();
                if id.is_empty() {
                    return Err(Stop::Usage("item id is empty".to_string()));
                }
                assign_once(&mut item_id, id.to_string(), "item id")?;
            }
        }
        i += 1;
    }
    Ok(ForgetArgs {
        memory: memory.ok_or_else(|| Stop::Usage("--memory is required".to_string()))?,
        person: person.ok_or_else(|| Stop::Usage("--person is required".to_string()))?,
        item_id: item_id.ok_or_else(|| Stop::Usage("item id is required".to_string()))?,
    })
}

fn parse_withdraw(args: &[OsString]) -> Result<PathBuf, Stop> {
    let mut learn = None;
    let mut i = 0;
    while i < args.len() {
        let flag = arg_str(&args[i])?;
        match flag {
            "--learn" => {
                let path = next_path(args, &mut i, "--learn")?;
                assign_once(&mut learn, path, "--learn")?;
            }
            other => return Err(Stop::Usage(format!("unknown argument {other}"))),
        }
        i += 1;
    }
    learn.ok_or_else(|| Stop::Usage("--learn is required".to_string()))
}

fn assign_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), Stop> {
    if slot.is_some() {
        return Err(Stop::Usage(format!("{name} was given twice")));
    }
    *slot = Some(value);
    Ok(())
}

fn next_path(args: &[OsString], i: &mut usize, flag: &str) -> Result<PathBuf, Stop> {
    *i += 1;
    let Some(raw) = args.get(*i) else {
        return Err(Stop::Usage(format!("{flag} needs a path")));
    };
    if raw.is_empty() {
        return Err(Stop::Usage(format!("{flag} needs a path")));
    }
    Ok(PathBuf::from(raw))
}

fn next_id(args: &[OsString], i: &mut usize, flag: &str) -> Result<String, Stop> {
    *i += 1;
    let Some(raw) = args.get(*i) else {
        return Err(Stop::Usage(format!("{flag} needs a value")));
    };
    let Some(text) = raw.to_str() else {
        return Err(Stop::Usage(format!("{flag} is not utf-8")));
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(Stop::Usage(format!("{flag} is empty")));
    }
    Ok(text.to_string())
}

fn arg_str(raw: &OsString) -> Result<&str, Stop> {
    raw.to_str()
        .ok_or_else(|| Stop::Usage("argument is not utf-8".to_string()))
}
