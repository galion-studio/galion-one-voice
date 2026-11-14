//! Emotion halo around the one dot.
//!
//! Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//!
//! The mark stays `#F5F5F7`. This module only places faint arcs in the
//! locked amber → violet wash. It does not recolour the dot, invent a
//! 13-way posterior from loudness, or run the nebula demo time-blend.
//!
//! A frame is a hold when `present` is false: no arcs. Quiet and
//! vulnerability are holds. The gas table has no row for them, and this
//! code does not borrow a neighbour. Joy, grief, and calm are the product
//! moods whose names already sit on a Demo 13 row (happy / joy, grief on
//! the sad line, calm). One weight, not a blend.
//!
//! Motion periods are the 432 Hz just lattice, slowed into a breath:
//! 4320 ms fundamental, octave 2:1, fifth 3:2, major third 5:4.
//! There is no audio in this file.

use crate::palette::{Rgb, AMBER, VIOLET};

/// Visual fundamental, milliseconds. Ten times a 432 Hz cycle, so the eye
/// can follow it. Not a tone and not the 80 ms heartbeat.
pub const FUNDAMENTAL_MS: f32 = 4320.0;
/// Octave, 2:1 against [`FUNDAMENTAL_MS`].
pub const OCTAVE_MS: f32 = FUNDAMENTAL_MS / 2.0;
/// Perfect fifth, 3:2 against [`FUNDAMENTAL_MS`].
pub const FIFTH_MS: f32 = FUNDAMENTAL_MS * 2.0 / 3.0;
/// Major third, 5:4 against [`FUNDAMENTAL_MS`].
pub const THIRD_MS: f32 = FUNDAMENTAL_MS * 4.0 / 5.0;

/// Demo 13 order. `motion` matches `nebula-elements` DEMO. `warmth` is a
/// placement on the amber–violet wash, not a spectral hex.
pub struct Gas {
    pub emotion: &'static str,
    pub motion: f32,
    pub warmth: f32,
}

pub const GASES: [Gas; 13] = [
    Gas {
        emotion: "neutral",
        motion: 0.30,
        warmth: 0.50,
    },
    Gas {
        emotion: "calm",
        motion: 0.22,
        warmth: 0.42,
    },
    Gas {
        emotion: "happy",
        motion: 0.72,
        warmth: 0.82,
    },
    Gas {
        emotion: "amused",
        motion: 0.80,
        warmth: 0.74,
    },
    Gas {
        emotion: "excited",
        motion: 1.40,
        warmth: 0.88,
    },
    Gas {
        emotion: "tender",
        motion: 0.30,
        warmth: 0.70,
    },
    Gas {
        emotion: "sad",
        motion: 0.24,
        warmth: 0.22,
    },
    Gas {
        emotion: "disappointed",
        motion: 0.34,
        warmth: 0.38,
    },
    Gas {
        emotion: "angry",
        motion: 1.45,
        warmth: 0.90,
    },
    Gas {
        emotion: "fearful",
        motion: 1.25,
        warmth: 0.12,
    },
    Gas {
        emotion: "surprised",
        motion: 1.15,
        warmth: 0.55,
    },
    Gas {
        emotion: "disgusted",
        motion: 0.78,
        warmth: 0.48,
    },
    Gas {
        emotion: "seductive",
        motion: 0.28,
        warmth: 0.78,
    },
];

/// One consented distribution, or a hold.
#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    pub present: bool,
    pub weights: [f32; 13],
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
    pub energy: f32,
}

impl Drive {
    pub const HOLD: Drive = Drive {
        present: false,
        weights: [0.0; 13],
        valence: 0.0,
        arousal: 0.0,
        dominance: 0.5,
        energy: 0.0,
    };
}

/// One faint arc in the wash. `radius` is a fraction of the short side.
/// `warmth` is 0 at violet and 1 at amber.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub radius: f32,
    pub angle: f32,
    pub warmth: f32,
    pub alpha: f32,
    pub emotion: &'static str,
}

