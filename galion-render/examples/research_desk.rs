//! G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
//! Developed and distributed by Galion Labs
//! Perpetual Open MIT License
//! Copyright © 2026 Galion Labs. All rights reserved.
//!
//! Galion Research desk on loopback. (c) 2026 Maciej Grajczyk.
//!
//! `GALION_RESEARCH_URL` unset: `/research/ask` answers Offline from attached
//! sources and the bundled local notes. No paid API. The key is never printed.

use galion_render::RenderSurface;

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7878".into());
    let addr: std::net::SocketAddr = addr.parse().expect("loopback address");
    let mut web =
        galion_render::WebSurface::serve(addr).with_page("<!doctype html><canvas></canvas>");
    match galion_cascade::HermesLlm::research_from_env() {
        Some(client) => {
            eprintln!("research gateway configured (key {})", client.key_state());
            web.set_research_hermes(client);
        }
        None => eprintln!("research gateway unset; /research/ask uses local notes"),
    }
    web.open().expect("bind");
    let bound = web.local_addr().expect("listening");
    eprintln!("research desk http://{bound}/research");
    std::thread::park();
}
