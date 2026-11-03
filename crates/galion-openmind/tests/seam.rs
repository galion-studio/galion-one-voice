// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
use galion_openmind::{
    NerveConfig, NerveEmitter, NerveSink, NoteDrop, VoiceBackendSlot, VoiceNote,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

struct Slow;
impl NerveSink for Slow {
    fn accept(&mut self, _: &str, _: &str) {
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn wait_line(em: &NerveEmitter) -> String {
    let t0 = Instant::now();
    loop {
        let lines = em.lines();
        if let Some(l) = lines.last() {
            return l.clone();
        }
        assert!(
            t0.elapsed() < Duration::from_secs(2),
            "nerve worker did not publish"
        );
        std::thread::yield_now();
    }
}

#[test]
fn local_ring_keeps_a_frame_miss_without_a_daemon() {
    let em = NerveEmitter::local(8);
    em.note(VoiceNote::frame_miss(4)).unwrap();
    let line = wait_line(&em);
    assert!(line.contains("\"kind\":\"voice.frame_miss\""));
    assert!(line.contains("\"group\":\"galion-os\""));
    assert!(line.contains("\"v\":1"));
    assert_eq!(em.stats().forwarded, 0);
    assert_eq!(em.stats().forward_failed, 0);
}

#[test]
fn note_does_not_block_when_the_worker_is_slow() {
    let em = NerveEmitter::spawn_sink(1, 4, Slow);
    em.note(VoiceNote::frame_miss(1)).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let _ = em.note(VoiceNote::frame_miss(2));
    let t0 = Instant::now();
    let mut full = 0u32;
    for i in 0..40 {
        if em.note(VoiceNote::frame_miss(i)).is_err() {
            full += 1;
        }
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed < Duration::from_millis(80),
        "note blocked the caller: {elapsed:?}"
    );
    assert!(full > 0, "expected the bounded queue to drop");
    assert_eq!(em.note(VoiceNote::frame_miss(99)), Err(NoteDrop::QueueFull));
}

#[test]
fn fallback_and_backend_up_land_in_the_ring() {
    let em = NerveEmitter::local(4);
    em.note(VoiceNote::fallback(
        1,
        8,
        VoiceBackendSlot::Relay,
        VoiceBackendSlot::Student,
    ))
    .unwrap();
    em.note(VoiceNote::backend_up(2, VoiceBackendSlot::Student))
        .unwrap();
    let t0 = Instant::now();
    loop {
        if em.lines().len() >= 2 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    let lines = em.lines();
    assert!(lines[0].contains("\"group\":\"hermes\""));
    assert!(lines[0].contains("\"kind\":\"voice.fallback\""));
    assert!(lines[1].contains("\"kind\":\"voice.backend_up\""));
    assert!(lines[1].contains("\"group\":\"galion-os\""));
}

#[test]
fn loopback_post_uses_the_b33_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let em = NerveEmitter::spawn(NerveConfig {
        queue: 4,
        line_cap: 4,
        forward: Some(addr),
        log_path: None,
    });
    em.note(VoiceNote::frame_miss(11)).unwrap();
    let t0 = Instant::now();
    let mut sock = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(t0.elapsed() < Duration::from_secs(2), "no POST /event");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    };
    sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut buf = vec![0u8; 2048];
    let n = sock.read(&mut buf).unwrap();
    let req = String::from_utf8_lossy(&buf[..n]);
    assert!(req.starts_with("POST /event "), "{req}");
    assert!(req.contains("\"kind\":\"voice.frame_miss\""));
    assert!(req.contains("\"source\":\"galion-voice\""));
    assert!(req.contains("\"unit\":\"galion-os-voice\""));
    assert!(!req.contains("\"seq\""));
    let resp = "HTTP/1.1 200 OK\r\nContent-Length: 21\r\nConnection: close\r\n\r\n{\"ok\":true,\"appended\":1}";
    sock.write_all(resp.as_bytes()).unwrap();
    let t1 = Instant::now();
    loop {
        if em.stats().forwarded == 1 {
            break;
        }
        assert!(
            t1.elapsed() < Duration::from_secs(2),
            "forward was not counted: {:?}",
            em.stats()
        );
        std::thread::yield_now();
    }
}

#[test]
fn log_file_is_written_off_the_caller() {
    let dir = std::env::temp_dir().join(format!("galion-nerve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    let em = NerveEmitter::spawn(NerveConfig {
        queue: 4,
        line_cap: 4,
        forward: None,
        log_path: Some(path.clone()),
    });
    em.note(VoiceNote::backend_up(0, VoiceBackendSlot::Mock))
        .unwrap();
    let t0 = Instant::now();
    loop {
        if em.stats().published >= 1 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    drop(em);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("voice.backend_up"));
    assert!(text.contains("\"backend\":\"mock\""));
    let _ = std::fs::remove_dir_all(&dir);
}
