//! Display-rate orb integrator.
//!
//! [`crate::dot::DotParams`] is the discrete golden: one value per 80 ms
//! heartbeat frame, hashed by G4. This engine does not replace it and is
//! not called from [`crate::NullSurface`]. A display loop (the phone page
//! today, a future softbuffer thread later) feeds those targets in and
//! steps radius and brightness with the real frame `dt`.
//!
//! Do not call [`OrbEngine::step`] from `Heartbeat::step`. The heartbeat
//! is the 12.5 Hz audio clock. This loop runs only on the animation
//! thread, at whatever vsync the display actually delivered.
//!
//! Thinking and offline are motions of the same mark. They are not
//! [`crate::dot::DotState`] values. `LOCKS.toml` `dot_states` stays
//! idle / listening / speaking.

use crate::dot::{DotParams, DotState};
use crate::spring::{integrate_dt, Spring, SpringGain};

/// Display rates the engine is tested at. The step itself takes a `dt`,
/// so a panel that vsyncs at some other rate passes that `dt` through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DisplayHz {
    Hz60 = 60,
    Hz120 = 120,
    Hz240 = 240,
}

impl DisplayHz {
    pub const ALL: [DisplayHz; 3] = [DisplayHz::Hz60, DisplayHz::Hz120, DisplayHz::Hz240];

    pub fn hz(self) -> u32 {
        self as u32
    }

    pub fn dt_s(self) -> f32 {
        1.0 / self.hz() as f32
    }
}

/// Page eases toward these aims. A mode change does not apply them in one frame.
/// Time constant matches `GAIN_TAU` (480 ms) in `v1-screen.html`.
const GAIN_TAU_S: f32 = 0.48;

/// Which spring the page uses. Aims match `SPRING` in `v1-screen.html`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrbMotion {
    Idle,
    Listening,
    /// Gap between a turn being accepted and playback starting.
    /// Not a locked dot state.
    Thinking,
    Speaking,
    Offline,
}

impl OrbMotion {
    pub fn as_str(self) -> &'static str {
        match self {
            OrbMotion::Idle => "idle",
            OrbMotion::Listening => "listening",
            OrbMotion::Thinking => "thinking",
            OrbMotion::Speaking => "speaking",
            OrbMotion::Offline => "offline",
        }
    }

    /// Radius-channel aim. [`OrbEngine::step`] eases toward this (480 ms)
    /// so a mode change does not retune the spring in one frame.
    /// Brightness uses [`OrbMotion::brightness_gain`] from the same aim.
    /// The phone uses brightness for glow and a little size, not fill pigment.
    pub fn radius_gain(self) -> SpringGain {
        let (omega, zeta) = match self {
            OrbMotion::Idle => (1.8, 0.92),
            OrbMotion::Listening => (3.4, 0.88),
            OrbMotion::Thinking => (2.4, 0.90),
            OrbMotion::Speaking => (3.0, 0.92),
            OrbMotion::Offline => (2.0, 1.02),
        };
        SpringGain { omega, zeta }
    }

    /// Page: brightness spring is `w * 0.85` and `min(1.2, z + 0.08)`.
    pub fn brightness_gain(self) -> SpringGain {
        let g = self.radius_gain();
        SpringGain {
            omega: g.omega * 0.85,
            zeta: (g.zeta + 0.08).min(1.2),
        }
    }

    fn from_dot_state(state: DotState) -> Self {
        match state {
            DotState::Idle => OrbMotion::Idle,
            DotState::Listening => OrbMotion::Listening,
            DotState::Speaking => OrbMotion::Speaking,
        }
    }
}

/// One display sample. `radius` and `brightness` are integrator positions
/// (they may sit just outside the draw range while the mark is in flight).
/// [`OrbSample::draw_radius`] and [`OrbSample::draw_brightness`] are what
/// a framebuffer should use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbSample {
    pub motion: OrbMotion,
    pub radius: f32,
    pub brightness: f32,
    pub radius_v: f32,
    pub brightness_v: f32,
    /// Held at 1. The mark is a circle. Velocity used to squash this
    /// (`radius_v * 6`, clamp ±0.14, draw clamp 0.82..=1.18) and the
    /// phone flipped between wide and tall ovals.
    pub aspect: f32,
    /// Seconds integrated for this sample. `0.0` if the step was refused.
    pub dt_s: f32,
}

