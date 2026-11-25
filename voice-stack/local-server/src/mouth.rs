// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Which mouth the 7881 server speaks with.
//!
//! The turn itself is [`crate::native`]. This module is only the mouth.
//! Boot default is CPU (`mouth_device = cpu`). The in-process tone is the
//! mouth when the Piper adapter is not linked. Piper `en_US-ljspeech-medium`
//! is the mouth only when this crate is built with `--features piper`.
//! The s711 clone is the Chatterbox worker in `samantha-voice-clone`. It is
//! selected only when `GALION_SAMANTHA_REF` is a local wav,
//! `GALION_SAMANTHA_WEIGHTS` is a local checkpoint directory, and
//! `GALION_SAMANTHA_DEVICE=cuda` with CUDA visible to that crate's probe.
//! `auto` stays on CPU, the same rule as the clone crate.
//!
//! CPU does not select s711. This process has no measured s711 CPU real-time
//! factor, and a Chatterbox `generate` on CPU is not treated as able to feed
//! the 80 ms speaker clock. The log line says that. It is not a new timing.
//! The 80 ms slice clock stays in the socket. This module returns PCM.
//!
//! `GALION_MOUTH=native` (also `horbm` or `text`) selects [`NativeMouth`].
//! That mouth does not spawn Piper. It speaks `horbm::TurnOut::first_clause`
//! and then `rest` as text on the socket, and it returns no PCM. Any other
//! value keeps today's mouth, so a build with `--features piper` still speaks
//! through `PiperMouth`.

use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "piper")]
use galion_tts_piper::{PiperTts, Tts as PiperSpeak};
use samantha_voice_clone::Tts as CloneSpeak;

use crate::audio::SPEAKER_RATE;
use crate::reply::Pad;

/// Empathy knobs for one spoken line. They are the turn's prosody, not a script.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MouthCue {
    pub mood: Pad,
    pub rate: f32,
    pub energy: f32,
    pub pitch_st: f32,
    pub pause_ms: f32,
}

impl MouthCue {
    #[cfg(test)]
    pub(crate) fn resting() -> Self {
        Self {
            mood: Pad::RESTING,
            rate: 1.0,
            energy: 1.0,
            pitch_st: 0.0,
            pause_ms: 0.0,
        }
    }

    pub(crate) fn from_spoken(line: &horbm_voice::SpokenLine) -> Self {
        Self {
            mood: crate::native::cue_pad(line.mood),
            rate: line.rate,
            energy: line.energy,
            pitch_st: line.pitch_st,
            pause_ms: line.pause_ms,
        }
    }
}

