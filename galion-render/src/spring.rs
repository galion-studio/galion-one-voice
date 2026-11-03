//! Scalar spring-damper used by the orb.
//!
//! This is the same semi-implicit Euler step as `function spring` in
//! `crates/galion-console/assets/v1-screen.html`: acceleration from a
//! stiffness and a damping term, then velocity, then position.
//!
//! `dt` is seconds of display time. It is not the 80 ms heartbeat.
//! Steps at 1/60, 1/120, and 1/240 all fall inside the page's clamp
//! and are integrated at that real `dt`. A non-positive or non-finite
//! `dt` does not move the spring. A stalled frame above 50 ms is
//! clamped so one hitch cannot blow the step up. The page also forces
//! a 1 ms minimum (`Math.max(0.001, dt/1000)`), including when `dt` is 0.
//! This step keeps that floor only for a positive finite `dt`.
//!
//! Non-finite position, velocity, target, or gain never comes back out
//! as NaN. The page returns `[target, 0]` after a bad step, which is
//! still NaN when `target` is NaN. Here a bad target holds the last
//! finite position and a bad step snaps to a finite target with zero
//! velocity.

/// Seconds. Matches the page: `Math.max(0.001, dt_seconds)`.
pub const DT_MIN_S: f32 = 0.001;
/// Seconds. Matches the page: `Math.min(0.05, …)`.
pub const DT_MAX_S: f32 = 0.05;

/// Angular frequency `w` (rad/s) and damping ratio `z`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringGain {
    pub omega: f32,
    pub zeta: f32,
}

/// Result of one [`Spring::step`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringStep {
    pub position: f32,
    pub velocity: f32,
    /// Seconds actually integrated. `0.0` when the call was refused.
    pub dt_s: f32,
}

/// One degree of freedom: position and velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub position: f32,
    pub velocity: f32,
}

impl Spring {
    pub fn new(position: f32, velocity: f32) -> Self {
        Self {
            position: if position.is_finite() { position } else { 0.0 },
            velocity: if velocity.is_finite() { velocity } else { 0.0 },
        }
    }

    /// Advance one display frame toward `target`.
    pub fn step(&mut self, target: f32, dt_s: f32, gain: SpringGain) -> SpringStep {
        if !self.position.is_finite() {
            self.position = if target.is_finite() { target } else { 0.0 };
            self.velocity = 0.0;
        } else if !self.velocity.is_finite() {
            self.velocity = 0.0;
        }
        let Some(h) = integrate_dt(dt_s) else {
            return SpringStep {
                position: self.position,
                velocity: self.velocity,
                dt_s: 0.0,
            };
        };
        if !target.is_finite() || !gain.omega.is_finite() || !gain.zeta.is_finite() {
            return SpringStep {
                position: self.position,
                velocity: self.velocity,
                dt_s: 0.0,
            };
        }
        let w = gain.omega;
        let z = gain.zeta;
        let a = -w * w * (self.position - target) - 2.0 * z * w * self.velocity;
        let v = self.velocity + a * h;
        let p = self.position + v * h;
        if !a.is_finite() || !v.is_finite() || !p.is_finite() {
            self.position = target;
            self.velocity = 0.0;
            return SpringStep {
                position: target,
                velocity: 0.0,
                dt_s: h,
            };
        }
        self.position = p;
        self.velocity = v;
        SpringStep {
            position: p,
            velocity: v,
            dt_s: h,
        }
    }
}

