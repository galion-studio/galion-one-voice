// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Long granted run of the real host.
//!
//! Boots the dev profile the way `end_to_end_dev_session` does, then sends
//! `GALION_HOST_SOAK_TURNS` text lines (default 1500) on the socket. Live heap
//! is marked after 300 turns. Growth after that mark is held to 128 KiB, the
//! same number as `horbm-soak`'s `HEAP_GROWTH_KIB`. `VmRSS` and, when readable,
//! exact `Rss` from `/proc/self/smaps_rollup` are printed and are not gated.
//!
//! Ignored. Laptop:
//! `cargo test --manifest-path voice-stack/local-server/Cargo.toml --test host_soak -- --ignored --nocapture`
//!
//! Shutdown must leave the diary file, the sealed learn pack, and a turn log
//! that holds the miss, the pin, and the session end. A granted retrieval
//! miss does not enter `Brain::gaps` on this base, so a gap row is counted
//! and printed. Every gap row that is present names its reason.
//!
//! Live heap is a counting allocator over `std::alloc::System`. On any other
//! target this test compiles to a no-op and prints a note.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "long: cargo test --test host_soak -- --ignored"]
async fn granted_host_soak_rss_stays_at_the_soak_gate() {
    #[cfg(target_os = "linux")]
    soak::run().await;
    #[cfg(not(target_os = "linux"))]
    {
        println!("live heap is measured on Linux; this target does not run the soak");
    }
}

#[cfg(target_os = "linux")]
mod soak {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use futures_util::{SinkExt, StreamExt};
    use horbm::{LEARN_PACK_DB, LEARN_PACK_LIVE};
    use samantha_local_server::protocol::{SCHEMA, WS_PATH};
    use samantha_local_server::{start_with_host, HostConfig, HostSession, HostSources, RefState};
    use serde_json::Value;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

    type Sock = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    /// Same number as `horbm-soak`'s `HEAP_GROWTH_KIB`. Not a second gate.
    const HEAP_GROWTH_KIB: u64 = 128;

    /// Requested bytes still live. Alloc adds, dealloc subtracts, realloc
    /// applies the size delta. The library crates do not install this.
    struct Counting;

    static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

    fn live_add(bytes: usize) {
        LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }

    fn live_sub(bytes: usize) {
        LIVE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
    }

    fn live_resize(old: usize, new: usize) {
        if new >= old {
            live_add(new - old);
        } else {
            live_sub(old - new);
        }
    }

    fn live_bytes() -> u64 {
        LIVE_BYTES.load(Ordering::Relaxed) as u64
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);
            if !ptr.is_null() {
                live_add(layout.size());
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            live_sub(layout.size());
            System.dealloc(ptr, layout);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let next = System.realloc(ptr, layout, new_size);
            if !next.is_null() || new_size == 0 {
                let kept = if next.is_null() { 0 } else { new_size };
                live_resize(layout.size(), kept);
            }
            next
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;
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
        let mut smaps_buf = String::new();
        // Size both buffers before the gated window. Later reads reuse them,
        // so the measurement itself is not live-heap growth.
        let rss_start = read_rss(&mut rss_buf);
        let smaps_start = read_smaps(&mut smaps_buf);
        let heap_start = live_bytes();
        println!(
            "host soak start heap {heap_start} bytes rss {} smaps {}",
            show_kib(&rss_start),
            show_kib(&smaps_start)
        );
        let mut mark: Option<u64> = None;
        let mut peak = 0u64;
        let mut end = 0u64;
        let mut heap_mark: Option<u64> = None;
        let mut heap_end = heap_start;
        for n in 1..=turns {
            let index = usize::try_from(n - 1).expect("turn") % LINES.len();
            say(&mut ws, LINES[index]).await;
            if n < MARK_AFTER {
                continue;
            }
            let rss_now = read_rss(&mut rss_buf);
            if let Ok(kib) = rss_now.as_ref() {
                if mark.is_none() {
                    mark = Some(*kib);
                }
                peak = peak.max(*kib);
                end = *kib;
            }
            if n == MARK_AFTER || n == turns {
                let smaps = read_smaps(&mut smaps_buf);
                let heap = live_bytes();
                if n == MARK_AFTER {
                    heap_mark = Some(heap);
                }
                if n == turns {
                    heap_end = heap;
                }
                println!(
                    "host soak turn {n} heap {heap} bytes rss {} smaps {}",
                    show_kib(&rss_now),
                    show_kib(&smaps)
                );
            }
        }
        drop(ws);
        let heap_mark = heap_mark.expect("heap mark");
        let wall_s = started.elapsed().as_secs_f64();
        let growth = heap_end.saturating_sub(heap_mark);
        let limit = HEAP_GROWTH_KIB.saturating_mul(1024);
        let rss_mark = mark.unwrap_or(0);
        println!("host soak end heap {heap_end} bytes");
        let summary = format!(
            "turns={turns} wall={wall_s:.3}s heap_start={heap_start} bytes heap_mark={heap_mark} bytes heap_end={heap_end} bytes growth={growth} bytes gate={HEAP_GROWTH_KIB} KiB rss_mark={rss_mark} KiB rss_peak={peak} KiB rss_end={end} KiB"
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
            growth <= limit,
            "heap growth {growth} bytes, gate {HEAP_GROWTH_KIB} KiB; {summary}"
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

    fn read_smaps(buf: &mut String) -> Result<u64, String> {
        buf.clear();
        let mut file = File::open("/proc/self/smaps_rollup")
            .map_err(|err| format!("reading /proc/self/smaps_rollup: {err}"))?;
        file.read_to_string(buf)
            .map_err(|err| format!("reading /proc/self/smaps_rollup: {err}"))?;
        smaps_rss_kib(buf)
    }

    fn smaps_rss_kib(text: &str) -> Result<u64, String> {
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix("Rss:") else {
                continue;
            };
            let Some(kb) = rest.split_whitespace().next() else {
                return Err("Rss has no number".to_string());
            };
            return kb.parse::<u64>().map_err(|err| format!("Rss {kb}: {err}"));
        }
        Err("Rss is not in /proc/self/smaps_rollup".to_string())
    }

    fn show_kib(value: &Result<u64, String>) -> String {
        match value {
            Ok(kib) => format!("{kib} KiB"),
            Err(err) => err.clone(),
        }
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
