//! Galion Voice brand palette (BRANDING.md, V4 one-dot rule).
//!
//! * [`DOT`] is the only foreground colour. It is solid and neutral; brightness may be
//!   scaled *uniformly* per frame (see [`crate::dot`]) but it is never tinted, never
//!   gradient-filled, never translucent.
//! * [`OBSIDIAN`] → [`AMBER`] → [`VIOLET`] are **background only**. Use
//!   [`background_at`] to sample the living wash; nothing in this crate ever feeds a
//!   background colour into the dot.

/// 8-bit sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn r(self) -> u8 { self.0 }
    pub const fn g(self) -> u8 { self.1 }
    pub const fn b(self) -> u8 { self.2 }

    /// `0x00RRGGBB` (softbuffer / framebuffer layout).
    pub const fn to_u32(self) -> u32 { ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32 }

    /// `#RRGGBB`.
    pub fn hex(self) -> String { format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2) }

    /// Linear interpolation in sRGB space, `t` clamped to 0..=1.
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }

    /// Uniform brightness scale (all channels by the same factor), clamped 0..=1.
    /// This is the *only* transform the dot colour is allowed.
    pub fn scaled(self, k: f32) -> Rgb {
        let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 0.0 };
        let m = |a: u8| (a as f32 * k).round().clamp(0.0, 255.0) as u8;
        Rgb(m(self.0), m(self.1), m(self.2))
    }
}

/// The dot. Solid neutral soft white.
pub const DOT: Rgb = Rgb(0xF5, 0xF5, 0xF7);
/// Background base and edges.
pub const OBSIDIAN: Rgb = Rgb(0x0A, 0x0A, 0x0C);
/// Background gradient start (background only).
pub const AMBER: Rgb = Rgb(0xF5, 0xA5, 0x24);
/// Background gradient end (background only).
pub const VIOLET: Rgb = Rgb(0x7C, 0x3A, 0xED);

/// Hex strings, for web / CSS consumers.
pub const DOT_HEX: &str = "#F5F5F7";
pub const OBSIDIAN_HEX: &str = "#0A0A0C";
pub const AMBER_HEX: &str = "#F5A524";
pub const VIOLET_HEX: &str = "#7C3AED";

/// How strongly the amber→violet wash shows over obsidian at the centre (0..=1).
/// Kept low: the wash is "slow, living", deeper obsidian toward the edges.
pub const WASH_STRENGTH: f32 = 0.38;
/// Period of the slow background drift, ms (the dot never follows it).
pub const WASH_PERIOD_MS: u64 = 24_000;

/// Sample the background at normalised coords `u, v` in 0..=1 at audio time `t_ms`.
///
/// Diagonal amber→violet ramp that drifts slowly, multiplied by a radial vignette so
/// edges fall back to pure obsidian. Background only — never call this for dot pixels.
pub fn background_at(u: f32, v: f32, t_ms: u64) -> Rgb {
    let phase = (t_ms % WASH_PERIOD_MS) as f32 / WASH_PERIOD_MS as f32 * std::f32::consts::TAU;
    let drift = 0.15 * phase.sin();
    let ramp = (0.5 * (u + v) + drift).clamp(0.0, 1.0);
    let hue = AMBER.lerp(VIOLET, ramp);
    let (du, dv) = (u - 0.5, v - 0.5);
    let r = (du * du + dv * dv).sqrt() / std::f32::consts::FRAC_1_SQRT_2; // 0 centre .. 1 corner
    let vignette = (1.0 - r).clamp(0.0, 1.0);
    OBSIDIAN.lerp(hue, WASH_STRENGTH * vignette * vignette)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_match_branding() {
        assert_eq!(DOT.hex(), DOT_HEX);
        assert_eq!(OBSIDIAN.hex(), OBSIDIAN_HEX);
        assert_eq!(AMBER.hex(), AMBER_HEX);
        assert_eq!(VIOLET.hex(), VIOLET_HEX);
        assert_eq!(DOT.to_u32(), 0x00F5F5F7);
    }
    #[test]
    fn corners_are_obsidian_and_scale_is_uniform() {
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            assert_eq!(background_at(u, v, 0), OBSIDIAN);
        }
        assert_ne!(background_at(0.5, 0.5, 0), OBSIDIAN);
        assert_eq!(DOT.scaled(1.0), DOT);
        let h = DOT.scaled(0.5);
        assert_eq!((h.0, h.1, h.2), (123, 123, 124));
    }
}
