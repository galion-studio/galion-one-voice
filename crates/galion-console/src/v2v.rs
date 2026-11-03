//! Voice-to-voice outline for the Samantha and Jarvis showcase.
//!
//! Host loop, after `Heartbeat::step` returns:
//!
//! ```text
//! Vosk final  →  instruction reply  →  Nano request line
//!                     ↑
//!         EmotionClock once per 80 ms (OKLCH + prosody weight)
//! playback mix of the speech asset and the Jarvis hub layer
//! ```
//!
//! The heartbeat step does not call this module. Nano generate is the
//! out-of-process worker. The mouth is Nano when that worker reports
//! `chatterbox-nano` or `nano`. `unwired`, `protocol-stub`, and an absent
//! child stay blocked. Piper remains the fallback the phone already has.
//! The hub layer is mixed here, at playback, and is absent from the worker line.
//!
//! Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

use galion_emotion::{EmotionClock, EmotionFrame, VoiceColor};
use galion_render::v2_persona::{self, Floor, Prosody};

/// The child did not report a Nano engine. Real generate reports `chatterbox-nano`.
pub const NANO_BLOCKED: &str = "BLOCKED: clone worker engine is unwired, protocol-stub, or absent. A real Nano mouth reports chatterbox-nano. Piper remains the fallback. This outline does not invent PCM.";

/// Wire key for the Jarvis hub layer. Absent on a toggle line means on.
pub const HUB_KEY: &str = "hub_texture";

const MIN_RATE: u32 = 16_000;

/// Cleaners. A toggle line that names one is refused. Off is the boolean.
const FORBIDDEN_CLEAN: &[&str] = &[
    "deepfilternet",
    "deepfilter",
    "rnnoise",
    "facebookdenoiser",
    "resembleenhance",
    "podcastenhance",
    "dialoguenhance",
    "speechenhance",
    "voiceenhance",
    "enhancevoice",
    "denoiser",
    "denoise",
    "noisereduce",
    "noisereduction",
    "noisered",
    "spectralsubtraction",
    "spectralsubtract",
    "spectralgate",
    "wiener",
    "afftdn",
    "anlmdn",
    "arnndn",
    "auphonic",
    "krisp",
    "izotope",
    "audiorestoration",
    "declick",
    "decrackle",
    "dehum",
    "dereverb",
    "deroom",
    "voiceisolation",
    "vocalisolation",
    "isolatevoice",
    "isolatevocal",
    "stemseparat",
    "demucs",
    "spleeter",
    "noisegate",
    "noisegating",
    "expandergate",
    "stripsilence",
    "silenceremove",
    "removesilence",
    "vadtrim",
    "removenoise",
    "removebackground",
    "backgroundremoval",
    "cleanreference",
    "cleanaudio",
    "cleanvoice",
    "audioclean",
    "highpass",
    "lowpass",
    "highcut",
    "lowcut",
];

/// Where the spoken asset comes from. This outline does not spawn the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouthSlot {
    /// Worker reported `unwired`, `protocol-stub`, or never started.
    /// Do not play that PCM as Nano.
    NanoBlocked,
    /// The phone's warm Piper worker. This outline does not start it.
    PiperFallback,
    /// Engine string is `chatterbox-nano` or `nano`. The host may send
    /// [`nano_request_line`] to the clone worker.
    Nano,
}

/// On mixes the hub layer at playback. Off returns the speech asset.
/// A missing flag is on: silence of the layer is the boolean, not a cleaner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureToggle {
    pub enabled: bool,
}

