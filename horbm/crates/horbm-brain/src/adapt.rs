// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Fixture seams for a caller that already holds a [`crate::Retriever`].
//!
//! The live link is crate `horbm`, not this crate. `Brain::turn` reads
//! `horbm::RetrievalThread`. `horbm::Int8Scorer` is implemented for
//! `horbm_keys::StringKeyArena`. These adapters stay function pointers so a
//! fixture test can pass a retriever through without a path dependency and
//! without a network call.
//!
//! Cargo features `retrieval` and `keys` compile the notes below. They do
//! not fetch or link those crates.

use crate::retrieve::{Retrieved, Retriever};

/// Fixture plug. The live diary is `horbm::RetrievalThread`, not this pointer.
pub struct RetrievalAdapter<T> {
    pub inner: T,
    pub search: fn(&T, &str, usize) -> Vec<Retrieved>,
}

impl<T> Retriever for RetrievalAdapter<T> {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        (self.search)(&self.inner, query, limit)
    }
}

/// Fixture plug. The live int8 scan is `horbm_keys::StringKeyArena`, and it
/// stays bit-exact with `galion-tensor`'s scalar i8 dot.
pub struct KeyAdapter<T> {
    pub inner: T,
    pub nearest: fn(&T, &str, usize) -> Vec<Retrieved>,
}

impl<T> Retriever for KeyAdapter<T> {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        (self.nearest)(&self.inner, query, limit)
    }
}

#[cfg(feature = "retrieval")]
pub mod retrieval_pending {
    //! This feature does not add a path dependency. Crate `horbm` links the diary.
    pub const STATUS: &str = "horbm-retrieval is not linked";
}

#[cfg(feature = "keys")]
pub mod keys_pending {
    //! This feature does not add a path dependency. Crate `horbm` implements `Int8Scorer`.
    pub const STATUS: &str = "horbm-keys is not linked";
}
