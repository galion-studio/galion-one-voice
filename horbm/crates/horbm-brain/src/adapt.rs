// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Plugs for the crates that are still landing.
//!
//! Swarm 2 owns FTS5 retrieval (`crates/horbm-retrieval`). Swarm 3 owns the
//! int8 key arena (`crates/horbm-keys`), calling `galion-tensor`'s i8 dot
//! with the scalar path as the oracle and SIMD only behind that crate's
//! existing features. Neither crate was on this branch. These adapters are
//! the seam: a function pointer, no path dependency, no network.
//!
//! Cargo features `retrieval` and `keys` compile the notes below. They do
//! not fetch or link those crates. Turn a feature into a path dependency
//! only after the crate's public search function exists.

use crate::retrieve::{Retrieved, Retriever};

/// Swarm 2 plug. `search` is the FTS5 call once `horbm-retrieval` lands.
pub struct RetrievalAdapter<T> {
    pub inner: T,
    pub search: fn(&T, &str, usize) -> Vec<Retrieved>,
}

impl<T> Retriever for RetrievalAdapter<T> {
    fn retrieve(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        (self.search)(&self.inner, query, limit)
    }
}

/// Swarm 3 plug. `nearest` is the int8 key scan once `horbm-keys` lands.
/// The scan must stay bit-exact with `galion-tensor`'s scalar i8 dot.
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
    //! `crates/horbm-retrieval` is not a dependency of this crate yet.
    pub const STATUS: &str = "horbm-retrieval is not linked";
}

#[cfg(feature = "keys")]
pub mod keys_pending {
    //! `crates/horbm-keys` is not a dependency of this crate yet.
    pub const STATUS: &str = "horbm-keys is not linked";
}
