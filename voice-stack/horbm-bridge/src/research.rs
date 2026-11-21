// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 5 call used by the 7881 card bridge.
//!
//! [`crate::ReasoningRouter`] is in this crate. The 7881 brain should call
//! `ReasoningRouter::turn` or `ReasoningRouter::route` with the knowledge
//! directory it already opened. [`route`] stays `None` so [`crate::CardEngine`]
//! keeps reading those cards. This function does not dial the Research
//! gateway, does not bind the Node desk port, and does not call a model.

use crate::engine::Reply;

/// Ask layer 5 from the 7881 card bridge.
///
/// Always `None`. The bridge then reads its own cards. A caller that wants
/// the layer 5 order uses [`crate::ReasoningRouter`] and passes the directory.
/// This function does not open a socket.
pub fn route(heard: &str) -> Option<Reply> {
    landed(heard)
}

fn landed(heard: &str) -> Option<Reply> {
    let _ = heard;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_does_not_dial_so_the_bridge_answers() {
        assert!(route("where is the kettle").is_none());
        assert!(route("why is the kettle on the stove today?").is_none());
        assert!(route("").is_none());
    }
}
