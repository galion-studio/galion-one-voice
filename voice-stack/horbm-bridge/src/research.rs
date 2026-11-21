// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 5 research router.
//!
//! The router and the local Research gateway are built separately. The only
//! call site in this crate is [`route`]. When that gateway is linked, `route`
//! should return its answer and nothing else. Until the gateway is in this
//! tree, [`route`] returns `None` and [`crate::CardEngine`] reads HORBM cards.
//!
//! This module does not open a socket and does not call a model.

use crate::engine::Reply;

/// Ask the layer 5 router. `None` means it has not landed; the caller uses
/// the HORBM card bridge.
pub fn route(heard: &str) -> Option<Reply> {
    landed(heard)
}

/// Body to replace with `galion_research_gateway::route(heard)` when that
/// crate is in the tree. It stays `None` on this build.
fn landed(heard: &str) -> Option<Reply> {
    let _ = heard;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_is_absent_so_the_bridge_answers() {
        assert!(route("where is the kettle").is_none());
        assert!(route("").is_none());
    }
}
