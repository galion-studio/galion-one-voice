// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Dev profile, absolute paths, the turn log, and one live session.

use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use horbm::{ClauseSource, EpisodicStore, LearnConfig, LearnPass, PersonMemory};
use samantha_local_server::protocol::{SCHEMA, WS_PATH};
use samantha_local_server::{
    start_with_host, HostConfig, HostSession, HostSources, RefState, ReplyEngine, Stop, TurnLog,
    TurnOut,
};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Sock = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const PERSON_LINE: &str = "my name is Northwind";
const MISS_LINE: &str = "who won the match";
const HIT_LINE: &str = "where is the kettle";
// Content words `spare` and `oar` are not in the shipped kettle card.
// The same clause is what `learned_turn.rs` `teach` writes.
const TAUGHT_LINE: &str = "You remember the spare oar is in the locker.";
// Both tokens sit in `TAUGHT_LINE` and in no shipped card.
const RECALL_ASK: &str = "spare oar";

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "galion-host-{label}-{n}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn shipped_profile() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dev.env")).unwrap()
}

fn shipped_knowledge() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

fn kettle_reply() -> String {
    let text = fs::read_to_string(shipped_knowledge().join("kettle.md")).unwrap();
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with("The kettle"))
        .expect("shipped kettle sentence")
        .to_string()
}

#[allow(clippy::too_many_arguments)]
fn resolve<'a>(
    dev: bool,
    learn: Option<&'a str>,
    remember: Option<&'a str>,
    memory_dir: Option<&'a str>,
    data_root: Option<&'a str>,
    learn_pack: Option<&'a str>,
    turn_log: Option<&'a str>,
    profile_text: Option<&'a str>,
    xdg_data_home: Option<&'a str>,
    home: Option<&'a str>,
) -> Result<HostConfig, String> {
    HostConfig::resolve(HostSources {
        dev,
        learn,
        remember,
        memory_dir,
        data_root,
        learn_pack,
        turn_log,
        turn_log_cap: None,
        profile_text,
        xdg_data_home,
        home,
    })
}

fn dev_under<'a>(root: &'a Path, profile: &'a str) -> HostConfig {
    resolve(
        true,
        None,
        None,
        None,
        Some(root.to_str().unwrap()),
        None,
        None,
        Some(profile),
        None,
        None,
    )
    .unwrap()
}

