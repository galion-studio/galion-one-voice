// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Serves the Samantha one-dot page on loopback.

use std::env;
use std::process::ExitCode;

use s11_one_dot_ui::{measure, ServeError, Server};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "serve".to_string());
    match cmd.as_str() {
        "help" | "-h" | "--help" => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        "check" => match measure() {
            Ok(m) => {
                println!("tick_samples={}", m.tick_samples);
                println!("tick_min_ms={:.3}", m.tick_min_ms);
                println!("tick_median_ms={:.3}", m.tick_median_ms);
                println!("tick_max_ms={:.3}", m.tick_max_ms);
                println!("http_us={}", m.http_us);
                println!("index_bytes={}", m.index_bytes);
                println!("caption_ms={:.3}", m.caption_ms);
                if (40.0..=140.0).contains(&m.tick_median_ms) {
                    ExitCode::SUCCESS
                } else {
                    eprintln!("tick median outside 40..=140 ms");
                    ExitCode::from(1)
                }
            }
            Err(err) => {
                eprintln!("check failed: {err}");
                ExitCode::from(1)
            }
        },
        "serve" => serve(args.collect()),
        other => {
            eprintln!("unknown command {other}");
            eprintln!("{HELP}");
            ExitCode::from(2)
        }
    }
}

fn serve(args: Vec<String>) -> ExitCode {
    let mut mock = false;
    let mut addr = env::var("SAMANTHA_DOT_ADDR").unwrap_or_else(|_| "127.0.0.1:8788".to_string());
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--mock" => mock = true,
            "--addr" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--addr needs a value");
                    return ExitCode::from(2);
                }
                addr = args[i].clone();
            }
            other => {
                eprintln!("unknown flag {other}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let server = match Server::spawn(&addr, mock) {
        Ok(server) => server,
        Err(ServeError::NotLoopback(a)) => {
            eprintln!("refusing non-loopback bind {a}");
            return ExitCode::from(1);
        }
        Err(err) => {
            eprintln!("serve failed: {err}");
            return ExitCode::from(1);
        }
    };
    let bound = server.addr();
    println!("Samantha dot  http://{bound}/");
    println!("WebSocket     ws://{bound}/v1");
    if mock {
        println!("mock brain    on (preview reply, no voice audio)");
    } else {
        println!("mock brain    off (clock and level only, no reply)");
    }
    println!("preview       http://{bound}/?mock=1");
    // The accept thread runs until the process is killed. Park this thread.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

const HELP: &str = "\
samantha-dot serve [--addr 127.0.0.1:8788] [--mock]
samantha-dot check

Loopback only. The page is one violet dot.
--mock answers the preview line. Without it the socket keeps the 80 ms clock
and does not invent a reply.
";
