// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Direct layer 5 call. The live 7881 turn is [`horbm::Brain::turn`], which
//! runs empathy and then this same order.
//!
//! [`route`] calls [`crate::ReasoningRouter::turn`], which calls
//! [`crate::ReasoningRouter::route`]. Cards answer first. A miss is
//! `I don't know.` This module does not bind a socket, does not dial the
//! Node desk, and does not call a model. A research bind is used only when
//! it is `127.0.0.1` and it is not the desk, Hermes, or this voice server.

use std::net::SocketAddr;

use galion_research_gateway::ResearchClient;

use crate::engine::{Reply, ReplyEngine};
use crate::router::{ReasoningRouter, ResearchPort};

/// Ports this process must not dial. 8787 is the Node desk. 8642 is Hermes.
/// 11434 is a local model server. 7881 and 7878 are this voice UI.
const REFUSED_PORTS: [u16; 5] = [8787, 8642, 11434, 7881, 7878];

/// `GALION_RESEARCH_BIND` when it is set and non-empty.
pub fn bind_from_env() -> Option<String> {
    std::env::var("GALION_RESEARCH_BIND")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

/// A research client, or nothing. `None` leaves the router on cards only.
/// The client does not connect until a later ask, and a refused address
/// never gets that far.
pub fn research_port(bind: Option<&str>) -> Option<Box<dyn ResearchPort>> {
    let bind = bind.map(str::trim).filter(|raw| !raw.is_empty())?;
    let client = ResearchClient::connect(bind).ok()?;
    if refused(client.addr()) {
        return None;
    }
    Some(Box::new(client))
}

fn refused(addr: SocketAddr) -> bool {
    REFUSED_PORTS.contains(&addr.port())
}

/// One turn through layer 5. The router reads its cards before any research.
pub fn route(router: &mut ReasoningRouter, heard: &str) -> Reply {
    router.turn(heard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{LoadedCard, MemoryCards};

    fn kettle() -> LoadedCard {
        LoadedCard::new(
            "kettle",
            "The kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
            Vec::new(),
        )
        .unwrap()
    }

    #[test]
    fn route_uses_the_router_cards_first() {
        let mut router = ReasoningRouter::new(MemoryCards::from_cards(vec![kettle()]), None);
        let hit = route(&mut router, "the kettle");
        assert_eq!(hit.text, "The kettle is on the hob.");
        assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
        let miss = route(&mut router, "where is the spare fuse?");
        assert_eq!(miss.text, "I don't know.");
        assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
        assert!(!miss.text.to_ascii_lowercase().contains("ollama"));
    }

    #[test]
    fn research_bind_is_loopback_and_not_the_desk() {
        assert!(research_port(None).is_none());
        assert!(research_port(Some("")).is_none());
        assert!(research_port(Some("   ")).is_none());
        assert!(research_port(Some("0.0.0.0:7911")).is_none());
        assert!(research_port(Some("[::1]:7911")).is_none());
        assert!(research_port(Some("localhost:7911")).is_none());
        assert!(research_port(Some("127.0.0.1:8787")).is_none());
        assert!(research_port(Some("127.0.0.1:8642")).is_none());
        assert!(research_port(Some("127.0.0.1:11434")).is_none());
        assert!(research_port(Some("127.0.0.1:7881")).is_none());
        assert!(research_port(Some("127.0.0.1:7878")).is_none());
        assert!(research_port(Some("127.0.0.1:7911")).is_some());
    }
}
