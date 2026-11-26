// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! TTS -> resample -> ASR -> WER.
//!
//! Piper speaks into memory. ffmpeg resamples to 16 kHz s16le the way
//! `wer_t648.py` does. Vosk transcribes. WER uses that script's tokeniser.
//! ECAPA runs only when `GALION_VOICE_REF` (or the deprecated
//! `GALION_SAMANTHA_REF` fallback) is a file and `galion-ecapa`
//! is on disk. The comparison wavs live in the temp directory and are
//! deleted before this function returns. Emotion stays unmeasured: the CPU
//! stub is not a score. Nothing is committed.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::gate::{self, Check, Rollout, Status};
use crate::locate::{self, Tools};
use crate::piper::Session;
use crate::vosk::{Transcript, Vosk};
use crate::wer;

pub const SENTENCES: &[&str] = &[
    "Hi. I'm here.",
    "I have moved the meeting to Thursday at four, and the notes are in your inbox.",
    "The left side sticks on the latch.",
];

#[derive(Clone, Debug)]
pub struct Utterance {
    pub reference: String,
    pub hypothesis: String,
    pub wer: Option<f64>,
    pub pcm_bytes: usize,
    pub audio_s: Option<f64>,
    pub first_audio_ms: Option<f64>,
    pub tts_wall_ms: f64,
    pub asr_wall_ms: Option<f64>,
    pub asr_first_partial_ms: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct RoundTrip {
    pub voice_note: String,
    pub piper: Option<String>,
    pub model: Option<String>,
    pub sample_rate: Option<u32>,
    pub model_load_ms: Option<f64>,
    pub utterances: Vec<Utterance>,
    pub wer: Check,
    pub ecapa: Check,
    pub emotion: Check,
    pub rollout: Rollout,
    pub tts_first_audio: Check,
    pub asr_first_partial: Check,
    pub notes: Vec<String>,
}

pub fn run() -> RoundTrip {
    let tools = Tools::discover();
    let mut notes = tools.notes.clone();
    if !tools.piper_ready() {
        return empty(
            &tools,
            notes,
            "Piper is not ready. The round-trip was not run.",
        );
    }
    let runtime = tools.piper.clone().expect("piper");
    let model = tools.model.clone().expect("model");
    let rate = tools.sample_rate.expect("rate");
    let voice_note = format!(
        "Piper pack {}. Deterministic scales noise_scale=0 noise_w=0 length_scale=1 sentence_silence=0. This is the A21 voice file on disk, not the configured persona's reference.",
        locate::display_path(&model)
    );
    let mut session = match Session::spawn(&runtime, &model) {
        Ok(session) => session,
        Err(err) => {
            notes.push(err);
            return empty(
                &tools,
                notes,
                "Piper was present and failed to start. No transcript was invented.",
            );
        }
    };
    let model_load_ms = session.model_load_ms;

    let vosk = if tools.asr_ready() {
        match Vosk::open(
            tools.vosk_lib.as_ref().expect("lib"),
            tools.vosk_model.as_ref().expect("model"),
        ) {
            Ok(vosk) => Some(vosk),
            Err(err) => {
                notes.push(err);
                None
            }
        }
    } else {
        None
    };
    let asr_opened = vosk.is_some();

    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut utterances = Vec::new();
    for sentence in SENTENCES {
        match session.say(sentence) {
            Ok(spoken) => {
                let audio_s = if spoken.pcm.len() >= 2 {
                    Some(spoken.pcm.len() as f64 / 2.0 / f64::from(rate))
                } else {
                    None
                };
                let mut utt = Utterance {
                    reference: (*sentence).to_string(),
                    hypothesis: String::new(),
                    wer: None,
                    pcm_bytes: spoken.pcm.len(),
                    audio_s,
                    first_audio_ms: spoken.first_audio_ms,
                    tts_wall_ms: spoken.wall_ms,
                    asr_wall_ms: None,
                    asr_first_partial_ms: None,
                };
                if let Some(engine) = vosk.as_ref() {
                    match resample(&tools, &spoken.pcm, rate) {
                        Ok(pcm16) => match engine.transcribe(&pcm16) {
                            Ok(tr) => fill_asr(&mut utt, sentence, tr),
                            Err(err) => notes.push(format!("{sentence}: {err}")),
                        },
                        Err(err) => notes.push(format!("{sentence}: {err}")),
                    }
                }
                kept.push(spoken.pcm);
                utterances.push(utt);
            }
            Err(err) => {
                notes.push(format!("{sentence}: {err}"));
                kept.push(Vec::new());
                utterances.push(Utterance {
                    reference: (*sentence).to_string(),
                    hypothesis: String::new(),
                    wer: None,
                    pcm_bytes: 0,
                    audio_s: None,
                    first_audio_ms: None,
                    tts_wall_ms: 0.0,
                    asr_wall_ms: None,
                    asr_first_partial_ms: None,
                });
            }
        }
    }
    drop(session);
    drop(vosk);

    let pairs: Vec<(String, String)> = utterances
        .iter()
        .filter(|u| u.wer.is_some())
        .map(|u| (u.reference.clone(), u.hypothesis.clone()))
        .collect();
    let wer = if !tools.asr_ready() {
        gate::Check::unmeasured(
            "wer",
            "<=",
            Some(gate::WER_MAX),
            "ASR is not ready. WER was not measured.",
        )
    } else if !asr_opened {
        gate::Check::error(
            "wer",
            "<=",
            Some(gate::WER_MAX),
            "libvosk was present and did not open. No transcript was invented.",
        )
    } else if pairs.is_empty() {
        gate::Check::error(
            "wer",
            "<=",
            Some(gate::WER_MAX),
            "no sentence was recognised. Mean WER was not invented.",
        )
    } else {
        let detail = format!(
            "mean of {} sentence rates, vosk-model-small-en-us-0.15, wer_t648 normalisation",
            pairs.len()
        );
        gate::judge_wer(wer::mean_wer(&pairs), &detail, true)
    };

    let (ecapa, ecapa_note) = score_ecapa(&tools, &kept, rate);
    if let Some(note) = ecapa_note {
        notes.push(note);
    }
    let emotion = gate::Check::unmeasured(
        "emotion_match",
        ">=",
        Some(gate::EMOTION_MIN),
        "emotion_match is 0.70*category_blend+0.30*emb_sim against a resting reference. Those weights are not run here. The CPU stub was not substituted.",
    );
    let rollout = gate::judge_rollout(&wer, &ecapa, &emotion);
    let tts_first_audio = tts_bar(&utterances);
    let asr_first_partial = asr_bar(&utterances);

    RoundTrip {
        voice_note,
        piper: Some(locate::display_path(&runtime)),
        model: Some(locate::display_path(&model)),
        sample_rate: Some(rate),
        model_load_ms,
        utterances,
        wer,
        ecapa,
        emotion,
        rollout,
        tts_first_audio,
        asr_first_partial,
        notes,
    }
}

fn fill_asr(utt: &mut Utterance, sentence: &str, tr: Transcript) {
    utt.hypothesis = tr.text;
    utt.wer = wer::wer(sentence, &utt.hypothesis);
    utt.asr_wall_ms = Some(tr.wall_ms);
    utt.asr_first_partial_ms = tr.first_partial_ms;
}

fn tts_bar(utterances: &[Utterance]) -> Check {
    let mut slowest: Option<(f64, String)> = None;
    for utt in utterances {
        if let Some(ms) = utt.first_audio_ms {
            let worse = slowest.as_ref().map(|(best, _)| ms > *best).unwrap_or(true);
            if worse {
                slowest = Some((ms, utt.reference.clone()));
            }
        }
    }
    if let Some((ms, reference)) = slowest {
        gate::judge_latency(
            "tts_first_audio_ms",
            Some(ms),
            &format!(
                "slowest time from the text write to the first raw PCM byte, after the model was already loaded, on \"{reference}\". The bar is the 80 ms heartbeat. Every line is listed under utterances."
            ),
        )
    } else {
        gate::Check::unmeasured(
            "tts_first_audio_ms",
            "<=",
            Some(gate::STREAM_BAR_MS),
            "Piper produced no timed audio.",
        )
    }
}

fn asr_bar(utterances: &[Utterance]) -> Check {
    let partial = utterances
        .iter()
        .filter_map(|u| u.asr_first_partial_ms)
        .fold(None, |acc, v| Some(acc.map_or(v, |a: f64| a.min(v))));
    if let Some(ms) = partial {
        return gate::judge_latency(
            "asr_first_partial_ms",
            Some(ms),
            "fastest time to a non-empty Vosk partial. The bar is the 80 ms heartbeat.",
        );
    }
    let wall = utterances
        .iter()
        .filter_map(|u| u.asr_wall_ms)
        .fold(None, |acc, v| Some(acc.map_or(v, |a: f64| a.min(v))));
    if let Some(ms) = wall {
        gate::judge_latency(
            "asr_first_partial_ms",
            Some(ms),
            "Vosk emitted no partial before the final. This is the fastest full-utterance wall time, not a first partial.",
        )
    } else {
        gate::Check::unmeasured(
            "asr_first_partial_ms",
            "<=",
            Some(gate::STREAM_BAR_MS),
            "ASR did not run.",
        )
    }
}

fn empty(tools: &Tools, mut notes: Vec<String>, why: &str) -> RoundTrip {
    notes.push(why.into());
    let wer = gate::Check::unmeasured("wer", "<=", Some(gate::WER_MAX), why);
    let ecapa = gate::Check::unmeasured("ecapa", ">", Some(gate::ECAPA_MIN), "ECAPA was not run.");
    let emotion = gate::Check::unmeasured(
        "emotion_match",
        ">=",
        Some(gate::EMOTION_MIN),
        "emotion_match was not run. The CPU stub was not substituted.",
    );
    RoundTrip {
        voice_note: "No synthesis. The round-trip did not run.".into(),
        piper: tools.piper.as_ref().map(|p| locate::display_path(p)),
        model: tools.model.as_ref().map(|p| locate::display_path(p)),
        sample_rate: tools.sample_rate,
        model_load_ms: None,
        utterances: Vec::new(),
        rollout: gate::judge_rollout(&wer, &ecapa, &emotion),
        wer,
        ecapa,
        emotion,
        tts_first_audio: gate::Check::unmeasured(
            "tts_first_audio_ms",
            "<=",
            Some(gate::STREAM_BAR_MS),
            why,
        ),
        asr_first_partial: gate::Check::unmeasured(
            "asr_first_partial_ms",
            "<=",
            Some(gate::STREAM_BAR_MS),
            why,
        ),
        notes,
    }
}

fn resample(tools: &Tools, pcm: &[u8], rate: u32) -> Result<Vec<u8>, String> {
    let ffmpeg = tools.ffmpeg.as_ref().ok_or("ffmpeg missing")?;
    let rate_s = rate.to_string();
    let mut child = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "s16le",
            "-ar",
            rate_s.as_str(),
            "-ac",
            "1",
            "-i",
            "pipe:0",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-f",
            "s16le",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg spawn: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("ffmpeg stdin missing")?;
    let pcm = pcm.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&pcm);
        drop(stdin);
    });
    let output = child
        .wait_with_output()
        .map_err(|e| format!("ffmpeg wait: {e}"))?;
    let _ = writer.join();
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg exited {}. {}", output.status, err.trim()));
    }
    if output.stdout.len() < 2 {
        return Err("ffmpeg wrote no PCM. No transcript was invented.".into());
    }
    Ok(output.stdout)
}