pub(crate) trait Mouth: Send {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String>;
    /// Hand each ready clause to `on_pcm` before the next clause is synthesised.
    /// Return `false` from `on_pcm` to stop. `cancel` is polled between clauses.
    fn speak_stream(
        &mut self,
        text: &str,
        cue: &MouthCue,
        cancel: &AtomicBool,
        on_pcm: &mut dyn FnMut(&[i16]) -> bool,
    ) -> Result<(), String> {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let (pcm, _) = self.speak(text, cue)?;
        if cancel.load(Ordering::SeqCst) || pcm.is_empty() {
            return Ok(());
        }
        let _ = on_pcm(&pcm);
        Ok(())
    }
    /// `false` when this mouth speaks text and returns no samples.
    fn emits_audio(&self) -> bool {
        true
    }
    /// Pieces from `horbm::TurnOut`, when the caller has them.
    fn set_clauses(&mut self, _first_clause: &str, _rest: &str) {}
    /// Text queued by the last [`Self::speak`]. Empty for an audio mouth.
    fn drain_lines(&mut self) -> Vec<String> {
        Vec::new()
    }
    fn mouth(&self) -> &'static str;
    fn voice(&self) -> &'static str;
    fn clone_status(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceRequest {
    Cpu,
    Cuda,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectInput {
    pub ref_set: bool,
    pub ref_is_file: bool,
    pub weights_is_dir: bool,
    pub device: DeviceRequest,
    pub cuda_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Piper {
        clone_status: &'static str,
        log: Option<&'static str>,
    },
    S711 {
        log: &'static str,
    },
}

pub(crate) const CPU_NOT_REALTIME_LOG: &str = "samantha-mouth: keeping Piper en_US-ljspeech-medium. s711 on CPU is not the live mouth: this process has no measured s711 CPU real-time factor under 1, and a Chatterbox generate on CPU is not treated as able to feed the 80 ms clock. Not a new timing.";

pub(crate) const S711_SELECTED_LOG: &str = "samantha-mouth: voice s711 selected (GALION_SAMANTHA_DEVICE=cuda, local reference, local checkpoint). Realtime on this host is not measured in this process.";

const REF_MISSING_LOG: &str = "samantha-mouth: GALION_SAMANTHA_REF is set but the file is not there. Keeping Piper en_US-ljspeech-medium.";

const WEIGHTS_MISSING_LOG: &str = "samantha-mouth: GALION_SAMANTHA_REF is a file but GALION_SAMANTHA_WEIGHTS is not a local checkpoint directory. Keeping Piper en_US-ljspeech-medium.";

const CUDA_UNAVAILABLE_LOG: &str = "samantha-mouth: GALION_SAMANTHA_DEVICE=cuda but the probe does not see CUDA. Keeping Piper en_US-ljspeech-medium.";

const DEVICE_UNKNOWN_LOG: &str = "samantha-mouth: GALION_SAMANTHA_DEVICE is not cpu, auto, or cuda. Keeping Piper en_US-ljspeech-medium.";

pub(crate) fn parse_device(raw: &str) -> DeviceRequest {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "auto" | "cpu" => DeviceRequest::Cpu,
        "cuda" => DeviceRequest::Cuda,
        _ => DeviceRequest::Unknown,
    }
}

/// Pure selection. Does not open a file, a model, or a process.
pub(crate) fn decide(input: &SelectInput) -> Choice {
    if !input.ref_set {
        return Choice::Piper {
            clone_status: "off",
            log: None,
        };
    }
    if !input.ref_is_file {
        return Choice::Piper {
            clone_status: "ref-missing",
            log: Some(REF_MISSING_LOG),
        };
    }
    if !input.weights_is_dir {
        return Choice::Piper {
            clone_status: "weights-missing",
            log: Some(WEIGHTS_MISSING_LOG),
        };
    }
    match input.device {
        DeviceRequest::Cpu => Choice::Piper {
            clone_status: "cpu-not-realtime",
            log: Some(CPU_NOT_REALTIME_LOG),
        },
        DeviceRequest::Unknown => Choice::Piper {
            clone_status: "device-unknown",
            log: Some(DEVICE_UNKNOWN_LOG),
        },
        DeviceRequest::Cuda if !input.cuda_visible => Choice::Piper {
            clone_status: "cuda-unavailable",
            log: Some(CUDA_UNAVAILABLE_LOG),
        },
        DeviceRequest::Cuda => Choice::S711 {
            log: S711_SELECTED_LOG,
        },
    }
}

pub(crate) fn read_input() -> SelectInput {
    let raw_ref = std::env::var("GALION_SAMANTHA_REF").unwrap_or_default();
    let ref_trimmed = raw_ref.trim();
    let ref_set = !ref_trimmed.is_empty();
    let ref_is_file = ref_set && std::path::Path::new(ref_trimmed).is_file();
    let raw_weights = std::env::var("GALION_SAMANTHA_WEIGHTS").unwrap_or_default();
    let weights = raw_weights.trim();
    let weights_is_dir = !weights.is_empty() && std::path::Path::new(weights).is_dir();
    let device = parse_device(&std::env::var("GALION_SAMANTHA_DEVICE").unwrap_or_default());
    let cuda_visible = if device == DeviceRequest::Cuda {
        samantha_voice_clone::probe_compute().kind == samantha_voice_clone::ComputeKind::Cuda
    } else {
        false
    };
    SelectInput {
        ref_set,
        ref_is_file,
        weights_is_dir,
        device,
        cuda_visible,
    }
}

fn log_once(line: &str) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| eprintln!("{line}"));
}

/// Device the selected adapter runs on. CPU unless the caller opted into cuda.
pub(crate) fn mouth_device(choice: &Choice) -> &'static str {
    match choice {
        Choice::S711 { .. } => "cuda",
        Choice::Piper { .. } => crate::native::BOOT_MOUTH_DEVICE,
    }
}

/// Name of the mouth this build will construct. Does not open a process.
#[cfg(test)]
pub(crate) fn linked_name(choice: &Choice) -> &'static str {
    match choice {
        Choice::S711 { .. } => "s711",
        Choice::Piper { .. } => {
            #[cfg(feature = "piper")]
            {
                "piper"
            }
            #[cfg(not(feature = "piper"))]
            {
                "fake"
            }
        }
    }
}

pub(crate) fn open_selected(choice: &Choice) -> Box<dyn Mouth> {
    match *choice {
        Choice::Piper { clone_status, log } => open_cpu(clone_status, log),
        Choice::S711 { log } => {
            log_once(log);
            match samantha_voice_clone::SamanthaTts::open_from_env() {
                Ok(tts) => Box::new(S711Mouth { tts }),
                Err(err) => {
                    eprintln!("samantha-mouth: s711 did not open ({err}). Keeping the CPU mouth.");
                    open_cpu("open-failed", None)
                }
            }
        }
    }
}

/// Which mouth `GALION_MOUTH` names. `Today` is the existing Piper, tone, or s711 choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouthPick {
    Native,
    Today,
}