impl OrbSample {
    pub fn draw_radius(self) -> f32 {
        clamp_finite(self.radius, 0.0, 0.5)
    }
    pub fn draw_brightness(self) -> f32 {
        clamp_finite(self.brightness, 0.0, 1.0)
    }

    /// Draw snapshot for a later web or wasm client. Not a framebuffer,
    /// and not a substitute for `/dot` targets.
    pub fn to_json(self) -> String {
        format!(
            "{{\"motion\":\"{}\",\"radius\":{:.4},\"brightness\":{:.4},\"aspect\":{:.4},\"radius_v\":{:.4},\"brightness_v\":{:.4},\"dt_s\":{:.6}}}",
            self.motion.as_str(),
            self.radius,
            self.brightness,
            self.aspect,
            self.radius_v,
            self.brightness_v,
            self.dt_s,
        )
    }
}

/// Radius and brightness, integrated in display time. Aspect stays 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbEngine {
    radius: Spring,
    brightness: Spring,
    aspect: f32,
    /// Smoothed toward [`OrbMotion::radius_gain`]. Starts at the idle aim.
    omega: f32,
    zeta: f32,
}

impl OrbEngine {
    /// Rest at a finite pose. Non-finite inputs fall back to the idle centre
    /// (`DotParams::IDLE_RADIUS`, brightness `0.84`).
    pub fn at_rest(radius: f32, brightness: f32) -> Self {
        let radius = if radius.is_finite() {
            radius.clamp(0.0, 0.5)
        } else {
            DotParams::IDLE_RADIUS
        };
        let brightness = if brightness.is_finite() {
            brightness.clamp(0.0, 1.0)
        } else {
            0.84
        };
        let idle = OrbMotion::Idle.radius_gain();
        Self {
            radius: Spring::new(radius, 0.0),
            brightness: Spring::new(brightness, 0.0),
            aspect: 1.0,
            omega: idle.omega,
            zeta: idle.zeta,
        }
    }

    /// Page rest pose: idle radius, brightness `0.84`.
    pub fn idle() -> Self {
        Self::at_rest(DotParams::IDLE_RADIUS, 0.84)
    }

    pub fn radius(&self) -> f32 {
        self.radius.position
    }
    pub fn radius_velocity(&self) -> f32 {
        self.radius.velocity
    }
    pub fn brightness(&self) -> f32 {
        self.brightness.position
    }
    pub fn brightness_velocity(&self) -> f32 {
        self.brightness.velocity
    }
    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    /// Step toward explicit targets. Targets outside the mark's range are
    /// clamped before the force is applied (`radius` 0..=0.5, `brightness`
    /// 0..=1). A non-finite target or `dt` holds the current pose.
    pub fn step(
        &mut self,
        motion: OrbMotion,
        radius_target: f32,
        brightness_target: f32,
        dt_s: f32,
    ) -> OrbSample {
        if !radius_target.is_finite()
            || !brightness_target.is_finite()
            || integrate_dt(dt_s).is_none()
        {
            return self.sample(motion, 0.0);
        }
        let radius_target = radius_target.clamp(0.0, 0.5);
        let brightness_target = brightness_target.clamp(0.0, 1.0);
        let radius_gain = self.ease_radius_gain(motion, dt_s);
        let bright_gain = SpringGain {
            omega: radius_gain.omega * 0.85,
            zeta: (radius_gain.zeta + 0.08).min(1.2),
        };
        let radius = self.radius.step(radius_target, dt_s, radius_gain);
        let bright = self.brightness.step(brightness_target, dt_s, bright_gain);
        self.aspect = 1.0;
        self.sample(motion, radius.dt_s.max(bright.dt_s))
    }

    /// Use a [`DotParams`] golden as the target. The dot is not modified.
    /// Rika lean-in stays idle motion: the lean is already in the radius.
    /// Thinking is not inferred from a dot state.
    pub fn step_toward_dot(&mut self, dot: &DotParams, dt_s: f32) -> OrbSample {
        self.step(
            OrbMotion::from_dot_state(dot.state),
            dot.radius,
            dot.brightness,
            dt_s,
        )
    }