#[test]
fn dev_profile_defaults_learn_and_remember_to_grant() {
    let root = scratch("dev-grant");
    let profile = shipped_profile();
    let cfg = dev_under(&root, &profile);
    assert!(cfg.dev);
    assert!(cfg.learn.granted, "{}", cfg.learn.source);
    assert!(cfg.remember.granted, "{}", cfg.remember.source);
    assert_eq!(cfg.learn.word, "grant");
    assert_eq!(cfg.remember.word, "grant");
    assert!(cfg.learn.source.contains("dev profile"));
    assert!(cfg.remember.source.contains("dev profile"));
    assert!(cfg.learn_config.granted);
    assert!(cfg.retrieval.remember);
    println!("{}", cfg.startup_line());
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    println!("{}", host.startup_line());
    assert!(
        root.join("memory").is_dir(),
        "a grant creates the memory dir"
    );
    host.finish();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn operator_deny_wins_in_dev() {
    let root = scratch("dev-deny");
    let root_s = root.to_str().unwrap();
    let err_profile = "GALION_HORBM_LEARN=grant\nGALION_HORBM_REMEMBER=grant\n";
    let cfg = resolve(
        true,
        Some("deny"),
        Some("nope"),
        None,
        Some(root_s),
        None,
        None,
        Some(err_profile),
        None,
        None,
    )
    .unwrap();
    assert!(!cfg.learn.granted, "{}", cfg.learn.source);
    assert!(!cfg.remember.granted, "{}", cfg.remember.source);
    assert!(cfg.learn.source.contains("operator"));
    assert!(cfg.remember.source.contains("operator"));
    assert!(!cfg.learn_config.granted);
    assert!(!cfg.retrieval.remember);
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    assert!(
        !root.join("memory").exists(),
        "deny does not create the memory dir"
    );
    host.finish();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn outside_dev_both_stay_off_unless_granted() {
    let root = scratch("off");
    let root_s = root.to_str().unwrap();
    let profile = "GALION_HORBM_LEARN=grant\nGALION_HORBM_REMEMBER=grant\n";
    let off = resolve(
        false,
        None,
        None,
        None,
        Some(root_s),
        None,
        None,
        Some(profile),
        None,
        None,
    )
    .unwrap();
    assert!(!off.dev);
    assert!(!off.learn.granted, "{}", off.learn.source);
    assert!(!off.remember.granted, "{}", off.remember.source);
    assert!(off.learn.source.contains("unset"));
    assert!(off.remember.source.contains("unset"));
    let learn_only = resolve(
        false,
        Some("grant"),
        None,
        None,
        Some(root_s),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(learn_only.learn.granted);
    assert!(!learn_only.remember.granted);
    assert!(learn_only.learn.source.contains("operator"));
    let both = resolve(
        false,
        Some("grant"),
        Some(" grant "),
        None,
        Some(root_s),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(both.learn.granted && both.remember.granted);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn relative_memory_dir_is_refused() {
    let root = scratch("rel");
    let root_s = root.to_str().unwrap();
    let memory = resolve(
        true,
        None,
        None,
        Some("memory"),
        Some(root_s),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(memory.contains("absolute"), "{memory}");
    assert!(memory.contains("GALION_HORBM_MEMORY_DIR"), "{memory}");
    let data = resolve(
        false,
        None,
        None,
        None,
        Some("var/horbm"),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(data.contains("absolute"), "{data}");
    let pack = resolve(
        true,
        None,
        None,
        None,
        Some(root_s),
        Some("learn"),
        None,
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(pack.contains("GALION_HORBM_LEARN_PACK"), "{pack}");
    let log = resolve(
        true,
        None,
        None,
        None,
        Some(root_s),
        None,
        Some("logs/turns.jsonl"),
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(log.contains("GALION_HORBM_TURN_LOG"), "{log}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_memory_dir_is_absolute_under_the_data_root() {
    let root = scratch("root");
    let cfg = resolve(
        false,
        None,
        None,
        None,
        Some(root.to_str().unwrap()),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(cfg.data_root.is_absolute());
    assert_eq!(cfg.memory_dir, root.join("memory"));
    assert!(cfg.memory_dir.is_absolute());
    assert_eq!(
        cfg.diary_path,
        root.join("memory").join("person-memory.sqlite")
    );
    assert_eq!(cfg.learn_pack, root.join("memory").join("learn"));
    assert!(cfg.learn_pack.is_absolute());
    assert_eq!(cfg.turn_log, root.join("logs").join("turns.jsonl"));
    assert!(cfg.turn_log.is_absolute());
    let xdg = scratch("xdg");
    let from_xdg = resolve(
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(xdg.to_str().unwrap()),
        None,
    )
    .unwrap();
    let xdg_root = xdg.join("galion").join("horbm");
    assert_eq!(from_xdg.data_root, xdg_root);
    assert_eq!(from_xdg.memory_dir, xdg_root.join("memory"));
    assert!(from_xdg.memory_dir.is_absolute());
    #[cfg(target_os = "linux")]
    {
        let home = scratch("home");
        let from_home = resolve(
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(""),
            Some(home.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(from_home.data_root, home.join(".local/share/galion/horbm"));
        assert!(from_home.memory_dir.is_absolute());
        let _ = fs::remove_dir_all(&home);
    }
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&xdg);
}

#[test]
fn turn_log_rotates_at_the_cap_and_never_holds_audio() {
    let root = scratch("rotate");
    let path = root.join("turns.jsonl");
    let log = TurnLog::open(&path, 180, 5).unwrap();
    for i in 0..40 {
        log.record_blocking(format!(
            r#"{{"ts":"t","session":"s","kind":"heard","turn":{i},"text":"kettle note {i}"}}"#
        ));
    }
    log.close();
    assert!(path.is_file(), "live log missing");
    assert!(rotated(&path, 1).is_file(), "rotation did not keep .1");
    assert!(rotated(&path, 5).is_file(), "rotation did not keep .5");
    assert!(
        !rotated(&path, 6).exists(),
        "kept more than five rotated files"
    );
    let live_len = fs::metadata(&path).unwrap().len();
    assert!(live_len < 180 + 120, "live log {live_len} is past the cap");
    for n in 0..=5 {
        let file = if n == 0 {
            path.clone()
        } else {
            rotated(&path, n)
        };
        if !file.is_file() {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("pcm"), "{}", file.display());
        assert!(!text.contains("RIFF"), "{}", file.display());
        assert!(!lower.contains("embedding"), "{}", file.display());
        assert!(!lower.contains("password"), "{}", file.display());
        for line in text.lines() {
            let value: Value = serde_json::from_str(line).unwrap();
            assert_eq!(value["kind"], "heard");
            assert!(value.get("audio").is_none());
        }
    }
    let _ = fs::remove_dir_all(&root);
}

fn rotated(path: &Path, n: usize) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{n}"));
    path.with_file_name(name)
}

#[test]
fn miss_then_teaching_line_is_the_audit_row() {
    let root = scratch("taught");
    let cfg = resolve(
        false,
        Some("grant"),
        Some("deny"),
        None,
        Some(root.to_str().unwrap()),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(cfg.learn.granted);
    assert!(!cfg.remember.granted, "a miss has to enter gaps");
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    let teaching = "Northwind won the match";
    for line in [MISS_LINE, teaching] {
        let out = host
            .mind()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .turn(line)
            .unwrap();
        host.on_turn(line, &out);
    }
    host.finish();
    let pack = root.join("memory").join("learn");
    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("learn audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    assert_eq!(audit.rows[0].text, teaching);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_sealed_pack_is_spoken_after_boot() {
    let root = scratch("sealed");
    let pack = root.join("memory").join("learn");
    write_taught(&pack, TAUGHT_LINE);
    let profile = shipped_profile();
    let host = HostSession::boot(dev_under(&root, &profile), shipped_knowledge()).unwrap();
    let line = host.startup_line();
    assert!(line.ends_with("learned_rows=1"), "{line}");
    let out = ask(&host, RECALL_ASK);
    assert_eq!(out.reply, TAUGHT_LINE);
    assert!(!out.sources.is_empty(), "{:?}", out.sources);
    assert!(out.sources[0].starts_with("learned:"), "{:?}", out.sources);
    host.finish();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn learn_denied_attaches_nothing() {
    let root = scratch("learn-deny");
    let pack = root.join("memory").join("learn");
    write_taught(&pack, TAUGHT_LINE);
    let before = pack_snapshot(&pack);
    assert!(!before.is_empty(), "the pack has bytes to compare");
    let cfg = resolve(
        false,
        Some("deny"),
        Some("grant"),
        None,
        Some(root.to_str().unwrap()),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(!cfg.learn.granted);
    assert!(cfg.remember.granted);
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    let line = host.startup_line();
    assert!(line.ends_with("learned_rows=0"), "{line}");
    let out = ask(&host, RECALL_ASK);
    assert_eq!(out.reply, "I don't know.");
    host.finish();
    assert_eq!(pack_snapshot(&pack), before);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remember_denied_attaches_nothing() {
    let root = scratch("remember-deny");
    let pack = root.join("memory").join("learn");
    write_taught(&pack, TAUGHT_LINE);
    let cfg = resolve(
        false,
        Some("grant"),
        Some("deny"),
        None,
        Some(root.to_str().unwrap()),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(cfg.learn.granted);
    assert!(!cfg.remember.granted);
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    let line = host.startup_line();
    assert!(line.ends_with("learned_rows=0"), "{line}");
    let out = ask(&host, RECALL_ASK);
    assert_eq!(out.reply, "I don't know.");
    host.finish();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn two_sessions_recall_the_taught_line() {
    let root = scratch("two");
    let profile = shipped_profile();
    let host = HostSession::boot(dev_under(&root, &profile), shipped_knowledge()).unwrap();
    for line in [MISS_LINE, TAUGHT_LINE] {
        let out = ask(&host, line);
        host.on_turn(line, &out);
    }
    host.finish();
    let pack = root.join("memory").join("learn");
    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("learn audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    // Remember is granted, so the miss window stays shut. The sealed row is
    // the last noted clause. The reply below is this audit text.
    let sealed = audit.rows[0].text.clone();
    assert_eq!(sealed, TAUGHT_LINE);
    let host = HostSession::boot(dev_under(&root, &profile), shipped_knowledge()).unwrap();
    let line = host.startup_line();
    assert!(line.ends_with("learned_rows=1"), "{line}");
    // `spare` and `oar` are content tokens of the sealed clause. They are
    // not in the shipped kettle card, and the ask is not about the person,
    // so the learned row is what the turn speaks.
    let lower = sealed.to_ascii_lowercase();
    assert!(lower.contains("spare") && lower.contains("oar"), "{sealed}");
    let out = ask(&host, RECALL_ASK);
    assert_eq!(out.reply, sealed);
    assert!(!out.sources.is_empty(), "{:?}", out.sources);
    assert!(out.sources[0].starts_with("learned:"), "{:?}", out.sources);
    host.finish();
    let _ = fs::remove_dir_all(&root);
}

fn write_taught(pack: &Path, text: &str) {
    let learn = LearnPass::open(LearnConfig::granted(pack));
    let source = ClauseSource {
        session: "sess-learn".to_string(),
        turn_id: 4,
        time: "2026-11-26T00:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    };
    learn
        .write_clause("user", text, &source)
        .expect("write clause");
}

fn ask(host: &HostSession, heard: &str) -> TurnOut {
    host.mind()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .turn(heard)
        .unwrap()
}

fn pack_snapshot(pack: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![pack.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(pack)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            out.push((rel, fs::read(&path).unwrap()));
        }
    }
    out.sort();
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_dev_session() {
    let root = scratch("e2e");
    let profile = shipped_profile();
    let cfg = dev_under(&root, &profile);
    println!("{}", cfg.startup_line());
    let host = HostSession::boot(cfg, shipped_knowledge()).unwrap();
    println!("{}", host.startup_line());
    let running = start_with_host(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        ui_dir(),
        host,
        RefState::Unset,
        false,
        Duration::from_secs(120),
    )
    .await
    .unwrap();
    let saw_inner = converse(running.addr).await;
    assert!(saw_inner, "the socket did not carry an inner frame");
    running.shutdown().await;
    assert_closed_session(&root);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigterm_closes_the_session() {
    if std::env::var("GALION_HORBM_SIGTERM_CHILD").ok().as_deref() == Some("1") {
        run_sigterm_child().await;
        return;
    }
    let root = scratch("term");
    let ready = root.join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("sigterm_closes_the_session")
        .arg("--exact")
        .env("GALION_HORBM_SIGTERM_CHILD", "1")
        .env("GALION_HORBM_PROFILE", "dev")
        .env("GALION_HORBM_DATA_ROOT", &root)
        .env("GALION_HORBM_READY_FILE", &ready)
        .env_remove("GALION_HORBM_LEARN")
        .env_remove("GALION_HORBM_REMEMBER")
        .env_remove("GALION_HORBM_MEMORY_DIR")
        .env_remove("GALION_HORBM_LEARN_PACK")
        .env_remove("GALION_HORBM_TURN_LOG")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let addr = wait_ready(&ready, &mut child);
    let saw_inner = converse(addr).await;
    assert!(saw_inner, "the child socket did not carry an inner frame");
    let status = Command::new("kill")
        .args(["-s", "TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(status.success(), "SIGTERM was not delivered");
    let status = wait_child(&mut child, Duration::from_secs(20));
    assert!(status.success(), "child exit {status}");
    assert_closed_session(&root);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
async fn run_sigterm_child() {
    let config = HostConfig::resolve_process(false).expect("child profile");
    println!("{}", config.startup_line());
    let host = HostSession::boot(config, shipped_knowledge()).unwrap();
    let running = start_with_host(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        ui_dir(),
        host,
        RefState::Unset,
        false,
        Duration::from_secs(120),
    )
    .await
    .unwrap();
    let stop = Stop::install().unwrap();
    let ready = std::env::var("GALION_HORBM_READY_FILE").unwrap();
    fs::write(&ready, running.addr.to_string()).unwrap();
    stop.wait().await;
    // The seal runs in `shutdown`. A runtime worker can still be parked
    // after that, and the parent only waits 20 s. Leave once the seal has
    // had this long, so a parked worker cannot hold the process open.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(15));
        std::process::exit(0);
    });
    running.shutdown().await;
}

#[cfg(unix)]
fn wait_ready(path: &Path, child: &mut std::process::Child) -> SocketAddr {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("child exited before it was ready: {status}");
        }
        if let Ok(text) = fs::read_to_string(path) {
            let text = text.trim();
            if !text.is_empty() {
                return text.parse().unwrap();
            }
        }
        if Instant::now() > deadline {
            let _ = Command::new("kill")
                .args(["-s", "TERM", &child.id().to_string()])
                .status();
            panic!("child did not write its listen address");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn wait_child(child: &mut std::process::Child, limit: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > deadline {
            let _ = Command::new("kill")
                .args(["-s", "KILL", &child.id().to_string()])
                .status();
            panic!("child did not exit after SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn assert_closed_session(root: &Path) {
    let kettle = kettle_reply();
    let lines = read_log(root);
    println!("--- jsonl ---");
    for line in &lines {
        println!("{line}");
    }
    let values: Vec<Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let kinds: Vec<&str> = values
        .iter()
        .filter_map(|value| match value["kind"].as_str() {
            Some("mood") => None,
            other => other,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "heard",
            "reply",
            "heard",
            "reply",
            "heard",
            "reply",
            "inner",
            "session_end"
        ]
    );
    // Remember is granted here, so a retrieval miss never enters `Brain::gaps`.
    // A gap line, when a later turn does record one, still has to name its
    // reason and must not carry a withheld secret.
    assert!(values
        .iter()
        .filter(|value| value["kind"] == "gap")
        .all(|gap| {
            let reason = gap["reason"].as_str().unwrap_or("");
            let heard = gap["heard"].as_str().unwrap_or("");
            !reason.is_empty()
                && heard != "withheld"
                && !gap.to_string().to_ascii_lowercase().contains("password")
                && !gap.to_string().contains("withheld")
        }));
    let replies: Vec<&str> = values
        .iter()
        .filter(|value| value["kind"] == "reply")
        .map(|value| value["text"].as_str().unwrap())
        .collect();
    assert_eq!(replies[0], kettle);
    assert_eq!(replies[1], "I don't know.");
    let heard: Vec<&str> = values
        .iter()
        .filter(|value| value["kind"] == "heard")
        .map(|value| value["text"].as_str().unwrap())
        .collect();
    assert_eq!(heard, [HIT_LINE, MISS_LINE, PERSON_LINE]);
    let inner = values
        .iter()
        .find(|value| value["kind"] == "inner")
        .unwrap();
    assert!(inner["verb"].as_str().unwrap().is_ascii());
    assert!(inner["slots"].is_array());
    assert!(inner.get("text").is_none());
    let mut previous: Option<(u64, u64, u64)> = None;
    for mood in values.iter().filter(|value| value["kind"] == "mood") {
        let key = (
            mood["arousal"].as_f64().unwrap().to_bits(),
            mood["valence"].as_f64().unwrap().to_bits(),
            mood["dominance"].as_f64().unwrap().to_bits(),
        );
        assert_ne!(previous, Some(key));
        previous = Some(key);
    }
    let diary = root.join("memory").join("person-memory.sqlite");
    assert!(diary.is_file(), "diary file missing");
    // `end_session` writes the session summary into the diary items and the
    // buffered heard lines into `session_turns`. `recall` reads that file.
    let saved =
        EpisodicStore::read_transcript_file(&diary, "Northwind").expect("reopen session_turns");
    assert!(
        saved.rows > 0,
        "session_turns rows {}, ring {}, turns {:?}",
        saved.rows,
        saved.ring_len,
        saved.turns
    );
    assert!(
        saved
            .turns
            .iter()
            .any(|turn| turn.text.contains(PERSON_LINE)),
        "session_turns missing the person line: {:?}",
        saved.turns
    );
    let mut store = EpisodicStore::open(&diary, true);
    let hits = store.recall("person", "Northwind");
    assert!(
        hits.iter().any(|hit| hit.text.contains("Northwind")),
        "diary did not keep the person line: {hits:?}"
    );
    let pack = root.join("memory").join("learn");
    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("learn audit");
    assert_eq!(audit.rows.len(), 1, "{:?}", audit.rows);
    // PERSON_LINE does not cover MISS_LINE (`won` and `match` are absent), so
    // `note_taught` returns false and the host falls back to `note`. Remember
    // is granted here, so the miss never enters `Brain::gaps` and that window
    // stays shut. Either way the sealed row is this last noted line.
    assert_eq!(audit.rows[0].text, PERSON_LINE);
    assert!(
        audit.rows[0].source.contains("dev profile"),
        "{}",
        audit.rows[0].source
    );
    assert_eq!(audit.archived, 0);
    println!(
        "sqlite diary holds the person line; learn rows {}",
        audit.rows.len()
    );
}

fn read_log(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for n in (1..=5).rev() {
        let path = rotated(&root.join("logs").join("turns.jsonl"), n);
        if path.is_file() {
            files.push(path);
        }
    }
    files.push(root.join("logs").join("turns.jsonl"));
    let mut lines = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("pcm"), "{}", path.display());
        assert!(!text.contains("RIFF"), "{}", path.display());
        assert!(!lower.contains("embedding"), "{}", path.display());
        for line in text.lines() {
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
    }
    lines
}

async fn converse(addr: SocketAddr) -> bool {
    let mut ws = connect(addr).await;
    send_text(
        &mut ws,
        &format!(r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#),
    )
    .await;
    let welcome = next_text(&mut ws, Duration::from_secs(3))
        .await
        .expect("welcome");
    assert_eq!(welcome["type"], "welcome");
    for line in [HIT_LINE, MISS_LINE, PERSON_LINE] {
        say(&mut ws, line).await;
    }
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw_inner = false;
    while Instant::now() < deadline {
        let Some(msg) = recv(&mut ws, Duration::from_millis(500)).await else {
            continue;
        };
        if let Message::Text(text) = msg {
            let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            if value["type"] == "inner" {
                assert!(value["verb"].is_string());
                assert!(value["slots"].is_array());
                assert!(value.get("reply").is_none());
                saw_inner = true;
                break;
            }
        }
    }
    drop(ws);
    saw_inner
}

async fn say(ws: &mut Sock, text: &str) {
    send_text(
        ws,
        &format!(r#"{{"schema":"{SCHEMA}","type":"text","text":"{text}"}}"#),
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut saw_turn = false;
    while Instant::now() < deadline {
        let Some(msg) = recv(ws, Duration::from_millis(800)).await else {
            if saw_turn {
                break;
            }
            continue;
        };
        if let Message::Text(body) = msg {
            let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            if value["type"] == "turn" {
                saw_turn = true;
            }
            if value["type"] == "dot" && value["state"] == "idle" && saw_turn {
                break;
            }
        }
    }
    assert!(saw_turn, "no turn for {text}");
}

async fn connect(addr: SocketAddr) -> Sock {
    let url = format!("ws://{addr}{}", WS_PATH);
    let (ws, _) = connect_async(url).await.expect("websocket");
    ws
}

async fn send_text(ws: &mut Sock, text: &str) {
    ws.send(Message::Text(text.into())).await.unwrap();
}

async fn next_text(ws: &mut Sock, timeout: Duration) -> Option<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let Some(msg) = recv(ws, Duration::from_millis(500)).await else {
            continue;
        };
        if let Message::Text(text) = msg {
            return serde_json::from_str(&text).ok();
        }
    }
    None
}

async fn recv(ws: &mut Sock, timeout: Duration) -> Option<Message> {
    match tokio::time::timeout(timeout, ws.next()).await {
        Ok(Some(Ok(msg))) => Some(msg),
        _ => None,
    }
}
