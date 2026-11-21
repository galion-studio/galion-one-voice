// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 11 process. Binds `127.0.0.1` and asks Hermes at `127.0.0.1:8642`.
//!
//! `GALION_RESEARCH_BIND` overrides the listen port. The host must stay
//! `127.0.0.1`. This process does not bind 8787 and does not dial any host
//! other than Hermes. It does not read a key.

use std::sync::Arc;
use std::thread;

use galion_research_gateway::{HermesLookup, Server, DEFAULT_BIND, RESEARCH_CAP};

fn main() {
    let bind = std::env::var("GALION_RESEARCH_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let lookup = HermesLookup::new(RESEARCH_CAP);
    let server = match Server::spawn(&bind, Arc::new(lookup), RESEARCH_CAP) {
        Ok(server) => server,
        Err(err) => {
            eprintln!("galion-research-gateway: {err}");
            std::process::exit(1);
        }
    };
    println!(
        "galion-research-gateway bind={} hermes={} cap_ms={}",
        server.addr(),
        galion_research_gateway::HERMES_ADDR,
        RESEARCH_CAP.as_millis()
    );
    thread::park();
}
