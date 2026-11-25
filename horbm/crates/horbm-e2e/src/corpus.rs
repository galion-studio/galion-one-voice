// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Flite sentences, recognised by the existing Vosk engine.
//!
//! The waveforms are generated here and kept in memory. Nothing is a
//! recording of a person. Word error uses `galion-sam-asr`'s counter.

use std::sync::{Arc, Mutex};

use galion_sam_asr::{
    corpus_counts, final_text, run_stream, word_error, Asr, VoskAsr, VoskConfig, Wer,
    HEARTBEAT_SAMPLES, SAMPLE_RATE,
};

use crate::model;
use crate::mouth::{self, EngineKind};

/// Generated lines. The first is the duplex question. The rest are short
/// neutral sentences written for this smoke. Flite speaks them at test time.
pub const SENTENCES: [&str; 11] = [
    "where is the kettle",
    "the window is open",
    "please pass the salt",
    "the lamp is on the table",
    "what time is lunch",
    "the door is closed",
    "a cup of water",
    "the book is on the shelf",
    "turn the page",
    "the room is quiet",
    "set the glass down",
];

const STOP_TRIES: [&str; 2] = ["stop now", "stop"];
const PAD_MS: usize = 400;

pub struct Utt {
    pub reference: String,
    pub hypothesis: String,
    pub wer: Wer,
    pub wall_us: u64,
}

pub enum EarPlan {
    Stub {
        reason: &'static str,
    },
    Live {
        kettle: Vec<i16>,
        stop: Vec<i16>,
        kettle_hyp: String,
        stop_hyp: String,
        utts: Vec<Utt>,
        overall: Wer,
        model_name: String,
    },
}

pub fn plan() -> Result<Arc<EarPlan>, String> {
    static CACHE: Mutex<Option<Arc<EarPlan>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(plan) = guard.as_ref() {
        return Ok(Arc::clone(plan));
    }
    let built = Arc::new(build()?);
    *guard = Some(Arc::clone(&built));
    Ok(built)
}

fn build() -> Result<EarPlan, String> {
    if !mouth::flite_ready() {
        return Ok(EarPlan::Stub {
            reason: "flite_unavailable",
        });
    }
    let ready = match model::ensure_model() {
        Ok(ready) => ready,
        Err(skip) => {
            return Ok(EarPlan::Stub {
                reason: skip.token(),
            })
        }
    };
    let config = VoskConfig {
        model_dir: ready.model_dir,
        lib_path: ready.lib,
        threads: vosk_threads()?,
    };
    // `open_engine` looks up a model on disk before it honours `EngineOpts.model`,
    // so a cache outside the repo never opens. `VoskAsr::open` takes the path.
    let mut engine: Box<dyn Asr> = match VoskAsr::open(config) {
        Ok(engine) => Box::new(engine),
        Err(err) => {
            eprintln!("asr_open: {}", one_line(&err));
            return Ok(EarPlan::Stub {
                reason: "lib_unavailable",
            });
        }
    };

    let mut utts = Vec::with_capacity(SENTENCES.len());
    let mut kettle = None;
    for (index, line) in SENTENCES.iter().enumerate() {
        let pcm = synth(line)?;
        let utt = score_utterance(&mut *engine, line, &pcm)?;
        if index == 0 {
            kettle = Some(pcm);
        }
        utts.push(utt);
    }
    if utts.iter().all(|utt| utt.hypothesis.trim().is_empty()) {
        return Err("vosk returned an empty transcript for every sentence".into());
    }
    let kettle = kettle.ok_or_else(|| "missing kettle audio".to_string())?;
    let kettle_hyp = utts
        .first()
        .map(|utt| utt.hypothesis.clone())
        .unwrap_or_default();
    let (stop, stop_hyp) = stop_audio(&mut *engine)?;
    eprintln!("asr_kettle: {kettle_hyp}");
    eprintln!("asr_stop: {stop_hyp}");
    let overall = corpus_counts(&utts.iter().map(|utt| utt.wer).collect::<Vec<_>>());
    Ok(EarPlan::Live {
        kettle,
        stop,
        kettle_hyp,
        stop_hyp,
        utts,
        overall,
        model_name: ready.model_name,
    })
}

