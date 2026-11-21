// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Layer 5 call used by the 7881 card bridge.
//!
//! [`route`] stays `None` until `ReasoningRouter` from pull request 239 is on
//! `main`. After that, the 7881 brain calls `ReasoningRouter::turn` or
//! `ReasoningRouter::route`. That router reads cards first and says
//! `I don't know.` on a miss. This process does not dial the Research
//! gateway, does not bind the Node desk port, and does not call a model.

use crate::engine::Reply;

/// Ask layer 5. `None` means `ReasoningRouter` is not on `main` yet, so
/// [`crate::CardEngine`] reads the cards itself.
pub fn route(heard: &str) -> Option<Reply> {
    landed(heard)
}

/// Replace this body with `ReasoningRouter::route` once that type is in the
/// tree. Do not call the gateway from here.
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
        assert!(route("why is the kettle on the stove today?").is_none());
        assert!(route("").is_none());
    }
}
