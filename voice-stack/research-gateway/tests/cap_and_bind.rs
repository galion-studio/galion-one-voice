// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Localhost bind and the 3 second cap, against the real socket.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use galion_research_gateway::{
    ClientReply, LocalAnswer, Lookup, NoRemoteLookup, ResearchClient, Server, RESEARCH_CAP,
};

struct Hold {
    slot: Mutex<Option<Receiver<()>>>,
}

impl Lookup for Hold {
    fn lookup(&self, _question: &str) -> Option<LocalAnswer> {
        if let Some(rx) = self.slot.lock().expect("hold").take() {
            let _ = rx.recv();
        }
        Some(LocalAnswer {
            text: "late guess".into(),
            source: "local".into(),
        })
    }
}

struct Release(Option<mpsc::Sender<()>>);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

#[test]
fn gateway_bind_is_127_0_0_1_and_refuses_other_hosts() {
    let server = Server::spawn("127.0.0.1:0", Arc::new(NoRemoteLookup), RESEARCH_CAP)
        .expect("loopback bind");
    let addr = server.addr();
    assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    assert_ne!(addr.port(), 0);
    let err = Server::spawn("0.0.0.0:7911", Arc::new(NoRemoteLookup), RESEARCH_CAP);
    assert!(err.is_err(), "a wildcard bind must fail before listen");
}

#[test]
fn three_second_cap_drops_a_late_answer() {
    assert_eq!(RESEARCH_CAP, Duration::from_secs(3));
    let (tx, rx) = mpsc::channel();
    let release = Release(Some(tx));
    let server = Server::spawn(
        "127.0.0.1:0",
        Arc::new(Hold {
            slot: Mutex::new(Some(rx)),
        }),
        RESEARCH_CAP,
    )
    .expect("loopback bind");
    let client = ResearchClient::connect(&server.addr().to_string()).expect("loopback client");
    assert_eq!(client.cap(), RESEARCH_CAP);
    let started = Instant::now();
    let reply = client.ask("why does the tide come in late?");
    let elapsed = started.elapsed();
    eprintln!("MEASURE cap_wait_ms={}", elapsed.as_millis());
    assert_eq!(reply, ClientReply::Timeout);
    assert!(
        elapsed >= Duration::from_millis(2_500),
        "cap returned in {} ms",
        elapsed.as_millis()
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "cap returned in {} ms",
        elapsed.as_millis()
    );
    drop(release);
}

#[test]
fn a_shorter_cap_returns_before_the_lookup_is_released() {
    let (tx, rx) = mpsc::channel();
    let release = Release(Some(tx));
    let cap = Duration::from_millis(200);
    let server = Server::spawn(
        "127.0.0.1:0",
        Arc::new(Hold {
            slot: Mutex::new(Some(rx)),
        }),
        cap,
    )
    .expect("loopback bind");
    let client = ResearchClient::connect(&server.addr().to_string())
        .expect("loopback client")
        .with_cap(cap);
    let started = Instant::now();
    let reply = client.ask("why?");
    let elapsed = started.elapsed();
    assert_eq!(reply, ClientReply::Timeout);
    assert!(elapsed >= Duration::from_millis(150), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    drop(release);
}

struct Immediate;

impl Lookup for Immediate {
    fn lookup(&self, question: &str) -> Option<LocalAnswer> {
        if question.contains("hob") {
            Some(LocalAnswer {
                text: "The hob is warm.".into(),
                source: "local".into(),
            })
        } else {
            None
        }
    }
}

#[test]
fn a_local_answer_inside_the_cap_is_returned() {
    let server = Server::spawn("127.0.0.1:0", Arc::new(Immediate), RESEARCH_CAP).unwrap();
    let client = ResearchClient::connect(&server.addr().to_string()).unwrap();
    assert_eq!(
        client.ask("how warm is the hob?"),
        ClientReply::Answer {
            text: "The hob is warm.".into(),
            source: "local".into(),
        }
    );
    assert_eq!(client.ask("why is the sky?"), ClientReply::NotFound);
}
