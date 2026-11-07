// Galion Voice — Perpetual Open MIT License
// RG offline gate. Std only: no network, no weights, no microphone.
// A row is PASS only when a command this process ran exited 0.
// A row that was not run stays BLOCKED. WER, ECAPA, MOS, latency, and TTFA
// stay BLOCKED unless a witness file from that command contains a line `exit=0`.
// This process does not write those witnesses and does not emit those numbers.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Pass,
    Fail,
    Blocked,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Blocked => "BLOCKED",
        }
    }
}

/// Scores that this gate must not invent. A PASS requires a witness line `exit=0`.
const METRICS: &[&str] = &["wer", "ecapa", "mos", "latency", "ttfa"];

const UNRUN: &[&str] = &[
    "wer",
    "ecapa",
    "mos",
    "latency",
    "ttfa",
    "microphone",
    "weights",
    "network",
];

struct Claim<'a> {
    name: &'a str,
    exit: Option<i32>,
    witness: Option<&'a Path>,
}

struct Row {
    name: &'static str,
    status: Status,
    exit: Option<i32>,
}

fn is_metric(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    METRICS.iter().any(|m| {
        n == *m || n.starts_with(&format!("{m}_")) || n.starts_with(&format!("{m}-"))
    })
}

/// Read `exit=<integer>` from a witness. Only the exact token `0` is a pass.
/// Leading zeros (`00`, `01`) are not a recorded zero exit.
fn witness_exit(path: &Path) -> Option<i32> {
    let text = fs::read_to_string(path).ok()?;
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("exit=") else {
            continue;
        };
        let rest = rest.trim();
        if rest == "0" {
            found = Some(0);
            continue;
        }
        let negative = rest.starts_with('-');
        let digits = if negative { &rest[1..] } else { rest };
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.starts_with('0')
        {
            return None;
        }
        found = rest.parse::<i32>().ok();
        if found.is_none() {
            return None;
        }
    }
    found
}

fn classify(claim: &Claim<'_>) -> Status {
    if is_metric(claim.name) {
        let Some(path) = claim.witness else {
            return Status::Blocked;
        };
        return match witness_exit(path) {
            Some(0) => Status::Pass,
            Some(_) => Status::Fail,
            None => Status::Blocked,
        };
    }
    match claim.exit {
        Some(0) => Status::Pass,
        Some(_) => Status::Fail,
        None => Status::Blocked,
    }
}

