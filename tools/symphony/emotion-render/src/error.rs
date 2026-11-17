// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    NotACategory(String),
    Split {
        name: String,
        options: Vec<String>,
    },
    /// A detector name, not one category. Cowen 2019 AP does not assign the sigh.
    BurstDetector {
        name: String,
        may_be: Vec<String>,
    },
    BadNumber(&'static str),
    BadBlend(String),
    Empty,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::NotACategory(name) => write!(f, "{name} is not one of the 24 categories"),
            RenderError::Split { name, options } => {
                write!(f, "{name} splits into {}; pick one", options.join(" and "))
            }
            RenderError::BurstDetector { name, may_be } => {
                write!(f, "{name} may be {}; pick one", may_be.join(", "))
            }
            RenderError::BadNumber(what) => write!(f, "{what} is not a finite number in range"),
            RenderError::BadBlend(what) => write!(f, "blend: {what}"),
            RenderError::Empty => write!(f, "no ticks"),
        }
    }
}

impl std::error::Error for RenderError {}
