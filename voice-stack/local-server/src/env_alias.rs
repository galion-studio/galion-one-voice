// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `GALION_VOICE_*` with a deprecated `GALION_SAMANTHA_*` fallback.
//!
//! [`read`] is the process edge. [`resolve`] is the pure decision the tests
//! call. Values are not trimmed: an empty string is set, and only a missing
//! variable is absent. The new value wins. The old value is used only when
//! the new name is missing, and that path carries one warning. When both are
//! set and differ, the new value wins and that is also one warning. Equal
//! values are not a warning. [`read`] prints each warning once per new name.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Why stderr should name the new variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasWarning {
    /// The new name is missing. The value came from the old name.
    Fallback,
    /// Both names are set and their values differ. The new value is kept.
    Conflict,
}

/// The value to use, and at most one warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub value: Option<String>,
    pub warning: Option<AliasWarning>,
}

/// Pure alias. `new` wins. `old` is the fallback.
pub fn resolve(new: Option<&str>, old: Option<&str>) -> Resolution {
    match (new, old) {
        (None, None) => Resolution {
            value: None,
            warning: None,
        },
        (Some(value), None) => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (None, Some(value)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Fallback),
        },
        (Some(value), Some(previous)) if value == previous => Resolution {
            value: Some(value.to_string()),
            warning: None,
        },
        (Some(value), Some(_)) => Resolution {
            value: Some(value.to_string()),
            warning: Some(AliasWarning::Conflict),
        },
    }
}

/// One line. Names the new variable. No trailing newline.
pub fn warning_text(new_name: &str, old_name: &str, warning: AliasWarning) -> String {
    match warning {
        AliasWarning::Fallback => format!("{old_name} is deprecated; set {new_name}"),
        AliasWarning::Conflict => {
            format!("{old_name} is deprecated; {new_name} and {old_name} differ; using {new_name}")
        }
    }
}

/// Read `new`, else `old`. Logs one warning per `new` name to stderr.
pub fn read(new: &str, old: &str) -> Option<String> {
    let new_val = std::env::var(new).ok();
    let old_val = std::env::var(old).ok();
    let resolved = resolve(new_val.as_deref(), old_val.as_deref());
    if let Some(warning) = resolved.warning {
        warn_once(new, old, warning);
    }
    resolved.value
}

fn warn_once(new_name: &str, old_name: &str, warning: AliasWarning) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = seen.lock().unwrap_or_else(|err| err.into_inner());
    if !guard.insert(new_name.to_string()) {
        return;
    }
    eprintln!("{}", warning_text(new_name, old_name, warning));
}