fn scratch_dir() -> Result<PathBuf, String> {
    let base = env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("galion-gates/.build"));
    let dir = base.join(format!("galion-rg-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn policy_self_check() -> Result<(), String> {
    if classify(&Claim {
        name: "wer",
        exit: Some(0),
        witness: None,
    }) != Status::Blocked
    {
        return Err("wer with exit 0 and no witness must stay BLOCKED".into());
    }
    if classify(&Claim {
        name: "wer_mean",
        exit: Some(0),
        witness: None,
    }) != Status::Blocked
    {
        return Err("wer_mean must stay BLOCKED without a witness".into());
    }
    if classify(&Claim {
        name: "honesty-contract",
        exit: None,
        witness: None,
    }) != Status::Blocked
    {
        return Err("a missing exit must stay BLOCKED".into());
    }
    if classify(&Claim {
        name: "honesty-contract",
        exit: Some(0),
        witness: None,
    }) != Status::Pass
    {
        return Err("exit 0 on a ran command must be PASS".into());
    }
    if classify(&Claim {
        name: "honesty-contract",
        exit: Some(1),
        witness: None,
    }) != Status::Fail
    {
        return Err("a nonzero exit must be FAIL".into());
    }

    let dir = scratch_dir()?;
    let ok_path = dir.join("ok.txt");
    let bad_path = dir.join("bad.txt");
    let num_path = dir.join("num.txt");
    let zero_path = dir.join("zero.txt");
    let empty_path = dir.join("empty.txt");
    let missing = dir.join("missing.txt");
    let write = |path: &Path, body: &str| fs::write(path, body).map_err(|e| e.to_string());
    write(&ok_path, "exit=0\n")?;
    write(&bad_path, "measured=true\nexit=1\n")?;
    write(&num_path, "wer=0.01\necapa=0.70\nmos=4.0\n")?;
    write(&zero_path, "exit=00\n")?;
    write(&empty_path, "")?;

    let cases = [
        (
            classify(&Claim {
                name: "wer",
                exit: None,
                witness: Some(&ok_path),
            }),
            Status::Pass,
            "witness exit 0",
        ),
        (
            classify(&Claim {
                name: "ECAPA",
                exit: Some(0),
                witness: Some(&bad_path),
            }),
            Status::Fail,
            "witness exit 1",
        ),
        (
            classify(&Claim {
                name: "mos",
                exit: Some(0),
                witness: Some(&num_path),
            }),
            Status::Blocked,
            "numbers without exit",
        ),
        (
            classify(&Claim {
                name: "latency",
                exit: Some(0),
                witness: Some(&missing),
            }),
            Status::Blocked,
            "missing witness",
        ),
        (
            classify(&Claim {
                name: "latency_ms",
                exit: Some(0),
                witness: Some(&zero_path),
            }),
            Status::Blocked,
            "leading zero is not exit 0",
        ),
        (
            classify(&Claim {
                name: "ttfa",
                exit: Some(0),
                witness: Some(&empty_path),
            }),
            Status::Blocked,
            "empty witness",
        ),
        (
            classify(&Claim {
                name: "ttfa",
                exit: None,
                witness: None,
            }),
            Status::Blocked,
            "ttfa not run",
        ),
        (
            classify(&Claim {
                name: "microphone",
                exit: None,
                witness: None,
            }),
            Status::Blocked,
            "microphone not run",
        ),
        (
            classify(&Claim {
                name: "weights",
                exit: None,
                witness: None,
            }),
            Status::Blocked,
            "weights not run",
        ),
        (
            classify(&Claim {
                name: "network",
                exit: None,
                witness: None,
            }),
            Status::Blocked,
            "network not run",
        ),
    ];
    for (got, want, label) in cases {
        if got != want {
            let _ = fs::remove_dir_all(&dir);
            return Err(format!(
                "{label}: got {} want {}",
                got.as_str(),
                want.as_str()
            ));
        }
    }
    let again = classify(&Claim {
        name: "mos",
        exit: Some(0),
        witness: Some(&num_path),
    });
    if again != Status::Blocked {
        let _ = fs::remove_dir_all(&dir);
        return Err("classify was not deterministic".into());
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

fn ledger(honesty_exit: i32) -> Vec<Row> {
    let mut rows = vec![Row {
        name: "honesty-contract",
        status: classify(&Claim {
            name: "honesty-contract",
            exit: Some(honesty_exit),
            witness: None,
        }),
        exit: Some(honesty_exit),
    }];
    for name in UNRUN {
        rows.push(Row {
            name,
            status: classify(&Claim {
                name,
                exit: None,
                witness: None,
            }),
            exit: None,
        });
    }
    rows
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn render(rows: &[Row]) -> String {
    let fails = rows.iter().filter(|r| r.status == Status::Fail).count();
    let mut body = format!(
        "{{\"gate\":\"RG\",\"used_network\":false,\"used_weights\":false,\"used_microphone\":false,\"fails\":{fails},\"rows\":["
    );
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            body.push(',');
        }
        body.push_str(&format!(
            "{{\"name\":\"{}\",\"status\":\"{}\"",
            json_escape(row.name),
            row.status.as_str()
        ));
        if let Some(code) = row.exit {
            body.push_str(&format!(",\"exit\":{code}"));
        }
        body.push('}');
    }
    body.push_str("]}");
    body
}

fn invented_number(json: &str) -> bool {
    const KEYS: &[&str] = &[
        "\"wer\":",
        "\"ecapa\":",
        "\"mos\":",
        "\"latency\":",
        "\"ttfa\":",
        "wer_mean",
        "ecapa_mean",
        "latency_ms",
        "ttfa_ms",
        "latency_us",
    ];
    KEYS.iter().any(|k| json.contains(k))
}

fn main() -> ExitCode {
    let err = policy_self_check().err();
    let honesty_exit = if err.is_none() { 0 } else { 1 };
    let rows = ledger(honesty_exit);
    let body = render(&rows);
    println!("{body}");
    if invented_number(&body) {
        let _ = writeln!(
            io::stderr(),
            "honesty-contract: ledger emitted a metric number"
        );
        return ExitCode::from(1);
    }
    if let Some(e) = err {
        let _ = writeln!(io::stderr(), "honesty-contract: {e}");
        return ExitCode::from(1);
    }
    if rows.iter().any(|r| r.status == Status::Fail) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_accepts_only_a_recorded_zero_exit() {
        policy_self_check().unwrap();
    }

    #[test]
    fn default_ledger_blocks_unmeasured_metrics() {
        let rows = ledger(0);
        let json = render(&rows);
        for name in ["wer", "ecapa", "mos", "latency", "ttfa", "microphone", "weights", "network"]
        {
            assert!(
                json.contains(&format!("\"name\":\"{name}\",\"status\":\"BLOCKED\"")),
                "{json}"
            );
        }
        assert!(json.contains("\"name\":\"honesty-contract\",\"status\":\"PASS\",\"exit\":0"));
        assert!(!invented_number(&json), "{json}");
        assert!(json.contains("\"used_network\":false"));
        assert!(json.contains("\"used_weights\":false"));
        assert!(json.contains("\"used_microphone\":false"));
        assert!(json.contains("\"fails\":0"));
    }

    #[test]
    fn nonzero_honesty_exit_is_fail_and_metrics_stay_blocked() {
        let rows = ledger(2);
        assert_eq!(rows[0].status, Status::Fail);
        assert!(rows[1..].iter().all(|r| r.status == Status::Blocked));
        let json = render(&rows);
        assert!(json.contains("\"fails\":1"));
        assert!(!invented_number(&json), "{json}");
    }
}