pub(crate) const NATIVE_MOUTH_LOG: &str = "samantha-mouth: native mouth selected (GALION_MOUTH). The turn text goes on the socket, first clause then the rest. No PCM until an MIT or Apache-2.0 speaker is linked. Piper is not started. Not a new timing.";

pub(crate) const UNKNOWN_MOUTH_LOG: &str =
    "samantha-mouth: GALION_MOUTH is not a known selector. Keeping today's mouth.";

/// `native`, `horbm`, and `text` select the socket mouth. A known alias of
/// today's mouth, or an empty value, keeps [`decide`]. Anything else falls
/// back to today's mouth as well.
pub(crate) fn parse_mouth_env(raw: &str) -> (MouthPick, bool) {
    match raw.trim().to_ascii_lowercase().as_str() {
        "native" | "horbm" | "text" => (MouthPick::Native, true),
        "" | "piper" | "cpu" | "auto" | "today" | "tone" | "s711" | "fake" => {
            (MouthPick::Today, true)
        }
        _ => (MouthPick::Today, false),
    }
}

/// Name a pure selection would build. Does not open a process.
#[cfg(test)]
pub(crate) fn resolved_mouth(pick: MouthPick, choice: &Choice) -> &'static str {
    match pick {
        MouthPick::Native => "native",
        MouthPick::Today => match choice {
            Choice::S711 { .. } => "s711",
            Choice::Piper { .. } => {
                #[cfg(feature = "piper")]
                {
                    "piper"
                }
                #[cfg(not(feature = "piper"))]
                {
                    "fake"
                }
            }
        },
    }
}

pub(crate) fn open_kind(pick: MouthPick, choice: &Choice, raw_env: &str) -> Box<dyn Mouth> {
    match pick {
        MouthPick::Native => {
            log_native();
            Box::new(NativeMouth::new())
        }
        MouthPick::Today => {
            if !parse_mouth_env(raw_env).1 {
                log_unknown();
            }
            open_selected(choice)
        }
    }
}

fn log_native() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| eprintln!("{NATIVE_MOUTH_LOG}"));
}

fn log_unknown() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| eprintln!("{UNKNOWN_MOUTH_LOG}"));
}

/// Same cut `horbm::Brain::turn` stores on `TurnOut.first_clause` and `rest`
/// for a card clause and for the miss. The miss stays one piece. A longer
/// clause yields two words, then the remainder. Locked against `horbm::TurnOut`
/// in the tests below.
pub(crate) fn clause_pair(reply: &str) -> (String, String) {
    let text = reply.trim();
    let atomic = text.is_empty() || text == horbm::UNKNOWN_REPLY;
    split_clause(text, atomic)
}

/// Spoken order from the fields `horbm` already computed. Does not split again.
#[cfg(test)]
pub(crate) fn pieces_from_turn(turn: &horbm::TurnOut) -> Vec<String> {
    pieces_from_fields(&turn.first_clause, &turn.rest)
}

fn pieces_from_fields(first_clause: &str, rest: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if !first_clause.is_empty() {
        lines.push(first_clause.to_string());
    }
    if !rest.is_empty() {
        lines.push(rest.to_string());
    }
    lines
}

fn split_clause(text: &str, atomic: bool) -> (String, String) {
    if text.is_empty() {
        return (String::new(), String::new());
    }
    if atomic || text.split_whitespace().count() <= 2 {
        return (text.to_string(), String::new());
    }
    let mut index = 0usize;
    let mut rest_at = None;
    for (seen, word) in text.split_whitespace().enumerate() {
        let pos = text[index..]
            .find(word)
            .map(|off| index + off)
            .unwrap_or(index);
        if seen == 2 {
            rest_at = Some(pos);
            break;
        }
        index = pos + word.len();
    }
    match rest_at {
        Some(at) => (text[..at].trim_end().to_string(), text[at..].to_string()),
        None => (text.to_string(), String::new()),
    }
}

/// HORBM mouth. Text on the socket. No Piper process and no samples.
pub(crate) struct NativeMouth {
    lead: String,
    rest: String,
    clauses_set: bool,
    lines: Vec<String>,
}

impl NativeMouth {
    pub(crate) fn new() -> Self {
        Self {
            lead: String::new(),
            rest: String::new(),
            clauses_set: false,
            lines: Vec::new(),
        }
    }
}

impl Mouth for NativeMouth {
    fn speak(&mut self, text: &str, _cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        self.lines = if self.clauses_set {
            pieces_from_fields(&self.lead, &self.rest)
        } else if text.trim().is_empty() {
            Vec::new()
        } else {
            vec![text.trim().to_string()]
        };
        Ok((Vec::new(), SPEAKER_RATE))
    }

    fn emits_audio(&self) -> bool {
        false
    }

    fn set_clauses(&mut self, first_clause: &str, rest: &str) {
        self.clauses_set = !first_clause.is_empty() || !rest.is_empty();
        self.lead = first_clause.to_string();
        self.rest = rest.to_string();
    }

