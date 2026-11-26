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
//! statements `horbm-memory` installs. A difference exits 3. This program
//! does not guess a newer or older shape.
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
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use horbm::{EpisodicStore, LearnConfig, LearnPass, PersonMemory, WriteOutcome};
use rusqlite::{params, Connection, OpenFlags};

const EXIT_OK: u8 = 0;
const EXIT_FAIL: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_SCHEMA: u8 = 3;

/// Columns and `user_version` taken from the `CREATE TABLE` statements
/// `horbm-memory` installs. Not a copy of that list.
struct DeclaredSchema {
    user_version: i32,
    items: Vec<(String, String)>,
    archive: Vec<(String, String)>,
}

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
    let conn = open_readonly(&args.memory)?;
    check_schema(&conn)?;
    let mut lines = read_items(&conn, args.person.as_deref())?;
    lines.extend(read_archive(&conn, args.person.as_deref())?);
    if let Some(pack) = &args.learn {
        lines.extend(read_learned(pack, args.person.as_deref())?);
    }
    for line in lines {
        let text = serde_json::to_string(&line).map_err(|err| Stop::Fail(err.to_string()))?;
        println!("{text}");
    }
    Ok(())
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

fn open_readonly(path: &Path) -> Result<Connection, Stop> {
    if !path.is_file() {
        return Err(Stop::Missing(format!("{} is not a file", path.display())));
    }
    let conn =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|err| {
            if path.is_file() {
                Stop::Fail(format!("{}: {err}", path.display()))
            } else {
                Stop::Missing(format!("{} is not a file", path.display()))
            }
        })?;
    conn.busy_timeout(Duration::from_millis(0))
        .map_err(|err| Stop::Fail(err.to_string()))?;
    conn.pragma_update(None, "query_only", true)
        .map_err(|err| Stop::Fail(err.to_string()))?;
    Ok(conn)
}

fn check_schema(conn: &Connection) -> Result<(), Stop> {
    let declared = declared_schema()?;
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|err| Stop::Fail(err.to_string()))?;
    if version != declared.user_version {
        return Err(Stop::Schema(format!(
            "schema mismatch: PRAGMA user_version is {version}, horbm-memory's schema declares {}",
            declared.user_version
        )));
    }
    expect_columns(conn, "items", &declared.items)?;
    expect_columns(conn, "archive", &declared.archive)?;
    Ok(())
}

fn declared_schema() -> Result<&'static DeclaredSchema, Stop> {
    static CELL: OnceLock<Result<DeclaredSchema, String>> = OnceLock::new();
    match CELL.get_or_init(load_declared_schema) {
        Ok(schema) => Ok(schema),
        Err(message) => Err(Stop::Fail(message.clone())),
    }
}

/// Open a throwaway database with `horbm_memory::Store` and read the
/// `CREATE TABLE` text that crate installed. The caller's file is not used.
fn load_declared_schema() -> Result<DeclaredSchema, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = env::temp_dir().join(format!(
        "horbm-memory-audit-schema-{}-{nanos}",
        std::process::id()
    ));
    let guard = RemoveDir(dir);
    fs::create_dir_all(guard.path()).map_err(|err| err.to_string())?;
    let path = guard.path().join("reference.sqlite");
    {
        let store = horbm_memory::Store::open(&path).map_err(|err| err.to_string())?;
        drop(store);
    }
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| err.to_string())?;
    let user_version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|err| err.to_string())?;
    let items = columns_from_create(&create_sql(&conn, "items")?)?;
    let archive = columns_from_create(&create_sql(&conn, "archive")?)?;
    drop(conn);
    Ok(DeclaredSchema {
        user_version,
        items,
        archive,
    })
}

fn create_sql(conn: &Connection, table: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )
    .map_err(|err| format!("horbm-memory installed no CREATE TABLE for {table}: {err}"))
}

fn columns_from_create(sql: &str) -> Result<Vec<(String, String)>, String> {
    let body = table_body(sql)?;
    let mut columns = Vec::new();
    for clause in split_clauses(body) {
        if let Some(column) = column_from_clause(clause)? {
            columns.push(column);
        }
    }
    if columns.is_empty() {
        return Err(format!(
            "horbm-memory CREATE TABLE declares no columns: {sql}"
        ));
    }
    Ok(columns)
}

