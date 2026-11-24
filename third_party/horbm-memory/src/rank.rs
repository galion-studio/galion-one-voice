// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Recency, importance, and pin boost for the memory short list.
//!
//! After reciprocal rank fusion the memory pool is rescored on at most
//! [`crate::store::POOL`] rows:
//!
//! `score' = rrf × recency × importance × pin_boost`
//!
//! Recency is `exp(-ln(2) / 30 × age_days)`: a 30-day half-life on the newer
//! of the fact's event date and `last_access`, when that date is stored.
//! Query does not write `last_access`. Pins multiply by [`PIN_BOOST`] (2).
//! The live ring is the current call and is not rescaled. Knowledge hits
//! never enter this function.
//!
//! Neither Galion Research's Rust tree nor the previous HORBM ranker had
//! this weight. The shape follows option A: pins outrank candidates when
//! the underlying fusion scores are close, and old unpinned chatter fades.

use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed multiplier for a pinned fact. Documented constant, not a learned weight.
pub const PIN_BOOST: f32 = 2.0;
/// Half-life of a memory fact, in days.
pub const RECENCY_HALF_LIFE_DAYS: f32 = 30.0;
/// Diary rows and episodes that were stored before importance existed.
pub const DEFAULT_IMPORTANCE: f32 = 0.5;
/// A raw transcript hit, when a session is kept whole, weighs less than a fact.
pub const TRANSCRIPT_IMPORTANCE: f32 = 0.35;

pub fn today_utc_days() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_secs() as i64 / 86_400,
        Err(_) => 0,
    }
}

/// `1.0` when the date is missing or in the future. Age is never negative.
pub fn recency_factor(event_date: &str, last_access: &str, today_days: i64) -> f32 {
    let event = parse_days(event_date);
    let access = parse_days(last_access);
    let when = match (event, access) {
        (Some(event_days), Some(access_days)) => Some(event_days.max(access_days)),
        (Some(event_days), None) => Some(event_days),
        (None, Some(access_days)) => Some(access_days),
        (None, None) => None,
    };
    let Some(when) = when else {
        return 1.0;
    };
    let age = (today_days - when).max(0) as f32;
    let lambda = std::f32::consts::LN_2 / RECENCY_HALF_LIFE_DAYS;
    (-lambda * age).exp()
}

pub fn rescale(rrf: f32, importance: f32, pinned: bool, recency: f32) -> f32 {
    let importance = if importance > 0.0 {
        importance
    } else {
        DEFAULT_IMPORTANCE
    };
    let pin = if pinned { PIN_BOOST } else { 1.0 };
    rrf * recency * importance * pin
}

pub(crate) fn parse_days(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.len() >= 10 {
        let bytes = raw.as_bytes();
        if bytes.get(4) == Some(&b'-') && bytes.get(7) == Some(&b'-') {
            let year: i32 = raw.get(0..4)?.parse().ok()?;
            let month: u32 = raw.get(5..7)?.parse().ok()?;
            let day: u32 = raw.get(8..10)?.parse().ok()?;
            if (1..=12).contains(&month) && (1..=31).contains(&day) {
                return Some(days_from_civil(year, month, day));
            }
        }
    }
    if raw.len() == 4 {
        if let Ok(year) = raw.parse::<i32>() {
            return Some(days_from_civil(year, 1, 1));
        }
    }
    None
}

/// Days since 1970-01-01. Howard Hinnant's `days_from_civil`.
pub(crate) fn days_from_civil(mut year: i32, month: u32, day: u32) -> i64 {
    year -= i32::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = (year - era * 400) as u32;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = yoe * 365 + yoe / 4 - yoe / 100 + day_of_year;
    era as i64 * 146_097 + day_of_era as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            days_from_civil(2026, 11, 18) - days_from_civil(2026, 10, 19),
            30
        );
    }

    #[test]
    fn half_life_and_pin_boost() {
        let today = days_from_civil(2026, 11, 18);
        let fresh = recency_factor("2026-11-18", "", today);
        assert!((fresh - 1.0).abs() < 0.001);
        let month = recency_factor("2026-10-19", "", today);
        assert!((month - 0.5).abs() < 0.02, "recency {month}");
        assert!((recency_factor("not a date", "", today) - 1.0).abs() < 0.001);
        let pinned = rescale(0.1, 1.0, true, 1.0);
        let candidate = rescale(0.1, 0.92, false, 1.0);
        assert!(pinned > candidate);
        assert!((rescale(0.1, 0.0, false, 1.0) - 0.1 * DEFAULT_IMPORTANCE).abs() < 0.0001);
    }
}
