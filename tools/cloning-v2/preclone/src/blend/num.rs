// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The same decimal rounding the pre-clone profile uses.
//!
//! A published number is the value you get by writing it and reading it
//! back, so the guard and the file judge the same digits.

use crate::profile::fmt_f64;

pub(crate) fn publish(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    fmt_f64(x).parse::<f64>().unwrap_or(x)
}

pub(crate) fn fmt(x: f64) -> String {
    fmt_f64(x)
}