impl Default for TextureToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl TextureToggle {
    pub fn from_flag(enabled: Option<bool>) -> Self {
        Self {
            enabled: enabled.unwrap_or(true),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubError {
    ToggleLine,
    Persona,
    Rate,
    Length,
    Cleaner(&'static str),
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HubError::ToggleLine => f.write_str("hub toggle line"),
            HubError::Persona => f.write_str("hub layer is jarvis only"),
            HubError::Rate => f.write_str("hub layer rate"),
            HubError::Length => f.write_str("hub layer length"),
            HubError::Cleaner(token) => f.write_fmt(format_args!("hub cleaner refused: {token}")),
        }
    }
}

/// One turn the host can speak or show. No audio samples live here.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakPlan {
    pub persona: &'static str,
    pub text: String,
    pub silence: bool,
    pub e: f32,
    pub oklch_l: f32,
    pub oklch_c: f32,
    pub oklch_h: f32,
    pub pitch_st: f32,
    pub pitch_range: f32,
    pub rate: f32,
    pub loudness: f32,
    pub warmth: f32,
    pub pre_pause_ms: u32,
    pub wash_amber: f32,
    pub slot: MouthSlot,
    pub blocked: Option<&'static str>,
    pub hub: TextureToggle,
}

impl SpeakPlan {
    /// Nano samples are not produced on this path.
    pub fn nano_pcm_bytes(&self) -> usize {
        0
    }
}

/// Duplex host. Feeling on the frame. Words when Vosk emits a final.
/// Playback of the hub layer when the host already has two buffers.
pub struct DuplexOutline {
    clock: EmotionClock,
    floor: Floor,
    sense: bool,
    slot: MouthSlot,
    hub: TextureToggle,
    text: String,
    silence: bool,
}

impl Default for DuplexOutline {
    fn default() -> Self {
        Self::new()
    }
}

impl DuplexOutline {
    pub fn new() -> Self {
        Self {
            clock: EmotionClock::new(),
            floor: Floor::new(),
            sense: false,
            slot: MouthSlot::Nano,
            hub: TextureToggle::default(),
            text: String::new(),
            silence: true,
        }
    }

    pub fn set_sense(&mut self, sense: bool) {
        self.sense = sense;
    }

    pub fn set_hub(&mut self, enabled: bool) {
        self.hub.enabled = enabled;
    }

    pub fn hub(&self) -> TextureToggle {
        self.hub
    }

    /// Worker stderr engine. `protocol-stub` and `unwired` stay blocked.
    pub fn set_worker_engine(&mut self, engine: &str) {
        self.slot = mouth_from_engine(engine);
    }

    pub fn slot(&self) -> MouthSlot {
        self.slot
    }

    /// Once per 80 ms frame, after `Heartbeat::step`. No consent drops the frame.
    pub fn on_heartbeat_frame(&mut self, caller: Option<&EmotionFrame>) -> VoiceColor {
        let caller = if self.sense { caller } else { None };
        self.clock.on_frame(caller)
    }

    /// Vosk final, off the tick. A canned empathy line is dropped.
    pub fn on_asr_final(&mut self, text: &str) -> SpeakPlan {
        let e = self.clock.last().e;
        let said = self.floor.say_turn(text, false, e, self.sense);
        if v2_persona::is_canned_empathy(&said.reply) {
            self.text.clear();
            self.silence = true;
        } else {
            self.silence = said.silence || said.reply.is_empty();
            self.text = said.reply;
        }
        self.plan()
    }

    pub fn plan(&self) -> SpeakPlan {
        let color = self.clock.last();
        let prosody = self.floor.prosody_at(color.e);
        plan_from(self, color, prosody)
    }

    /// Worker request. Text, model, persona. The hub layer is not on this line.
    pub fn nano_request_line(&self) -> String {
        nano_request_line(&self.text, self.floor.id())
    }

