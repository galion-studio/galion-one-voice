// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.

use std::fmt;

/// Failures of the translator and the load path. No partial pack is a success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Filesystem failure, with the path already in the message.
    Io(String),
    /// The bytes are not a pack this loader accepts.
    Bad(String),
    /// A weight, extension, or magic this stack will not carry.
    Refused(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(m) | Error::Bad(m) | Error::Refused(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(ctx: &str, err: impl fmt::Display) -> Self {
        Error::Io(format!("{ctx}: {err}"))
    }
}
