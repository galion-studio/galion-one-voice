// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Fant's Rd mapped to an open quotient.
//!
//! Fant and Kruckenberg (1996), TMH-QPSR 37(4), the same coefficients as
//! Fant (1995): Ra and Rk from Rd, Rg from their rearranged Rd definition,
//! open quotient (1 + Rk) / (2 Rg). This is the algebra only. No period is
//! sampled. The waveform sampler in `tools/symphony/throat` is not linked.

use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shape {
    pub ra: f64,
    pub rk: f64,
    pub rg: f64,
    pub open_quotient: f64,
}

pub fn shape(rd: f64) -> Result<Shape, Error> {
    if !rd.is_finite() {
        return Err(Error::NonFinite("rd"));
    }
    if !(0.3..=2.7).contains(&rd) {
        return Err(Error::OutOfRange("rd"));
    }
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    if ra <= 0.0 || rk <= 0.0 {
        return Err(Error::OutOfRange("rd"));
    }
    let a = 0.5 + 1.2 * rk;
    let denom = 0.44 * rd - 4.0 * ra * a;
    if denom <= 0.0 {
        return Err(Error::OutOfRange("rd"));
    }
    let rg = rk * a / denom;
    Ok(Shape {
        ra,
        rk,
        rg,
        open_quotient: (1.0 + rk) / (2.0 * rg),
    })
}

pub fn open_quotient_from_rd(rd: f64) -> Result<f64, Error> {
    Ok(shape(rd)?.open_quotient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_rd_rows_keep_their_open_quotient() {
        // Fant and Kruckenberg 1996, Table 2, percentages.
        let published = [
            (0.3, 0.44, 25.94, 35.0),
            (0.5, 1.40, 28.3, 47.0),
            (0.7, 2.36, 30.7, 55.5),
            (1.0, 3.80, 34.2, 65.0),
            (1.4, 5.72, 38.9, 73.0),
            (2.0, 8.60, 46.0, 78.0),
            (2.7, 11.96, 54.3, 79.0),
        ];
        for (rd, ra_pct, rk_pct, oq_pct) in published {
            let s = shape(rd).unwrap();
            assert!((s.ra * 100.0 - ra_pct).abs() < 0.05, "Ra {rd}");
            assert!((s.rk * 100.0 - rk_pct).abs() < 0.15, "Rk {rd}");
            assert!(
                (s.open_quotient * 100.0 - oq_pct).abs() < 1.0,
                "OQ {rd} got {}",
                s.open_quotient * 100.0
            );
        }
    }

    #[test]
    fn a_higher_rd_is_a_more_open_glottis() {
        let pressed = open_quotient_from_rd(0.5).unwrap();
        let modal = open_quotient_from_rd(1.0).unwrap();
        let breathy = open_quotient_from_rd(2.7).unwrap();
        assert!(pressed < modal && modal < breathy);
        assert!(open_quotient_from_rd(0.0).is_err());
        assert!(open_quotient_from_rd(f64::NAN).is_err());
    }
}