fn score_ecapa(tools: &Tools, pcm: &[Vec<u8>], rate: u32) -> (Check, Option<String>) {
    let Some(bin) = tools.ecapa_bin.clone() else {
        return (
            gate::Check::unmeasured(
                "ecapa",
                ">",
                Some(gate::ECAPA_MIN),
                "galion-ecapa is not on this machine. No cosine was invented.",
            ),
            None,
        );
    };
    let Some(reference) = tools.voice_ref.clone() else {
        return (
            gate::Check::unmeasured(
                "ecapa",
                ">",
                Some(gate::ECAPA_MIN),
                "GALION_VOICE_REF is unset. GALION_SAMANTHA_REF is the deprecated fallback. No reference clip was loaded. No cosine was invented.",
            ),
            None,
        );
    };
    let usable: Vec<&[u8]> = pcm
        .iter()
        .map(|p| p.as_slice())
        .filter(|p| p.len() >= 2)
        .collect();
    if usable.is_empty() {
        return (
            gate::Check::error(
                "ecapa",
                ">",
                Some(gate::ECAPA_MIN),
                "ECAPA was not given any audio.",
            ),
            None,
        );
    }
    let dir = std::env::temp_dir().join(format!("galion-eval-golden-{}", std::process::id()));
    if let Err(err) = fs::create_dir_all(&dir) {
        return (
            gate::Check::error(
                "ecapa",
                ">",
                Some(gate::ECAPA_MIN),
                &format!("temp dir: {err}"),
            ),
            None,
        );
    }
    let mut wavs = Vec::new();
    for (i, bytes) in usable.iter().enumerate() {
        let path = dir.join(format!("utt-{i}.wav"));
        if let Err(err) = write_wav(&path, bytes, rate) {
            let _ = fs::remove_dir_all(&dir);
            return (
                gate::Check::error("ecapa", ">", Some(gate::ECAPA_MIN), &err),
                None,
            );
        }
        wavs.push(path);
    }
    let out = dir.join("ecapa.json");
    let mut cmd = Command::new(&bin);
    cmd.arg("--ref")
        .arg(&reference)
        .arg("--threads")
        .arg("2")
        .arg("--out")
        .arg(&out);
    for wav in &wavs {
        cmd.arg(wav);
    }
    let ran = cmd.output();
    let parsed = match ran {
        Err(err) => Err(format!("could not start galion-ecapa: {err}")),
        Ok(output) if !output.status.success() => {
            let tail = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "galion-ecapa exited {}. {}",
                output.status,
                tail.chars().take(300).collect::<String>()
            ))
        }
        Ok(_) => fs::read_to_string(&out)
            .map_err(|e| format!("galion-ecapa wrote no JSON: {e}"))
            .and_then(|text| {
                parse_mean(&text).ok_or_else(|| "galion-ecapa JSON has no mean".into())
            }),
    };
    let _ = fs::remove_dir_all(&dir);
    match parsed {
        Ok(mean) => (
            gate::judge_ecapa(
                Some(mean),
                "galion-ecapa mean cosine against GALION_VOICE_REF. Temp wavs deleted.",
                true,
            ),
            None,
        ),
        Err(err) => (
            gate::Check::error("ecapa", ">", Some(gate::ECAPA_MIN), &err),
            Some(err),
        ),
    }
}

