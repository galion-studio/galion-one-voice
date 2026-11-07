//! Emission, display grade, and a single-pass dust model.
//!
//! Spectral colour is the physical line. Display colour is the semantic
//! grade. Samantha gold and Jarvis blue are the illuminating star and a
//! small overall grade. They tint the veil. They do not replace the emotion.

use nebula_elements::{linear_rgb, DemoLine};

use crate::noise::henyey_greenstein;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Samantha,
    Jarvis,
}

impl Voice {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "samantha" | "sam" => Some(Self::Samantha),
            "jarvis" => Some(Self::Jarvis),
            _ => None,
        }
    }

    /// Reflection-nebula illuminant. Not an emotion colour.
    pub fn star_linear(self) -> [f32; 3] {
        match self {
            Self::Samantha => [1.0, 0.72, 0.38],
            Self::Jarvis => [0.35, 0.48, 1.0],
        }
    }

    pub fn grade_linear(self) -> [f32; 3] {
        match self {
            Self::Samantha => [1.05, 0.86, 0.55],
            Self::Jarvis => [0.55, 0.70, 1.10],
        }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn scale(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}

fn mul(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

/// One shade evaluation. `samples` repeats the scatter estimate for ultra.
pub fn shade(line: &DemoLine, weight: f32, density: f32, cos_theta: f32, voice: Voice, samples: u32) -> [f32; 3] {
    let weight = weight.clamp(0.0, 1.0);
    let spec = linear_rgb(line.spectral_srgb);
    let display = linear_rgb(line.display_srgb);
    let physical = scale(spec, line.intrinsic * weight);
    let semantic = scale(display, weight);
    let emission = mix(physical, semantic, 0.84);
    let lambda = line.wavelength_nm as f32;
    let rayleigh = (550.0 / lambda).powi(4).clamp(0.05, 12.0);
    let tau = (density.max(0.0) * 0.65 * rayleigh).min(4.0);
    let trans = (-tau).exp();
    let g = 0.58;
    let mut scatter = 0.0f32;
    let taps = samples.clamp(1, 4);
    for tap in 0..taps {
        let shift = (tap as f32 - (taps as f32 - 1.0) * 0.5) * 0.08;
        scatter += henyey_greenstein((cos_theta + shift).clamp(-1.0, 1.0), g);
    }
    scatter /= taps as f32;
    let phase = (scatter * 4.0 * std::f32::consts::PI).clamp(0.0, 1.6);
    // Optically thin dust still scatters the star. Dense gas shows the line.
    let veil = scale(voice.star_linear(), (0.22 + 0.78 * (1.0 - trans)) * phase.min(1.0) * 0.55);
    let core = scale(emission, 0.55 + 0.9 * trans);
    let dense = smoothstep(density);
    let mut rgb = mix(veil, core, 0.15 + 0.85 * dense);
    let graded = mul(rgb, voice.grade_linear());
    rgb = mix(rgb, graded, 0.08);
    rgb
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebula_elements::demo;

    #[test]
    fn emotion_colour_survives_both_illuminants() {
        let angry = demo("angry").unwrap();
        for voice in [Voice::Samantha, Voice::Jarvis] {
            let rgb = shade(angry, 1.0, 0.9, 0.2, voice, 1);
            assert!(rgb[0] > rgb[2], "{voice:?} angry should stay red, got {rgb:?}");
        }
    }

    #[test]
    fn thin_dust_takes_the_voice_illuminant() {
        let neutral = demo("neutral").unwrap();
        let sam = shade(neutral, 0.15, 0.02, 0.0, Voice::Samantha, 1);
        let jar = shade(neutral, 0.15, 0.02, 0.0, Voice::Jarvis, 1);
        assert!(sam[0] > sam[2], "samantha veil {sam:?}");
        assert!(jar[2] > jar[0], "jarvis veil {jar:?}");
    }
}
