// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Serves the configured persona's one-dot page on loopback.

use std::env;
use std::process::ExitCode;

use one_dot_ui::{measure, ServeError, Server, DEFAULT_ADDR};

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
    let voice = env::var("GALION_VOICE_WS").ok();
    let old = env::var("GALION_SAMANTHA_WS").ok();
    let resolved = resolve(voice.as_deref(), old.as_deref());
    if let Some(warning) = resolved.warning {
        eprintln!(
            "{}",
            warning_text("GALION_VOICE_WS", "GALION_SAMANTHA_WS", warning)
        );
    }
    let mut addr = resolved
        .value
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());
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
    println!("configured persona  http://{bound}/");
    println!("WebSocket            ws://{bound}/v1/samantha");
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AliasWarning {
    Fallback,
    Conflict,
}

struct Resolution {
    value: Option<String>,
    warning: Option<AliasWarning>,
}

/// Pure alias. `new` wins. `old` is the fallback. Empty strings count as set.
fn resolve(new: Option<&str>, old: Option<&str>) -> Resolution {
    match (new, old) {
        (None, None) => Resolution {
            value: None,
            warning: None,
        },
        (Some(value), None) => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (None, Some(value)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Fallback),
        },
        (Some(value), Some(previous)) if value == previous => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (Some(value), Some(_)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Conflict),
        },
    }
}

fn warning_text(new_name: &str, old_name: &str, warning: AliasWarning) -> String {
    match warning {
        AliasWarning::Fallback => format!("{old_name} is deprecated; set {new_name}"),
        AliasWarning::Conflict => {
            format!("{old_name} is deprecated; {new_name} and {old_name} differ; using {new_name}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve, warning_text, AliasWarning};

    #[test]
    fn new_wins() {
        let only_new = resolve(Some("voice"), None);
        assert_eq!(only_new.value.as_deref(), Some("voice"));
        assert!(only_new.warning.is_none());
        let same = resolve(Some("voice"), Some("voice"));
        assert_eq!(same.value.as_deref(), Some("voice"));
        assert!(same.warning.is_none());
    }

    #[test]
    fn old_is_fallback_and_warns_once() {
        let choice = resolve(None, Some("old"));
        assert_eq!(choice.value.as_deref(), Some("old"));
        assert_eq!(choice.warning, Some(AliasWarning::Fallback));
        let line = warning_text("GALION_VOICE_WS", "GALION_SAMANTHA_WS", AliasWarning::Fallback);
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("GALION_VOICE_WS"));
    }

    #[test]
    fn none_is_none() {
        let choice = resolve(None, None);
        assert!(choice.value.is_none());
        assert!(choice.warning.is_none());
    }
}

const HELP: &str = "\
samantha-dot serve [--addr 127.0.0.1:7881] [--mock]
samantha-dot check

Loopback only. GALION_VOICE_WS overrides the bind. GALION_SAMANTHA_WS is a
deprecated fallback and logs one warning. The page is one violet dot for the
configured persona.
--mock answers the preview line on galion-samantha-ws/1. Without it the socket
keeps the 80 ms resting mood and does not invent a reply.
";