    /// Playback mix. Jarvis only. Off copies `asset`. On adds `layer`.
    /// Neither buffer is written. Samantha returns the speech asset alone.
    pub fn playback(
        &self,
        asset: &[f32],
        asset_rate: u32,
        layer: &[f32],
        layer_rate: u32,
    ) -> Result<Vec<f32>, HubError> {
        if self.floor.id() != "jarvis" {
            return Ok(asset.to_vec());
        }
        mix_hub_playback("jarvis", asset, asset_rate, layer, layer_rate, self.hub)
    }
}

fn plan_from(outline: &DuplexOutline, color: VoiceColor, prosody: Prosody) -> SpeakPlan {
    let blocked = match outline.slot {
        MouthSlot::NanoBlocked => Some(NANO_BLOCKED),
        MouthSlot::PiperFallback | MouthSlot::Nano => None,
    };
    SpeakPlan {
        persona: outline.floor.id(),
        text: outline.text.clone(),
        silence: outline.silence,
        e: color.e,
        oklch_l: color.oklch.l,
        oklch_c: color.oklch.c,
        oklch_h: color.oklch.h,
        pitch_st: prosody.pitch_st,
        pitch_range: prosody.pitch_range,
        rate: prosody.rate,
        loudness: prosody.loudness,
        warmth: prosody.warmth,
        pre_pause_ms: prosody.pre_pause_ms,
        wash_amber: prosody.wash_amber,
        slot: outline.slot,
        blocked,
        hub: outline.hub,
    }
}

/// `chatterbox-nano` and `nano` open the Nano mouth. `piper` is the fallback.
/// `unwired`, `protocol-stub`, `absent`, and an empty report stay blocked.
pub fn mouth_from_engine(engine: &str) -> MouthSlot {
    match engine.trim() {
        "nano" | "chatterbox-nano" => MouthSlot::Nano,
        "piper" => MouthSlot::PiperFallback,
        _ => MouthSlot::NanoBlocked,
    }
}

/// JSON line for the clone worker. No reference, no hub samples, no PCM.
pub fn nano_request_line(text: &str, persona: &str) -> String {
    let persona = if persona == "jarvis" {
        "jarvis"
    } else {
        "samantha"
    };
    format!(
        "{{\"text\":{},\"model\":\"nano\",\"persona\":\"{persona}\"}}",
        v2_persona::json_string(text)
    )
}

fn normalize_stage(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn match_cleaner(name: &str) -> Option<&'static str> {
    let n = normalize_stage(name);
    if n.is_empty() {
        return None;
    }
    let mut best: Option<&'static str> = None;
    let mut best_len = 0usize;
    for token in FORBIDDEN_CLEAN {
        if token.len() > best_len && n.contains(token) {
            best = Some(*token);
            best_len = token.len();
        }
    }
    best
}

fn json_bool_field(line: &str, key: &str) -> Result<Option<bool>, HubError> {
    let needle = format!("\"{key}\"");
    let Some(i) = line.find(&needle) else {
        return Ok(None);
    };
    let rest = line[i + needle.len()..].trim_start();
    let rest = rest
        .strip_prefix(':')
        .ok_or(HubError::ToggleLine)?
        .trim_start();
    if rest.starts_with("true") {
        Ok(Some(true))
    } else if rest.starts_with("false") {
        Ok(Some(false))
    } else {
        Err(HubError::ToggleLine)
    }
}

fn json_string_field<'a>(line: &'a str, key: &str) -> Result<Option<&'a str>, HubError> {
    let needle = format!("\"{key}\"");
    let Some(i) = line.find(&needle) else {
        return Ok(None);
    };
    let rest = line[i + needle.len()..].trim_start();
    let rest = rest
        .strip_prefix(':')
        .ok_or(HubError::ToggleLine)?
        .trim_start();
    let rest = rest.strip_prefix('"').ok_or(HubError::ToggleLine)?;
    let end = rest.find('"').ok_or(HubError::ToggleLine)?;
    Ok(Some(&rest[..end]))
}

/// Parse `{"persona":"jarvis","hub_texture":false}`. A missing flag is on.
pub fn parse_toggle_line(line: &str) -> Result<TextureToggle, HubError> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(HubError::ToggleLine);
    }
    if let Some(token) = match_cleaner(trimmed) {
        return Err(HubError::Cleaner(token));
    }
    if let Some(persona) = json_string_field(trimmed, "persona")? {
        if persona != "jarvis" {
            return Err(HubError::Persona);
        }
    }
    Ok(TextureToggle::from_flag(json_bool_field(trimmed, HUB_KEY)?))
}

