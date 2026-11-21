// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Hermes is 127.0.0.1:8642 and no other socket.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::thread;
use std::time::{Duration, Instant};

use galion_research_gateway::{
    hermes_spoken, is_hermes_socket, HermesLookup, Lookup, HERMES_ADDR, HERMES_MODEL, HERMES_PATH,
    HERMES_PORT, RESEARCH_CAP,
};

#[test]
fn hermes_socket_is_127_0_0_1_8642_and_8787_is_not_it() {
    assert_eq!(HERMES_ADDR, "127.0.0.1:8642");
    assert_eq!(HERMES_PORT, 8642);
    assert_eq!(HERMES_PATH, "/v1/chat/completions");
    assert_eq!(HERMES_MODEL, "hermes-agent");
    assert_eq!(HermesLookup::socket(), "127.0.0.1:8642".parse().unwrap());
    assert!(is_hermes_socket(HermesLookup::socket()));
    let refused = [
        "127.0.0.1:8787",
        "127.0.0.1:7911",
        "127.0.0.1:7881",
        "0.0.0.0:8642",
        "192.0.2.1:8642",
    ];
    for raw in refused {
        let addr: SocketAddr = raw.parse().unwrap();
        assert!(!is_hermes_socket(addr), "{raw}");
    }
    let v6: SocketAddr = "[::1]:8642".parse().unwrap();
    assert!(!is_hermes_socket(v6));
    assert!(hermes_spoken("The hob is warm.").is_some());
    assert!(hermes_spoken("I don't know.").is_none());
    assert!(hermes_spoken("See https://example.test/a").is_none());
}

#[test]
fn hermes_lookup_talks_only_to_127_0_0_1_8642_and_stops_on_its_cap() {
    let listener = TcpListener::bind(HERMES_ADDR).expect("127.0.0.1:8642 is free for this test");
    let serve = thread::spawn(move || {
        let (mut stream, peer) = listener.accept().expect("first hermes client");
        assert_eq!(peer.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        let req = read_request(&mut stream);
        assert!(
            req.starts_with("POST /v1/chat/completions "),
            "path was {req}"
        );
        assert!(req.contains("Host: 127.0.0.1:8642\r\n"), "{req}");
        assert!(req.contains("\"model\":\"hermes-agent\""), "{req}");
        assert!(!req.to_ascii_lowercase().contains("authorization"), "{req}");
        assert!(!req.contains("8787"), "{req}");
        assert!(!req.contains("://"), "{req}");
        let body = r#"{"choices":[{"message":{"content":"The hob is warm."}}]}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(resp.as_bytes()).expect("reply");

        let (_stalled, _) = listener.accept().expect("second hermes client");
        thread::sleep(Duration::from_secs(2));
    });

    let lookup = HermesLookup::new(RESEARCH_CAP);
    assert_eq!(lookup.cap(), Duration::from_secs(3));
    let answer = lookup
        .lookup("why is the hob warm?")
        .expect("local hermes answer");
    assert_eq!(answer.text, "The hob is warm.");
    assert_eq!(answer.source, "hermes");

    let short = HermesLookup::new(Duration::from_millis(200));
    let started = Instant::now();
    assert!(short.lookup("why is the tide late?").is_none());
    let elapsed = started.elapsed();
    eprintln!("MEASURE hermes_cap_wait_ms={}", elapsed.as_millis());
    assert!(
        elapsed >= Duration::from_millis(150),
        "hermes returned in {} ms",
        elapsed.as_millis()
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "hermes returned in {} ms",
        elapsed.as_millis()
    );
    serve.join().expect("hermes stub");
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).expect("read hermes request");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&buf[..pos]).to_string();
            let need = header
                .split("\r\n")
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if buf.len() >= pos + 4 + need {
                break;
            }
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}
