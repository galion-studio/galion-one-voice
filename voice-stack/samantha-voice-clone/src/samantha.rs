// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `SamanthaTts` implements the shared [`crate::Tts`] trait.
//! Each sentence is one worker request, seed `711 + index`, conditioned on
//! the original reference. A cache hit does not call the worker.

use crate::cache::PhraseCache;
use crate::consent::Consent;
use crate::device::{probe_compute, resolve_device};
use crate::error::TtsError;
use crate::frames::SAMPLE_RATE_HZ;
use crate::recipe::{plan_sentences, SentenceJob};
use crate::reference::Reference;
use crate::tts::{Pad, SentenceSpeaker, Speech, Tts};
use std::time::Instant;

pub struct SamanthaTts<S> {
    reference: Reference,
    speaker: S,
    cache: Option<PhraseCache>,
    device: String,
    consent: Consent,
}

impl SamanthaTts<crate::worker::WorkerSpeaker> {
    /// Open from `GALION_SAMANTHA_REF`. The file is hashed before any speech.
    /// `GALION_SAMANTHA_STUB=1` uses protocol PCM. Otherwise the worker loads
    /// a local checkpoint from `GALION_SAMANTHA_WEIGHTS` on the first sentence.
    pub fn open_from_env() -> Result<Self, TtsError> {
        let reference = Reference::from_env()?;
        let requested = std::env::var("GALION_SAMANTHA_DEVICE").unwrap_or_else(|_| "auto".into());
        let probe = probe_compute();
        let resolved = resolve_device(requested.trim(), &probe)?;
        let stub = std::env::var("GALION_SAMANTHA_STUB")
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        let device = if stub { "stub".to_string() } else { resolved };
        let cache = match std::env::var("GALION_SAMANTHA_CACHE") {
            Ok(value) if !value.trim().is_empty() => {
                Some(PhraseCache::open(std::path::Path::new(value.trim()))?)
            }
            _ => {
                let dir = std::env::temp_dir().join("galion-samantha-s711");
                Some(PhraseCache::open(&dir)?)
            }
        };
        let speaker =
            crate::worker::WorkerSpeaker::spawn(if stub { "cpu" } else { &device }, stub)?;
        Ok(Self {
            reference,
            speaker,
            cache,
            device,
            consent: Consent::local_zero_shot(),
        })
    }
}

impl<S: SentenceSpeaker> SamanthaTts<S> {
    pub fn with_speaker(
        reference: Reference,
        speaker: S,
        cache: Option<PhraseCache>,
        device: impl Into<String>,
    ) -> Self {
        Self {
            reference,
            speaker,
            cache,
            device: device.into(),
            consent: Consent::local_zero_shot(),
        }
    }

    pub fn speaker(&self) -> &S {
        &self.speaker
    }

    pub fn speaker_mut(&mut self) -> &mut S {
        &mut self.speaker
    }

