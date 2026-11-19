// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Liljencrants–Fant glottal flow derivative, driven by Fant's Rd.
//!
//! Shape, from Fant (1995), STL-QPSR 36(2–3), as restated with the same
//! coefficients in Fant and Kruckenberg (1996), TMH-QPSR 37(4), equations
//! (6), (7) and (4):
//!
//!   Ra = (-1 + 4.8 Rd) / 100
//!   Rk = (22.4 + 11.8 Rd) / 100
//!   Rd = (1 / 0.11) (0.5 + 1.2 Rk) (Rk / (4 Rg) + Ra)
//!
//! solved for Rg. Open quotient without the return phase, their equation (1)
//! with the Ra term dropped, as they say they usually do:
//!
//!   OQ = (1 + Rk) / (2 Rg)
//!
//! Fa = F0 / (2 π Ra), the frequency where the flow-derivative spectrum
//! steepens by a further 6 dB/octave.
//!
//! The waveform is Fant, Liljencrants and Lin (1985). E0 is fixed so the
//! derivative equals -Ee at closure. Alpha is the unique value that makes
//! the signed area of one period zero (flow starts and ends at the same
//! level). Gobl (2017) restates that area constraint; the solver here is
//! bisection on that constraint, not Gobl's later Newton correction of Rd.

use crate::config::Config;

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub rd: f64,
    pub rd_clamped: bool,
    pub ra: f64,
    pub rk: f64,
    pub rg: f64,
    /// (1 + Rk) / (2 Rg). Fant and Kruckenberg's tabulated open quotient.
    pub open_quotient: f64,
    /// Their equation (1) with the return phase kept: OQ + Ra.
    pub open_quotient_with_return: f64,
    pub f0_hz: f64,
    pub ee: f64,
    pub fa_hz: f64,
    pub t0: f64,
    pub tp: f64,
    pub te: f64,
    pub ta: f64,
    pub tb: f64,
    pub omega_g: f64,
    pub alpha: f64,
    pub e0: f64,
    pub spectral_tilt_db_oct: f64,
}

pub fn shape_for(rd: f64, f0_hz: f64, ee: f64, cfg: &Config) -> Result<Shape, String> {
    let mut shape = wave(rd, f0_hz, ee, cfg.rd_min, cfg.rd_max)?;
    let period = sample_period(&shape, cfg.source_hz);
    shape.spectral_tilt_db_oct =
        spectral_tilt_db_oct(&period, cfg.tilt_harmonic_from, cfg.tilt_harmonic_to)
            .ok_or("spectral tilt could not be estimated from the LF period")?;
    Ok(shape)
}

/// Waveform parameters without the harmonic-slope estimate.
/// Alpha does not depend on Ee: the area constraint scales with Ee.
pub fn wave(rd: f64, f0_hz: f64, ee: f64, rd_min: f64, rd_max: f64) -> Result<Shape, String> {
    if !rd.is_finite() || !f0_hz.is_finite() || !ee.is_finite() {
        return Err("Rd, F0 and Ee must be finite".into());
    }
    if f0_hz <= 0.0 {
        return Err("F0 must be positive".into());
    }
    if ee < 0.0 {
        return Err("Ee must be non-negative".into());
    }
    let rd_clamped = rd < rd_min || rd > rd_max;
    let rd = rd.clamp(rd_min, rd_max);
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    if ra <= 0.0 || rk <= 0.0 {
        return Err("Rd produced a non-positive Ra or Rk".into());
    }
    let a = 0.5 + 1.2 * rk;
    let denom = 0.44 * rd - 4.0 * ra * a;
    if denom <= 0.0 {
        return Err("Rd produced a non-positive Rg denominator".into());
    }
    let rg = rk * a / denom;
    let open_quotient = (1.0 + rk) / (2.0 * rg);
    let t0 = 1.0 / f0_hz;
    let tp = t0 / (2.0 * rg);
    let te = tp * (1.0 + rk);
    let ta = ra * t0;
    let tb = t0 - te;
    if tb <= 0.0 || ta <= 0.0 {
        return Err("LF timing left no return phase".into());
    }
    let omega_g = std::f64::consts::PI / tp;
    let alpha = solve_alpha(1.0, te, omega_g, ta, tb)?;
    let s = (omega_g * te).sin();
    let e0 = if ee == 0.0 {
        0.0
    } else {
        -ee / ((alpha * te).exp() * s)
    };
    let fa_hz = f0_hz / (2.0 * std::f64::consts::PI * ra);
    Ok(Shape {
        rd,
        rd_clamped,
        ra,
        rk,
        rg,
        open_quotient,
        open_quotient_with_return: open_quotient + ra,
        f0_hz,
        ee,
        fa_hz,
        t0,
        tp,
        te,
        ta,
        tb,
        omega_g,
        alpha,
        e0,
        spectral_tilt_db_oct: 0.0,
    })
}

