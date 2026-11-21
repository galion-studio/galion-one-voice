// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Sense consent, default deny. Same rule as `LOCKS.toml` `[consent]` and
//! `galion-emotion`: the gate is checked before PCM is read. This sector asks
//! only for sense. It has no remember bit and no share bit, and it writes no
//! audio store.

use crate::error::{EmotionError, EmotionErrorKind};
use std::fmt;

/// Why a frame would be read. Only sense exists here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Sense,
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Purpose::Sense => write!(f, "sense"),
        }
    }
}

/// Session grant. `sense: false` is the start state.
pub trait ConsentGate {
    fn permit(&self, purpose: Purpose) -> Result<(), EmotionError>;
}

/// In-memory grant for this process. Dropping it drops the grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Session {
    pub sense: bool,
}

impl Session {
    pub const DENY: Session = Session { sense: false };

    pub fn grant_sense() -> Session {
        Session { sense: true }
    }

    /// `--sense` or the exact environment value `GALION_EMOTION_SENSE=1`.
    /// Any other environment value stays deny.
    pub fn from_flag_and_env(sense_flag: bool, env: Option<&str>) -> Session {
        Session {
            sense: sense_flag || env == Some("1"),
        }
    }
}

impl ConsentGate for Session {
    fn permit(&self, purpose: Purpose) -> Result<(), EmotionError> {
        match purpose {
            Purpose::Sense if self.sense => Ok(()),
            Purpose::Sense => Err(EmotionError::new(
                EmotionErrorKind::Denied,
                "sense is denied; pass --sense or set GALION_EMOTION_SENSE=1 for this process",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_deny_and_only_exact_env_grants() {
        assert_eq!(Session::default(), Session::DENY);
        assert!(Session::DENY.permit(Purpose::Sense).is_err());
        assert!(Session::grant_sense().permit(Purpose::Sense).is_ok());
        assert!(!Session::from_flag_and_env(false, None).sense);
        assert!(!Session::from_flag_and_env(false, Some("true")).sense);
        assert!(!Session::from_flag_and_env(false, Some("")).sense);
        assert!(Session::from_flag_and_env(false, Some("1")).sense);
        assert!(Session::from_flag_and_env(true, None).sense);
    }
}
