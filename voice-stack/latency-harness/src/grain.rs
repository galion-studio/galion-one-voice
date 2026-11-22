// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! First speakable piece of a reply.
//!
//! The cut is [`galion_sam_horbm::speakable_chunks`], the same splitter the
//! 7881 Piper mouth uses. The reply-to-first-slice clock is this piece.

use galion_sam_horbm::{speakable_chunks, SliceGrain};

/// Text the mouth synthesises before the rest of the reply, and which unit it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstPiece {
    pub text: String,
    /// `"clause"` or `"sentence"`.
    pub grain: &'static str,
}

/// The piece whose PCM is the first slice. A miss of `I don't know.` is one sentence.
pub fn first_piece(reply: &str) -> FirstPiece {
    match speakable_chunks(reply).into_iter().next() {
        Some(chunk) => FirstPiece {
            text: chunk.text,
            grain: chunk.grain.as_str(),
        },
        None => FirstPiece {
            text: reply.trim().to_string(),
            grain: SliceGrain::Sentence.as_str(),
        },
    }
}