fn parse_mean(text: &str) -> Option<f64> {
    let key = "\"mean\"";
    let at = text.find(key)?;
    let rest = &text[at + key.len()..];
    let rest = rest.trim_start().trim_start_matches(':').trim_start();
    let end = rest.find(|c: char| {
        !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+')
    })?;
    rest[..end].parse().ok()
}

fn write_wav(path: &PathBuf, pcm: &[u8], rate: u32) -> Result<(), String> {
    let mut body = Vec::with_capacity(44 + pcm.len());
    let data_len = pcm.len() as u32;
    let byte_rate = rate * 2;
    body.extend_from_slice(b"RIFF");
    body.extend_from_slice(&(36 + data_len).to_le_bytes());
    body.extend_from_slice(b"WAVEfmt ");
    body.extend_from_slice(&16u32.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&rate.to_le_bytes());
    body.extend_from_slice(&byte_rate.to_le_bytes());
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&16u16.to_le_bytes());
    body.extend_from_slice(b"data");
    body.extend_from_slice(&data_len.to_le_bytes());
    body.extend_from_slice(pcm);
    fs::write(path, body).map_err(|e| format!("temp wav: {e}"))
}

pub fn wer_status_measured(trip: &RoundTrip) -> bool {
    matches!(trip.wer.status, Status::Pass | Status::Fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_parser_reads_ecapa_json() {
        let text = r#"{"mean": 0.7123, "n": 1}"#;
        assert_eq!(parse_mean(text), Some(0.7123));
        assert_eq!(parse_mean("{}"), None);
    }

    #[test]
    fn roundtrip_is_honest_about_what_this_machine_can_score() {
        let trip = run();
        assert!(!trip.rollout.claimed);
        assert_eq!(trip.emotion.status, Status::Unmeasured);
        assert!(trip.emotion.value.is_none());
        assert_eq!(trip.ecapa.status, Status::Unmeasured);
        assert!(trip.ecapa.value.is_none());
        if wer_status_measured(&trip) {
            assert!(trip.wer.value.is_some());
            assert!(trip.utterances.iter().any(|u| u.pcm_bytes > 1000));
            assert_ne!(trip.rollout.status, Status::Pass);
            assert!(trip.rollout.pass.is_none());
        } else {
            assert!(trip.wer.value.is_none());
            assert_ne!(trip.wer.status, Status::Pass);
        }
        let blob = format!("{trip:?}");
        assert!(!blob.contains("pcm_b64"));
    }
}
