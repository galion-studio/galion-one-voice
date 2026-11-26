// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Long granted run of the real host.
//!
//! Boots the dev profile the way `end_to_end_dev_session` does, then sends
//! `GALION_HOST_SOAK_TURNS` text lines (default 1500) on the socket. `VmRSS`
//! is marked after 300 turns. Growth after that mark is held to 128 KiB, the
//! same number as `horbm-soak`'s `RSS_GROWTH_KIB`.
//!
//! Ignored. Laptop:
//! `cargo test --manifest-path voice-stack/local-server/Cargo.toml --test host_soak -- --ignored --nocapture`
//!
//! Shutdown must leave the diary file, the sealed learn pack, and a turn log
//! that holds the miss, the pin, and the session end. A granted retrieval
//! miss does not enter `Brain::gaps` on this base, so a gap row is counted
//! and printed. Every gap row that is present names its reason.
//!
//! `VmRSS` comes from `/proc/self/status`. On any other target this test
//! compiles to a no-op and prints a note.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "long: cargo test --test host_soak -- --ignored"]
async fn granted_host_soak_rss_stays_at_the_soak_gate() {
    #[cfg(target_os = "linux")]
    soak::run().await;
    #[cfg(not(target_os = "linux"))]
    {
        println!(
            "VmRSS is read from /proc/self/status on Linux; this target does not run the soak"
        );
    }
}

#[cfg(target_os = "linux")]
mod soak {
    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use futures_util::{SinkExt, StreamExt};
    use horbm::{LEARN_PACK_DB, LEARN_PACK_LIVE};
    use samantha_local_server::protocol::{SCHEMA, WS_PATH};
    use samantha_local_server::{start_with_host, HostConfig, HostSession, HostSources, RefState};
    use serde_json::Value;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

    type Sock = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    /// Same number as `horbm-soak`'s `RSS_GROWTH_KIB`. Not a second gate.
    const RSS_GROWTH_KIB: u64 = 128;
    const MARK_AFTER: u64 = 300;
    const DEFAULT_TURNS: u64 = 1500;

    /// Card hit, miss, a statement that teaches that miss, the pin, the recall, a forget.
    const LINES: [&str; 6] = [
        "where is the kettle",
        "who won the match",
        "northwind won the match",
        "remember that my favourite colour is blue",
        "do you remember my favourite colour",
        "please forget my favourite colour",
    ];