fn stop_audio(engine: &mut dyn Asr) -> Result<(Vec<i16>, String), String> {
    let mut chosen_pcm = None;
    let mut chosen_hyp = String::new();
    for line in STOP_TRIES {
        let pcm = synth(line)?;
        let utt = score_utterance(engine, line, &pcm)?;
        let heard = utt.hypothesis.to_ascii_lowercase().contains("stop");
        if chosen_pcm.is_none() || heard {
            chosen_hyp = utt.hypothesis;
            chosen_pcm = Some(pcm);
        }
        if heard {
            break;
        }
    }
    let pcm = chosen_pcm.ok_or_else(|| "stop audio missing".to_string())?;
    Ok((pcm, chosen_hyp))
}

fn vosk_threads() -> Result<u32, String> {
    match std::env::var("GALION_VOSK_THREADS") {
        Err(_) => Ok(2),
        Ok(value) if value.is_empty() => Ok(2),
        Ok(value) => value
            .parse::<u32>()
            .map_err(|_| format!("GALION_VOSK_THREADS={value} is not an integer"))
            .and_then(|threads| {
                if threads == 0 {
                    Err("GALION_VOSK_THREADS=0 is not a thread count".into())
                } else {
                    Ok(threads)
                }
            }),
    }
}

fn synth(text: &str) -> Result<Vec<i16>, String> {
    match mouth::speak_16k(EngineKind::Flite, text)? {
        Some(pcm) if pcm.iter().any(|sample| *sample != 0) => Ok(pcm),
        _ => Err(format!("flite produced no audio for {text}")),
    }
}

fn score_utterance(engine: &mut dyn Asr, reference: &str, pcm: &[i16]) -> Result<Utt, String> {
    engine.reset().map_err(|err| one_line(&err))?;
    let audio = padded(pcm);
    let report = run_stream(engine, &audio, HEARTBEAT_SAMPLES).map_err(|err| one_line(&err))?;
    let hypothesis = final_text(&report.events);
    let wer = word_error(reference, &hypothesis);
    Ok(Utt {
        reference: reference.to_string(),
        hypothesis,
        wer,
        wall_us: report.wall_us,
    })
}

fn padded(pcm: &[i16]) -> Vec<i16> {
    let mut out = pcm.to_vec();
    let extra = usize::try_from(SAMPLE_RATE).unwrap_or(16_000) * PAD_MS / 1000;
    out.extend(std::iter::repeat(0).take(extra));
    out
}

fn one_line(err: &impl std::fmt::Display) -> String {
    err.to_string()
        .replace(['\n', '\r'], " ")
        .chars()
        .take(400)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_sam_asr::{AsrError, AsrEvent, AsrKind};

    struct Fixed {
        text: String,
    }

    impl Asr for Fixed {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }

        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(vec![AsrEvent {
                kind: AsrKind::Final,
                text: self.text.clone(),
                audio_ms: 0,
                words: Vec::new(),
                why: "finish",
            }])
        }

        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }

        fn name(&self) -> &str {
            "fixed"
        }
    }

    #[test]
    fn per_sentence_rates_add_up_to_the_corpus_rate() {
        let mut engine = Fixed {
            text: "where is kettle".into(),
        };
        let pcm = vec![0i16; 1600];
        let first = score_utterance(&mut engine, "where is the kettle", &pcm).unwrap();
        engine.text = "the window is open".into();
        let second = score_utterance(&mut engine, "the window is open", &pcm).unwrap();
        assert_eq!(first.wer.deletions, 1);
        assert_eq!(first.wer.errors(), 1);
        assert_eq!(second.wer.errors(), 0);
        let total = corpus_counts(&[first.wer, second.wer]);
        assert_eq!(total.reference_words, 8);
        assert_eq!(total.errors(), 1);
        assert!((total.rate().unwrap() - 0.125).abs() < 1e-12);
    }
}
