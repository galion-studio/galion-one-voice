//! Persona conduct for one 80 ms frame.
//!
//! CPU arithmetic only. No accelerator crate, no weight file, no phrase table.
//! [`Feeling::live`] calls [`crate::express::StubExpressor::voice_color`] so the
//! empathy weight and the OKLCH colour are the live galion-emotion decision.
//! Prosody is that weight on this persona's column. Jarvis does not become
//! Samantha when the weight rises. Samantha does not borrow his dryness.
//!
//! The reference path does not denoise. [`STRIP_REFERENCE_TEXTURE`] stays false.
//! Jarvis's Tony Stark hub bed (clicks, alarms, room tone) is a character layer
//! on the me band. It is toggleable. It never reads or writes caller audio.
//!
//! Situational mood is this band. [`Feeling::situational`] is [`Feeling::live`]
//! plus [`Timbre::REFERENCE`]. It does not scale timbre. `None` is consent deny:
//! `e` is [`crate::express::empathy_weight`] of no frame, which is 0. The mood
//! is not a second score.

use crate::express::{Oklch, ProsodyControl, StubExpressor};
use crate::frame::{Affect, EmotionFrame};

/// The path never strips a persona reference.
pub const STRIP_REFERENCE_TEXTURE: bool = false;
/// Empathy is the live weight. There is no scripted line to speak.
pub const CANNED_EMPATHY: bool = false;
/// Peak of a hub click on the me band, before the frame's loudness scale.
pub const HUB_GAIN: f32 = 0.04;

/// Register the heartbeat is speaking in. Not a film checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoicePersona {
    Samantha,
    Jarvis,
}

impl VoicePersona {
    pub fn as_str(self) -> &'static str {
        match self {
            VoicePersona::Samantha => "samantha",
            VoicePersona::Jarvis => "jarvis",
        }
    }
}

/// How the voice behaves this frame. Not a ninth affect label, and not a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manner {
    /// Weight is 0 and the caller is not in joy. The column at rest.
    Plain,
    /// Live weight is up. Answer in place: quieter, not a script.
    Answered,
    /// Joy is shared slightly under the caller. The weight stays 0.
    Shared,
}

/// Hub clicks, alarms, and room tone for Jarvis. Not a denoise, and not the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HubLayer {
    /// True only for Jarvis when the host has the layer on.
    pub enabled: bool,
    /// 0 when the layer is off. [`HUB_GAIN`] when it is on.
    pub gain: f32,
}

impl HubLayer {
    /// Samantha has no bed. Jarvis has one when `on` is true. Default in
    /// [`Feeling::live`] is on, so the character is present until the host mutes it.
    pub fn character(persona: VoicePersona, on: bool) -> Self {
        if persona == VoicePersona::Jarvis && on {
            Self { enabled: true, gain: HUB_GAIN }
        } else {
            Self { enabled: false, gain: 0.0 }
        }
    }

    /// Add the bed to `me`. Caller audio is not an argument and is not touched.
    /// Off is a no-op, so muting the layer does not rewrite the reference.
    pub fn mix_me(self, me: &mut [f32]) {
        if !self.enabled || !(self.gain > 0.0) || me.is_empty() {
            return;
        }
        let n = me.len();
        let click = n / 8;
        let alarm = n / 2;
        if click < n {
            me[click] = (me[click] + self.gain).clamp(-1.0, 1.0);
        }
        if alarm < n && alarm != click {
            me[alarm] = (me[alarm] + self.gain * 0.65).clamp(-1.0, 1.0);
        }
        let mut i = 0;
        while i < n {
            if i != click && i != alarm {
                me[i] = (me[i] + self.gain * 0.08).clamp(-1.0, 1.0);
            }
            i = i.saturating_add(64);
        }
    }
}

/// Voice body. Not the feeling band.
///
/// `match_strength` is an LTAS timbre match. Zero leaves the reference
/// bit-identical. Situational mood does not write this value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timbre {
    pub match_strength: f32,
}

impl Timbre {
    /// Reference body. Mood is not a parameter.
    pub const REFERENCE: Self = Self { match_strength: 0.0 };
}

/// Feeling band for one frame. Copy, stack only. No text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feeling {
    pub persona: VoicePersona,
    /// 0 = no empathy blend. From [`crate::express::empathy_weight`].
    pub e: f32,
    pub oklch: Oklch,
    pub prosody: ProsodyControl,
    /// Amber share of the background wash. Not a circle colour.
    pub wash_amber: f32,
    pub manner: Manner,
    /// Character bed. Separate from the you band.
    pub hub: HubLayer,
    /// Control tokens may exist later. Jarvis is always false. Piper must not speak them.
    pub tags: bool,
}

