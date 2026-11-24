// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Session consent for the retrieval cell.
//!
//! `LOCKS.toml` `[consent]` sets `remember = "deny"` and `default = "deny"`.
//! This crate does not edit that file. [`SessionConsent::default`] is deny.

/// Whether this session may publish retrieval hits into the cell.
///
/// World-card bytes already on disk are a read. Publishing them into the
/// live cell, and creating a memory database, both require `remember`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionConsent {
    remember: bool,
}

impl SessionConsent {
    pub const fn denied() -> Self {
        Self { remember: false }
    }

    /// Explicit grant. Nothing in the environment turns this on.
    pub const fn granted_remember() -> Self {
        Self { remember: true }
    }

    pub const fn remember(self) -> bool {
        self.remember
    }
}

impl Default for SessionConsent {
    fn default() -> Self {
        Self::denied()
    }
}
