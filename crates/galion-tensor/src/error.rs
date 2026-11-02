// Galion Voice — Perpetual Open MIT License
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TensorError {
    ShapeMismatch { op: &'static str, lhs: Vec<usize>, rhs: Vec<usize> },
    InvalidShape(String),
    DTypeUnsupported { op: &'static str, dtype: crate::DType },
    IndexOutOfBounds { index: usize, bound: usize },
    Unimplemented(&'static str),
}

impl fmt::Display for TensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShapeMismatch { op, lhs, rhs } => write!(f, "{op}: shape mismatch {lhs:?} vs {rhs:?}"),
            Self::InvalidShape(s) => write!(f, "invalid shape: {s}"),
            Self::DTypeUnsupported { op, dtype } => write!(f, "{op}: dtype {dtype:?} unsupported"),
            Self::IndexOutOfBounds { index, bound } => write!(f, "index {index} out of bounds {bound}"),
            Self::Unimplemented(s) => write!(f, "unimplemented: {s}"),
        }
    }
}
impl std::error::Error for TensorError {}
pub type Result<T> = std::result::Result<T, TensorError>;
