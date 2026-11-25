// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `samantha-local` — loopback UI server. See README.md.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use samantha_local_server::{
    knowledge_dir, live_factory_with, serve_until_ctrl_c, session_ttl_from_env, HostConfig,
    HostSession, RefState,
};

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
    let mut addr_s =
        std::env::var("GALION_SAMANTHA_WS").unwrap_or_else(|_| "127.0.0.1:7881".into());
    let mut ui = std::env::var("GALION_SAMANTHA_UI").unwrap_or_else(|_| "ui".into());
    let mut cli_dev = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "samantha-local [--addr 127.0.0.1:7881] [--ui DIR] [--profile dev]\n\
                     Binds 127.0.0.1 or ::1. GALION_SAMANTHA_WS overrides the default when --addr is absent.\n\
                     GALION_HORBM_PROFILE=dev or --profile dev loads dev.env. Learn and remember\n\
                     then default to grant unless the operator set a value. Outside that profile\n\
                     both stay off unless the operator sets grant.\n\
                     GALION_SAMANTHA_REF is reported as set, missing, or unset.\n\
                     Turns run through horbm-voice. brain=horbm and mouth_device=cpu unless cuda is explicit.\n\
                     The CPU mouth is Piper LJ Speech only when this binary was built with --features piper.\n\
                     Otherwise the CPU mouth is an in-process tone. GALION_MOUTH=native speaks the turn\n\
                     text on this socket and returns no PCM. The s711 mouth reads the wav only when\n\
                     GALION_SAMANTHA_WEIGHTS is a local directory and GALION_SAMANTHA_DEVICE=cuda."
                );
                return Ok(());
            }
            "--addr" => {
                addr_s = args
                    .next()
                    .ok_or_else(|| "--addr needs 127.0.0.1:PORT or [::1]:PORT".to_string())?;
            }
            "--ui" => {
                ui = args
                    .next()
                    .ok_or_else(|| "--ui needs a directory".to_string())?;
            }
            "--profile" => {
                let name = args
                    .next()
                    .ok_or_else(|| "--profile needs dev".to_string())?;
                if name != "dev" {
                    return Err(format!("unknown profile {name}"));
                }
                cli_dev = true;
            }
            other => return Err(format!("unknown argument {other} (try --help)")),
        }
    }
    let addr: SocketAddr = addr_s
        .parse()
        .map_err(|_| format!("address must be 127.0.0.1:PORT or [::1]:PORT, got {addr_s}"))?;
    let ui_dir = PathBuf::from(&ui);
    if !ui_dir.join("index.html").is_file() {
        eprintln!(
            "samantha-local: no index.html in {} (health and the websocket still start)",
            ui_dir.display()
        );
    }
    let require_session = std::env::var("GALION_REQUIRE_SESSION").ok().as_deref() == Some("1");
    let config = HostConfig::resolve_process(cli_dev)?;
    let host = HostSession::boot(config, knowledge_dir())?;
    eprintln!("{}", host.startup_line());
    let factory = live_factory_with(Arc::clone(&host));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime
        .block_on(serve_until_ctrl_c(
            addr,
            ui_dir,
            factory,
            RefState::from_env(),
            require_session,
            session_ttl_from_env(),
            host,
        ))
        .map_err(|e| e.to_string())
}
