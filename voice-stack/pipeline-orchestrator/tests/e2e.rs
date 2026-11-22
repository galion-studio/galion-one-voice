// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! End-to-end loop on the fake stages, plus the loopback dot.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use galion_sam_orch::clause::clauses;
use galion_sam_orch::clock::{STAGE_BAR_US, VAD_HANG_MS};
use galion_sam_orch::demo::{self, MOOD, REPLY};
use galion_sam_orch::samantha_traits::SAMANTHA_REF_ENV;
use galion_sam_orch::{Phase, HEARTBEAT_MS};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scripted_turn_reports_each_stage() {
    let run = demo::measure_scripted().await;
    eprintln!("MEASURE {}", run.to_json());
    assert!(
        contains_in_order(
            &run.phases,
            &[
                Phase::Listening,
                Phase::Thinking,
                Phase::Speaking,
                Phase::Idle
            ]
        ),
        "phases {:?}",
        run.phases
    );
    assert_eq!(run.texts, clauses(REPLY));
    assert_eq!(run.moods.len(), 2);
    assert!(run.moods.iter().all(|pad| *pad == MOOD));
    assert_eq!(run.mood_after, MOOD);
    assert_eq!(run.played.len(), 8);
    assert!(run.report.spoken);
    assert!(!run.report.stale);
    assert_eq!(run.report.chunks_started, 2);
    assert!(run.report.listen_audio_ms >= 60 + VAD_HANG_MS as u32);
    assert!(
        run.report.asr_us < STAGE_BAR_US,
        "asr_us {} bar {}",
        run.report.asr_us,
        STAGE_BAR_US
    );
    assert!(
        run.report.think_us < STAGE_BAR_US,
        "think_us {} bar {}",
        run.report.think_us,
        STAGE_BAR_US
    );
    let first = run.report.first_audio_us.expect("first audio");
    assert!(first < STAGE_BAR_US, "first_audio_us {first}");
    let eot = run.report.eot_to_audio_us.expect("eot");
    assert!(eot < STAGE_BAR_US * 3, "eot_to_audio_us {eot}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn barge_in_cancels_the_speaker() {
    let barge = demo::measure_barge().await;
    eprintln!("MEASURE {}", barge.to_json());
    assert!(barge.stale);
    assert!(barge.cancels >= 1);
    assert_eq!(barge.queued_len, 0);
    assert!(barge.played_len > 0, "the first chunk was audible");
    assert!(
        barge.played_len < 8,
        "the rest of the utterance kept playing: {}",
        barge.played_len
    );
    assert!(
        barge.barge_us < STAGE_BAR_US,
        "barge_us {} bar {}",
        barge.barge_us,
        STAGE_BAR_US
    );
    assert!(barge.phases.contains(&Phase::Speaking));
    assert!(barge.phases.contains(&Phase::Listening));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn barge_in_during_think_does_not_speak() {
    assert!(demo::measure_think_cancel().await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn silence_does_not_invent_a_turn() {
    let running = demo::spawn_fake(
        galion_sam_orch::FakeReply::new(REPLY, MOOD),
        galion_sam_orch::FakeTts::pcm(vec![1, 2, 3, 4]),
    );
    demo::send_frames(&running.tx, 0, 12).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(running.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(running.handle.reports().is_empty());
    assert_eq!(running.handle.phase(), Phase::Idle);
    assert_eq!(running.speaker.played_len(), 0);
    running.handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mood_tick_is_one_heartbeat() {
    let ticks = demo::measure_mood().await;
    eprintln!("MEASURE {}", ticks.to_json());
    assert!(
        ticks.intervals_ms.len() >= 3,
        "intervals {:?}",
        ticks.intervals_ms
    );
    for gap in &ticks.intervals_ms {
        assert!(
            (40..250).contains(gap),
            "mood interval {gap} ms, heartbeat {HEARTBEAT_MS}"
        );
    }
    for us in &ticks.advance_us {
        assert!(*us < STAGE_BAR_US, "advance_us {us}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_a_gated_mouth_does_not_pin_the_runtime() {
    let gate = galion_sam_orch::samantha_traits::WaitGate::new();
    let running = demo::spawn_fake(
        galion_sam_orch::FakeReply::new(REPLY, MOOD),
        galion_sam_orch::FakeTts::gated(vec![7, 7], gate),
    );
    demo::send_frames(&running.tx, 6, 10).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while running.handle.phase() != Phase::Speaking {
        if tokio::time::Instant::now() > deadline {
            panic!("did not reach speaking");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // The gate stays closed. Drop has to cancel the parked mouth, or this
    // runtime never finishes joining that worker.
    drop(running);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dot_page_follows_the_phase() {
    let gate = galion_sam_orch::samantha_traits::WaitGate::new();
    let running = demo::spawn_fake(
        galion_sam_orch::FakeReply::new(REPLY, MOOD),
        galion_sam_orch::FakeTts::gated(vec![7, 7], gate.clone()),
    );
    let server = galion_sam_orch::ui::DotServer::bind(
        "127.0.0.1:0".parse().unwrap(),
        running.handle.subscribe(),
    )
    .expect("loopback");
    demo::send_frames(&running.tx, 6, 10).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut saw_speaking = false;
    while tokio::time::Instant::now() < deadline {
        let body = http_get(server.addr(), "/state");
        if body.contains("\"phase\":\"speaking\"") {
            saw_speaking = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(saw_speaking, "dot never showed speaking");
    let page = http_get(server.addr(), "/");
    assert!(page.contains("Samantha"));
    assert!(page.contains("id=\"dot\""));
    gate.release
        .store(true, std::sync::atomic::Ordering::SeqCst);
    running.handle.shutdown().await;
    server.shutdown();
}

#[test]
fn reference_env_points_at_a_local_file() {
    let path = std::env::temp_dir().join("samantha-core-s09-not-audio.txt");
    std::fs::write(&path, b"not audio").unwrap();
    std::env::set_var(SAMANTHA_REF_ENV, &path);
    let got = galion_sam_orch::samantha_ref_from_env().expect("path");
    assert_eq!(got, path);
    std::env::remove_var(SAMANTHA_REF_ENV);
    assert!(galion_sam_orch::samantha_ref_from_env().is_none());
    let _ = std::fs::remove_file(path);
}

fn contains_in_order(phases: &[Phase], want: &[Phase]) -> bool {
    let mut index = 0;
    for phase in phases {
        if *phase == want[index] {
            index += 1;
            if index == want.len() {
                return true;
            }
        }
    }
    false
}

fn http_get(addr: SocketAddr, path: &str) -> String {
    // Connect and each read are bounded. A reset or a slow accept used to
    // unwind the test while the mouth was still parked, and the runtime then
    // waited forever to join that worker.
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        Ok(stream) => stream,
        Err(_) => return String::new(),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return String::new();
    }
    let mut body = Vec::new();
    let mut buf = [0u8; 2048];
    let deadline = std::time::Instant::now() + Duration::from_millis(800);
    while std::time::Instant::now() < deadline && body.len() < 65_536 {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}