/// Flow derivative at time `t` seconds from the start of the period.
/// Ee is the positive excitation strength; the value at closure is -Ee.
pub fn derivative_at(shape: &Shape, t: f64) -> f64 {
    if t < 0.0 || t > shape.t0 {
        return 0.0;
    }
    if t <= shape.te {
        shape.e0 * (shape.alpha * t).exp() * (shape.omega_g * t).sin()
    } else {
        let u = t - shape.te;
        let eps = 1.0 - (-shape.tb / shape.ta).exp();
        -shape.ee * ((-u / shape.ta).exp() - (-shape.tb / shape.ta).exp()) / eps
    }
}

pub fn sample_period(shape: &Shape, sr: u32) -> Vec<f64> {
    let n = (sr as f64 / shape.f0_hz).round().max(2.0) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / sr as f64;
        out.push(if t >= shape.t0 {
            0.0
        } else {
            derivative_at(shape, t)
        });
    }
    out
}

/// Least-squares slope of harmonic magnitude, dB per octave, from harmonic
/// `k0` through `k1`. This is a property of the model pulse, not a measured
/// spectrum of a person.
pub fn spectral_tilt_db_oct(period: &[f64], k0: u32, k1: u32) -> Option<f64> {
    let n = period.len();
    if n < 8 || k1 <= k0 {
        return None;
    }
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for k in k0..=k1 {
        if k as usize >= n / 2 {
            break;
        }
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, x) in period.iter().enumerate() {
            let ang = -2.0 * std::f64::consts::PI * k as f64 * i as f64 / n as f64;
            re += x * ang.cos();
            im += x * ang.sin();
        }
        let mag = (re * re + im * im).sqrt();
        if mag <= 0.0 {
            return None;
        }
        xs.push((k as f64).log2());
        ys.push(20.0 * mag.log10());
    }
    if xs.len() < 4 {
        return None;
    }
    let mx = xs.iter().sum::<f64>() / xs.len() as f64;
    let my = ys.iter().sum::<f64>() / ys.len() as f64;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

fn solve_alpha(ee: f64, te: f64, wg: f64, ta: f64, tb: f64) -> Result<f64, String> {
    let s = (wg * te).sin();
    let c = (wg * te).cos();
    if s.abs() < 1e-8 {
        return Err("LF open phase is singular at closure".into());
    }
    let eps = 1.0 - (-tb / ta).exp();
    if eps.abs() < 1e-12 {
        return Err("LF return phase is degenerate".into());
    }
    let iret = -ee * (ta - tb * (-tb / ta).exp() / eps);
    let f = |alpha: f64| -> f64 {
        let e0 = -ee / ((alpha * te).exp() * s);
        let den = alpha * alpha + wg * wg;
        let iopen = e0 / den * ((alpha * te).exp() * (alpha * s - wg * c) + wg);
        iopen + iret
    };
    let mut lo = -5_000.0;
    let mut hi = 80_000.0;
    let mut ylo = f(lo);
    let mut yhi = f(hi);
    for _ in 0..6 {
        if ylo * yhi < 0.0 {
            break;
        }
        lo *= 2.0;
        hi *= 2.0;
        ylo = f(lo);
        yhi = f(hi);
    }
    if ylo * yhi > 0.0 {
        return Err("LF alpha has no bracket".into());
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let ym = f(mid);
        if ylo * ym <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
            ylo = ym;
        }
    }
    Ok(0.5 * (lo + hi))
}

