// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MouthError {
    Split { name: String, options: Vec<String> },
    BurstDetector { name: String, may_be: Vec<String> },
    NotACategory(String),
    BadBlend(String),
    BadNumber(&'static str),
    BadScale(String),
    Rate(String),
    Io(String),
    Json(String),
    Binary(String),
}

impl fmt::Display for MouthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MouthError::Split { name, options } => {
                write!(f, "{name} splits into {}; name one", options.join(" and "))
            }
            MouthError::BurstDetector { name, may_be } => {
                write!(
                    f,
                    "{name} is a burst detector; it may be {}",
                    may_be.join(", ")
                )
            }
            MouthError::NotACategory(name) => write!(f, "{name} is not one of the 24"),
            MouthError::BadBlend(msg) => write!(f, "{msg}"),
            MouthError::BadNumber(name) => write!(f, "{name} is out of range"),
            MouthError::BadScale(msg) => write!(f, "{msg}"),
            MouthError::Rate(msg) => write!(f, "{msg}"),
            MouthError::Io(msg) => write!(f, "{msg}"),
            MouthError::Json(msg) => write!(f, "{msg}"),
            MouthError::Binary(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MouthError {}