    pub fn reference(&self) -> &Reference {
        &self.reference
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn pre_render(&mut self, phrases: &[&str], mood: &Pad) -> Result<u128, TtsError> {
        let started = Instant::now();
        for phrase in phrases {
            self.synthesize(phrase, mood)?;
        }
        Ok(started.elapsed().as_micros())
    }

    fn one_sentence(
        &mut self,
        job: &SentenceJob,
    ) -> Result<(Vec<i16>, bool, Option<u128>, String), TtsError> {
        let key = PhraseCache::key(job);
        if let Some(cache) = &self.cache {
            if let Some(hit) = cache.get(&key)? {
                return Ok((hit.samples, true, None, hit.device));
            }
        }
        let spoken = self.speaker.speak(job)?;
        if let Some(cache) = &mut self.cache {
            cache.put(&key, job, &spoken)?;
        }
        Ok((spoken.samples, false, spoken.worker_ttfa_us, spoken.device))
    }
}

impl<S: SentenceSpeaker> Tts for SamanthaTts<S> {
    fn synthesize(&mut self, text: &str, mood: &Pad) -> Result<Speech, TtsError> {
        if self.consent.scope() != crate::consent::CONSENT_SCOPE {
            return Err(TtsError::new("consent", "scope drifted"));
        }
        let jobs = plan_sentences(text, &self.reference, &self.device, *mood)?;
        let started = Instant::now();
        let mut samples = Vec::new();
        let mut sentences = Vec::with_capacity(jobs.len());
        let mut all_cached = true;
        let mut device = if self.cache.is_some() {
            "cache".to_string()
        } else {
            self.device.clone()
        };
        for job in &jobs {
            let (pcm, cached, ttfa, spoken_device) = self.one_sentence(job)?;
            if !cached {
                all_cached = false;
                device = spoken_device;
            }
            samples.extend_from_slice(&pcm);
            sentences.push(crate::tts::SentenceAudio {
                index: job.index,
                seed: job.seed,
                text: job.text.clone(),
                samples: pcm,
                cached,
                worker_ttfa_us: ttfa,
                reference_sha256: job.reference_sha256.clone(),
            });
        }
        if all_cached {
            device = "cache".to_string();
        }
        Ok(Speech {
            sample_rate_hz: SAMPLE_RATE_HZ,
            channels: 1,
            bits: 16,
            samples,
            sentences,
            cached: all_cached,
            device,
            took_us: started.elapsed().as_micros(),
            recipe: "s711",
            mood: *mood,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::PhraseCache;
    use crate::frames::FRAME_SAMPLES;
    use crate::recipe::REF_SHA_PREFIX;
    use crate::reference::Reference;
    use crate::tts::{Pad, RecordingSpeaker};
    use std::path::PathBuf;

    fn voice(cache: Option<PhraseCache>) -> SamanthaTts<RecordingSpeaker> {
        let reference = Reference::trusted_for_tests(
            PathBuf::from("/tmp/galion-samantha-ref.wav"),
            format!("{REF_SHA_PREFIX}{}", "cd".repeat(28)),
        )
        .unwrap();
        SamanthaTts::with_speaker(reference, RecordingSpeaker::default(), cache, "cpu")
    }

    #[test]
    fn two_sentences_keep_the_original_reference_and_the_recipe() {
        let mut voice = voice(None);
        let mood = Pad {
            arousal: 0.9,
            valence: 0.1,
            dominance: 0.4,
        };
        let speech = voice.synthesize("Hello. Again!", &mood).unwrap();
        assert_eq!(speech.sample_rate_hz, 24_000);
        assert_eq!(speech.channels, 1);
        assert_eq!(speech.bits, 16);
        assert_eq!(speech.recipe, "s711");
        assert!(!speech.cached);
        assert_eq!(speech.sentences.len(), 2);
        assert_eq!(speech.sentences[0].seed, 711);
        assert_eq!(speech.sentences[1].seed, 712);
        assert_eq!(
            speech.sentences[0].reference_sha256,
            speech.sentences[1].reference_sha256
        );
        assert_eq!(speech.samples.len(), FRAME_SAMPLES * 2);
        assert_eq!(speech.full_frames().count(), 2);
        let jobs = &voice.speaker().jobs;
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].reference_path, jobs[1].reference_path);
        assert!(!jobs[0].reference_path.contains(".pcm"));
        let line = jobs[0].to_json_line().unwrap();
        assert!(line.contains("\"exaggeration\":0.5"));
        assert!(line.contains("\"cfg_weight\":0.5"));
        assert!(line.contains("\"mode\":\"expr\""));
        assert!(line.contains("\"chain\":false"));
        assert!(line.contains("\"raw\":true"));
    }

    #[test]
    fn a_repeated_phrase_hits_the_cache_and_does_not_change_with_mood() {
        let dir = std::env::temp_dir().join(format!(
            "galion-s711-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = PhraseCache::open(&dir).unwrap();
        let mut voice = voice(Some(cache));
        let first = voice.synthesize("Hi.", &Pad::RESTING).unwrap();
        assert!(!first.cached);
        let second = voice
            .synthesize(
                "Hi.",
                &Pad {
                    arousal: 1.0,
                    valence: -0.2,
                    dominance: 0.1,
                },
            )
            .unwrap();
        assert!(second.cached);
        assert_eq!(second.device, "cache");
        assert_eq!(first.samples, second.samples);
        assert_eq!(voice.speaker().jobs.len(), 1);
        let took = voice.pre_render(&["Hi.", "Bye."], &Pad::RESTING).unwrap();
        assert!(took < 5_000_000, "pre-render took {took} us");
        assert_eq!(voice.speaker().jobs.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
