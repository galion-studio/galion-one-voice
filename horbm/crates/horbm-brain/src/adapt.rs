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
//! Cargo features `retrieval` and `keys` compile the notes below. `retrieval`
//! does not link a diary crate. `keys` does not change the pick. Every build
//! of this crate links `horbm-keys` and `galion-tensor` for [`crate::StoredKeys`].

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

/// Fixture plug. The turn crate still scores with `horbm_keys::StringKeyArena`,
/// which binds strings in memory. The file that stores those strings and a
/// per-key int8 scale is [`crate::StoredKeys`]. The dot stays `galion-tensor`'s.
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
    //! The feature does not change the pick. The stored file is [`crate::StoredKeys`].
    //! Crate `horbm` still implements `Int8Scorer` for `horbm_keys::StringKeyArena`.
    pub const STATUS: &str = "stored HKB1 strings and scales are in horbm_brain::StoredKeys";
}