/// Display `dt` in seconds, after the page's safety clamp.
///
/// `None` for a non-positive or non-finite value (no step).
/// `Some(dt)` unchanged when `dt` is inside `[DT_MIN_S, DT_MAX_S]`.
pub fn integrate_dt(dt_s: f32) -> Option<f32> {
    if !dt_s.is_finite() || dt_s <= 0.0 {
        None
    } else {
        Some(dt_s.clamp(DT_MIN_S, DT_MAX_S))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain() -> SpringGain {
        SpringGain {
            omega: 11.0,
            zeta: 0.70,
        }
    }

    #[test]
    fn display_rates_keep_their_dt() {
        for dt in [1.0 / 60.0, 1.0 / 120.0, 1.0 / 240.0] {
            let used = integrate_dt(dt).unwrap();
            assert_eq!(used, dt);
            let mut s = Spring::new(0.1, 0.0);
            let step = s.step(0.2, dt, gain());
            assert_eq!(step.dt_s, dt);
            assert!(step.position.is_finite() && step.velocity.is_finite());
        }
    }

    #[test]
    fn hitch_clamps_to_50ms_and_zero_dt_holds() {
        assert_eq!(integrate_dt(0.080).unwrap(), DT_MAX_S);
        assert_eq!(integrate_dt(1.0).unwrap(), DT_MAX_S);
        assert_eq!(integrate_dt(1e-6).unwrap(), DT_MIN_S);
        assert!(integrate_dt(0.0).is_none());
        assert!(integrate_dt(-0.016).is_none());
        assert!(integrate_dt(f32::NAN).is_none());
        assert!(integrate_dt(f32::INFINITY).is_none());

        let mut s = Spring::new(0.12, 0.4);
        let before = s;
        let step = s.step(0.2, 0.0, gain());
        assert_eq!(step.dt_s, 0.0);
        assert_eq!(s, before);
    }

    #[test]
    fn equilibrium_is_stationary() {
        let mut s = Spring::new(0.15, 0.0);
        for dt in [1.0 / 60.0, 1.0 / 120.0, 1.0 / 240.0] {
            for _ in 0..32 {
                s.step(0.15, dt, gain());
            }
            assert!((s.position - 0.15).abs() < 1e-6);
            assert!(s.velocity.abs() < 1e-6);
        }
    }

    #[test]
    fn settles_at_three_rates() {
        for dt in [1.0 / 60.0, 1.0 / 120.0, 1.0 / 240.0] {
            let mut s = Spring::new(0.0, 0.0);
            let mut t = 0.0f32;
            while t < 4.0 {
                let step = s.step(0.2, dt, gain());
                assert!(step.position.is_finite() && step.velocity.is_finite());
                assert!(step.position.abs() < 2.0);
                t += dt;
            }
            assert!(
                (s.position - 0.2).abs() < 1e-3,
                "dt {dt} pos {}",
                s.position
            );
            assert!(s.velocity.abs() < 1e-2, "dt {dt} vel {}", s.velocity);
        }
    }

    #[test]
    fn finer_dt_is_closer_to_a_fine_step() {
        fn at(dt: f32, frames: u32) -> f32 {
            let mut s = Spring::new(0.02, 0.0);
            for _ in 0..frames {
                s.step(
                    0.2,
                    dt,
                    SpringGain {
                        omega: 18.0,
                        zeta: 0.58,
                    },
                );
            }
            s.position
        }
        // ~50 ms of a stiff numerical spring (w = 18). That gain is a
        // step-size check, not the phone's speaking aim (see OrbMotion).
        // Finer dt lands closer to the 960 Hz reference. 240 Hz stays
        // within a few thousandths of a radius-fraction (~0.18 of travel).
        let fine = at(1.0 / 960.0, 48);
        let e60 = (at(1.0 / 60.0, 3) - fine).abs();
        let e120 = (at(1.0 / 120.0, 6) - fine).abs();
        let e240 = (at(1.0 / 240.0, 12) - fine).abs();
        assert!(
            e240 < e120 && e120 < e60,
            "e240 {e240} e120 {e120} e60 {e60} fine {fine}"
        );
        assert!(e240 < 4.0e-3, "e240 {e240} fine {fine}");
    }

    #[test]
    fn variable_mix_stays_finite_and_near_the_target() {
        let dts = [1.0 / 60.0, 1.0 / 240.0, 1.0 / 120.0, 0.080, 1.0 / 240.0];
        let mut s = Spring::new(0.0, 1.0);
        let mut t = 0.0f32;
        let mut i = 0usize;
        while t < 3.0 {
            let dt = dts[i % dts.len()];
            let step = s.step(
                0.11,
                dt,
                SpringGain {
                    omega: 6.5,
                    zeta: 0.48,
                },
            );
            assert!(step.position.is_finite() && step.velocity.is_finite());
            assert!(step.position > -1.0 && step.position < 1.0);
            t += dt;
            i += 1;
        }
        assert!((s.position - 0.11).abs() < 2e-2, "pos {}", s.position);
    }

    #[test]
    fn non_finite_inputs_do_not_produce_nan() {
        let mut s = Spring::new(0.1, -0.2);
        let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        for target in bad {
            let step = s.step(target, 1.0 / 60.0, gain());
            assert!(step.position.is_finite() && step.velocity.is_finite());
            assert!((step.position - 0.1).abs() < 1e-6);
        }
        for g in [
            SpringGain {
                omega: f32::NAN,
                zeta: 0.5,
            },
            SpringGain {
                omega: 4.0,
                zeta: f32::INFINITY,
            },
            SpringGain {
                omega: f32::NEG_INFINITY,
                zeta: f32::NAN,
            },
        ] {
            let step = s.step(0.2, 1.0 / 120.0, g);
            assert!(step.position.is_finite() && step.velocity.is_finite());
        }
        let step = s.step(0.2, f32::NAN, gain());
        assert!(step.position.is_finite() && step.dt_s == 0.0);

        let mut wild = Spring::new(f32::NAN, f32::NAN);
        let step = wild.step(0.3, 1.0 / 240.0, gain());
        assert!(step.position.is_finite() && step.velocity.is_finite());
    }

    #[test]
    fn unstable_gain_snaps_instead_of_returning_nan() {
        let mut s = Spring::new(0.0, 0.0);
        for _ in 0..20_000 {
            let step = s.step(
                0.2,
                1.0 / 60.0,
                SpringGain {
                    omega: 80.0,
                    zeta: -4.0,
                },
            );
            assert!(step.position.is_finite() && step.velocity.is_finite());
        }
    }
}