fn table_body(sql: &str) -> Result<&str, String> {
    let Some(start) = sql.find('(') else {
        return Err(format!(
            "horbm-memory CREATE TABLE has no column list: {sql}"
        ));
    };
    let bytes = sql.as_bytes();
    let mut depth = 0;
    let mut quote: Option<u8> = None;
    let mut i = start;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(mark) = quote {
            if byte == mark {
                if mark == b'\'' && bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                quote = None;
            }
            i += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&sql[start + 1..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(format!(
        "horbm-memory CREATE TABLE column list is not closed: {sql}"
    ))
}

fn split_clauses(body: &str) -> Vec<&str> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(mark) = quote {
            if byte == mark {
                if mark == b'\'' && bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                quote = None;
            }
            i += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => quote = Some(byte),
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b',' if depth == 0 => {
                out.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&body[start..]);
    out
}

fn column_from_clause(clause: &str) -> Result<Option<(String, String)>, String> {
    let clause = clause.trim();
    if clause.is_empty() {
        return Ok(None);
    }
    let Some((name, rest)) = take_ident(clause) else {
        return Err(format!(
            "horbm-memory schema has an unreadable declaration: {clause}"
        ));
    };
    if is_table_constraint(name) {
        return Ok(None);
    }
    let Some((ty, _)) = take_ident(rest) else {
        return Err(format!(
            "horbm-memory schema column {name} has no declared type"
        ));
    };
    Ok(Some((name.to_string(), ty.to_string())))
}

fn take_ident(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if first == '"' || first == '`' || first == '[' {
        let end = if first == '[' { ']' } else { first };
        let close = text[1..].find(end)?;
        return Some((&text[1..1 + close], &text[1 + close + 1..]));
    }
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, ch) in chars {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    Some((&text[..end], &text[end..]))
}

fn is_table_constraint(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "PRIMARY" | "UNIQUE" | "CHECK" | "FOREIGN" | "CONSTRAINT"
    )
}

fn expect_columns(
    conn: &Connection,
    table: &str,
    expected: &[(String, String)],
) -> Result<(), Stop> {
    let found = table_columns(conn, table)?;
    if found.is_empty() {
        return Err(Stop::Schema(format!(
            "schema mismatch: table {table} is missing; horbm-memory's schema declares it"
        )));
    }
    let mut matched = vec![false; expected.len()];
    for (name, ty) in &found {
        let Some(index) = expected.iter().position(|(want, _)| want == name) else {
            return Err(Stop::Schema(format!(
                "schema mismatch: table {table} has column {name} ({ty}), which horbm-memory's schema does not declare"
            )));
        };
        let want_ty = &expected[index].1;
        if !ty.eq_ignore_ascii_case(want_ty) {
            return Err(Stop::Schema(format!(
                "schema mismatch: {table}.{name} type is {ty}, horbm-memory's schema declares {want_ty}"
            )));
        }
        if matched[index] {
            return Err(Stop::Schema(format!(
                "schema mismatch: table {table} lists column {name} twice"
            )));
        }
        matched[index] = true;
    }
    for (index, seen) in matched.iter().enumerate() {
        if !seen {
            let (name, ty) = &expected[index];
            return Err(Stop::Schema(format!(
                "schema mismatch: table {table} has no column {name}; horbm-memory's schema declares {name} {ty}"
            )));
        }
    }
    Ok(())
}

struct RemoveDir(PathBuf);

impl RemoveDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<(String, String)>, Stop> {
    let sql = match table {
        "items" => "PRAGMA table_info(items)",
        "archive" => "PRAGMA table_info(archive)",
        _ => {
            return Err(Stop::Fail(format!(
                "schema check does not know table {table}"
            )))
        }
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| Stop::Fail(err.to_string()))?);
    }
    Ok(out)
}

fn read_items(conn: &Connection, person: Option<&str>) -> Result<Vec<serde_json::Value>, Stop> {
    let mut stmt = conn
        .prepare(
            "SELECT item_id, layer, said_by, source_date, source, pinned, marked, current, body
             FROM items
             WHERE layer IN ('episode', 'diary') AND (?1 IS NULL OR said_by = ?1)
             ORDER BY rowid ASC",
        )
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut rows = stmt
        .query(params![person])
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|err| Stop::Fail(err.to_string()))? {
        out.push(serde_json::json!({
            "kind": "row",
            "item_id": text_at(row, 0)?,
            "layer": text_at(row, 1)?,
            "said_by": text_at(row, 2)?,
            "date": text_at(row, 3)?,
            "source": text_at(row, 4)?,
            "pinned": flag_at(row, 5)?,
            "marked": flag_at(row, 6)?,
            "current": flag_at(row, 7)?,
            "text": text_at(row, 8)?,
        }));
    }
    Ok(out)
}

fn read_archive(conn: &Connection, person: Option<&str>) -> Result<Vec<serde_json::Value>, Stop> {
    let mut stmt = conn
        .prepare(
            "SELECT item_id, layer, said_by, source_date, source, body, reason
             FROM archive
             WHERE (?1 IS NULL OR said_by = ?1)
             ORDER BY rowid ASC",
        )
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut rows = stmt
        .query(params![person])
        .map_err(|err| Stop::Fail(err.to_string()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|err| Stop::Fail(err.to_string()))? {
        out.push(serde_json::json!({
            "kind": "tombstone",
            "item_id": text_at(row, 0)?,
            "layer": text_at(row, 1)?,
            "said_by": text_at(row, 2)?,
            "date": text_at(row, 3)?,
            "source": text_at(row, 4)?,
            "text": text_at(row, 5)?,
            "reason": text_at(row, 6)?,
        }));
    }
    Ok(out)
}

fn text_at(row: &rusqlite::Row<'_>, index: usize) -> Result<String, Stop> {
    row.get(index).map_err(|err| Stop::Fail(err.to_string()))
}

fn flag_at(row: &rusqlite::Row<'_>, index: usize) -> Result<bool, Stop> {
    let value: i64 = row.get(index).map_err(|err| Stop::Fail(err.to_string()))?;
    Ok(value != 0)
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
