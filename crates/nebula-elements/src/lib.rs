//! Fixed emission-line tables for Galion nebula visuals.
//!
//! Demo 13 is the only table the visualiser renders. `UNIVERSAL` is a lookup
//! vocabulary. Neither table is configurable. Spectral hex and the display
//! target are separate fields. Display targets were checked with CIEDE2000
//! and left unchanged: the closest pair (Amused `#FF9C64`, Excited `#FF6300`)
//! is about 12.7, above the floor of 10.
//!
//! Spectral hex values are the admin Demo 13 / universal table swatches.
//! They were produced by the method in
//! `docs/emotion-v2/UNIVERSAL-EMOTION-GAS-TABLE-20261107.md`: CIE 1931 2°
//! colour-matching functions, D65 XYZ-to-linear-sRGB, clamp negatives, scale
//! the peak channel to 1, then the sRGB transfer. This crate does not
//! recompute them.

pub const TABLE_ID: &str = "galion-nebula-elements/demo-13-v2";
pub const DEMO_LEN: usize = 13;
pub const UNIVERSAL_LEN: usize = 62;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DemoLine {
    pub emotion: &'static str,
    pub doc_name: &'static str,
    pub gas: &'static str,
    pub element_id: &'static str,
    pub wavelength_nm: f64,
    pub spectral_hex: &'static str,
    pub display_hex: &'static str,
    pub motion: f32,
    pub pulse: f32,
    pub intrinsic: f32,
    pub lab_line: bool,
    pub samantha_only: bool,
    pub evidence: &'static str,
    pub physics: &'static str,
    pub spectral_srgb: [u8; 3],
    pub display_srgb: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UniversalLine {
    pub emotion: &'static str,
    pub doc_name: &'static str,
    pub gas: &'static str,
    pub wavelength_nm: f64,
    pub spectral_hex: &'static str,
    pub spectral_srgb: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabeledWeight {
    pub emotion: &'static str,
    pub weight: f32,
    pub element_id: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelError {
    UnknownEmotion,
    ElementMismatch,
}

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

const fn hex_rgb(s: &str) -> [u8; 3] {
    let b = s.as_bytes();
    [
        nibble(b[1]) * 16 + nibble(b[2]),
        nibble(b[3]) * 16 + nibble(b[4]),
        nibble(b[5]) * 16 + nibble(b[6]),
    ]
}

const fn row(
    emotion: &'static str,
    doc_name: &'static str,
    gas: &'static str,
    element_id: &'static str,
    wavelength_nm: f64,
    spectral_hex: &'static str,
    display_hex: &'static str,
    motion: f32,
    pulse: f32,
    intrinsic: f32,
    lab_line: bool,
    samantha_only: bool,
    evidence: &'static str,
    physics: &'static str,
) -> DemoLine {
    DemoLine {
        emotion,
        doc_name,
        gas,
        element_id,
        wavelength_nm,
        spectral_hex,
        display_hex,
        motion,
        pulse,
        intrinsic,
        lab_line,
        samantha_only,
        evidence,
        physics,
        spectral_srgb: hex_rgb(spectral_hex),
        display_srgb: hex_rgb(display_hex),
    }
}

const fn urow(
    emotion: &'static str,
    doc_name: &'static str,
    gas: &'static str,
    wavelength_nm: f64,
    spectral_hex: &'static str,
) -> UniversalLine {
    UniversalLine {
        emotion,
        doc_name,
        gas,
        wavelength_nm,
        spectral_hex,
        spectral_srgb: hex_rgb(spectral_hex),
    }
}

pub static DEMO: [DemoLine; DEMO_LEN] = [
    row("neutral", "Neutral", "He II", "HeII-541.2", 541.2, "#00FF00", "#9DA3A8", 0.30, 0.00, 0.22, false, false, "none", "He II 541.2 is a weak high-excitation recombination line; the green carrier is fully desaturated."),
    row("calm", "Calm / calmness", "H I H-beta", "H-beta", 486.1, "#00C0FF", "#47B5D1", 0.22, 0.00, 0.90, false, false, "moderate", "H-beta is a strong permitted Balmer line and the usual optical reference."),
    row("happy", "Happy / joy", "[N II]", "NII-575.5", 575.5, "#FFD300", "#FFCF40", 0.72, 0.00, 0.28, false, false, "strong", "[N II] 575.5 is a weak temperature-sensitive auroral line, not a dominant nebular feature."),
    row("amused", "Amused / amusement", "He I", "HeI-587.6", 587.6, "#FF8700", "#FF9C64", 0.80, 0.00, 0.70, false, false, "strong", "He I 587.6 is usually the strongest optical helium recombination line, still below the Balmer lines."),
    row("excited", "Excited / excitement", "Ne I (lab)", "NeI-594.5", 594.5, "#FF6300", "#FF6300", 1.40, 0.00, 0.35, true, false, "moderate", "Ne I 594.5 is a laboratory discharge line, not a typical ionised-nebula emission line."),
    row("tender", "Tender / tenderness", "He I", "HeI-667.8", 667.8, "#FF0000", "#FFB5C5", 0.30, 0.00, 0.26, false, false, "weak", "He I 667.8 is a weak permitted recombination line; pink is a white mix, not a wavelength."),
    row("sad", "Sad / sadness", "He II", "HeII-468.6", 468.6, "#0017FF", "#354766", 0.24, 0.00, 0.50, false, false, "strong", "He II 468.6 can be strong in high-excitation planetary nebulae and is weak in ordinary H II regions."),
    row("disappointed", "Disappointed / disappointment", "[Cl III]", "ClIII-551.7", 551.7, "#00FF00", "#676A70", 0.34, 0.00, 0.24, false, false, "strong", "[Cl III] 551.7 is a weak forbidden density diagnostic."),
    row("angry", "Angry / anger", "[S III]", "SIII-631.2", 631.2, "#FF0000", "#D90000", 1.45, 0.00, 0.30, false, false, "strong", "[S III] 631.2 is a weak auroral line, far below the strong near-infrared [S III] lines."),
    row("fearful", "Fearful / fear", "H I H-delta", "H-delta", 410.2, "#7000FF", "#171027", 1.25, 0.00, 0.42, false, false, "strong", "H-delta is a moderate Balmer line, weaker than H-gamma and H-beta."),
    row("surprised", "Surprised / surprise", "[O III]", "OIII-495.9", 495.9, "#00FFCB", "#00FFCB", 1.15, 1.00, 0.85, false, false, "none", "[O III] 495.9 is the strong but weaker companion of the 500.7 line."),
    row("disgusted", "Disgusted / disgust", "[Cl III]", "ClIII-553.8", 553.8, "#39FF00", "#8CAF24", 0.78, 0.00, 0.28, false, false, "moderate", "[Cl III] 553.8 is the density-sensitive partner of 551.7 and is a weak forbidden line."),
    row("seductive", "Seductive (Samantha only)", "H I H-alpha", "H-alpha", 656.3, "#FF0000", "#C42F63", 0.28, 0.00, 1.00, false, true, "weak", "H-alpha is often the strongest Balmer line; rose is a dimmed mix, not a different wavelength."),
];

/// Lookup vocabulary only. The visualiser does not spawn a particle layer
/// for these rows unless the emotion is also in [`DEMO`].
pub static UNIVERSAL: [UniversalLine; UNIVERSAL_LEN] = [
    urow("neutral", "Neutral", "He II", 541.2, "#00FF00"),
    urow("calm", "Calm / calmness", "H I H-beta", 486.1, "#00C0FF"),
    urow("happy", "Happy / joy", "[N II]", 575.5, "#FFD300"),
    urow("amused", "Amused / amusement", "He I", 587.6, "#FF8700"),
    urow("excited", "Excited / excitement", "Ne I (lab)", 594.5, "#FF6300"),
    urow("tender", "Tender / tenderness", "He I", 667.8, "#FF0000"),
    urow("sad", "Sad / sadness", "He II", 468.6, "#0017FF"),
    urow("disappointed", "Disappointed / disappointment", "[Cl III]", 551.7, "#00FF00"),
    urow("angry", "Angry / anger", "[S III]", 631.2, "#FF0000"),
    urow("fearful", "Fearful / fear", "H I H-delta", 410.2, "#7000FF"),
    urow("surprised", "Surprised / surprise", "[O III]", 495.9, "#00FFCB"),
    urow("disgusted", "Disgusted / disgust", "[Cl III]", 553.8, "#39FF00"),
    urow("seductive", "Seductive (Samantha only)", "H I H-alpha", 656.3, "#FF0000"),
    urow("shame", "Shame", "[N II]", 658.4, "#FF0000"),
    urow("guilt", "Guilt", "He II", 541.2, "#00FF00"),
    urow("pride", "Pride", "[N II]", 575.5, "#FFD300"),
    urow("relief", "Relief", "He II", 541.2, "#00FF00"),
    urow("awe", "Awe", "H I H-gamma", 434.0, "#6700FF"),
    urow("nostalgia", "Nostalgia", "Ne I (lab)", 585.2, "#FF9500"),
    urow("longing", "Longing", "He I", 447.1, "#5600FF"),
    urow("contempt", "Contempt", "He II", 541.2, "#00FF00"),
    urow("embarrassment", "Embarrassment", "[O I]", 630.0, "#FF0000"),
    urow("hope", "Hope", "[O III]", 500.7, "#00FF9C"),
    urow("despair", "Despair", "H I H-delta", 410.2, "#7000FF"),
    urow("serenity", "Serenity", "H I H-beta", 486.1, "#00C0FF"),
    urow("love", "Love", "H I H-alpha", 656.3, "#FF0000"),
    urow("jealousy", "Jealousy", "[N II]", 658.4, "#FF0000"),
    urow("envy", "Envy", "[O III]", 500.7, "#00FF9C"),
    urow("gratitude", "Gratitude", "He I", 587.6, "#FF8700"),
    urow("compassion", "Compassion", "He I", 667.8, "#FF0000"),
    urow("loneliness", "Loneliness", "He I", 447.1, "#5600FF"),
    urow("boredom", "Boredom", "[Cl III]", 551.7, "#00FF00"),
    urow("anxiety", "Anxiety", "H I H-delta", 410.2, "#7000FF"),
    urow("interest", "Interest / curiosity", "[O III]", 495.9, "#00FFCB"),
    urow("confusion", "Confusion", "H I H-gamma", 434.0, "#6700FF"),
    urow("triumph", "Triumph", "[N II]", 575.5, "#FFD300"),
    urow("admiration", "Admiration", "He I", 587.6, "#FF8700"),
    urow("adoration", "Adoration", "He I", 667.8, "#FF0000"),
    urow("aesthetic_appreciation", "Aesthetic appreciation", "[Ar IV]", 474.0, "#0054FF"),
    urow("awkwardness", "Awkwardness", "[O I]", 630.0, "#FF0000"),
    urow("craving", "Craving", "[O I]", 630.0, "#FF0000"),
    urow("empathic_pain", "Empathic pain", "He I", 447.1, "#5600FF"),
    urow("entrancement", "Entrancement", "[Ar IV]", 474.0, "#0054FF"),
    urow("horror", "Horror", "H I H-delta", 410.2, "#7000FF"),
    urow("romance", "Romance", "He I", 667.8, "#FF0000"),
    urow("satisfaction", "Satisfaction", "H I H-beta", 486.1, "#00C0FF"),
    urow("sexual_desire", "Sexual desire", "H I H-alpha", 656.3, "#FF0000"),
    urow("sympathy", "Sympathy", "He I", 667.8, "#FF0000"),
    urow("contentment", "Contentment", "H I H-beta", 486.1, "#00C0FF"),
    urow("pleasure", "Pleasure", "He I", 667.8, "#FF0000"),
    urow("regret", "Regret", "[Cl III]", 551.7, "#00FF00"),
    urow("hate", "Hate", "[S II]", 671.6, "#FF0000"),
    urow("annoyance", "Annoyance", "[O I]", 630.0, "#FF0000"),
    urow("approval", "Approval", "[O III]", 500.7, "#00FF9C"),
    urow("caring", "Caring", "He I", 667.8, "#FF0000"),
    urow("desire", "Desire (general)", "Ne I (lab)", 585.2, "#FF9500"),
    urow("disapproval", "Disapproval", "[O I]", 630.0, "#FF0000"),
    urow("grief", "Grief", "He II", 468.6, "#0017FF"),
    urow("nervousness", "Nervousness", "H I H-delta", 410.2, "#7000FF"),
    urow("optimism", "Optimism", "[N II]", 575.5, "#FFD300"),
    urow("realization", "Realization", "[O III]", 495.9, "#00FFCB"),
    urow("remorse", "Remorse", "[Cl III]", 551.7, "#00FF00"),
];

pub fn demo(emotion: &str) -> Option<&'static DemoLine> {
    let key = emotion.trim();
    DEMO.iter().find(|line| eq_ignore_ascii(line.emotion, key) || eq_ignore_ascii(line.doc_name, key) || eq_ignore_ascii(line.element_id, key))
}

pub fn universal(emotion: &str) -> Option<&'static UniversalLine> {
    let key = emotion.trim();
    UNIVERSAL.iter().find(|line| eq_ignore_ascii(line.emotion, key) || eq_ignore_ascii(line.doc_name, key))
}

fn eq_ignore_ascii(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Attach the hard-coded element id. A supplied id that does not match the
/// table is an error: the mapping is not configurable.
pub fn label_weight(emotion: &str, weight: f32, supplied_element: Option<&str>) -> Result<LabeledWeight, LabelError> {
    let line = demo(emotion).ok_or(LabelError::UnknownEmotion)?;
    if let Some(id) = supplied_element {
        if !id.is_empty() && !eq_ignore_ascii(id, line.element_id) {
            return Err(LabelError::ElementMismatch);
        }
    }
    Ok(LabeledWeight { emotion: line.emotion, weight, element_id: line.element_id })
}

pub fn srgb_u8_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

pub fn linear_rgb(srgb: [u8; 3]) -> [f32; 3] {
    [srgb_u8_to_linear(srgb[0]), srgb_u8_to_linear(srgb[1]), srgb_u8_to_linear(srgb[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_is_thirteen_unique_lines_and_universal_is_sixty_two() {
        assert_eq!(DEMO.len(), 13);
        assert_eq!(UNIVERSAL.len(), 62);
        let mut waves = DEMO.map(|line| (line.wavelength_nm * 10.0).round() as i32);
        waves.sort_unstable();
        for pair in waves.windows(2) {
            assert_ne!(pair[0], pair[1], "demo wavelengths must be unique");
        }
        for line in DEMO {
            let found = universal(line.emotion).unwrap();
            assert!((found.wavelength_nm - line.wavelength_nm).abs() < 1e-9);
            assert_eq!(found.spectral_hex, line.spectral_hex);
        }
    }

    #[test]
    fn admin_display_targets_are_unchanged() {
        let expect = [
            ("neutral", "#00FF00", "#9DA3A8"),
            ("calm", "#00C0FF", "#47B5D1"),
            ("happy", "#FFD300", "#FFCF40"),
            ("amused", "#FF8700", "#FF9C64"),
            ("excited", "#FF6300", "#FF6300"),
            ("tender", "#FF0000", "#FFB5C5"),
            ("sad", "#0017FF", "#354766"),
            ("disappointed", "#00FF00", "#676A70"),
            ("angry", "#FF0000", "#D90000"),
            ("fearful", "#7000FF", "#171027"),
            ("surprised", "#00FFCB", "#00FFCB"),
            ("disgusted", "#39FF00", "#8CAF24"),
            ("seductive", "#FF0000", "#C42F63"),
        ];
        for (id, spectral, display) in expect {
            let line = demo(id).unwrap();
            assert_eq!(line.spectral_hex, spectral);
            assert_eq!(line.display_hex, display);
        }
    }

    #[test]
    fn element_ids_are_not_configurable() {
        let ok = label_weight("angry", 0.4, None).unwrap();
        assert_eq!(ok.element_id, "SIII-631.2");
        assert!(label_weight("angry", 0.4, Some("H-alpha")).is_err());
        assert!(label_weight("love", 1.0, None).is_err(), "universal-only rows are not demo layers");
        assert!(universal("love").is_some());
        assert!(demo("seductive").unwrap().samantha_only);
        assert!(demo("excited").unwrap().lab_line);
        assert!(demo("surprised").unwrap().pulse > 0.0);
        assert!(demo("angry").unwrap().motion > demo("seductive").unwrap().motion);
    }
}