/// Product mood → one Demo 13 weight, or a hold.
///
/// Joy is the happy row (the gas table's Happy/joy alias). Grief uses the
/// sad row (the universal table puts grief on that same He II line). Calm
/// is the calm row. Anything else, including quiet and vulnerability, holds.
pub fn drive_for_mood(mood: &str, valence: f32, arousal: f32) -> Drive {
    let emotion = match mood {
        "joy" => "happy",
        "grief" => "sad",
        "calm" => "calm",
        _ => return Drive::HOLD,
    };
    let mut weights = [0.0; 13];
    let Some(index) = GASES.iter().position(|gas| gas.emotion == emotion) else {
        return Drive::HOLD;
    };
    weights[index] = 1.0;
    let arousal = if arousal.is_finite() {
        arousal.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let valence = if valence.is_finite() {
        valence.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    Drive {
        present: true,
        weights,
        valence,
        arousal,
        dominance: 0.5,
        energy: arousal,
    }
}

/// Demo 13 element ids. The picture binds these, not the live-feed gas-line ids.
pub const ELEMENT_ID: [&str; 13] = [
    "HeII-541.2",
    "H-beta",
    "NII-575.5",
    "HeI-587.6",
    "NeI-594.5",
    "HeI-667.8",
    "HeII-468.6",
    "ClIII-551.7",
    "SIII-631.2",
    "H-delta",
    "OIII-495.9",
    "ClIII-553.8",
    "H-alpha",
];

/// Live-feed gas-line ids. Opaque labels. Not particle keys.
const GAS_LINE_ID: [&str; 13] = [
    "HeII-541.2",
    "Hbeta-486.1",
    "[NII]-575.5",
    "HeI-587.6",
    "NeI-594.5",
    "HeI-667.8",
    "HeII-468.6",
    "[ClIII]-551.7",
    "[SIII]-631.2",
    "Hdelta-410.2",
    "[OIII]-495.9",
    "[ClIII]-553.8",
    "Halpha-656.3",
];

pub const FEED_SCHEMA: &str = "galion-voice-v2-emotion-feed/1";

/// JSON object field for `/dot`, including the key.
///
/// `estimate: none` has no weights. A distribution names `element_id`.
/// The schema field stays on both.
pub fn dot_nebula_json(mood: &str, valence: f32, arousal: f32) -> String {
    let drive = drive_for_mood(mood, valence, arousal);
    if !drive.present {
        return format!(
            "\"nebula\":{{\"schema\":\"{FEED_SCHEMA}\",\"estimate\":\"none\",\"present\":false,\"distribution\":false,\"reason\":null}}"
        );
    }
    let mut weights = String::new();
    for (index, gas) in GASES.iter().enumerate() {
        if index > 0 {
            weights.push(',');
        }
        let weight = if drive.weights[index] > 0.5 { 1 } else { 0 };
        weights.push_str(&format!(
            "\"{}\":{{\"weight\":{weight},\"element_id\":\"{}\",\"gas_line\":\"{}\"}}",
            gas.emotion, ELEMENT_ID[index], GAS_LINE_ID[index]
        ));
    }
    format!(
        "\"nebula\":{{\"schema\":\"{FEED_SCHEMA}\",\"estimate\":\"distribution\",\"present\":true,\"distribution\":true,\"weights\":{{{weights}}},\"valence\":{:.3},\"arousal\":{:.3},\"dominance\":0.500,\"energy\":{:.3}}}",
        drive.valence, drive.arousal, drive.energy
    )
}

/// Arcs around the mark. Empty on a hold, and empty when every weight is
/// below the paint floor. Seductive is omitted when `samantha` is false.
///
/// `mark` is the live mark radius as a fraction of the short side.
pub fn arcs(drive: &Drive, t_ms: f32, mark: f32, samantha: bool) -> Vec<Arc> {
    if !drive.present {
        return Vec::new();
    }
    let mark = if mark.is_finite() {
        mark.clamp(0.04, 0.45)
    } else {
        0.10
    };
    let valence = drive.valence.clamp(-1.0, 1.0);
    let arousal = drive.arousal.clamp(0.0, 1.0);
    let dominance = drive.dominance.clamp(0.0, 1.0);
    let energy = drive.energy.clamp(0.0, 1.0);
    let clump = 0.35 + 0.65 * dominance;
    let mut out = Vec::new();
    let mut i = 0.0f32;
    for (index, gas) in GASES.iter().enumerate() {
        if gas.emotion == "seductive" && !samantha {
            continue;
        }
        let weight = drive.weights.get(index).copied().unwrap_or(0.0);
        if weight < 0.02 {
            continue;
        }
        let n = (weight * 18.0).round().max(1.0) as usize;
        for k in 0..n {
            let breathe = 0.5 + 0.5 * ((t_ms / THIRD_MS) * std::f32::consts::TAU + i).sin();
            // Arousal is the wave amplitude. It never pulls an arc onto the mark.
            let wave = 0.04 * arousal * gas.motion;
            let reach = mark
                + 0.06
                + wave
                + (0.16 + 0.10 * (1.0 - clump))
                    * (0.65 + 0.35 * gas.motion)
                    * (0.85 + 0.15 * breathe);
            let radius = reach.max(mark * 1.18);
            let angle = (t_ms / FIFTH_MS) * gas.motion
                + i * 0.37
                + k as f32 * std::f32::consts::TAU / n as f32;
            let warmth = (gas.warmth * 0.75 + (0.5 + 0.5 * valence) * 0.25).clamp(0.0, 1.0);
            let alpha = (0.035 + 0.05 * energy) * weight;
            out.push(Arc {
                radius,
                angle,
                warmth,
                alpha,
                emotion: gas.emotion,
            });
            i += 1.0;
        }
    }
    out
}

/// Brand wash colour for an arc. Only amber and violet.
pub fn wash_rgb(warmth: f32) -> Rgb {
    VIOLET.lerp(AMBER, warmth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_is_just_intonation_on_432() {
        assert!((FUNDAMENTAL_MS - 4320.0).abs() < 1e-3);
        assert!((OCTAVE_MS * 2.0 - FUNDAMENTAL_MS).abs() < 1e-3);
        assert!((FUNDAMENTAL_MS / FIFTH_MS - 1.5).abs() < 1e-4);
        assert!((FUNDAMENTAL_MS / THIRD_MS - 1.25).abs() < 1e-4);
    }

    #[test]
    fn hold_paints_nothing() {
        assert!(arcs(&Drive::HOLD, 1000.0, 0.10, true).is_empty());
        let mut lied = Drive::HOLD;
        lied.weights[2] = 1.0;
        assert!(
            arcs(&lied, 1000.0, 0.10, true).is_empty(),
            "present false wins over weights"
        );
    }

    #[test]
    fn moods_use_one_documented_row_or_hold() {
        let joy = drive_for_mood("joy", 0.72, 0.66);
        assert!(joy.present);
        assert!((joy.weights[2] - 1.0).abs() < 1e-6);
        assert!(joy.weights.iter().filter(|w| **w > 0.0).count() == 1);
        let grief = drive_for_mood("grief", -0.55, 0.42);
        assert!(grief.present && (grief.weights[6] - 1.0).abs() < 1e-6);
        let calm = drive_for_mood("calm", 0.10, 0.18);
        assert!(calm.present && (calm.weights[1] - 1.0).abs() < 1e-6);
        assert!(!drive_for_mood("quiet", 0.04, 0.08).present);
        assert!(!drive_for_mood("vulnerability", -0.32, 0.36).present);
        assert!(
            !drive_for_mood("angry", 0.0, 0.8).present,
            "a name with no product mood stays a hold"
        );
    }

    #[test]
    fn arcs_stay_outside_the_mark_and_inside_the_wash() {
        let drive = drive_for_mood("joy", 0.72, 0.66);
        let painted = arcs(&drive, 800.0, 0.10, true);
        assert!(!painted.is_empty());
        for arc in &painted {
            assert!(arc.radius >= 0.10 * 1.18 - 1e-4, "{}", arc.radius);
            assert!(arc.alpha < 0.10, "the wash stays faint: {}", arc.alpha);
            assert!((0.0..=1.0).contains(&arc.warmth));
            let rgb = wash_rgb(arc.warmth);
            let straight = VIOLET.lerp(AMBER, arc.warmth);
            assert_eq!(rgb, straight);
        }
        let jarvis = arcs(&drive_for_mood("calm", 0.1, 0.2), 0.0, 0.10, false);
        assert!(jarvis.iter().all(|arc| arc.emotion != "seductive"));
    }

    #[test]
    fn dot_json_holds_or_names_one_gas() {
        let grief = dot_nebula_json("grief", -0.55, 0.42);
        assert!(grief.contains("\"schema\":\"galion-voice-v2-emotion-feed/1\""), "{grief}");
        assert!(grief.contains("\"estimate\":\"distribution\""), "{grief}");
        assert!(grief.contains("\"element_id\":\"HeII-468.6\""), "{grief}");
        assert!(grief.contains("\"gas_line\":\"HeII-468.6\""), "{grief}");
        assert!(!grief.contains("\"element_id\":\"Hbeta-486.1\""), "{grief}");
        let quiet = dot_nebula_json("quiet", 0.04, 0.08);
        assert!(quiet.contains("\"schema\":\"galion-voice-v2-emotion-feed/1\""), "{quiet}");
        assert!(quiet.contains("\"estimate\":\"none\""), "{quiet}");
        assert!(!quiet.contains("weights"), "{quiet}");
        assert!(!quiet.contains("\"weight\":1"), "{quiet}");
    }

    #[test]
    fn page_shares_the_lattice_and_the_hold() {
        let page = include_str!("../../crates/galion-console/assets/v1-screen.html");
        assert!(page.contains("drawNebula"));
        assert!(page.contains("/nebula/vocabulary.mjs"));
        assert!(page.contains("id:\"arrive\",hz:216"));
        assert!(page.contains("id:\"listen\",hz:432"));
        assert!(page.contains("id:\"think\",hz:540"));
        assert!(page.contains("id:\"speak\",hz:648"));
        assert!(page.contains("id:\"yes\",hz:864"));
        assert!(page.contains("id:\"no\",hz:288"));
        assert!(page.contains("id:\"leave\",hz:108"));
        assert!(page.contains("id=\"decide\""));
        assert!(page.contains("#decide{display:none}"));
        assert!(page.contains("function productIsOne"));
        assert!(page.contains("Galion One"));
        assert!(page.contains("v2/decide"));
        for q in [
            "Keep this session on this machine",
            "Stay quiet when the turn ends",
            "Keep replies in English",
        ] {
            assert!(page.contains(q), "{q}");
        }
        assert!(page.contains("galion-nebula/1"));
        assert!(page.contains("function pictureLine"));
        assert!(page.contains("function isPicture"));
        assert!(page.contains("cv.dataset.mode=shown"));
        assert!(page.contains("paintHalo(now-t0,W,H,rad)"));
        assert!(!page.contains("rad/side"));
        assert!(page.contains("NII-575.5"));
        assert!(page.contains("present!==true"));
        assert!(page.contains("estimate===\"none\""));
        assert!(page.contains("#F5F5F7"));
        assert!(!page.contains("return {neutral:1}"));
        let vocab = include_str!("../../crates/galion-console/assets/nebula/vocabulary.mjs");
        assert!(vocab.contains("export function drawNebula"));
        assert!(vocab.contains("\"referenceHz\": 432"));
        assert!(vocab.contains("\"2:1\""));
        assert!(vocab.contains("\"3:2\""));
        assert!(vocab.contains("\"5:4\""));
        assert!(vocab.contains("#F5A524"));
        assert!(vocab.contains("#7C3AED"));
    }
}