    fn drain_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    fn mouth(&self) -> &'static str {
        "native"
    }

    fn voice(&self) -> &'static str {
        "text"
    }

    fn clone_status(&self) -> &'static str {
        "off"
    }
}

fn open_cpu(clone_status: &'static str, log: Option<&'static str>) -> Box<dyn Mouth> {
    #[cfg(feature = "piper")]
    {
        if let Some(line) = log {
            log_once(line);
        }
        open_piper(clone_status)
    }
    #[cfg(not(feature = "piper"))]
    {
        let _ = log;
        log_once(
            "samantha-mouth: Piper is not linked in this build. The CPU mouth is the in-process tone. Not a new timing.",
        );
        Box::new(ToneMouth { clone_status })
    }
}

/// First clause is handed to `on_pcm` before the next clause starts.
#[allow(clippy::type_complexity)]
#[cfg(any(test, feature = "piper"))]
pub(crate) fn speak_pieces(
    text: &str,
    cancel: &AtomicBool,
    one: &mut dyn FnMut(&str, &mut dyn FnMut(&[i16]) -> bool) -> Result<(), String>,
    on_pcm: &mut dyn FnMut(&[i16]) -> bool,
) -> Result<(), String> {
    let pieces = galion_sam_horbm::speakable_chunks(text);
    let pieces: Vec<String> = if pieces.is_empty() {
        vec![text.to_string()]
    } else {
        pieces.into_iter().map(|chunk| chunk.text).collect()
    };
    for piece in pieces {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut stop = false;
        one(&piece, &mut |pcm| {
            if stop || cancel.load(Ordering::SeqCst) {
                stop = true;
                return false;
            }
            if !on_pcm(pcm) {
                stop = true;
                return false;
            }
            true
        })?;
        if stop {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(feature = "piper")]
fn open_piper(clone_status: &'static str) -> Box<dyn Mouth> {
    match PiperTts::open_resolved() {
        Ok(tts) => Box::new(PiperMouth { tts, clone_status }),
        Err(_) => Box::new(MissingMouth { clone_status }),
    }
}

/// CPU mouth when the Piper adapter is not in this build. A tone, not a voice.
#[cfg(not(feature = "piper"))]
struct ToneMouth {
    clone_status: &'static str,
}

#[cfg(not(feature = "piper"))]
impl Mouth for ToneMouth {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        if text.trim().is_empty() {
            return Ok((Vec::new(), SPEAKER_RATE));
        }
        let tone = crate::audio::hold_tone(cue.mood.valence);
        Ok((
            apply_empathy_prosody(&tone, SPEAKER_RATE, cue),
            SPEAKER_RATE,
        ))
    }

    fn mouth(&self) -> &'static str {
        "fake"
    }

    fn voice(&self) -> &'static str {
        "none"
    }

    fn clone_status(&self) -> &'static str {
        self.clone_status
    }
}

#[cfg(feature = "piper")]
struct MissingMouth {
    clone_status: &'static str,
}

#[cfg(feature = "piper")]
impl Mouth for MissingMouth {
    fn speak(&mut self, _: &str, _: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        Err("piper-missing".into())
    }
    fn mouth(&self) -> &'static str {
        "missing"
    }
    fn voice(&self) -> &'static str {
        "none"
    }
    fn clone_status(&self) -> &'static str {
        self.clone_status
    }
}

#[cfg(feature = "piper")]
struct PiperMouth {
    tts: PiperTts,
    clone_status: &'static str,
}

#[cfg(feature = "piper")]
impl Mouth for PiperMouth {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        let mut pcm = Vec::new();
        let cancel = AtomicBool::new(false);
        self.speak_stream(text, cue, &cancel, &mut |chunk| {
            pcm.extend_from_slice(chunk);
            true
        })?;
        if pcm.is_empty() {
            return Err("piper-missing".into());
        }
        Ok((pcm, SPEAKER_RATE))
    }

    fn speak_stream(
        &mut self,
        text: &str,
        cue: &MouthCue,
        cancel: &AtomicBool,
        on_pcm: &mut dyn FnMut(&[i16]) -> bool,
    ) -> Result<(), String> {
        let mut any = false;
        let mut first = true;
        speak_pieces(
            text,
            cancel,
            &mut |piece, emit| {
                if cancel.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let mut part = Vec::new();
                let mut rate = SPEAKER_RATE;
                let mut stop = false;
                // Resting pad keeps Piper's own shaper as a copy. Empathy is applied
                // once per clause below, so pitch and gain are not applied twice.
                // The leading pause stays on the first clause.
                self.tts
                    .speak(piece, galion_tts_piper::Pad::RESTING, &mut |chunk| {
                        if stop || cancel.load(Ordering::SeqCst) {
                            stop = true;
                            return;
                        }
                        rate = chunk.sample_rate;
                        part.extend_from_slice(chunk.samples);
                    })
                    .map_err(|_| "piper-missing".to_string())?;
                if stop || cancel.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let pcm = resample_linear(&part, rate, SPEAKER_RATE);
                let shaped = if first {
                    first = false;
                    apply_empathy_prosody(&pcm, SPEAKER_RATE, cue)
                } else {
                    let mut later = *cue;
                    later.pause_ms = 0.0;
                    apply_empathy_prosody(&pcm, SPEAKER_RATE, &later)
                };
                if shaped.is_empty() {
                    return Ok(());
                }
                any = true;
                if !emit(&shaped) {
                    return Ok(());
                }
                Ok(())
            },
            on_pcm,
        )?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        if !any {
            return Err("piper-missing".into());
        }
        Ok(())
    }
    fn mouth(&self) -> &'static str {
        "piper"
    }
    fn voice(&self) -> &'static str {
        "ljspeech"
    }
    fn clone_status(&self) -> &'static str {
        self.clone_status
    }
}