    pub async fn run() {
        let turns = turn_count();
        let root = scratch();
        println!("host soak root {}", root.display());
        let started = Instant::now();
        let profile = fs::read_to_string(manifest("dev.env")).unwrap();
        let cfg = dev_under(&root, &profile);
        assert!(cfg.dev, "{}", cfg.startup_line());
        assert!(cfg.learn.granted, "{}", cfg.learn.source);
        assert!(cfg.remember.granted, "{}", cfg.remember.source);
        println!("{}", cfg.startup_line());
        let diary = cfg.diary_path.clone();
        let pack = cfg.learn_pack.clone();
        let turn_log = cfg.turn_log.clone();
        let host = HostSession::boot(cfg, manifest("knowledge")).expect("boot host");
        println!("{}", host.startup_line());
        let running = start_with_host(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            manifest("ui"),
            host,
            RefState::Unset,
            false,
            Duration::from_secs(120),
        )
        .await
        .expect("listen");
        let mut ws = connect(running.addr).await;
        hello(&mut ws).await;

        let mut rss_buf = String::new();
        let mut mark: Option<u64> = None;
        let mut peak = 0u64;
        let mut end = 0u64;
        for n in 1..=turns {
            let index = usize::try_from(n - 1).expect("turn") % LINES.len();
            say(&mut ws, LINES[index]).await;
            if n < MARK_AFTER {
                continue;
            }
            let kib =
                read_rss(&mut rss_buf).unwrap_or_else(|err| panic!("VmRSS after turn {n}: {err}"));
            if mark.is_none() {
                mark = Some(kib);
            }
            peak = peak.max(kib);
            end = kib;
            if n == MARK_AFTER || n == turns {
                println!("host soak turn {n} rss {kib} KiB");
            }
        }
        drop(ws);
        let mark = mark.expect("rss mark");
        let wall_s = started.elapsed().as_secs_f64();
        let growth = end.saturating_sub(mark);
        let peak_growth = peak.saturating_sub(mark);
        let summary = format!(
            "turns={turns} wall={wall_s:.3}s rss_mark={mark} KiB rss_peak={peak} KiB rss_end={end} KiB growth={growth} KiB peak_growth={peak_growth} KiB gate={RSS_GROWTH_KIB} KiB"
        );

        running.shutdown().await;
        let live = pack.join(LEARN_PACK_LIVE).join(LEARN_PACK_DB);
        assert!(
            diary.is_file(),
            "diary missing {}; {summary}",
            diary.display()
        );
        assert!(
            fs::metadata(&diary).expect("diary metadata").len() > 0,
            "diary is empty; {summary}"
        );
        assert!(
            pack.is_dir(),
            "learn pack missing {}; {summary}",
            pack.display()
        );
        assert!(
            live.is_file(),
            "learn pack was not sealed {}; {summary}",
            live.display()
        );
        assert!(
            fs::metadata(&live).expect("learn pack metadata").len() > 0,
            "learn pack is empty; {summary}"
        );
        println!("host soak: {summary}");
        let _ = std::io::stdout().flush();
        let gaps = count_gaps(&turn_log);
        println!("host soak gaps={gaps}");
        assert!(
            growth <= RSS_GROWTH_KIB && peak_growth <= RSS_GROWTH_KIB,
            "rss growth {growth} KiB peak growth {peak_growth} KiB, gate {RSS_GROWTH_KIB} KiB; {summary}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// Read once. This test does not write the variable.
    fn turn_count() -> u64 {
        const NAME: &str = "GALION_HOST_SOAK_TURNS";
        match std::env::var(NAME) {
            Err(std::env::VarError::NotPresent) => DEFAULT_TURNS,
            Err(err) => panic!("{NAME}: {err}"),
            Ok(raw) => {
                let raw = raw.trim();
                let turns: u64 = raw
                    .parse()
                    .unwrap_or_else(|err| panic!("{NAME} {raw:?}: {err}"));
                assert!(
                    turns >= MARK_AFTER,
                    "{NAME} is {turns}; the mark is after {MARK_AFTER} turns"
                );
                turns
            }
        }
    }

    fn manifest(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name)
    }

    fn scratch() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("galion-host-soak-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn dev_under(root: &Path, profile: &str) -> HostConfig {
        HostConfig::resolve(HostSources {
            dev: true,
            learn: None,
            remember: None,
            memory_dir: None,
            data_root: Some(root.to_str().unwrap()),
            learn_pack: None,
            turn_log: None,
            turn_log_cap: None,
            profile_text: Some(profile),
            xdg_data_home: None,
            home: None,
        })
        .unwrap()
    }

    fn read_rss(buf: &mut String) -> Result<u64, String> {
        buf.clear();
        let mut file = File::open("/proc/self/status")
            .map_err(|err| format!("reading /proc/self/status: {err}"))?;
        file.read_to_string(buf)
            .map_err(|err| format!("reading /proc/self/status: {err}"))?;
        vm_rss_kib(buf)
    }

    fn vm_rss_kib(text: &str) -> Result<u64, String> {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("VmRSS:") else {
                continue;
            };
            let Some(kb) = rest.split_whitespace().next() else {
                return Err("VmRSS has no number".to_string());
            };
            return kb
                .parse::<u64>()
                .map_err(|err| format!("VmRSS {kb}: {err}"));
        }
        Err("VmRSS is not in /proc/self/status".to_string())
    }

    /// Live log plus the five rotated files the host keeps.
    fn count_gaps(path: &Path) -> usize {
        assert!(path.is_file(), "turn log missing: {}", path.display());
        let mut files = Vec::new();
        for n in (1..=5).rev() {
            let old = rotated(path, n);
            if old.is_file() {
                files.push(old);
            }
        }
        files.push(path.to_path_buf());
        let mut gaps = 0usize;
        let mut saw_miss = false;
        let mut saw_pin = false;
        let mut saw_end = false;
        for file in files {
            let text =
                fs::read_to_string(&file).unwrap_or_else(|err| panic!("{}: {err}", file.display()));
            for line in text.lines() {
                if line.is_empty() {
                    continue;
                }
                if line.contains("who won the match") {
                    saw_miss = true;
                }
                if line.contains("my favourite colour is blue") {
                    saw_pin = true;
                }
                let value: Value = serde_json::from_str(line)
                    .unwrap_or_else(|err| panic!("turn log {err}: {line}"));
                if value["kind"] == "session_end" {
                    saw_end = true;
                }
                if value["kind"] == "gap" {
                    let reason = value["reason"].as_str().unwrap_or("");
                    assert!(!reason.is_empty(), "gap line has no reason: {value}");
                    gaps += 1;
                }
            }
        }
        assert!(saw_miss, "turn log is missing the miss");
        assert!(saw_pin, "turn log is missing the pin");
        assert!(saw_end, "turn log has no session end");
        gaps
    }

    fn rotated(path: &Path, n: usize) -> PathBuf {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{n}"));
        path.with_file_name(name)
    }

    async fn hello(ws: &mut Sock) {
        send_text(
            ws,
            &format!(r#"{{"schema":"{SCHEMA}","type":"hello","session":null}}"#),
        )
        .await;
        let welcome = next_text(ws, Duration::from_secs(3))
            .await
            .expect("welcome");
        assert_eq!(welcome["type"], "welcome");
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
        let url = format!("ws://{addr}{WS_PATH}");
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
}
