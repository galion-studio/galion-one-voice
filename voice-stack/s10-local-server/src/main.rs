// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `samantha-local` — loopback UI server. See README.md.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use samantha_local_server::{serve_until_ctrl_c, unwired_factory, RefState};

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("samantha-local: {err}");
            ExitCode::from(1)
        }
    }
}

fn real_main() -> Result<(), String> {
    let mut addr_s = std::env::var("GALION_S10_ADDR").unwrap_or_else(|_| "127.0.0.1:8790".into());
    let mut ui = std::env::var("GALION_S10_UI").unwrap_or_else(|_| "ui".into());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "samantha-local [--addr 127.0.0.1:8790] [--ui DIR]\n\
                     Binds 127.0.0.1 only. GALION_S10_ADDR and GALION_S10_UI override the defaults.\n\
                     GALION_SAMANTHA_REF is reported as set, missing, or unset. The file is not read."
                );
                return Ok(());
            }
            "--addr" => {
                addr_s = args
                    .next()
                    .ok_or_else(|| "--addr needs 127.0.0.1:PORT".to_string())?;
            }
            "--ui" => {
                ui = args
                    .next()
                    .ok_or_else(|| "--ui needs a directory".to_string())?;
            }
            other => return Err(format!("unknown argument {other} (try --help)")),
        }
    }
    let addr: SocketAddr = addr_s
        .parse()
        .map_err(|_| format!("address must be an IPv4 literal, got {addr_s}"))?;
    let ui_dir = PathBuf::from(&ui);
    if !ui_dir.join("index.html").is_file() {
        eprintln!(
            "samantha-local: no index.html in {} (health and the websocket still start)",
            ui_dir.display()
        );
    }
    let samantha_ref = RefState::from_env();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime
        .block_on(serve_until_ctrl_c(
            addr,
            ui_dir,
            unwired_factory(),
            samantha_ref,
        ))
        .map_err(|e| e.to_string())
}