pub fn phonation_label(rd: f64, creaky: bool, cfg: &Config) -> &'static str {
    if creaky {
        return "creaky";
    }
    if rd <= cfg.pressed_rd_max {
        "pressed"
    } else if rd >= cfg.breathy_rd_min {
        "breathy"
    } else {
        "modal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        crate::config::parse(include_str!("../../../../configs/symphony/throat.toml")).unwrap()
    }

    #[test]
    fn rd_table_matches_fant_kruckenberg_1996() {
        // Table 2, percentages. Fap for Rd = 0.5 in the typeset table does not
        // match Fa = F0 / (2 π Ra); every other row does, so Fa is the formula.
        let published = [
            (0.3, 0.44, 25.94, 179.0, 35.0),
            (0.5, 1.40, 28.3, 137.0, 47.0),
            (0.7, 2.36, 30.7, 118.0, 55.5),
            (1.0, 3.80, 34.2, 103.0, 65.0),
            (1.4, 5.72, 38.9, 95.0, 73.0),
            (2.0, 8.60, 46.0, 93.5, 78.0),
            (2.7, 11.96, 54.3, 98.0, 79.0),
        ];
        let cfg = cfg();
        for (rd, ra_pct, rk_pct, rg_pct, oq_pct) in published {
            let s = shape_for(rd, 100.0, 1.0, &cfg).unwrap();
            assert!((s.ra * 100.0 - ra_pct).abs() < 0.05, "Ra {rd}");
            assert!((s.rk * 100.0 - rk_pct).abs() < 0.15, "Rk {rd}");
            assert!(
                (s.rg * 100.0 - rg_pct).abs() < 1.0,
                "Rg {rd} got {}",
                s.rg * 100.0
            );
            assert!(
                (s.open_quotient * 100.0 - oq_pct).abs() < 1.0,
                "OQ {rd} got {}",
                s.open_quotient * 100.0
            );
            let fa = 100.0 / (2.0 * std::f64::consts::PI * s.ra);
            assert!((s.fa_hz - fa).abs() < 1e-6);
            let at_closure = derivative_at(&s, s.te);
            assert!((at_closure + 1.0).abs() < 1e-6, "closure {at_closure}");
            assert!(derivative_at(&s, s.t0).abs() < 1e-6);
        }
    }

    #[test]
    fn higher_rd_is_more_open_and_more_tilted() {
        let cfg = cfg();
        let pressed = shape_for(0.5, 100.0, 1.0, &cfg).unwrap();
        let breathy = shape_for(2.7, 100.0, 1.0, &cfg).unwrap();
        assert!(breathy.open_quotient > pressed.open_quotient);
        assert!(breathy.fa_hz < pressed.fa_hz);
        assert!(breathy.spectral_tilt_db_oct < pressed.spectral_tilt_db_oct);
        assert_eq!(phonation_label(pressed.rd, false, &cfg), "pressed");
        assert_eq!(phonation_label(breathy.rd, false, &cfg), "breathy");
        assert_eq!(phonation_label(1.0, false, &cfg), "modal");
        assert_eq!(phonation_label(1.0, true, &cfg), "creaky");
    }

    #[test]
    fn flow_returns_and_stays_non_negative() {
        let cfg = cfg();
        let s = shape_for(1.0, 120.0, 1.0, &cfg).unwrap();
        let period = sample_period(&s, cfg.source_hz);
        let dt = 1.0 / cfg.source_hz as f64;
        let mut flow = 0.0;
        let mut min_flow = 0.0_f64;
        for x in &period {
            flow += x * dt;
            min_flow = min_flow.min(flow);
        }
        assert!(flow.abs() < 0.02, "end flow {flow}");
        assert!(min_flow > -0.02, "min flow {min_flow}");
    }
}
