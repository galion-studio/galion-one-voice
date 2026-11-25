// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! One scripted turn, one barge-in, and a short mood-tick sample.
//! `--ui` also serves the one-dot page on 127.0.0.1:7879 for that turn.

use std::net::SocketAddr;
use std::time::Duration;

use galion_sam_orch::demo::{self, MOOD, REPLY};
use galion_sam_orch::ui::DotServer;
use galion_sam_orch::voice_traits::{FakeReply, FakeTts, WaitGate};

fn main() {
    let ui = std::env::args().any(|arg| arg == "--ui");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async move {
        println!(
            "reference_present={}",
            galion_sam_orch::voice_ref_from_env().is_some()
        );
        if ui {
            run_ui().await;
        }
        let scripted = demo::measure_scripted().await;
        println!("{}", scripted.to_json());
        let barge = demo::measure_barge().await;
        println!("{}", barge.to_json());
        let mood = demo::measure_mood().await;
        println!("{}", mood.to_json());
    });
}

async fn run_ui() {
    let gate = WaitGate::new();
    let running = demo::spawn_fake(
        FakeReply::new(REPLY, MOOD),
        FakeTts::gated(vec![1000, -1000], gate.clone()),
    );
    let addr: SocketAddr = "127.0.0.1:7879".parse().expect("loopback addr");
    let server = match DotServer::bind(addr, running.handle.subscribe()) {
        Ok(server) => server,
        Err(err) => {
            eprintln!("ui: {err}");
            running.handle.shutdown().await;
            return;
        }
    };
    println!("ui=http://{}/", server.addr());
    demo::send_frames(&running.tx, 6, 10).await;
    let _ = tokio::time::timeout(Duration::from_secs(3), async {
        while running.handle.phase() != galion_sam_orch::Phase::Speaking {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    gate.release
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = tokio::time::timeout(Duration::from_secs(3), async {
        while running.handle.reports().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    server.shutdown();
    running.handle.shutdown().await;
}