struct S711Mouth {
    tts: samantha_voice_clone::SamanthaTts<samantha_voice_clone::WorkerSpeaker>,
}

impl Mouth for S711Mouth {
    fn speak(&mut self, text: &str, cue: &MouthCue) -> Result<(Vec<i16>, u32), String> {
        render_s711(&mut self.tts, text, cue)
    }
    fn mouth(&self) -> &'static str {
        "s711"
    }
    fn voice(&self) -> &'static str {
        "s711"
    }
    fn clone_status(&self) -> &'static str {
        "selected"
    }
}

fn render_s711<S: samantha_voice_clone::SentenceSpeaker>(
    voice: &mut samantha_voice_clone::SamanthaTts<S>,
    text: &str,
    cue: &MouthCue,
) -> Result<(Vec<i16>, u32), String> {
    let mood = samantha_voice_clone::Pad {
        arousal: cue.mood.arousal,
        valence: cue.mood.valence,
        dominance: cue.mood.dominance,
    };
    let speech =
        CloneSpeak::synthesize(voice, text, &mood).map_err(|err| format!("s711-{}", err.kind))?;
    let pcm = if speech.sample_rate_hz == SPEAKER_RATE {
        speech.samples
    } else {
        resample_linear(&speech.samples, speech.sample_rate_hz, SPEAKER_RATE)
    };
    if pcm.is_empty() {
        return Err("s711-empty".into());
    }
    Ok((apply_empathy_prosody(&pcm, SPEAKER_RATE, cue), SPEAKER_RATE))
}

/// Pitch, gain, leading pause, then speaking rate. A resting cue is a copy.
/// This is a sample-domain map. It is not a measured voice.
pub(crate) fn apply_empathy_prosody(pcm: &[i16], sample_rate: u32, cue: &MouthCue) -> Vec<i16> {
    let pitch = if cue.pitch_st.is_finite() {
        cue.pitch_st
    } else {
        0.0
    };
    let energy = if cue.energy.is_finite() && cue.energy > 0.0 {
        cue.energy
    } else {
        1.0
    };
    let pause_ms = if cue.pause_ms.is_finite() {
        cue.pause_ms.clamp(0.0, 400.0).round() as u32
    } else {
        0
    };
    let rate = if cue.rate.is_finite() && cue.rate > 0.0 {
        cue.rate.clamp(0.25, 4.0)
    } else {
        1.0
    };
    let body = if pitch.abs() < 1.0e-3 && (energy - 1.0).abs() < 1.0e-3 {
        pcm.to_vec()
    } else {
        pitch_gain(pcm, pitch, energy)
    };
    let pause_n = if sample_rate == 0 {
        0
    } else {
        ((u64::from(pause_ms) * u64::from(sample_rate)) / 1000) as usize
    };
    let mut out = vec![0i16; pause_n];
    out.extend(stretch_rate(&body, rate));
    out
}

fn pitch_gain(input: &[i16], pitch_st: f32, energy: f32) -> Vec<i16> {
    let ratio = (pitch_st.clamp(-6.0, 6.0) / 12.0).exp2();
    if input.is_empty() {
        return Vec::new();
    }
    if (ratio - 1.0).abs() < 1.0e-6 {
        return input.iter().map(|sample| knee(*sample, energy)).collect();
    }
    let mut out = Vec::new();
    let mut pos = 0.0f64;
    let last = input.len() - 1;
    while pos < input.len() as f64 {
        let index = pos.floor() as usize;
        let frac = (pos - index as f64) as f32;
        let a = f32::from(input[index.min(last)]);
        let b = f32::from(input[(index + 1).min(last)]);
        out.push(knee_f(a + (b - a) * frac, energy));
        pos += f64::from(ratio);
    }
    out
}

