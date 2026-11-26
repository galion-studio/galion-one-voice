// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::env;
use std::io::{self, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

use galion_samantha::{
    default_home, health_json, parse_bind, reference_state, resolve_bind_choice, serve_blocking,
    warning_text, warn_once,
    ModelReport, Paths, TickClock, DEFAULT_BIND, TICK_MS, VERSION,
};

fn main() {
    let mut args = env::args().skip(1);
    let code = match args.next().as_deref() {
        Some("serve") => cmd_serve(),
        Some("check") => cmd_check(args.any(|a| a == "--allow-missing")),
        Some("open") => cmd_open(),
        Some("version") | Some("--version") => {
            println!("galion-samantha {VERSION}");
            0
        }
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            0
        }
        Some(other) => {
            eprintln!("galion-samantha: unknown command {other}");
            print_help();
            2
        }
        None => {
            print_help();
            0
        }
    };
    std::process::exit(code);
}

fn print_help() {
    println!(
        "galion-samantha {VERSION}\n\
         local loopback supervisor. HORBM is not linked.\n\
         \n\
         commands:\n\
           serve     bind GALION_VOICE_WS (default {DEFAULT_BIND})\n\
           check     report public model files (--allow-missing exits 0 if absent)\n\
           open      start the user service if needed and print the one-dot URL\n\
           version   print the version\n\
         \n\
         GALION_VOICE_REF is a local file path. GALION_SAMANTHA_REF is a deprecated\n\
         fallback and logs one warning. This program does not copy the file."
    );
}

fn configured_bind() -> String {
    let voice = env::var("GALION_VOICE_WS").ok();
    let ws = env::var("GALION_SAMANTHA_WS").ok();
    let bind = env::var("GALION_SAMANTHA_BIND").ok();
    let (value, warning) = resolve_bind_choice(voice.as_deref(), ws.as_deref(), bind.as_deref());
    if let Some(warning) = warning {
        let old = if ws.is_some() {
            "GALION_SAMANTHA_WS"
        } else {
            "GALION_SAMANTHA_BIND"
        };
        warn_once(
            "GALION_VOICE_WS",
            &warning_text("GALION_VOICE_WS", old, warning),
        );
    }
    value
}

fn cmd_serve() -> i32 {
    let bind_raw = configured_bind();
    let bind = match parse_bind(&bind_raw) {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("galion-samantha: {err}");
            return 2;
        }
    };
    let home = match default_home() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("galion-samantha: {err}");
            return 2;
        }
    };
    match serve_blocking(bind, home) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("galion-samantha: {err}");
            1
        }
    }
}

fn cmd_check(allow_missing: bool) -> i32 {
    let home = match default_home() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("galion-samantha: {err}");
            return 2;
        }
    };
    let report = ModelReport::scan(&Paths::from_home(&home));
    let voice = env::var("GALION_VOICE_REF").ok();
    let old = env::var("GALION_SAMANTHA_REF").ok();
    let resolved = galion_samantha::resolve(voice.as_deref(), old.as_deref());
    if let Some(warning) = resolved.warning {
        warn_once(
            "GALION_VOICE_REF",
            &warning_text("GALION_VOICE_REF", "GALION_SAMANTHA_REF", warning),
        );
    }
    let reference = reference_state(resolved.value.as_deref());
    let (clock, _handle) = TickClock::spawn();
    let body = health_json(&home, &clock, reference);
    println!("{body}");
    println!(
        "tick_ms={TICK_MS} horbm=unlinked reference={reference} public_models_ready={}",
        if report.public_models_ready() {
            "yes"
        } else {
            "no"
        }
    );
    if report.public_models_ready() || allow_missing {
        0
    } else {
        eprintln!(
            "galion-samantha: public models are not all present under {}",
            home.display()
        );
        1
    }
}

fn cmd_open() -> i32 {
    let bind_raw = configured_bind();
    let bind = match parse_bind(&bind_raw) {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("galion-samantha: {err}");
            return 2;
        }
    };
    if bind.port() == 0 {
        eprintln!("galion-samantha: open needs a fixed port, not 0");
        return 2;
    }
    if !port_open(&bind) {
        match Command::new("systemctl")
            .args(["--user", "start", "galion-samantha.service"])
            .status()
        {
            Ok(status) if status.success() => {
                let mut up = false;
                for _ in 0..15 {
                    if port_open(&bind) {
                        up = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                if !up {
                    eprintln!("galion-samantha: user service started but {bind} did not open");
                    return 1;
                }
            }
            Ok(status) => {
                eprintln!(
                    "galion-samantha: systemctl --user start exited {}",
                    status.code().unwrap_or(1)
                );
                return 1;
            }
            Err(err) => {
                eprintln!("galion-samantha: systemctl --user is unavailable ({err})");
                eprintln!("start the server with: galion-samantha serve");
                return 1;
            }
        }
    }
    let url = format!("http://{bind}/");
    println!("{url}");
    match Command::new("xdg-open").arg(&url).status() {
        Ok(status) if status.success() => 0,
        Ok(status) => {
            eprintln!(
                "galion-samantha: xdg-open exited {}",
                status.code().unwrap_or(1)
            );
            0
        }
        Err(_) => {
            eprintln!("galion-samantha: xdg-open is not installed");
            let _ = io::stdout().flush();
            0
        }
    }
}

fn port_open(addr: &std::net::SocketAddr) -> bool {
    TcpStream::connect_timeout(addr, Duration::from_millis(200)).is_ok()
}
