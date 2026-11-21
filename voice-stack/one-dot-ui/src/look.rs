// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! How the Samantha mark moves.
//!
//! Radii and the idle / listening / speaking levels are `DotParams` in
//! `galion-render/src/dot.rs`. Thinking is the extra motion already drawn
//! by `crates/galion-console/assets/v1-screen.html`. The spring step is
//! `function spring` on that page (and `galion-render` `Spring::step`).
//! Luti gains are `OrbMotion::radius_gain` in `galion-render/src/orb_engine.rs`.
//!
//! The Galion One disc stays `#F5F5F7`. This sector's mark is the violet
//! stop `#7C3AED` (`palette::VIOLET`), because the assignment is a violet
//! dot. The wash stays background-only: obsidian `#0A0A0C`, amber
//! `#F5A524`, violet `#7C3AED`, strength 0.38, 24 s drift. That is
//! `palette::background_at`.

use crate::protocol::{Pad, VoiceState};

pub const IDLE_RADIUS: f32 = 0.100;
pub const LISTEN_RADIUS: f32 = 0.120;
pub const SPEAK_RADIUS: f32 = 0.115;
pub const THINK_RADIUS: f32 = 0.108;
pub const OFFLINE_RADIUS: f32 = 0.060;
pub const OFFLINE_BRIGHTNESS: f32 = 0.08;
pub const LISTEN_PERIOD_MS: u32 = 3_200;
pub const WASH_STRENGTH: f32 = 0.38;
pub const WASH_PERIOD_MS: u32 = 24_000;