    /// Ease ω and ζ toward the motion's aim. One frame never snaps the gain.
    fn ease_radius_gain(&mut self, motion: OrbMotion, dt_s: f32) -> SpringGain {
        let aim = motion.radius_gain();
        if !self.omega.is_finite() {
            self.omega = aim.omega;
        }
        if !self.zeta.is_finite() {
            self.zeta = aim.zeta;
        }
        if let Some(h) = integrate_dt(dt_s) {
            let k = 1.0 - (-h / GAIN_TAU_S).exp();
            self.omega += (aim.omega - self.omega) * k;
            self.zeta += (aim.zeta - self.zeta) * k;
        }
        SpringGain {
            omega: self.omega,
            zeta: self.zeta,
        }
    }

    fn sample(&self, motion: OrbMotion, dt_s: f32) -> OrbSample {
        OrbSample {
            motion,
            radius: finite_or(self.radius.position, 0.0),
            brightness: finite_or(self.brightness.position, 0.0),
            radius_v: finite_or(self.radius.velocity, 0.0),
            brightness_v: finite_or(self.brightness.velocity, 0.0),
            aspect: 1.0,
            dt_s,
        }
    }
}

fn finite_or(x: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        fallback
    }
}

fn clamp_finite(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Emotion, VizFrame};
    use crate::surface::{NullSurface, RenderSurface};

    fn assert_draw(s: OrbSample) {
        assert!(s.radius.is_finite() && s.brightness.is_finite() && s.aspect.is_finite());
        assert!(s.radius_v.is_finite() && s.brightness_v.is_finite() && s.dt_s.is_finite());
        assert!((0.0..=0.5).contains(&s.draw_radius()));
        assert!((0.0..=1.0).contains(&s.draw_brightness()));
        assert!((s.aspect - 1.0).abs() < 1e-6, "aspect {}", s.aspect);
        assert!(!s.to_json().contains("NaN"));
    }

    #[test]
    fn gains_match_the_phone_page() {
        let cases = [
            (OrbMotion::Idle, 1.8_f32, 0.92_f32),
            (OrbMotion::Listening, 3.4, 0.88),
            (OrbMotion::Thinking, 2.4, 0.90),
            (OrbMotion::Speaking, 3.0, 0.92),
            (OrbMotion::Offline, 2.0, 1.02),
        ];
        for (motion, w, z) in cases {
            let r = motion.radius_gain();
            assert_eq!(r.omega, w);
            assert_eq!(r.zeta, z);
            let b = motion.brightness_gain();
            assert!((b.omega - w * 0.85).abs() < 1e-6);
            assert!((b.zeta - (z + 0.08).min(1.2)).abs() < 1e-6);
        }
    }

    #[test]
    fn display_hz_covers_the_three_rates() {
        let hz: Vec<u32> = DisplayHz::ALL.iter().map(|h| h.hz()).collect();
        assert_eq!(hz, vec![60, 120, 240]);
        for h in DisplayHz::ALL {
            let dt = h.dt_s();
            assert!((dt * h.hz() as f32 - 1.0).abs() < 1e-5);
            assert_eq!(crate::spring::integrate_dt(dt), Some(dt));
        }
    }

    #[test]
    fn settles_at_60_120_and_240() {
        let motions = [
            OrbMotion::Idle,
            OrbMotion::Listening,
            OrbMotion::Thinking,
            OrbMotion::Speaking,
            OrbMotion::Offline,
        ];
        for motion in motions {
            for hz in DisplayHz::ALL {
                let mut engine = OrbEngine::at_rest(0.02, 0.20);
                let mut t = 0.0f32;
                while t < 8.0 {
                    let sample = engine.step(motion, 0.18, 0.90, hz.dt_s());
                    assert_draw(sample);
                    assert!(engine.radius() > -0.05 && engine.radius() < 0.75);
                    assert!(engine.brightness() > -0.05 && engine.brightness() < 1.25);
                    t += hz.dt_s();
                }
                assert!(
                    (engine.radius() - 0.18).abs() < 1e-3,
                    "{motion:?} {} Hz radius {}",
                    hz.hz(),
                    engine.radius()
                );
                assert!(
                    (engine.brightness() - 0.90).abs() < 1e-3,
                    "{motion:?} {} Hz brightness {}",
                    hz.hz(),
                    engine.brightness()
                );
                assert!(engine.radius_velocity().abs() < 1e-2);
                assert!(engine.brightness_velocity().abs() < 1e-2);
            }
        }
    }

    #[test]
    fn variable_dt_mix_settles() {
        let dts = [
            1.0 / 60.0,
            1.0 / 240.0,
            1.0 / 120.0,
            1.0 / 60.0,
            1.0 / 240.0,
        ];
        let mut engine = OrbEngine::idle();
        let mut t = 0.0f32;
        let mut i = 0usize;
        while t < 4.0 {
            let sample = engine.step(OrbMotion::Speaking, 0.16, 0.95, dts[i % dts.len()]);
            assert_draw(sample);
            t += dts[i % dts.len()];
            i += 1;
        }
        assert!(
            (engine.radius() - 0.16).abs() < 2e-2,
            "radius {}",
            engine.radius()
        );
        assert!(
            (engine.brightness() - 0.95).abs() < 2e-2,
            "brightness {}",
            engine.brightness()
        );
    }

    #[test]
    fn one_heartbeat_dt_is_not_the_display_substeps() {
        let mut fine = OrbEngine::at_rest(0.05, 0.40);
        let mut coarse = OrbEngine::at_rest(0.05, 0.40);
        for _ in 0..5 {
            fine.step(OrbMotion::Speaking, 0.20, 0.95, 1.0 / 60.0);
        }
        coarse.step(OrbMotion::Speaking, 0.20, 0.95, 0.080);
        assert!(
            (fine.radius() - coarse.radius()).abs() > 1e-4,
            "fine {} coarse {}",
            fine.radius(),
            coarse.radius()
        );
        assert!(fine.radius().is_finite() && coarse.radius().is_finite());
    }

    #[test]
    fn spring_moves_inside_a_single_dotparams_frame() {
        let dot = DotParams::compute(DotState::Speaking, 0.3, 0);
        let inside = DotParams::compute(DotState::Speaking, 0.3, 79);
        assert_eq!(dot, inside, "DotParams stays the 80 ms golden");
        let mut engine = OrbEngine::idle();
        let a = engine.step_toward_dot(&dot, DisplayHz::Hz60.dt_s());
        let b = engine.step_toward_dot(&dot, DisplayHz::Hz60.dt_s());
        assert_ne!(a.radius, b.radius);
        assert_eq!(dot, DotParams::compute(DotState::Speaking, 0.3, 0));
        assert_eq!(a.motion, OrbMotion::Speaking);
    }

    #[test]
    fn n_frames_at_each_display_rate_are_deterministic() {
        fn trail(hz: DisplayHz) -> Vec<(u32, u32)> {
            let mut engine = OrbEngine::idle();
            let mut out = Vec::new();
            let n = hz.hz();
            for i in 0..n {
                let t_ms = (i as u64) * 1000 / (hz.hz() as u64);
                let dot = if t_ms < 500 {
                    DotParams::compute(DotState::Idle, 0.0, t_ms)
                } else {
                    DotParams::compute(DotState::Speaking, 0.6, t_ms)
                };
                let sample = engine.step_toward_dot(&dot, hz.dt_s());
                assert_draw(sample);
                if i % (hz.hz() / 4) == 0 {
                    out.push((
                        sample.draw_radius().to_bits(),
                        sample.draw_brightness().to_bits(),
                    ));
                }
            }
            out
        }
        for hz in DisplayHz::ALL {
            let a = trail(hz);
            let b = trail(hz);
            assert_eq!(a, b);
            assert!(a.len() >= 4);
            assert!(a.iter().any(|p| *p != a[0]), "{hz:?} did not move");
        }
    }

    #[test]
    fn thinking_is_motion_only() {
        for state in [DotState::Idle, DotState::Listening, DotState::Speaking] {
            let dot = DotParams::compute(state, 0.5, 160);
            let mut engine = OrbEngine::idle();
            let sample = engine.step_toward_dot(&dot, DisplayHz::Hz120.dt_s());
            assert_ne!(sample.motion, OrbMotion::Thinking);
            assert_eq!(dot.state, state);
        }
        let mut engine = OrbEngine::idle();
        let sample = engine.step(OrbMotion::Thinking, 0.112, 0.80, DisplayHz::Hz240.dt_s());
        assert_eq!(sample.motion, OrbMotion::Thinking);
        assert_draw(sample);
    }

    #[test]
    fn rika_lean_stays_idle_motion_and_does_not_edit_the_dot() {
        let frame = VizFrame {
            speak: crate::frame::BandState {
                emotion: Emotion {
                    label: "lean-in",
                    intensity: 1.0,
                    valence: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let dot = DotParams::from_frame(&frame);
        assert_eq!(dot.state, DotState::Idle);
        let mut engine = OrbEngine::idle();
        let sample = engine.step_toward_dot(&dot, DisplayHz::Hz60.dt_s());
        assert_eq!(sample.motion, OrbMotion::Idle);
        assert_eq!(dot, DotParams::from_frame(&frame));
    }

    #[test]
    fn null_surface_still_records_dotparams() {
        let frame = VizFrame {
            t_ms: 80,
            speak: crate::frame::BandState {
                active: true,
                level: 0.4,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut sink = NullSurface::new();
        sink.present(&frame).unwrap();
        assert_eq!(sink.last_dot, Some(DotParams::from_frame(&frame)));
        let mut engine = OrbEngine::idle();
        let _ = engine.step_toward_dot(&sink.last_dot.unwrap(), DisplayHz::Hz240.dt_s());
        assert_eq!(sink.last_dot, Some(DotParams::from_frame(&frame)));
    }

    #[test]
    fn draw_clamp_holds_when_the_target_is_outside_the_mark() {
        let mut engine = OrbEngine::at_rest(0.0, 0.0);
        // Soft speaking aim (ω 3) needs longer than the old ω 18 to reach the clamp.
        for _ in 0..240 {
            let sample = engine.step(OrbMotion::Speaking, 10.0, -4.0, DisplayHz::Hz60.dt_s());
            assert_draw(sample);
            assert!(sample.draw_radius() <= 0.5);
            assert!(sample.draw_brightness() <= 1.0);
        }
        assert!((engine.radius() - 0.5).abs() < 1e-2);
        assert!(engine.brightness() < 1e-2);
    }

    #[test]
    fn aspect_stays_a_circle_when_radius_velocity_is_large() {
        let mut engine = OrbEngine::idle();
        for i in 0..80 {
            let target = if i % 2 == 0 { 0.05 } else { 0.24 };
            let sample = engine.step(OrbMotion::Speaking, target, 1.0, DisplayHz::Hz60.dt_s());
            assert_draw(sample);
            assert!((sample.aspect - 1.0).abs() < 1e-6, "{}", sample.aspect);
        }
        assert!((engine.aspect() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mode_change_does_not_snap_the_gain() {
        let mut engine = OrbEngine::idle();
        let before = engine.omega;
        let _ = engine.step(OrbMotion::Speaking, 0.20, 0.9, DisplayHz::Hz60.dt_s());
        let aim = OrbMotion::Speaking.radius_gain().omega;
        let idle = OrbMotion::Idle.radius_gain().omega;
        assert!(
            (engine.omega - before).abs() < (aim - idle) * 0.2,
            "omega snapped from {before} to {}",
            engine.omega
        );
        assert!(engine.omega < idle + (aim - idle) * 0.25);
    }

    #[test]
    fn non_finite_targets_hold_and_idle_constructor_rejects_nan() {
        let mut engine = OrbEngine::idle();
        let before = engine.radius();
        let sample = engine.step(OrbMotion::Idle, f32::NAN, 0.9, DisplayHz::Hz60.dt_s());
        assert_eq!(sample.dt_s, 0.0);
        assert_eq!(engine.radius(), before);
        assert_draw(sample);
        let sample = engine.step(OrbMotion::Idle, 0.12, f32::INFINITY, 1.0 / 120.0);
        assert_eq!(sample.dt_s, 0.0);
        let nan = OrbEngine::at_rest(f32::NAN, f32::NAN);
        assert_eq!(nan.radius(), DotParams::IDLE_RADIUS);
        assert_eq!(nan.brightness(), 0.84);
        assert_eq!(nan.aspect(), 1.0);
    }

    #[test]
    fn json_carries_motion_and_dt() {
        let mut engine = OrbEngine::idle();
        let sample = engine.step(OrbMotion::Listening, 0.14, 0.96, DisplayHz::Hz240.dt_s());
        let json = sample.to_json();
        assert!(json.contains("\"motion\":\"listening\""));
        assert!(json.contains("\"dt_s\":"));
        assert!(json.starts_with('{') && json.ends_with('}'));
    }
}