/// Playback mix for the Jarvis hub. Reads both slices. Writes neither.
/// Off is a copy of `asset`. On is `asset[i] + layer[i]`.
pub fn mix_hub_playback(
    persona: &str,
    asset: &[f32],
    asset_rate: u32,
    layer: &[f32],
    layer_rate: u32,
    toggle: TextureToggle,
) -> Result<Vec<f32>, HubError> {
    if persona != "jarvis" {
        return Err(HubError::Persona);
    }
    if asset_rate < MIN_RATE || asset_rate != layer_rate {
        return Err(HubError::Rate);
    }
    if asset.len() != layer.len() {
        return Err(HubError::Length);
    }
    if !toggle.enabled {
        return Ok(asset.to_vec());
    }
    Ok(asset.iter().zip(layer.iter()).map(|(a, b)| a + b).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_emotion::{EMPATHY_OKLCH, NEUTRAL_OKLCH};
    use galion_heartbeat::FRAME_MS;

    fn frame(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
        EmotionFrame {
            seq: 1,
            t_ms: 80,
            dur_ms: 80,
            valence: v,
            arousal: a,
            dominance: d,
            confidence: c,
        }
    }

    #[test]
    fn eighty_millisecond_clock_reaches_heather_only_with_sense() {
        assert_eq!(FRAME_MS, 80);
        let mut path = DuplexOutline::new();
        let tired = frame(-0.6, 0.2, 0.4, 1.0);
        let hidden = path.on_heartbeat_frame(Some(&tired));
        assert_eq!(hidden.e, 0.0);
        assert_eq!(hidden.oklch, NEUTRAL_OKLCH);
        path.set_sense(true);
        let shown = path.on_heartbeat_frame(Some(&tired));
        assert!((shown.e - 1.0).abs() < 1e-6);
        assert_eq!(shown.oklch, EMPATHY_OKLCH);
        let plan = path.plan();
        assert!((plan.warmth - 0.75).abs() < 1e-3);
        assert_eq!(plan.pre_pause_ms, 250);
        assert!((plan.pitch_st - -0.5).abs() < 1e-3);
        assert!((plan.wash_amber - 0.50).abs() < 1e-3);
        assert_eq!(plan.slot, MouthSlot::Nano);
        assert!(plan.blocked.is_none());
        assert_eq!(plan.nano_pcm_bytes(), 0);
    }

    #[test]
    fn asr_final_stays_on_the_matter_and_the_worker_line_has_no_hub() {
        let mut path = DuplexOutline::new();
        path.set_sense(true);
        path.on_heartbeat_frame(Some(&frame(-0.6, 0.2, 0.4, 1.0)));
        let plan = path.on_asr_final("My mother died this morning.");
        assert_eq!(plan.persona, "samantha");
        // #23: matter content goes to Hermes; with no GALION_LLM_URL the
        // honest unreachable line is spoken, never a canned echo.
        assert_eq!(plan.text, "model can't be reached");
        assert!(!v2_persona::is_canned_empathy(&plan.text));
        let line = path.nano_request_line();
        assert!(line.contains("\"model\":\"nano\""));
        assert!(line.contains("\"persona\":\"samantha\""));
        assert!(line.contains("model can't be reached"));
        assert!(!line.contains(HUB_KEY));
        assert!(!line.contains("reference"));
        assert!(!line.contains("pcm"));
        assert_eq!(mouth_from_engine("unwired"), MouthSlot::NanoBlocked);
        assert_eq!(mouth_from_engine("protocol-stub"), MouthSlot::NanoBlocked);
        assert_eq!(mouth_from_engine("absent"), MouthSlot::NanoBlocked);
        assert_eq!(mouth_from_engine(""), MouthSlot::NanoBlocked);
        assert_eq!(mouth_from_engine("piper"), MouthSlot::PiperFallback);
        assert_eq!(mouth_from_engine("nano"), MouthSlot::Nano);
        assert_eq!(mouth_from_engine("chatterbox-nano"), MouthSlot::Nano);
        path.set_worker_engine("chatterbox-nano");
        assert_eq!(path.slot(), MouthSlot::Nano);
        assert!(path.plan().blocked.is_none());
        path.set_worker_engine("unwired");
        assert_eq!(path.slot(), MouthSlot::NanoBlocked);
        assert!(path.plan().blocked.unwrap().contains("BLOCKED"));
    }

    #[test]
    fn joy_does_not_raise_e_and_jarvis_is_the_drier_column() {
        let mut path = DuplexOutline::new();
        path.set_sense(true);
        let joy = path.on_heartbeat_frame(Some(&frame(0.8, 0.8, 0.5, 0.9)));
        assert_eq!(joy.e, 0.0);
        let said = path.on_asr_final("I got the job. I can't stop smiling.");
        assert_eq!(said.text, "model can't be reached");
        assert!(!v2_persona::is_canned_empathy(&said.text));
        assert!((said.warmth - 0.62).abs() < 1e-3);
        assert_eq!(said.oklch_l, NEUTRAL_OKLCH.l);
        path.on_asr_final("jarvis");
        path.on_heartbeat_frame(Some(&frame(-0.8, 0.9, 0.8, 1.0)));
        let jarvis = path.plan();
        assert_eq!(jarvis.persona, "jarvis");
        assert!((jarvis.e - 1.0).abs() < 1e-6);
        assert!((jarvis.warmth - 0.55).abs() < 1e-3);
        assert!(jarvis.warmth < said.warmth);
        assert!((jarvis.pitch_st - -0.4).abs() < 1e-3);
        assert_eq!(jarvis.pre_pause_ms, 120);
        assert!(jarvis.pre_pause_ms < 250);
    }

    #[test]
    fn a_canned_line_is_dropped() {
        assert!(v2_persona::is_canned_empathy("I hear you."));
        let mut path = DuplexOutline::new();
        let plan = path.on_asr_final("I hear you.");
        assert!(plan.text.is_empty() || !v2_persona::is_canned_empathy(&plan.text));
        assert!(!plan.text.to_ascii_lowercase().contains("i hear you"));
    }

    #[test]
    fn hub_mix_is_playback_and_off_copies_the_asset() {
        let asset = [0.10_f32, -0.20, 0.30];
        let layer = [0.01_f32, 0.50, -0.10];
        let before_a = asset;
        let before_l = layer;
        let off = mix_hub_playback(
            "jarvis",
            &asset,
            24_000,
            &layer,
            24_000,
            TextureToggle { enabled: false },
        )
        .unwrap();
        assert_eq!(off, asset);
        let on = mix_hub_playback(
            "jarvis",
            &asset,
            24_000,
            &layer,
            24_000,
            TextureToggle { enabled: true },
        )
        .unwrap();
        let summed: Vec<f32> = asset.iter().zip(layer).map(|(a, b)| a + b).collect();
        assert_eq!(on, summed);
        assert_eq!(asset, before_a);
        assert_eq!(layer, before_l);
        assert!(mix_hub_playback(
            "samantha",
            &asset,
            24_000,
            &layer,
            24_000,
            TextureToggle::default()
        )
        .is_err());
        assert!(mix_hub_playback(
            "jarvis",
            &asset,
            8_000,
            &layer,
            8_000,
            TextureToggle::default()
        )
        .is_err());
        assert!(mix_hub_playback(
            "jarvis",
            &asset,
            24_000,
            &[0.0],
            24_000,
            TextureToggle::default()
        )
        .is_err());

        let mut path = DuplexOutline::new();
        let samantha = path.playback(&asset, 24_000, &layer, 24_000).unwrap();
        assert_eq!(samantha, asset);
        path.on_asr_final("jarvis");
        path.set_hub(false);
        assert_eq!(
            path.playback(&asset, 24_000, &layer, 24_000).unwrap(),
            asset
        );
        path.set_hub(true);
        assert_eq!(
            path.playback(&asset, 24_000, &layer, 24_000).unwrap(),
            summed
        );
        let line = parse_toggle_line("{\"persona\":\"jarvis\",\"hub_texture\":false}").unwrap();
        assert!(!line.enabled);
        let missing = parse_toggle_line("{\"persona\":\"jarvis\"}").unwrap();
        assert!(missing.enabled);
        assert!(parse_toggle_line("{\"persona\":\"jarvis\",\"denoise\":true}").is_err());
        assert!(parse_toggle_line("{\"persona\":\"samantha\",\"hub_texture\":true}").is_err());
    }

    #[test]
    fn nano_worker_and_heartbeat_do_not_mix_the_hub() {
        let worker = include_str!("../../../scripts/clone/chatterbox_worker.py");
        assert!(!worker.contains(HUB_KEY));
        assert!(!worker.contains("mix_hub_playback"));
        let beat = include_str!("../../galion-heartbeat/src/lib.rs");
        assert!(!beat.contains("mix_hub_playback"));
        assert!(!beat.contains("DuplexOutline"));
        assert!(!beat.contains("EmotionClock"));
    }
}