/// pitch st, range, rate, loudness, warmth, pre-pause ms, wash amber.
type Column = (f32, f32, f32, f32, f32, f32, f32);

/// Samantha at rest. Warm, no extra pause. Design table in the voice draft.
const SAMANTHA_E0: Column = (0.0, 1.00, 1.00, 1.00, 0.62, 0.0, 0.62);
/// Samantha at e = 1. Same profile as [`EMPATHY_PROSODY`].
const SAMANTHA_E1: Column = (-0.5, 0.88, 0.92, 0.80, 0.75, 250.0, 0.50);
/// Jarvis at rest. Lower, drier, a short pre-pause so he does not step on them.
const JARVIS_E0: Column = (-0.3, 0.92, 0.96, 0.92, 0.42, 80.0, 0.28);
/// Jarvis at e = 1. Still drier than Samantha. Warmth stays under her ceiling.
const JARVIS_E1: Column = (-0.4, 0.90, 0.94, 0.88, 0.55, 120.0, 0.50);

impl Feeling {
    /// Persona at rest. e = 0, neutral mark, no empathy colour. Hub layer on for Jarvis.
    pub fn rest(persona: VoicePersona) -> Self {
        Self::live(persona, None)
    }

    /// One consented caller frame → this persona's conduct. Jarvis hub layer on.
    /// `None` is no frame (no consent, or nothing to answer): e stays 0.
    pub fn live(persona: VoicePersona, caller: Option<&EmotionFrame>) -> Self {
        Self::conduct(persona, caller, true)
    }

    /// Situational mood on the feeling band.
    ///
    /// `caller` is a frame the host already consented to read. `None` is deny.
    /// `e` is [`crate::express::empathy_weight`] of that frame, never a mood
    /// valence copied into a score. The timbre is always [`Timbre::REFERENCE`].
    pub fn situational(persona: VoicePersona, caller: Option<&EmotionFrame>) -> (Self, Timbre) {
        (Self::live(persona, caller), Timbre::REFERENCE)
    }

    /// Same as [`Self::live`], with the hub layer forced on or off.
    /// The flag does not change `e`, OKLCH, or prosody. Samantha stays silent either way.
    pub fn conduct(persona: VoicePersona, caller: Option<&EmotionFrame>, hub_on: bool) -> Self {
        let color = StubExpressor.voice_color(caller);
        let e = color.e;
        let (lo, hi) = match persona {
            VoicePersona::Samantha => (SAMANTHA_E0, SAMANTHA_E1),
            VoicePersona::Jarvis => (JARVIS_E0, JARVIS_E1),
        };
        let mut prosody = lerp_column(lo, hi, e);
        let manner = manner_of(caller, e);
        if manner == Manner::Shared && persona == VoicePersona::Samantha {
            // Share the rise, slightly under a bright caller. Not an exclamation.
            prosody.pitch_shift_st = (prosody.pitch_shift_st + 0.12).min(0.18);
            prosody.rate = (prosody.rate + 0.03).min(1.04);
            prosody = prosody.clamped();
        }
        Self {
            persona,
            e,
            oklch: color.oklch,
            prosody,
            wash_amber: lerp(lo.6, hi.6, e),
            manner,
            hub: HubLayer::character(persona, hub_on),
            tags: persona == VoicePersona::Samantha,
        }
    }
}