/// Deep violet, the locked violet, and a light violet. The body stays in this family.
pub const VIOLET_DEEP: [u8; 3] = [0x4C, 0x1D, 0x95];
pub const VIOLET: [u8; 3] = [0x7C, 0x3A, 0xED];
pub const VIOLET_LIGHT: [u8; 3] = [0xC4, 0xB5, 0xFD];
pub const OBSIDIAN: [u8; 3] = [0x0A, 0x0A, 0x0C];
pub const AMBER: [u8; 3] = [0xF5, 0xA5, 0x24];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotLook {
    pub breath_ms: u32,
    pub rgb: [u8; 3],
    pub glow: f32,
    pub radius_bias: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotGeom {
    pub radius: f32,
    pub brightness: f32,
}

/// Page spring. `dt_ms` is display time, not the 80 ms tick.
/// A finite `dt_ms` is clamped to 1..=50 ms, matching `function spring`
/// in `v1-screen.html` (`Math.max(0.001, dt/1000)` and `Math.min(0.05, …)`).
pub fn spring_step(
    position: f32,
    velocity: f32,
    target: f32,
    dt_ms: f32,
    omega: f32,
    zeta: f32,
) -> (f32, f32) {
    if !dt_ms.is_finite() || !omega.is_finite() || !zeta.is_finite() {
        return (if position.is_finite() { position } else { 0.0 }, 0.0);
    }
    if !target.is_finite() {
        return (
            if position.is_finite() { position } else { 0.0 },
            if velocity.is_finite() { velocity } else { 0.0 },
        );
    }
    let h = (dt_ms / 1000.0).clamp(0.001, 0.05);
    let p0 = if position.is_finite() {
        position
    } else {
        target
    };
    let v0 = if velocity.is_finite() { velocity } else { 0.0 };
    let a = -omega * omega * (p0 - target) - 2.0 * zeta * omega * v0;
    let v = v0 + a * h;
    let p = p0 + v * h;
    if !a.is_finite() || !v.is_finite() || !p.is_finite() {
        (target, 0.0)
    } else {
        (p, v)
    }
}

pub fn look_of(pad: Pad) -> DotLook {
    let pad = pad.sane();
    let breath = (2200.0 - pad.arousal * 2000.0).round().clamp(800.0, 2400.0) as u32;
    DotLook {
        breath_ms: breath,
        rgb: violet_for(pad.valence),
        glow: 0.25 + 0.50 * pad.dominance,
        radius_bias: (pad.dominance - 0.5) * 0.012,
    }
}

fn violet_for(valence: f32) -> [u8; 3] {
    // Resting valence 0.25 is exactly #7C3AED. Below that, toward deep
    // violet at -1. Above that, toward the light violet at +1.
    if valence <= 0.25 {
        let t = (valence + 1.0) / 1.25;
        lerp_rgb(VIOLET_DEEP, VIOLET, t)
    } else {
        let t = (valence - 0.25) / 0.75;
        lerp_rgb(VIOLET, VIOLET_LIGHT, t)
    }
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [
        lerp_u8(a[0], b[0], t),
        lerp_u8(a[1], b[1], t),
        lerp_u8(a[2], b[2], t),
    ]
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

pub fn geometry(state: VoiceState, level: f32, anim_ms: u32, pad: Pad, reduced: bool) -> DotGeom {
    let look = look_of(pad);
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (radius, brightness) = match state {
        VoiceState::Idle => {
            let s = sine(anim_ms, look.breath_ms, reduced);
            (IDLE_RADIUS + 0.006 * s + look.radius_bias, 0.84 + 0.03 * s)
        }
        VoiceState::Listening => {
            let s = sine(anim_ms, LISTEN_PERIOD_MS, reduced);
            (
                LISTEN_RADIUS + 0.002 * s + 0.004 * level + look.radius_bias,
                0.96,
            )
        }
        VoiceState::Thinking => {
            let s = sine(anim_ms, look.breath_ms, reduced);
            (THINK_RADIUS + 0.008 * s + look.radius_bias, 0.90 + 0.04 * s)
        }
        VoiceState::Speaking => {
            let step = ((anim_ms / 80) % 4) as usize;
            let rhythm = [0.0f32, 0.5, 1.0, 0.5][step];
            let drive = level.max(0.35 * rhythm);
            (
                SPEAK_RADIUS + 0.030 * drive + look.radius_bias,
                0.97 + 0.03 * drive,
            )
        }
    };
    DotGeom {
        radius: radius.clamp(0.0, 0.5),
        brightness: brightness.clamp(0.0, 1.0),
    }
}

fn sine(anim_ms: u32, period_ms: u32, reduced: bool) -> f32 {
    if reduced || period_ms == 0 {
        return 0.0;
    }
    let p = (anim_ms % period_ms) as f32 / period_ms as f32;
    (p * std::f32::consts::TAU).sin()
}

/// `palette::background_at`. Background only. `u` and `v` are 0..=1.
pub fn background_at(u: f32, v: f32, t_ms: u32) -> [u8; 3] {
    let u = if u.is_finite() {
        u.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let v = if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let phase = (t_ms % WASH_PERIOD_MS) as f32 / WASH_PERIOD_MS as f32 * std::f32::consts::TAU;
    let drift = 0.15 * phase.sin();
    let ramp = (0.5 * (u + v) + drift).clamp(0.0, 1.0);
    let hue = lerp_rgb(AMBER, VIOLET, ramp);
    let du = u - 0.5;
    let dv = v - 0.5;
    let r = (du * du + dv * dv).sqrt() * std::f32::consts::SQRT_2;
    let vignette = (1.0 - r).clamp(0.0, 1.0);
    lerp_rgb(OBSIDIAN, hue, WASH_STRENGTH * vignette * vignette)
}

/// Preview drift around rest. Not a measured feeling. Tick 0 is rest.
pub fn preview_mood(t_ms: u32) -> Pad {
    let t = t_ms as f32;
    Pad {
        arousal: (Pad::RESTING.arousal + 0.05 * (t / 1000.0).sin()).clamp(0.0, 1.0),
        valence: (Pad::RESTING.valence + 0.04 * (t / 1300.0).sin()).clamp(-1.0, 1.0),
        dominance: (Pad::RESTING.dominance + 0.03 * (t / 1700.0).sin()).clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn resting_look_is_the_violet_dot_on_a_1600_ms_breath() {
        let look = look_of(Pad::RESTING);
        assert_eq!(look.breath_ms, 1600);
        assert_eq!(look.rgb, VIOLET);
        assert!(close(look.glow, 0.5), "{}", look.glow);
        assert!(close(look.radius_bias, 0.0), "{}", look.radius_bias);
    }

    #[test]
    fn arousal_shortens_the_breath_and_valence_stays_violet() {
        assert_eq!(
            look_of(Pad {
                arousal: 0.0,
                valence: 0.25,
                dominance: 0.5
            })
            .breath_ms,
            2200
        );
        assert_eq!(
            look_of(Pad {
                arousal: 1.0,
                valence: 0.25,
                dominance: 0.5
            })
            .breath_ms,
            800
        );
        assert_eq!(
            look_of(Pad {
                arousal: 0.3,
                valence: -1.0,
                dominance: 0.5
            })
            .rgb,
            VIOLET_DEEP
        );
        assert_eq!(
            look_of(Pad {
                arousal: 0.3,
                valence: 1.0,
                dominance: 0.5
            })
            .rgb,
            VIOLET_LIGHT
        );
    }

    #[test]
    fn geometry_matches_the_dot_params_at_known_times() {
        let rest = Pad::RESTING;
        let idle0 = geometry(VoiceState::Idle, 0.0, 0, rest, false);
        assert!(close(idle0.radius, 0.100), "{}", idle0.radius);
        assert!(close(idle0.brightness, 0.84), "{}", idle0.brightness);

        let idle400 = geometry(VoiceState::Idle, 0.0, 400, rest, false);
        assert!(close(idle400.radius, 0.106), "{}", idle400.radius);
        assert!(close(idle400.brightness, 0.87), "{}", idle400.brightness);

        let held = geometry(VoiceState::Idle, 0.0, 400, rest, true);
        assert!(close(held.radius, 0.100), "{}", held.radius);

        let listen = geometry(VoiceState::Listening, 1.0, 0, rest, false);
        assert!(close(listen.radius, 0.124), "{}", listen.radius);
        assert!(close(listen.brightness, 0.96), "{}", listen.brightness);

        let speak = geometry(VoiceState::Speaking, 0.0, 160, rest, false);
        assert!(close(speak.radius, 0.1255), "{}", speak.radius);
        assert!(close(speak.brightness, 0.9805), "{}", speak.brightness);

        let env = geometry(VoiceState::Speaking, 0.5, 0, rest, false);
        assert!(close(env.radius, 0.130), "{}", env.radius);
        assert!(close(env.brightness, 0.985), "{}", env.brightness);
    }

    #[test]
    fn wash_centre_and_corner_match_the_palette_formula() {
        assert_eq!(background_at(0.5, 0.5, 0), [77, 49, 60]);
        assert_eq!(background_at(0.0, 0.0, 0), OBSIDIAN);
    }

    #[test]
    fn preview_mood_starts_at_rest() {
        let m = preview_mood(0);
        assert!(close(m.arousal, Pad::RESTING.arousal));
        assert!(close(m.valence, Pad::RESTING.valence));
        assert!(close(m.dominance, Pad::RESTING.dominance));
    }

    #[test]
    fn spring_moves_toward_the_target_with_the_page_step() {
        let (p, v) = spring_step(0.1, 0.0, 0.2, 1000.0 / 60.0, 1.8, 0.92);
        let h = (1000.0f32 / 60.0 / 1000.0).clamp(0.001, 0.05);
        let w = 1.8f32;
        let z = 0.92f32;
        let a = -w * w * (0.1 - 0.2) - 2.0 * z * w * 0.0;
        let expect_v = 0.0 + a * h;
        let expect_p = 0.1 + expect_v * h;
        assert!(close(p, expect_p), "{p} vs {expect_p}");
        assert!(close(v, expect_v), "{v} vs {expect_v}");
        assert!(p > 0.1 && p < 0.2);
        let (held, _) = spring_step(0.1, 0.0, f32::NAN, 16.0, 1.8, 0.92);
        assert!(close(held, 0.1));
    }
}