fn knee(sample: i16, gain: f32) -> i16 {
    knee_f(f32::from(sample), gain)
}

fn knee_f(sample: f32, gain: f32) -> i16 {
    let y = sample * gain.clamp(0.25, 2.0) / 32768.0;
    let a = y.abs();
    let shaped = if a <= 0.8 {
        y
    } else {
        let t = ((a - 0.8) / 0.6).min(1.0);
        (0.8 + 0.18 * (1.5 * t - 0.5 * t * t * t)) * y.signum()
    };
    (shaped * 32767.0).round().clamp(-32768.0, 32767.0) as i16
}

fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() || from == to {
        return input.to_vec();
    }
    let out_len =
        ((input.len() as u64 * u64::from(to) + u64::from(from) / 2) / u64::from(from)) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let num = i as u64 * u64::from(from);
        let idx = (num / u64::from(to)) as usize;
        let frac = (num % u64::from(to)) as f64 / f64::from(to);
        let a = f64::from(input[idx.min(last)]);
        let b = f64::from(input[(idx + 1).min(last)]);
        let value = a + (b - a) * frac;
        out.push(
            value
                .round()
                .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16,
        );
    }
    out
}

fn stretch_rate(input: &[i16], rate: f32) -> Vec<i16> {
    if input.is_empty() || (rate - 1.0).abs() < 1.0e-3 {
        return input.to_vec();
    }
    let out_len = ((input.len() as f32) / rate).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() - 1;
    for i in 0..out_len {
        let pos = (i as f32) * rate;
        let i0 = (pos.floor() as usize).min(last);
        let i1 = (i0 + 1).min(last);
        let frac = pos - i0 as f32;
        let sample = f32::from(input[i0]) * (1.0 - frac) + f32::from(input[i1]) * frac;
        out.push(sample.round().clamp(-32768.0, 32767.0) as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use samantha_voice_clone::{
        Reference, SamanthaTts, SentenceJob, SentenceSpeaker, Spoken, TtsError, REF_SHA_PREFIX,
    };
    use std::path::PathBuf;

    struct Click {
        jobs: Vec<SentenceJob>,
    }

    impl SentenceSpeaker for Click {
        fn speak(&mut self, job: &SentenceJob) -> Result<Spoken, TtsError> {
            self.jobs.push(job.clone());
            Ok(Spoken {
                samples: vec![711, 0, 0, 0],
                device: "synthetic".into(),
                worker_ttfa_us: None,
                from_cache: false,
            })
        }
    }

    fn files(device: DeviceRequest, cuda: bool) -> SelectInput {
        SelectInput {
            ref_set: true,
            ref_is_file: true,
            weights_is_dir: true,
            device,
            cuda_visible: cuda,
        }
    }

    #[test]
    fn cpu_boot_does_not_open_a_process_to_name_the_mouth() {
        let choice = decide(&SelectInput {
            ref_set: false,
            ref_is_file: false,
            weights_is_dir: false,
            device: DeviceRequest::Cpu,
            cuda_visible: false,
        });
        assert_eq!(mouth_device(&choice), "cpu");
        #[cfg(not(feature = "piper"))]
        assert_eq!(linked_name(&choice), "fake");
        #[cfg(feature = "piper")]
        assert_eq!(linked_name(&choice), "piper");
        assert_eq!(
            linked_name(&decide(&files(DeviceRequest::Cuda, true))),
            "s711"
        );
    }

    #[test]
    fn unset_ref_stays_on_ljspeech_with_no_clone_log() {
        let choice = decide(&SelectInput {
            ref_set: false,
            ref_is_file: false,
            weights_is_dir: false,
            device: DeviceRequest::Cpu,
            cuda_visible: false,
        });
        assert_eq!(
            choice,
            Choice::Piper {
                clone_status: "off",
                log: None,
            }
        );
    }

    #[test]
    fn cpu_with_a_local_clone_keeps_piper_and_says_why() {
        let choice = decide(&files(DeviceRequest::Cpu, true));
        match choice {
            Choice::Piper { clone_status, log } => {
                assert_eq!(clone_status, "cpu-not-realtime");
                let line = log.expect("log");
                assert!(line.contains("keeping Piper"));
                assert!(line.contains("Not a new timing"));
                assert!(line.contains("no measured s711"));
                assert!(!line.contains("\"voice\":\"s711\""));
            }
            Choice::S711 { .. } => panic!("cpu must not select s711"),
        }
        assert!(matches!(
            decide(&files(DeviceRequest::Cpu, false)),
            Choice::Piper {
                clone_status: "cpu-not-realtime",
                ..
            }
        ));
    }

    #[test]
    fn missing_file_or_weights_or_cuda_stay_on_piper() {
        let missing_file = decide(&SelectInput {
            ref_set: true,
            ref_is_file: false,
            weights_is_dir: true,
            device: DeviceRequest::Cuda,
            cuda_visible: true,
        });
        assert!(matches!(
            missing_file,
            Choice::Piper {
                clone_status: "ref-missing",
                ..
            }
        ));
        let missing_weights = decide(&SelectInput {
            ref_set: true,
            ref_is_file: true,
            weights_is_dir: false,
            device: DeviceRequest::Cuda,
            cuda_visible: true,
        });
        assert!(matches!(
            missing_weights,
            Choice::Piper {
                clone_status: "weights-missing",
                ..
            }
        ));
        let no_gpu = decide(&files(DeviceRequest::Cuda, false));
        assert!(matches!(
            no_gpu,
            Choice::Piper {
                clone_status: "cuda-unavailable",
                ..
            }
        ));
        assert_eq!(parse_device("auto"), DeviceRequest::Cpu);
        assert_eq!(parse_device("CUDA"), DeviceRequest::Cuda);
        assert_eq!(parse_device("gpu"), DeviceRequest::Unknown);
    }

    #[test]
    fn cuda_with_local_files_selects_s711_without_a_realtime_claim() {
        match decide(&files(DeviceRequest::Cuda, true)) {
            Choice::S711 { log } => {
                assert!(log.contains("voice s711"));
                assert!(log.contains("not measured"));
            }
            Choice::Piper { .. } => panic!("cuda with files should select s711"),
        }
    }

    #[test]
    fn synthetic_pcm_follows_the_empathy_cue_and_is_not_a_voice() {
        let raw = vec![1000i16, -1000, 500, -500];
        let rest = MouthCue::resting();
        assert_eq!(
            apply_empathy_prosody(&raw, 24_000, &rest),
            raw,
            "resting cue must be a copy; this is not a listening test"
        );
        let loud = MouthCue {
            energy: 1.4,
            ..rest
        };
        let shaped = apply_empathy_prosody(&raw, 24_000, &loud);
        assert!(shaped[0] > raw[0]);
        let paused = MouthCue {
            pause_ms: 80.0,
            ..rest
        };
        let with_pause = apply_empathy_prosody(&raw, 24_000, &paused);
        assert_eq!(with_pause.len(), raw.len() + 1920);
        assert!(with_pause[..1920].iter().all(|sample| *sample == 0));
        let faster = MouthCue { rate: 2.0, ..rest };
        let short = apply_empathy_prosody(&raw, 24_000, &faster);
        assert!(short.len() < raw.len());
    }

    #[test]
    fn s711_stand_in_keeps_the_mood_and_the_gain_on_synthetic_samples() {
        let reference = Reference::trusted_for_tests(
            PathBuf::from("/tmp/galion-synthetic-mouth-ref.wav"),
            format!("{REF_SHA_PREFIX}{}", "ab".repeat(28)),
        )
        .unwrap();
        let mut voice =
            SamanthaTts::with_speaker(reference, Click { jobs: Vec::new() }, None, "cpu");
        let cue = MouthCue {
            mood: Pad {
                arousal: 0.8,
                valence: 0.2,
                dominance: 0.4,
            },
            energy: 1.5,
            ..MouthCue::resting()
        };
        let (loud, rate) = render_s711(&mut voice, "Hello.", &cue).unwrap();
        assert_eq!(rate, 24_000);
        let (plain, _) = render_s711(&mut voice, "Hello.", &MouthCue::resting()).unwrap();
        assert_ne!(
            plain[0], 0,
            "stand-in is a constant click, not a recorded voice"
        );
        assert!(loud[0].abs() > plain[0].abs());
        let job = &voice.speaker().jobs[0];
        assert!((job.mood.arousal - 0.8).abs() < 1e-6);
        assert!((job.mood.valence - 0.2).abs() < 1e-6);
        assert_eq!(job.seed, 711);
    }

    #[test]
    fn start_script_sets_the_ref_only_when_the_placeholder_exists() {
        let cmd = include_str!("../../../scripts/windows/START-SAMANTHA-7881.cmd");
        assert!(cmd.contains("set \"SAMANTHA_REF=G:\\galion\\samantha-ref.wav\""));
        assert!(cmd.contains("set \"SAMANTHA_WEIGHTS=G:\\galion\\chatterbox-expr\""));
        let exist = cmd
            .find("if exist \"%SAMANTHA_REF%\"")
            .expect("ref exist gate");
        let set_ref = cmd
            .find("set \"GALION_SAMANTHA_REF=%SAMANTHA_REF%\"")
            .expect("ref assignment");
        let else_at = cmd[exist..]
            .find(") else (")
            .map(|n| exist + n)
            .expect("else");
        assert!(exist < set_ref && set_ref < else_at);
        let weights = cmd
            .find("if exist \"%SAMANTHA_WEIGHTS%\\*\"")
            .expect("weights gate");
        let set_weights = cmd
            .find("set \"GALION_SAMANTHA_WEIGHTS=%SAMANTHA_WEIGHTS%\"")
            .expect("weights assignment");
        assert!(weights < set_weights);
        assert!(!cmd.contains("RIFF"));
    }

    struct KettleCard;

    impl horbm::CardLookup for KettleCard {
        fn len(&self) -> usize {
            1
        }

        fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
            if query.to_ascii_lowercase().contains("kettle") {
                Some(horbm::CardHit {
                    id: "kettle".into(),
                    gist: "The kettle is on the stove.".into(),
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn mouth_env_selects_native_and_an_unknown_value_falls_back() {
        let today = SelectInput {
            ref_set: false,
            ref_is_file: false,
            weights_is_dir: false,
            device: DeviceRequest::Cpu,
            cuda_visible: false,
        };
        let (native, known) = parse_mouth_env("native");
        assert!(known);
        assert_eq!(native, MouthPick::Native);
        assert_eq!(parse_mouth_env("HORBM").0, MouthPick::Native);
        assert_eq!(parse_mouth_env("text").0, MouthPick::Native);
        assert_eq!(resolved_mouth(MouthPick::Native, &decide(&today)), "native");
        let (unset, unset_known) = parse_mouth_env("");
        assert!(unset_known);
        assert_eq!(unset, MouthPick::Today);
        assert_eq!(parse_mouth_env("piper").0, MouthPick::Today);
        let (unknown, unknown_known) = parse_mouth_env("flute");
        assert!(!unknown_known);
        assert_eq!(unknown, MouthPick::Today);
        assert!(UNKNOWN_MOUTH_LOG.contains("Keeping today's mouth"));
        let cuda = files(DeviceRequest::Cuda, true);
        assert_eq!(
            resolved_mouth(MouthPick::Today, &decide(&cuda)),
            "s711",
            "an unknown selector falls back through the existing choice"
        );
        assert_eq!(resolved_mouth(MouthPick::Native, &decide(&cuda)), "native");
        #[cfg(feature = "piper")]
        assert_eq!(resolved_mouth(MouthPick::Today, &decide(&today)), "piper");
        #[cfg(not(feature = "piper"))]
        assert_eq!(resolved_mouth(MouthPick::Today, &decide(&today)), "fake");
    }

    #[test]
    fn native_mouth_speaks_horbm_first_clause_then_the_rest_and_returns_no_pcm() {
        let mut brain = horbm::Brain::with_cards(KettleCard, None);
        let hit = brain.turn("where is the kettle");
        assert_eq!(hit.reply, "The kettle is on the stove.");
        assert_eq!(
            clause_pair(&hit.reply),
            (hit.first_clause.clone(), hit.rest.clone())
        );
        let pieces = pieces_from_turn(&hit);
        assert_eq!(pieces[0], hit.first_clause);
        assert_eq!(pieces[1], hit.rest);
        assert_eq!(format!("{} {}", hit.first_clause, hit.rest), hit.reply);

        let mut mouth = NativeMouth::new();
        mouth.set_clauses(&hit.first_clause, &hit.rest);
        let (pcm, rate) = mouth
            .speak("not the clause order", &MouthCue::resting())
            .unwrap();
        assert!(pcm.is_empty(), "native mouth returns no PCM");
        assert_eq!(rate, SPEAKER_RATE);
        assert!(!mouth.emits_audio());
        assert_eq!(mouth.mouth(), "native");
        assert_eq!(mouth.voice(), "text");
        assert_eq!(mouth.drain_lines(), pieces);
        assert!(NATIVE_MOUTH_LOG.contains("No PCM"));
        assert!(NATIVE_MOUTH_LOG.contains("Piper is not started"));

        let miss = brain.turn("who won the match");
        assert_eq!(miss.reply, horbm::UNKNOWN_REPLY);
        assert_eq!(miss.first_clause, miss.reply);
        assert!(miss.rest.is_empty());
        assert_eq!(clause_pair(&miss.reply).0, miss.first_clause);
        mouth.set_clauses(&miss.first_clause, &miss.rest);
        let (pcm, _) = mouth.speak(&miss.reply, &MouthCue::resting()).unwrap();
        assert!(pcm.is_empty());
        assert_eq!(mouth.drain_lines(), vec![miss.first_clause.clone()]);
    }

    #[test]
    fn native_mouth_without_clause_fields_speaks_the_whole_line() {
        let mut mouth = NativeMouth::new();
        let (pcm, _) = mouth
            .speak("The fence is loose.", &MouthCue::resting())
            .unwrap();
        assert!(pcm.is_empty());
        assert_eq!(mouth.drain_lines(), vec!["The fence is loose.".to_string()]);
        mouth.set_clauses("", "");
        let (pcm, _) = mouth.speak("", &MouthCue::resting()).unwrap();
        assert!(pcm.is_empty());
        assert!(mouth.drain_lines().is_empty());
    }
}