fn manner_of(caller: Option<&EmotionFrame>, e: f32) -> Manner {
    if e > 0.0 {
        return Manner::Answered;
    }
    match caller.map(EmotionFrame::label) {
        Some(Affect::Joy | Affect::Excited) => Manner::Shared,
        _ => Manner::Plain,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_column(lo: Column, hi: Column, e: f32) -> ProsodyControl {
    let e = if e.is_finite() { e.clamp(0.0, 1.0) } else { 0.0 };
    ProsodyControl {
        pitch_shift_st: lerp(lo.0, hi.0, e),
        pitch_range: lerp(lo.1, hi.1, e),
        rate: lerp(lo.2, hi.2, e),
        energy: lerp(lo.3, hi.3, e),
        warmth: lerp(lo.4, hi.4, e),
        pre_pause_ms: lerp(lo.5, hi.5, e).round() as u16,
    }
    .clamped()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::express::{EMPATHY_OKLCH, EMPATHY_PROSODY, NEUTRAL_OKLCH};

    fn frame(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
        EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: v, arousal: a, dominance: d, confidence: c }
    }

    #[test]
    fn no_frame_is_rest_and_not_a_script() {
        assert!(!CANNED_EMPATHY);
        assert!(!STRIP_REFERENCE_TEXTURE);
        let s = Feeling::rest(VoicePersona::Samantha);
        let j = Feeling::rest(VoicePersona::Jarvis);
        assert_eq!(s.e, 0.0);
        assert_eq!(s.manner, Manner::Plain);
        assert_eq!(s.oklch, NEUTRAL_OKLCH);
        assert!(!s.hub.enabled);
        assert!(s.tags);
        assert!(j.hub.enabled);
        assert!((j.hub.gain - HUB_GAIN).abs() < 1e-6);
        assert!(!j.tags);
        assert!((j.prosody.warmth - 0.42).abs() < 1e-4);
        assert_eq!(j.prosody.pre_pause_ms, 80);
        let src = include_str!("persona.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        for line in [
            "i hear you",
            "that sounds so hard",
            "i'm here for you",
            "take a deep breath",
            "i know this is difficult",
            "i'm worried about you",
        ] {
            assert!(!code.to_ascii_lowercase().contains(line), "{line}");
        }
        let low = code.to_ascii_lowercase();
        assert!(!low.contains("gguf"));
        assert!(!low.contains("cuda"));
    }

    #[test]
    fn tired_caller_is_live_empathy_in_each_column() {
        let caller = frame(-0.6, 0.2, 0.4, 1.0);
        let s = Feeling::live(VoicePersona::Samantha, Some(&caller));
        assert!((s.e - 1.0).abs() < 1e-6);
        assert_eq!(s.manner, Manner::Answered);
        assert_eq!(s.prosody.pitch_shift_st, EMPATHY_PROSODY.pitch_shift_st);
        assert_eq!(s.prosody.pitch_range, EMPATHY_PROSODY.pitch_range);
        assert_eq!(s.prosody.rate, EMPATHY_PROSODY.rate);
        assert_eq!(s.prosody.energy, EMPATHY_PROSODY.energy);
        assert_eq!(s.prosody.warmth, EMPATHY_PROSODY.warmth);
        assert_eq!(s.prosody.pre_pause_ms, 250);
        assert_eq!(s.oklch, EMPATHY_OKLCH);

        let j = Feeling::live(VoicePersona::Jarvis, Some(&caller));
        assert!((j.e - 1.0).abs() < 1e-6);
        assert_eq!(j.oklch, EMPATHY_OKLCH);
        assert!((j.prosody.pitch_shift_st - -0.4).abs() < 1e-4);
        assert!((j.prosody.warmth - 0.55).abs() < 1e-4);
        assert_eq!(j.prosody.pre_pause_ms, 120);
        assert!(j.prosody.warmth < s.prosody.warmth);
        assert!(j.prosody.pre_pause_ms < s.prosody.pre_pause_ms);
        assert!(j.hub.enabled);
    }

    #[test]
    fn hub_layer_toggles_without_touching_caller_audio() {
        let caller = frame(-0.6, 0.2, 0.4, 1.0);
        let on = Feeling::conduct(VoicePersona::Jarvis, Some(&caller), true);
        let off = Feeling::conduct(VoicePersona::Jarvis, Some(&caller), false);
        assert!(on.hub.enabled);
        assert!(!off.hub.enabled);
        assert_eq!(off.hub.gain, 0.0);
        assert_eq!(on.e, off.e);
        assert_eq!(on.oklch, off.oklch);
        assert_eq!(on.prosody, off.prosody);
        let forced = Feeling::conduct(VoicePersona::Samantha, Some(&caller), true);
        assert!(!forced.hub.enabled, "samantha has no hub bed to enable");

        let you = [0.25f32; 128];
        let you_before = you;
        let mut me = [0.0f32; 128];
        on.hub.mix_me(&mut me);
        assert_eq!(you, you_before, "mix_me has no caller buffer");
        assert!(me.iter().any(|s| *s != 0.0));
        let mut muted = [0.0f32; 128];
        off.hub.mix_me(&mut muted);
        assert!(muted.iter().all(|s| *s == 0.0));
        assert!(!STRIP_REFERENCE_TEXTURE);
    }

    #[test]
    fn anger_is_answered_not_copied_and_joy_is_shared() {
        let angry = frame(-0.8, 0.9, 0.8, 0.9);
        let s = Feeling::live(VoicePersona::Samantha, Some(&angry));
        let j = Feeling::live(VoicePersona::Jarvis, Some(&angry));
        assert!(s.e > 0.8 && j.e > 0.8);
        assert_eq!(s.manner, Manner::Answered);
        assert!(s.prosody.pitch_shift_st < 0.0, "samantha does not rise to match anger");
        assert!(j.prosody.pitch_shift_st <= -0.3, "jarvis stays at or under his rest pitch");
        assert!(j.prosody.warmth <= 0.55 + 1e-4);

        let joy = frame(0.8, 0.4, 0.5, 0.9);
        let sj = Feeling::live(VoicePersona::Samantha, Some(&joy));
        let jj = Feeling::live(VoicePersona::Jarvis, Some(&joy));
        assert_eq!(sj.e, 0.0);
        assert_eq!(sj.manner, Manner::Shared);
        assert!(sj.prosody.pitch_shift_st > 0.0 && sj.prosody.pitch_shift_st <= 0.18);
        assert_eq!(jj.manner, Manner::Shared);
        assert!((jj.prosody.pitch_shift_st - -0.3).abs() < 1e-4, "jarvis amusement does not brighten");
        assert_eq!(jj.oklch, NEUTRAL_OKLCH);
    }

    #[test]
    fn situational_mood_stays_on_the_feeling_band() {
        use crate::consent::{ConsentDecision, ConsentGate, DefaultConsentGate, Purpose};
        use crate::express::empathy_weight;

        let gate = DefaultConsentGate::default();
        assert_eq!(gate.check(Purpose::PersonalAssistant), ConsentDecision::DenyNoConsent);

        let (s_rest, s_body) = Feeling::situational(VoicePersona::Samantha, None);
        let (j_rest, j_body) = Feeling::situational(VoicePersona::Jarvis, None);
        assert_eq!(s_rest, Feeling::rest(VoicePersona::Samantha));
        assert_eq!(j_rest, Feeling::rest(VoicePersona::Jarvis));
        assert_eq!(s_rest.e, empathy_weight(None));
        assert_eq!(s_rest.e, 0.0);
        assert_eq!(j_rest.e, 0.0);
        assert_eq!(s_body, Timbre::REFERENCE);
        assert_eq!(j_body, Timbre::REFERENCE);
        assert_eq!(s_body.match_strength, 0.0);

        let tired = frame(-0.6, 0.2, 0.4, 1.0);
        let (s, body) = Feeling::situational(VoicePersona::Samantha, Some(&tired));
        let (j, jbody) = Feeling::situational(VoicePersona::Jarvis, Some(&tired));
        assert_eq!(s, Feeling::live(VoicePersona::Samantha, Some(&tired)));
        assert_eq!(j, Feeling::live(VoicePersona::Jarvis, Some(&tired)));
        assert_eq!(s.e, empathy_weight(Some(&tired)));
        assert!((s.e - 1.0).abs() < 1e-6);
        assert_eq!(s.manner, Manner::Answered);
        assert_eq!(body, Timbre::REFERENCE);
        assert_eq!(jbody.match_strength, 0.0);
        assert!(j.hub.enabled, "hub is character on the me band, not timbre");

        let joy = frame(0.8, 0.4, 0.5, 0.9);
        let (shared, joy_body) = Feeling::situational(VoicePersona::Samantha, Some(&joy));
        assert_eq!(shared.e, 0.0, "joy is not an empathy score");
        assert_eq!(shared.e, empathy_weight(Some(&joy)));
        assert_eq!(shared.manner, Manner::Shared);
        assert_eq!(joy_body, Timbre::REFERENCE);

        let src = include_str!("persona.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src).to_ascii_lowercase();
        assert!(!code.contains(".safetensors"));
        assert!(!code.contains(".gguf"));
        assert!(!code.contains("cuda"));
    }

    #[test]
    fn columns_match_the_voice_drafts() {
        let sam = include_str!("../../docs/galion-voice-v2/personas/voice/samantha.toml");
        let jar = include_str!("../../docs/galion-voice-v2/personas/voice/jarvis.toml");
        assert!(sam.contains("pitch_st = 0.0"));
        assert!(sam.contains("warmth = 0.62"));
        assert!(sam.contains("pre_pause_ms = 250"));
        assert!(jar.contains("pitch_st = -0.3"));
        assert!(jar.contains("warmth = 0.42"));
        assert!(jar.contains("pre_pause_ms = 120"));
        assert!(jar.contains("warmth = 0.55"));
    }
}
