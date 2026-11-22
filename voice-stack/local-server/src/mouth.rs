// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Which mouth the 7881 server speaks with.
//!
//! Stock speech is Piper `en_US-ljspeech-medium`. The s711 clone is the
//! Chatterbox worker in `samantha-voice-clone`. It is selected only when
//! `GALION_SAMANTHA_REF` is a local wav, `GALION_SAMANTHA_WEIGHTS` is a local
//! checkpoint directory, and `GALION_SAMANTHA_DEVICE=cuda` with CUDA visible
//! to that crate's probe. `auto` stays on CPU, the same rule as the clone crate.
//!
//! CPU does not select s711. This process has no measured s711 CPU real-time
//! factor, and a Chatterbox `generate` on CPU is not treated as able to feed
//! the 80 ms speaker clock. The log line says that. It is not a new timing.
//! The 80 ms slice clock stays in the socket. This module returns PCM.

use std::sync::atomic::{AtomicBool, Ordering};

use galion_tts_piper::{resample_linear, PiperTts, Tts as PiperSpeak};
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

    pub(crate) fn from_turn(turn: &crate::reply::TurnOut) -> Self {
        Self {
            mood: turn.mood,
            rate: turn.prosody_rate,
            energy: turn.prosody_energy,
            pitch_st: turn.prosody_pitch_st,
            pause_ms: turn.prosody_pause_ms,
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

fn read_input() -> SelectInput {
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

pub(crate) fn open() -> Box<dyn Mouth> {
    let choice = decide(&read_input());
    match choice {
        Choice::Piper { clone_status, log } => {
            if let Some(line) = log {
                log_once(line);
            }
            open_piper(clone_status)
        }
        Choice::S711 { log } => {
            log_once(log);
            match samantha_voice_clone::SamanthaTts::open_from_env() {
                Ok(tts) => Box::new(S711Mouth { tts }),
                Err(err) => {
                    eprintln!(
                        "samantha-mouth: s711 did not open ({err}). Keeping Piper en_US-ljspeech-medium."
                    );
                    open_piper("open-failed")
                }
            }
        }
    }
}

/// First clause is handed to `on_pcm` before the next clause starts.
#[allow(clippy::type_complexity)]
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

fn open_piper(clone_status: &'static str) -> Box<dyn Mouth> {
    match PiperTts::open_resolved() {
        Ok(tts) => Box::new(PiperMouth { tts, clone_status }),
        Err(_) => Box::new(MissingMouth { clone_status }),
    }
}

struct MissingMouth {
    clone_status: &'static str,
}

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

struct PiperMouth {
    tts: PiperTts,
    clone_status: &'static str,
}

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
    let shaped = if pitch.abs() < 1.0e-3 && (energy - 1.0).abs() < 1.0e-3 && pause_ms == 0 {
        pcm.to_vec()
    } else {
        galion_tts_piper::shape_pcm(
            pcm,
            galion_tts_piper::Prosody {
                pitch_st: pitch,
                energy,
                pre_pause_ms: pause_ms,
            },
            sample_rate,
            true,
        )
    };
    let pause_n = if sample_rate == 0 {
        0
    } else {
        ((u64::from(pause_ms) * u64::from(sample_rate)) / 1000) as usize
    };
    let pause_n = pause_n.min(shaped.len());
    let mut out = shaped[..pause_n].to_vec();
    out.extend(stretch_rate(&shaped[pause_n..], rate));
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
}
