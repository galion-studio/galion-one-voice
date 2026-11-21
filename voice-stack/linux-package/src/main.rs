// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::env;
use std::io::{self, Write};
use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

use galion_samantha::{
    default_home, health_json, parse_bind, reference_state, serve_blocking, ModelReport, Paths,
    TickClock, DEFAULT_BIND, TICK_MS, VERSION,
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
           serve     bind GALION_SAMANTHA_BIND (default {DEFAULT_BIND})\n\
           check     report public model files (--allow-missing exits 0 if absent)\n\
           open      start the user service if needed and print the one-dot URL\n\
           version   print the version\n\
         \n\
         GALION_SAMANTHA_REF is a local file path. This program does not copy it."
    );
}

fn cmd_serve() -> i32 {
    let bind_raw = env::var("GALION_SAMANTHA_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
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
    let reference = reference_state(env::var("GALION_SAMANTHA_REF").ok().as_deref());
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
    let bind_raw = env::var("GALION_SAMANTHA_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
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
