// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear prosody sensing switch.
//!
//! Sensing is on unless the caller passes the off switch or
//! `GALION_EAR_PROSODY` is `0` or `off`. Any other value, including an unset
//! environment value, leaves it on. This does not change `galion-emotion-in`'s
//! session, which stays deny until a caller grants sense.

/// `switch_off` is the CLI `--no-prosody` flag.
pub fn ear_prosody_enabled(switch_off: bool) -> bool {
    if switch_off {
        return false;
    }
    match std::env::var("GALION_EAR_PROSODY") {
        Ok(value) => {
            let value = value.trim();
            !(value == "0" || value.eq_ignore_ascii_case("off"))
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensing_is_on_unless_the_switch_is_set() {
        assert!(!ear_prosody_enabled(true));
        if std::env::var_os("GALION_EAR_PROSODY").is_none() {
            assert!(ear_prosody_enabled(false));
        }
    }
}
