// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Real ear and mouth for the closed gate.
//!
//! Default builds do not compile the workers. `--features real` links
//! `galion-asr-vosk` and the Piper/Flite worker in `galion-ml-voice` as they
//! are. This file is glue: it does not change those crates. The transcript
//! is free decoding. A second pass spots only the backchannel list, `stop`,
//! and a few affect words, then may rewrite one overlapping token. The
//! scripted lines and the held-out lines are never put in that list.
//!
//! The runtime path is Linux x86_64 because the pinned libvosk archive and
//! the pinned Piper runtime are that platform. Other targets compile and
//! return a skip. Paths are built from the cache directory and
//! [`std::env::consts`]; nothing is hard-coded as a Windows `.exe`.
//!
//! Models are downloaded into the cache on demand and are not written into
//! the repository. `GALION_HORBM_RESEARCH_DIR` is not opened. Samantha is
//! the persona. A clone mouth, including Aria, is refused.

use crate::gate::{Config, Report};

pub(crate) struct GateErr {
    pub skip: bool,
    pub message: String,
}

impl GateErr {
    pub(crate) fn skip(message: impl Into<String>) -> Self {
        Self {
            skip: true,
            message: message.into(),
        }
    }

    pub(crate) fn fail(message: impl Into<String>) -> Self {
        Self {
            skip: false,
            message: message.into(),
        }
    }
}

pub(crate) fn execute(config: &Config) -> Result<Report, GateErr> {
    #[cfg(not(feature = "real"))]
    {
        let _ = config;
        Err(GateErr::skip(
            "real ear and mouth are not compiled into this binary. Rebuild horbm-eval with --features real. The default gate stays on FakeEar and FakeMouth.",
        ))
    }
    #[cfg(all(
        feature = "real",
        not(all(target_os = "linux", target_arch = "x86_64"))
    ))]
    {
        let _ = config;
        Err(GateErr::skip(format!(
            "skip: real ear and mouth run on Linux x86_64. This process is {} {}. The pinned libvosk archive and the Piper runtime are Linux x86_64. The fake gate is unchanged.",
            std::env::consts::OS,
            std::env::consts::ARCH
        )))
    }
    #[cfg(all(feature = "real", target_os = "linux", target_arch = "x86_64"))]
    {
        linux::execute(config)
    }
}

#[cfg(all(feature = "real", target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::GateErr;
    use crate::gate::{
        self, anchors, conductor_gap, load_truth, measure_emotion, out, samantha_window, Config,
        Mode, Report, Scores, SCRIPT,
    };
    use crate::score::{
        mean_wer, percentile, score_emotion, score_grounding, score_latency, score_persona,
        score_unknown, score_wer, wer_tokens, LatencySample, Spoken, VoiceTurn,
    };
    use galion_asr_vosk::{clean_transcript, VoskClient, Word, SAMPLE_RATE};
    use galion_emotion::text_affect;
    use galion_ml_voice::speech::{
        resample, sha256, FrameProsody, SpeechBackend, SpeechStack, SPEECH_RATE,
    };
    use horbm::Brain;
    use horbm_voice::{
        is_listed_backchannel, listed_backchannels, AudioFrame, Ear, EarEvent, HorbmTurn, Mouth,
        MouthPull, SpokenLine, TickEvent, TurnBrain, VoiceLoop,
    };
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    const MODEL_NAME: &str = "vosk-model-small-en-us-0.15";
    const MODEL_URL: &str = "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip";
    const MODEL_SHA: &str = "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498";
    const LIB_URL: &str =
        "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip";
    const LIB_SHA: &str = "bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2";
    const LIB_DIR_NAME: &str = "vosk-linux-x86_64-0.3.45";
    const PIPER_URL: &str =
        "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz";
    const PIPER_SHA: &str = "a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992";
    const VOICE_NAME: &str = "en_US-ljspeech-medium";
    const VOICE_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx";
    const VOICE_SHA: &str = "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22";
    const VOICE_JSON_SHA: &str = "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860";

    /// Single affect words. Not sentences, and not the eval script.
    const AFFECT_WORDS: &[&str] = &[
        "sad",
        "happy",
        "tired",
        "angry",
        "afraid",
        "lonely",
        "glad",
        "upset",
        "worried",
        "excited",
        "calm",
        "anxious",
        "scared",
        "miserable",
        "joyful",
    ];

    /// Piper lines that are not keywords and are not the seven scripted turns.
    /// The recognizer is not given this text.
    const HELD_OUT: &[&str] = &[
        "can you pass the salt",
        "what time is dinner",
        "the bus was late today",
        "I left my keys on the table",
        "let's go for a walk",
        "the coffee is still hot",
        "did you lock the door",
        "I need a glass of water",
        "the meeting starts at noon",
        "please turn the light off",
        "I feel sad today",
        "I'm so tired",
        "that's amazing",
        "I feel really happy",
        "this makes me angry",
        "I'm a bit worried",
        "what a lovely surprise",
        "I miss you tonight",
        "where did you put the book",
        "how was your morning",
        "can you say that again",
        "what should we cook tonight",
        "is the window open",
        "who is at the door",
    ];

    pub(super) fn execute(config: &Config) -> Result<Report, GateErr> {
        if !config
            .thresholds
            .persona
            .id
            .eq_ignore_ascii_case("samantha")
        {
            return Err(GateErr::fail(
                "Samantha is the only persona. Aria stays inactive.",
            ));
        }
        let root = repo_root();
        let bin = ensure_vosk_bin(&root)?;
        let cache = cache_dir();
        let vosk = prepare_vosk(&cache)?;
        let piper = prepare_piper(&cache);
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let lock = LOCK.get_or_init(|| Mutex::new(()));
        let _held = lock.lock().unwrap_or_else(|err| err.into_inner());
        let model_bytes = vosk.model_bytes;
        let (ear, mouth, _env) = open_pair(&bin, &vosk, piper.as_ref(), config.prosody)?;
        run_gate(config, ear, mouth, model_bytes)
    }

    fn run_gate(
        config: &Config,
        mut ear: VoskEar,
        mut mouth: StackMouth,
        model_bytes: u64,
    ) -> Result<Report, GateErr> {
        let voice_name = mouth.voice_name();
        let mouth_id = mouth.id().to_string();
        if mouth_id == "clone-worker" || voice_name.to_ascii_lowercase().contains("aria") {
            return Err(GateErr::fail(
                "a clone mouth is not the eval mouth. Aria stays inactive. Samantha is the persona.",
            ));
        }
        mouth.warm();
        let truth = load_truth().map_err(GateErr::fail)?;
        let mut spoken = Vec::new();
        let mut voices = Vec::new();
        let mut eager_ms = Vec::new();
        let mut pairs = Vec::new();
        let mut research_merged = false;
        let mut kettle_hyp = None;
        for (query, expect) in SCRIPT {
            let pcm = mouth.render_user(query)?;
            let hyp = ear.recognize(&pcm)?;
            pairs.push(((*query).to_string(), hyp.clone()));
            if *query == "where is the kettle" {
                kettle_hyp = Some(hyp.clone());
            }
            let turn = one_turn(&truth, query, *expect, &hyp, &mut ear, &mut mouth)?;
            research_merged |= turn.research_merged;
            eager_ms.push(turn.eager_ms);
            voices.push(turn.voice);
            spoken.push(turn.spoken);
        }
        let script_ms = ear.decode_ms.clone();
        let script_audio = ear.audio_ms.clone();
        if script_ms.len() != SCRIPT.len() || script_audio.len() != SCRIPT.len() {
            return Err(GateErr::fail(
                "the seven scripted lines were not all decoded",
            ));
        }
        let sad_hyp = pairs
            .iter()
            .find(|(reference, _)| reference == "I am sad")
            .map(|(_, hypothesis)| hypothesis.clone())
            .ok_or_else(|| GateErr::fail("the sad line was not heard"))?;
        if !heard_word(&sad_hyp, "sad") || heard_word(&sad_hyp, "said") {
            return Err(GateErr::fail(format!(
                "I am sad was heard as {sad_hyp:?}. The word sad has to come back as sad."
            )));
        }
        let kettle_hyp = kettle_hyp.ok_or_else(|| GateErr::fail("kettle turn was not heard"))?;
        let (barge_ms, barge_virtual) = barge_in(&mut ear, &mut mouth, &truth.cards, &kettle_hyp)?;
        let (backchannel_cancel, backchannel_heard) =
            backchannel(&mut ear, &mut mouth, &truth.cards, &kettle_hyp)?;
        let held_at = ear.decode_ms.len();
        let mut held = Vec::new();
        for line in HELD_OUT {
            let pcm = mouth.render_user(line)?;
            let hyp = ear.recognize(&pcm)?;
            held.push(((*line).to_string(), hyp));
        }
        if held.len() < 20 {
            return Err(GateErr::fail("the held-out set has fewer than 20 lines"));
        }
        let held_ms = ear.decode_ms[held_at..].to_vec();
        let held_audio = ear.audio_ms[held_at..].to_vec();
        let emotion = measure_emotion(&config.thresholds).map_err(GateErr::fail)?;
        if config.prosody && ear.sensed_frames == 0 {
            return Err(GateErr::fail(
                "ear prosody sensing is on and saw no frame. It was not switched off.",
            ));
        }

        let grounded = score_grounding(
            &spoken,
            &truth.pack,
            &truth.forbidden,
            &config.thresholds.grounding,
        );
        let mut unknown = score_unknown(
            &spoken,
            &truth.pack,
            &truth.forbidden,
            &config.thresholds.grounding,
        );
        if research_merged {
            unknown.pass = false;
            unknown
                .failures
                .push("conductor merged a research clause".into());
        }
        let persona = score_persona(
            &voices,
            samantha_window(),
            anchors(),
            &config.thresholds.persona,
        );
        let emotion_check = score_emotion(&emotion.sample, &config.thresholds.emotion);
        let latency = score_latency(
            &LatencySample {
                eager_to_voice_ms: eager_ms.clone(),
                barge_in_ms: barge_ms,
                backchannel_cancel,
                clock_advanced_ms: 0,
                wall_sleep_ms: 0,
            },
            &config.thresholds.real.latency,
        );
        let (wer_check, held_wer) = score_wer(&held, config.thresholds.asr.wer_max);
        let script_wer = mean_wer(&pairs).unwrap_or(1.0);
        let (rtf, p50_ms, p95_ms) = script_timing(&script_ms, &script_audio);
        let (held_rtf, held_p50_ms, held_p95_ms) = script_timing(&held_ms, &held_audio);
        if rtf > 1.0 || held_rtf > 1.0 {
            return Err(GateErr::fail(format!(
                "ear real-time factor script {rtf:.3} held-out {held_rtf:.3} is over 1"
            )));
        }
        let (rss_kb, hwm_kb) = worker_mem_kb();
        let pass = grounded.pass
            && unknown.pass
            && persona.pass
            && emotion_check.pass
            && latency.pass
            && wer_check.pass;
        let prosody = if config.prosody { "on" } else { "off" };
        let pair_rows = pair_rows_of(&pairs);
        let held_rows = pair_rows_of(&held);
        Ok(Report {
            schema: "horbm-eval-report/1",
            mode: Mode::Real.as_str(),
            pass,
            skipped: false,
            research_dir: "unset",
            truth_cards: truth.ids,
            stand_ins: vec![
                format!("ear:galion-asr-vosk ({MODEL_NAME}, free decode + keyword spot)"),
                format!("mouth:{mouth_id} ({voice_name})"),
                "research:OfflineClient (GALION_HORBM_RESEARCH_DIR unset)".into(),
                "persona:samantha (Aria inactive)".into(),
                format!("prosody:{prosody}"),
                "platform:linux-x86_64".into(),
            ],
            scores: Scores {
                grounded: out(
                    &grounded,
                    serde_json::json!({
                        "hits": spoken.iter().filter(|turn| turn.expect_id.is_some()).count(),
                    }),
                ),
                unknown: out(
                    &unknown,
                    serde_json::json!({
                        "misses": spoken.iter().filter(|turn| turn.expect_id.is_none()).count(),
                    }),
                ),
                persona: out(
                    &persona,
                    serde_json::json!({
                        "id": config.thresholds.persona.id,
                        "turns": voices.len(),
                    }),
                ),
                emotion: out(
                    &emotion_check,
                    serde_json::json!({
                        "frame_ms": emotion.sample.frame_ms,
                        "step_p95_us": emotion.sample.step_p95_us,
                        "budget_us": config.thresholds.emotion.frame_budget_us,
                        "fatigue_arousal_drop": emotion.sample.fatigue_drop,
                        "valence_rise": emotion.sample.valence_rise,
                    }),
                ),
                latency: out(
                    &latency,
                    serde_json::json!({
                        "clock": "wall",
                        "eager_to_voice_p50_ms": percentile(&eager_ms, 50),
                        "eager_to_voice_p95_ms": percentile(&eager_ms, 95),
                        "samples": eager_ms.len(),
                        "barge_in_ms": barge_ms,
                        "barge_in_virtual_ms": barge_virtual,
                        "backchannel_cancel": backchannel_cancel,
                        "backchannel_heard": backchannel_heard,
                        "ear": "galion-asr-vosk",
                        "mouth": mouth_id,
                    }),
                ),
                asr: Some(out(
                    &wer_check,
                    serde_json::json!({
                        "wer": held_wer,
                        "wer_max": config.thresholds.asr.wer_max,
                        "held_out_wer": held_wer,
                        "held_out_n": held.len(),
                        "held_out_rtf": held_rtf,
                        "held_out_p50_ms": held_p50_ms,
                        "held_out_p95_ms": held_p95_ms,
                        "script_wer": script_wer,
                        "script_n": pairs.len(),
                        "script_rtf": rtf,
                        "script_p50_ms": p50_ms,
                        "script_p95_ms": p95_ms,
                        "model": MODEL_NAME,
                        "decode": "free+keyword",
                        "rtf": held_rtf,
                        "latency_p50_ms": held_p50_ms,
                        "latency_p95_ms": held_p95_ms,
                        "rss_mb": rss_kb / 1024,
                        "hwm_mb": hwm_kb / 1024,
                        "model_bytes": model_bytes,
                        "yeah": backchannel_heard,
                        "sad": sad_hyp,
                        "mouth": mouth_id,
                        "voice": voice_name,
                        "prosody": prosody,
                        "sensed_frames": ear.sensed_frames,
                        "pairs": pair_rows,
                        "held_out": held_rows,
                    }),
                )),
            },
            error: None,
        })
    }

    struct TurnDone {
        spoken: Spoken,
        voice: VoiceTurn,
        eager_ms: u32,
        research_merged: bool,
    }

    fn one_turn(
        truth: &gate::Truth,
        query: &str,
        expect: Option<&str>,
        hyp: &str,
        ear: &mut VoskEar,
        mouth: &mut StackMouth,
    ) -> Result<TurnDone, GateErr> {
        let mut brain = Brain::with_cards(truth.cards.clone(), None);
        let before = brain.emotion().baseline().identity_bits();
        let scored = brain.turn(hyp);
        if brain.emotion().baseline().identity_bits() != before {
            return Err(GateErr::fail(format!(
                "{query}: the turn rewrote the voice baseline"
            )));
        }
        let (_gap, research_merged, clause) =
            conductor_gap(&truth.cards, hyp).map_err(GateErr::fail)?;
        if clause != scored.reply {
            return Err(GateErr::fail(format!(
                "{query}: conductor spoke {clause:?}, brain spoke {:?}",
                scored.reply
            )));
        }
        ear.prime(EarEvent::end(hyp));
        let mut voice = VoiceLoop::new(
            std::mem::replace(ear, VoskEar::empty()),
            std::mem::replace(mouth, StackMouth::empty()),
            HorbmTurn::with_cards(truth.cards.clone()),
        );
        let started = Instant::now();
        let end = voice.tick(&[]);
        let eager_ms = elapsed_ms(started);
        if end.event != TickEvent::EndOfSpeech {
            return Err(GateErr::fail(format!("{query}: ear did not end the turn")));
        }
        if end.reply.as_deref() != Some(scored.reply.as_str()) {
            return Err(GateErr::fail(format!("{query}: mouth path reply drifted")));
        }
        if end.audio.is_some() {
            return Err(GateErr::fail(format!(
                "{query}: audio arrived on the end-of-speech frame"
            )));
        }
        let audio = voice.tick(&[]);
        if audio.event != TickEvent::FirstAudio {
            return Err(GateErr::fail(format!(
                "{query}: first mouth frame was {:?}",
                audio.event
            )));
        }
        let frame = audio
            .audio
            .as_ref()
            .ok_or_else(|| GateErr::fail("mouth returned no frame"))?;
        if frame.samples.is_empty() {
            return Err(GateErr::fail("mouth frame is empty"));
        }
        if !voice.mouth().audible {
            return Err(GateErr::fail("mouth rendered silence"));
        }
        drain(&mut voice)?;
        let (next_ear, next_mouth, _) = voice.into_parts();
        *ear = next_ear;
        *mouth = next_mouth;
        Ok(TurnDone {
            voice: VoiceTurn {
                persona: "samantha".into(),
                rate: scored.mood_meta.rate,
                energy: scored.mood_meta.energy,
                pitch_st: scored.mood_meta.pitch_st,
                neutral: text_affect(hyp).valence.is_none(),
            },
            spoken: Spoken {
                query: query.to_string(),
                reply: scored.reply,
                sources: scored.sources,
                sideband_posted: scored
                    .sideband
                    .as_ref()
                    .map(|mark| mark.posted)
                    .unwrap_or(false),
                expect_id: expect.map(str::to_string),
            },
            eager_ms,
            research_merged,
        })
    }

    fn barge_in(
        ear: &mut VoskEar,
        mouth: &mut StackMouth,
        cards: &horbm::KnowledgeCards,
        kettle_hyp: &str,
    ) -> Result<(u32, u32), GateErr> {
        let stop = mouth.render_user("stop")?;
        let mut voice = speaking_kettle(ear, mouth, cards, kettle_hyp)?;
        let started = Instant::now();
        let heard = voice.ear_mut().recognize(&stop)?;
        if is_listed_backchannel(&heard) {
            return Err(GateErr::fail(format!(
                "barge-in transcript {heard:?} is a listed backchannel"
            )));
        }
        voice.ear_mut().prime(EarEvent::partial(heard.clone()));
        voice.ear_mut().prime(EarEvent::partial(heard.clone()));
        voice.ear_mut().prime(EarEvent::partial(heard));
        let mut yielded = None;
        for _ in 0..4 {
            let frame = voice.tick(&[]);
            if frame.event == TickEvent::BargeIn {
                yielded = Some(frame.cancel);
                break;
            }
        }
        let wall = elapsed_ms(started);
        let virtual_ms = u32::try_from(voice.barge_in_ms().unwrap_or(0)).unwrap_or(u32::MAX);
        let cancel = yielded.ok_or_else(|| GateErr::fail("voice barge-in did not cancel"))?;
        if !cancel {
            return Err(GateErr::fail("voice barge-in did not cancel the mouth"));
        }
        let (next_ear, next_mouth, _) = voice.into_parts();
        *ear = next_ear;
        *mouth = next_mouth;
        Ok((wall, virtual_ms))
    }

    fn backchannel(
        ear: &mut VoskEar,
        mouth: &mut StackMouth,
        cards: &horbm::KnowledgeCards,
        kettle_hyp: &str,
    ) -> Result<(bool, String), GateErr> {
        let heard = hear_backchannel(ear, mouth)?;
        let mut voice = speaking_kettle(ear, mouth, cards, kettle_hyp)?;
        let cancels_before = voice.mouth().cancels;
        for _ in 0..6 {
            voice.ear_mut().prime(EarEvent::partial(heard.clone()));
        }
        let epoch = voice.epoch();
        let mut cancel = false;
        for _ in 0..6 {
            let frame = voice.tick(&[]);
            cancel |= frame.cancel;
            if frame.event != TickEvent::Backchannel || frame.epoch != epoch {
                return Err(GateErr::fail("voice backchannel changed the floor"));
            }
        }
        if voice.mouth().cancels != cancels_before {
            cancel = true;
        }
        let (next_ear, next_mouth, _) = voice.into_parts();
        *ear = next_ear;
        *mouth = next_mouth;
        Ok((cancel, heard))
    }

    /// The mouth says `yeah`. The transcript has to be that word. Another
    /// listed backchannel is not a substitute.
    fn hear_backchannel(ear: &mut VoskEar, mouth: &mut StackMouth) -> Result<String, GateErr> {
        let pcm = mouth.render_user("yeah")?;
        let text = ear.recognize(&pcm)?;
        if !text.eq_ignore_ascii_case("yeah") {
            return Err(GateErr::fail(format!(
                "yeah was heard as {text:?}. A listed backchannel substitute was not used."
            )));
        }
        Ok(text)
    }

    fn speaking_kettle(
        ear: &mut VoskEar,
        mouth: &mut StackMouth,
        cards: &horbm::KnowledgeCards,
        hyp: &str,
    ) -> Result<VoiceLoop<VoskEar, StackMouth, HorbmTurn>, GateErr> {
        ear.prime(EarEvent::end(hyp));
        let mut voice = VoiceLoop::new(
            std::mem::replace(ear, VoskEar::empty()),
            std::mem::replace(mouth, StackMouth::empty()),
            HorbmTurn::with_cards(cards.clone()),
        );
        let end = voice.tick(&[]);
        if end.event != TickEvent::EndOfSpeech {
            return Err(GateErr::fail("kettle ear did not end the turn"));
        }
        let audio = voice.tick(&[]);
        if audio.event != TickEvent::FirstAudio || !voice.mouth().speaking() {
            return Err(GateErr::fail("kettle mouth was not speaking"));
        }
        Ok(voice)
    }

    fn drain<B: TurnBrain>(voice: &mut VoiceLoop<VoskEar, StackMouth, B>) -> Result<(), GateErr> {
        let mut frames = 0u32;
        while voice.mouth().speaking() {
            let _ = voice.tick(&[]);
            frames = frames.saturating_add(1);
            if frames > 500 {
                return Err(GateErr::fail("mouth did not finish the utterance"));
            }
        }
        Ok(())
    }

    struct VoskEar {
        client: Option<VoskClient>,
        primed: VecDeque<EarEvent>,
        sense: bool,
        sensed_frames: u32,
        grammar: Vec<String>,
        decode_ms: Vec<u32>,
        audio_ms: Vec<u32>,
    }

    impl VoskEar {
        fn open(bin: &Path, model: &Path, lib: &Path, sense: bool) -> Result<Self, GateErr> {
            let client = VoskClient::from_bin(
                bin,
                &[
                    "--model".into(),
                    model.display().to_string(),
                    "--lib".into(),
                    lib.display().to_string(),
                ],
            )
            .map_err(GateErr::fail)?;
            Ok(Self {
                client: Some(client),
                primed: VecDeque::new(),
                sense,
                sensed_frames: 0,
                grammar: keyword_list(),
                decode_ms: Vec::new(),
                audio_ms: Vec::new(),
            })
        }

        fn empty() -> Self {
            Self {
                client: None,
                primed: VecDeque::new(),
                sense: false,
                sensed_frames: 0,
                grammar: Vec::new(),
                decode_ms: Vec::new(),
                audio_ms: Vec::new(),
            }
        }

        fn recognize(&mut self, pcm: &[i16]) -> Result<String, GateErr> {
            if pcm.is_empty() {
                return Err(GateErr::fail(
                    "empty audio. No transcript was invented.".to_string(),
                ));
            }
            if self.sense {
                self.sense_buffer(pcm)?;
            }
            let grammar = self.grammar.clone();
            let client = self
                .client
                .as_mut()
                .ok_or_else(|| GateErr::fail("vosk worker is not open"))?;
            let started = Instant::now();
            let free = client.recognize(pcm, None).map_err(GateErr::fail)?;
            let spot = client
                .recognize(pcm, Some(&grammar))
                .map_err(GateErr::fail)?;
            let decode_ms = elapsed_ms(started);
            let samples = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
            let audio_ms = samples.saturating_mul(1000) / SAMPLE_RATE;
            self.decode_ms.push(decode_ms);
            self.audio_ms.push(audio_ms);
            let text = merge_keyword(&free.words, &spot.words, &clean_transcript(&free.text));
            if text.is_empty() {
                Err(GateErr::fail(
                    "vosk returned an empty transcript. No words were invented.",
                ))
            } else {
                Ok(text)
            }
        }

        /// Sense this synthetic buffer. `galion-emotion-in` stays default-deny;
        /// the grant is this call, and only when the ear switch is on.
        fn sense_buffer(&mut self, pcm: &[i16]) -> Result<(), GateErr> {
            let samples: Vec<f32> = pcm.iter().map(|sample| *sample as f32 / 32768.0).collect();
            let gate = galion_emotion_in::Session::grant_sense();
            let report = galion_emotion_in::analyse_pcm(&samples, SAMPLE_RATE, &gate)
                .map_err(|err| GateErr::fail(format!("ear prosody sensing failed: {err}")))?;
            if report.frames == 0 {
                return Err(GateErr::fail(
                    "ear prosody sensing saw no 80 ms frame. The switch is still on.",
                ));
            }
            self.sensed_frames = self.sensed_frames.saturating_add(report.frames);
            let _ = report.reading.prosody.rms;
            Ok(())
        }

        fn prime(&mut self, event: EarEvent) {
            self.primed.push_back(event);
        }
    }

    impl Ear for VoskEar {
        fn id(&self) -> &str {
            "galion-asr-vosk"
        }

        fn poll(&mut self, _pcm: &[i16]) -> EarEvent {
            self.primed.pop_front().unwrap_or(EarEvent::Quiet)
        }
    }

    struct StackMouth {
        stack: Option<SpeechStack>,
        queue: VecDeque<Vec<i16>>,
        rate: u32,
        pending_first: bool,
        cancels: u32,
        audible: bool,
        id: String,
        voice: String,
    }

    impl StackMouth {
        fn new(stack: SpeechStack) -> Self {
            let id = stack.backend.id().to_string();
            let voice = voice_of(&stack.backend);
            Self {
                stack: Some(stack),
                queue: VecDeque::new(),
                rate: SPEECH_RATE,
                pending_first: false,
                cancels: 0,
                audible: false,
                id,
                voice,
            }
        }

        fn empty() -> Self {
            Self {
                stack: None,
                queue: VecDeque::new(),
                rate: SPEECH_RATE,
                pending_first: false,
                cancels: 0,
                audible: false,
                id: String::new(),
                voice: String::new(),
            }
        }

        fn voice_name(&self) -> String {
            self.voice.clone()
        }

        fn warm(&mut self) {
            if let Some(stack) = self.stack.as_mut() {
                stack.prewarm();
            }
            if self.id == "system-flite" {
                let _ = self.render_user("Hello.");
            }
        }

        fn render_user(&mut self, text: &str) -> Result<Vec<i16>, GateErr> {
            let audio = self.speak(text, FrameProsody::identity())?;
            to_vosk_pcm(&audio.samples, audio.sample_rate)
        }

        fn speak(
            &mut self,
            text: &str,
            prosody: FrameProsody,
        ) -> Result<galion_ml_voice::Audio, GateErr> {
            let stack = self
                .stack
                .as_mut()
                .ok_or_else(|| GateErr::fail("mouth is not open"))?;
            stack.set_frame_prosody(prosody);
            stack
                .speak(text)
                .map_err(|err| GateErr::fail(format!("mouth: {err}")))
        }
    }

    impl Mouth for StackMouth {
        fn id(&self) -> &str {
            &self.id
        }

        fn start(&mut self, line: &SpokenLine) {
            self.queue.clear();
            self.pending_first = false;
            let prosody = FrameProsody {
                pitch_st: line.pitch_st,
                pitch_range: 1.0,
                rate: line.rate,
                energy: line.energy,
                warmth: 0.5,
                pre_pause_ms: if line.pause_ms.is_finite() {
                    line.pause_ms.round().clamp(0.0, 400.0) as u16
                } else {
                    0
                },
            };
            let Ok(audio) = self.speak(&line.text, prosody) else {
                return;
            };
            self.rate = audio.sample_rate;
            let samples = to_i16(&audio.samples);
            self.audible = samples.iter().any(|sample| *sample != 0);
            let width = frame_width(self.rate);
            if samples.is_empty() || width == 0 {
                return;
            }
            let mut at = 0;
            while at < samples.len() {
                let end = (at + width).min(samples.len());
                self.queue.push_back(samples[at..end].to_vec());
                at = end;
            }
            self.pending_first = !self.queue.is_empty();
        }

        fn pull(&mut self) -> MouthPull {
            let Some(samples) = self.queue.pop_front() else {
                return MouthPull::Done;
            };
            let first = self.pending_first;
            self.pending_first = false;
            MouthPull::Frame(AudioFrame {
                samples,
                sample_rate: self.rate,
                first,
            })
        }

        fn cancel(&mut self) {
            if !self.queue.is_empty() {
                self.cancels = self.cancels.saturating_add(1);
            }
            self.queue.clear();
            self.pending_first = false;
        }

        fn speaking(&self) -> bool {
            !self.queue.is_empty()
        }
    }

    fn voice_of(backend: &SpeechBackend) -> String {
        match backend {
            SpeechBackend::A21Weights { voice } | SpeechBackend::SystemFlite { voice } => {
                voice.clone()
            }
            SpeechBackend::Clone { persona, .. } => persona.clone(),
            SpeechBackend::None { reason } => reason.clone(),
        }
    }

    fn frame_width(rate: u32) -> usize {
        (usize::try_from(rate).unwrap_or(0).saturating_mul(80)) / 1000
    }

    fn to_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|sample| {
                let finite = if sample.is_finite() { *sample } else { 0.0 };
                (finite.clamp(-1.0, 1.0) * 32_767.0).round() as i16
            })
            .collect()
    }

    fn peak_normalize(samples: &mut [f32]) {
        let peak = samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0f32, f32::max);
        if !peak.is_finite() || peak < 1.0e-3 || (0.2..=0.95).contains(&peak) {
            return;
        }
        let gain = 0.8 / peak;
        for sample in samples {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }

    fn to_vosk_pcm(samples: &[f32], rate: u32) -> Result<Vec<i16>, GateErr> {
        if rate == 0 {
            return Err(GateErr::fail("mouth sample rate is 0"));
        }
        let mut audio = if rate == SAMPLE_RATE {
            samples.to_vec()
        } else {
            resample(samples, rate, SAMPLE_RATE)
        };
        peak_normalize(&mut audio);
        if audio.iter().all(|sample| sample.abs() < 1.0e-4) {
            return Err(GateErr::fail(
                "mouth rendered silence. No transcript was invented.",
            ));
        }
        let lead = SAMPLE_RATE as usize / 10;
        let tail = SAMPLE_RATE as usize / 2;
        let mut pcm = vec![0i16; lead];
        pcm.extend(to_i16(&audio));
        pcm.extend(std::iter::repeat(0).take(tail));
        Ok(pcm)
    }

    fn elapsed_ms(started: Instant) -> u32 {
        u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    struct VoskPaths {
        model: PathBuf,
        lib: PathBuf,
        model_bytes: u64,
    }

    struct PiperPaths {
        runtime: PathBuf,
        onnx: PathBuf,
    }

    fn open_pair(
        bin: &Path,
        vosk: &VoskPaths,
        piper: Option<&PiperPaths>,
        sense: bool,
    ) -> Result<(VoskEar, StackMouth, EnvGuard), GateErr> {
        let mut env = EnvGuard::new();
        if let Some(dir) = vosk.lib.parent() {
            env.prepend_path("LD_LIBRARY_PATH", dir);
        }
        if let Some(dir) = piper.and_then(|item| item.runtime.parent()) {
            env.prepend_path("LD_LIBRARY_PATH", dir);
        }
        let mouth = open_mouth(&mut env, piper)?;
        let ear = VoskEar::open(bin, &vosk.model, &vosk.lib, sense)?;
        Ok((ear, mouth, env))
    }

    fn open_mouth(env: &mut EnvGuard, piper: Option<&PiperPaths>) -> Result<StackMouth, GateErr> {
        env.set("GALION_CLONE_LOCAL_REFS", "0");
        if let Some(piper) = piper {
            env.set("GALION_SPEECH", "a21");
            env.set("GALION_TTS_WEIGHTS", &piper.onnx.display().to_string());
            env.set("GALION_TTS_WEIGHTS_SHA256", VOICE_SHA);
            env.set("GALION_A21_RUNTIME", &piper.runtime.display().to_string());
        } else {
            env.set("GALION_SPEECH", "flite");
            env.set("GALION_FLITE_VOICE", "slt");
        }
        let stack = SpeechStack::detect();
        if matches!(stack.backend, SpeechBackend::Clone { .. }) {
            return Err(GateErr::fail(
                "clone mouth was selected. The eval mouth is Piper or Flite. Aria stays inactive.",
            ));
        }
        if !stack.can_speak() {
            return Err(GateErr::skip(format!(
                "skip: mouth cannot speak ({})",
                stack.backend.label()
            )));
        }
        Ok(StackMouth::new(stack))
    }

    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self { saved: Vec::new() }
        }

        fn set(&mut self, key: &'static str, value: &str) {
            if !self.saved.iter().any(|(saved, _)| *saved == key) {
                self.saved.push((key, std::env::var_os(key)));
            }
            std::env::set_var(key, value);
        }

        fn prepend_path(&mut self, key: &'static str, dir: &Path) {
            let dir = dir.display().to_string();
            let current = std::env::var(key).unwrap_or_default();
            if current.split(':').any(|part| part == dir) {
                return;
            }
            let joined = if current.is_empty() {
                dir
            } else {
                format!("{dir}:{current}")
            };
            self.set(key, &joined);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, previous) in self.saved.drain(..).rev() {
                match previous {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn cache_dir() -> PathBuf {
        if let Ok(path) = std::env::var("GALION_HORBM_EVAL_CACHE") {
            let path = path.trim();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
        if let Ok(path) = std::env::var("XDG_CACHE_HOME") {
            let path = path.trim();
            if !path.is_empty() {
                return PathBuf::from(path).join("galion").join("horbm-eval");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let home = home.trim();
            if !home.is_empty() {
                return PathBuf::from(home)
                    .join(".cache")
                    .join("galion")
                    .join("horbm-eval");
            }
        }
        std::env::temp_dir().join("galion-horbm-eval")
    }

    fn shared_lib_name() -> String {
        format!("libvosk{}", std::env::consts::DLL_SUFFIX)
    }

    fn piper_bin_name() -> String {
        format!("piper{}", std::env::consts::EXE_SUFFIX)
    }

    fn ensure_vosk_bin(root: &Path) -> Result<PathBuf, GateErr> {
        let bin = root
            .join("target")
            .join("debug")
            .join(format!("galion-asr-vosk{}", std::env::consts::EXE_SUFFIX));
        if bin.is_file() {
            return Ok(bin);
        }
        let status = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .arg("-p")
            .arg("galion-asr-vosk")
            .arg("--bin")
            .arg("galion-asr-vosk")
            .status()
            .map_err(|err| GateErr::fail(format!("cargo build galion-asr-vosk: {err}")))?;
        if !status.success() || !bin.is_file() {
            return Err(GateErr::fail(
                "galion-asr-vosk binary was not built. No transcript was invented.",
            ));
        }
        Ok(bin)
    }

    fn prepare_vosk(cache: &Path) -> Result<VoskPaths, GateErr> {
        let model = cache.join("vosk").join(MODEL_NAME);
        let ready = model.join("am").join("final.mdl").is_file()
            && model.join("conf").join("model.conf").is_file();
        if !ready {
            let zip = cache.join("vosk").join(format!("{MODEL_NAME}.zip"));
            fetch_file(MODEL_URL, &zip, MODEL_SHA)?;
            unzip(&zip, &cache.join("vosk"))?;
            let _ = std::fs::remove_file(&zip);
        }
        if !model.join("am").join("final.mdl").is_file() {
            return Err(GateErr::skip(
                "skip: vosk-model-small-en-us-0.15 is absent. No transcript was invented.",
            ));
        }
        let lib = match find_lib(cache) {
            Some(path) => path,
            None => {
                let zip = cache.join("vosk").join(format!("{LIB_DIR_NAME}.zip"));
                fetch_file(LIB_URL, &zip, LIB_SHA)?;
                unzip(&zip, &cache.join("vosk"))?;
                let _ = std::fs::remove_file(&zip);
                find_lib(cache).ok_or_else(|| {
                    GateErr::skip("skip: libvosk is absent. No transcript was invented.")
                })?
            }
        };
        Ok(VoskPaths {
            model_bytes: dir_bytes(&model),
            model,
            lib,
        })
    }

    fn find_lib(cache: &Path) -> Option<PathBuf> {
        let name = shared_lib_name();
        [
            cache.join("vosk").join("libvosk").join(&name),
            cache.join("vosk").join(LIB_DIR_NAME).join(&name),
        ]
        .into_iter()
        .find(|path| path.is_file())
    }

    fn prepare_piper(cache: &Path) -> Option<PiperPaths> {
        match fetch_piper(cache) {
            Ok(paths) => Some(paths),
            Err(err) => {
                eprintln!(
                    "horbm-eval: piper pack unavailable ({}); trying flite",
                    err.message
                );
                None
            }
        }
    }

    fn fetch_piper(cache: &Path) -> Result<PiperPaths, GateErr> {
        let tts = cache.join("tts");
        let onnx = tts.join(format!("{VOICE_NAME}.onnx"));
        let json = tts.join(format!("{VOICE_NAME}.onnx.json"));
        fetch_file(VOICE_URL, &onnx, VOICE_SHA)?;
        fetch_file(&format!("{VOICE_URL}.json"), &json, VOICE_JSON_SHA)?;
        let runtime_root = cache.join("runtime");
        let runtime = runtime_root.join("piper").join(piper_bin_name());
        if !runtime.is_file() {
            let archive = runtime_root.join("piper_linux_x86_64.tar.gz");
            fetch_file(PIPER_URL, &archive, PIPER_SHA)?;
            untar(&archive, &runtime_root)?;
            let _ = std::fs::remove_file(&archive);
        }
        if !runtime.is_file() {
            return Err(GateErr::skip("skip: piper runtime is absent"));
        }
        Ok(PiperPaths { runtime, onnx })
    }

    fn fetch_file(url: &str, dest: &Path, sha: &str) -> Result<(), GateErr> {
        if dest.is_file() && hash_ok(dest, sha) {
            return Ok(());
        }
        if dest.is_file() {
            let _ = std::fs::remove_file(dest);
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|err| GateErr::fail(err.to_string()))?;
        }
        let part = dest.with_extension("part");
        let status = Command::new("curl")
            .args([
                "-fL",
                "--retry",
                "3",
                "--retry-delay",
                "2",
                "-A",
                "galion-horbm-eval",
                "-o",
            ])
            .arg(&part)
            .arg(url)
            .status()
            .map_err(|err| GateErr::skip(format!("skip: curl is not available ({err})")))?;
        if !status.success() {
            let _ = std::fs::remove_file(&part);
            return Err(GateErr::skip(format!("skip: download failed for {url}")));
        }
        if !hash_ok(&part, sha) {
            let _ = std::fs::remove_file(&part);
            return Err(GateErr::fail(format!(
                "sha256 mismatch for {url}; the partial file was deleted"
            )));
        }
        std::fs::rename(&part, dest).map_err(|err| GateErr::fail(err.to_string()))?;
        Ok(())
    }

    fn hash_ok(path: &Path, sha: &str) -> bool {
        sha256::file_hex(path)
            .map(|hex| hex.eq_ignore_ascii_case(sha))
            .unwrap_or(false)
    }

    fn unzip(archive: &Path, dest: &Path) -> Result<(), GateErr> {
        std::fs::create_dir_all(dest).map_err(|err| GateErr::fail(err.to_string()))?;
        let status = Command::new("unzip")
            .args(["-q", "-o"])
            .arg(archive)
            .arg("-d")
            .arg(dest)
            .status()
            .map_err(|err| GateErr::skip(format!("skip: unzip is not available ({err})")))?;
        if status.success() {
            Ok(())
        } else {
            Err(GateErr::fail("unzip failed"))
        }
    }

    fn untar(archive: &Path, dest: &Path) -> Result<(), GateErr> {
        std::fs::create_dir_all(dest).map_err(|err| GateErr::fail(err.to_string()))?;
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .status()
            .map_err(|err| GateErr::skip(format!("skip: tar is not available ({err})")))?;
        if status.success() {
            Ok(())
        } else {
            Err(GateErr::fail("tar extract failed"))
        }
    }

    /// Keyword spotter list. Backchannels, `stop`, and single affect words.
    /// `yep` and `yea` are omitted: on this Piper voice they outrank `yeah`.
    /// `[unk]` lets the spotter ignore the rest of a sentence. No scripted
    /// line and no held-out line is included.
    fn keyword_list() -> Vec<String> {
        let mut phrases = Vec::new();
        for phrase in listed_backchannels() {
            if *phrase == "yep" || *phrase == "yea" {
                continue;
            }
            phrases.push((*phrase).to_string());
        }
        phrases.push("stop".to_string());
        for word in AFFECT_WORDS {
            phrases.push((*word).to_string());
        }
        phrases.push("[unk]".to_string());
        phrases.sort_unstable();
        phrases.dedup();
        phrases
    }

    fn is_keyword(word: &str) -> bool {
        word != "[unk]" && word != "<unk>" && keyword_list().iter().any(|item| item == word)
    }

    /// Free transcript, with a keyword written over one token when the
    /// spotter localises it. A one-word free result that is not itself a
    /// keyword can be replaced by the single spotted keyword (`that` →
    /// `yeah`). A longer result changes a token only when the keyword is
    /// confident, short, overlaps that token, and is one edit away
    /// (`said` → `sad`). Partials from the free pass are not rewritten.
    fn merge_keyword(free: &[Word], spot: &[Word], free_text: &str) -> String {
        let mut words: Vec<Word> = free
            .iter()
            .filter(|word| word.word != "[unk]" && word.word != "<unk>")
            .cloned()
            .collect();
        if words.is_empty() {
            return free_text.to_string();
        }
        let spots: Vec<&Word> = spot.iter().filter(|word| is_keyword(&word.word)).collect();
        if words.len() == 1 && spots.len() == 1 && spots[0].conf >= 0.5 {
            let free_word = words[0].word.as_str();
            if !is_keyword(free_word) || token_distance(free_word, &spots[0].word) <= 1 {
                words[0].word = spots[0].word.clone();
            }
            return join_words(&words);
        }
        for kw in spots {
            if kw.conf < 0.9 || kw.end - kw.start > 0.75 {
                continue;
            }
            let mut best: Option<(usize, f32)> = None;
            for (index, word) in words.iter().enumerate() {
                let start = word.start.max(kw.start);
                let end = word.end.min(kw.end);
                let overlap = (end - start).max(0.0);
                let span = (word.end - word.start).max(0.05);
                if overlap / span < 0.5 {
                    continue;
                }
                if best.map(|(_, have)| overlap > have).unwrap_or(true) {
                    best = Some((index, overlap));
                }
            }
            if let Some((index, _)) = best {
                if words[index].word != kw.word && token_distance(&words[index].word, &kw.word) <= 1
                {
                    words[index].word = kw.word.clone();
                }
            }
        }
        join_words(&words)
    }

    fn join_words(words: &[Word]) -> String {
        words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn token_distance(left: &str, right: &str) -> usize {
        let left: Vec<char> = left.chars().collect();
        let right: Vec<char> = right.chars().collect();
        let mut prev: Vec<usize> = (0..=right.len()).collect();
        let mut cur = vec![0; right.len() + 1];
        for (i, ca) in left.iter().enumerate() {
            cur[0] = i + 1;
            for (j, cb) in right.iter().enumerate() {
                let cost = usize::from(ca != cb);
                cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        prev[right.len()]
    }

    fn pair_rows_of(pairs: &[(String, String)]) -> Vec<serde_json::Value> {
        pairs
            .iter()
            .map(|(reference, hypothesis)| {
                serde_json::json!({
                    "ref": reference,
                    "hyp": hypothesis,
                    "wer": mean_wer(&[(reference.clone(), hypothesis.clone())]),
                })
            })
            .collect()
    }

    #[cfg(test)]
    fn spoken_phrase(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|ch| {
                if ch.is_ascii_lowercase()
                    || ch.is_ascii_digit()
                    || ch == '\''
                    || ch == '-'
                    || ch == ' '
                {
                    ch
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn heard_word(text: &str, word: &str) -> bool {
        wer_tokens(text).iter().any(|token| token == word)
    }

    fn script_timing(decode_ms: &[u32], audio_ms: &[u32]) -> (f64, u32, u32) {
        let audio: u64 = audio_ms.iter().map(|ms| u64::from(*ms)).sum();
        let wall: u64 = decode_ms.iter().map(|ms| u64::from(*ms)).sum();
        let rtf = if audio == 0 {
            0.0
        } else {
            wall as f64 / audio as f64
        };
        (rtf, percentile(decode_ms, 50), percentile(decode_ms, 95))
    }

    /// Linux only. The worker is a child process; its resident set is not
    /// this process. `comm` holds at most 15 bytes, and `galion-asr-vosk`
    /// is exactly that long.
    fn worker_mem_kb() -> (u64, u64) {
        let mut rss = 0u64;
        let mut hwm = 0u64;
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return (0, 0);
        };
        for entry in entries.flatten() {
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            if comm.trim() != "galion-asr-vosk" {
                continue;
            }
            let status = std::fs::read_to_string(entry.path().join("status")).unwrap_or_default();
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    rss = rss.max(leading_kb(rest));
                } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                    hwm = hwm.max(leading_kb(rest));
                }
            }
        }
        (rss, hwm)
    }

    fn leading_kb(rest: &str) -> u64 {
        rest.split_whitespace()
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    fn dir_bytes(path: &Path) -> u64 {
        let mut sum = 0u64;
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    stack.push(entry.path());
                } else if let Ok(meta) = entry.metadata() {
                    sum = sum.saturating_add(meta.len());
                }
            }
        }
        sum
    }

    #[cfg(test)]
    mod tests {
        use super::{keyword_list, merge_keyword, spoken_phrase, AFFECT_WORDS, HELD_OUT, SCRIPT};
        use galion_asr_vosk::Word;

        fn word(text: &str, start: f32, end: f32, conf: f32) -> Word {
            Word {
                word: text.to_string(),
                start,
                end,
                conf,
            }
        }

        #[test]
        fn the_keyword_list_is_not_the_script_or_the_held_out_set() {
            let phrases = keyword_list();
            assert!(phrases.iter().any(|item| item == "yeah"));
            assert!(phrases.iter().any(|item| item == "stop"));
            assert!(phrases.iter().any(|item| item == "sad"));
            assert!(!phrases.iter().any(|item| item == "yep"));
            assert!(!phrases.iter().any(|item| item == "yea"));
            assert!(!phrases.iter().any(|item| item == "i am sad"));
            for (line, _) in SCRIPT {
                let phrase = spoken_phrase(line);
                assert!(
                    !phrases.iter().any(|item| item == &phrase),
                    "script line {phrase} is in the keyword list"
                );
            }
            assert!(HELD_OUT.len() >= 20);
            for line in HELD_OUT {
                let phrase = spoken_phrase(line);
                assert!(
                    !phrases.iter().any(|item| item == &phrase),
                    "held-out line {phrase} is in the keyword list"
                );
                assert!(phrase.split_whitespace().count() > 1, "{phrase}");
            }
            for word in AFFECT_WORDS {
                assert!(phrases.iter().any(|item| item == word), "{word}");
                assert!(!word.contains(' '));
            }
        }

        #[test]
        fn a_keyword_rewrites_said_and_yeah_and_leaves_other_words() {
            let said = merge_keyword(
                &[
                    word("i", 0.09, 0.30, 1.0),
                    word("am", 0.30, 0.42, 1.0),
                    word("said", 0.42, 0.96, 0.77),
                ],
                &[word("hm", 0.09, 0.39, 0.78), word("sad", 0.40, 0.96, 1.0)],
                "i am said",
            );
            assert_eq!(said, "i am sad");
            let yeah = merge_keyword(
                &[word("that", 0.0, 0.39, 0.57)],
                &[word("yeah", 0.09, 0.39, 0.58)],
                "that",
            );
            assert_eq!(yeah, "yeah");
            let today = merge_keyword(
                &[
                    word("i", 0.09, 0.27, 1.0),
                    word("feel", 0.27, 0.60, 1.0),
                    word("sad", 0.60, 0.96, 0.76),
                    word("today", 0.96, 1.50, 1.0),
                ],
                &[word("sad", 0.63, 0.96, 1.0), word("da", 0.99, 1.50, 1.0)],
                "i feel sad today",
            );
            assert_eq!(today, "i feel sad today");
            let happy = merge_keyword(
                &[
                    word("i", 0.09, 0.30, 0.89),
                    word("am", 0.30, 0.42, 0.89),
                    word("happy", 0.42, 0.99, 1.0),
                ],
                &[word("hm", 0.09, 0.42, 1.0), word("happy", 0.42, 0.99, 1.0)],
                "i am happy",
            );
            assert_eq!(happy, "i am happy");
        }
    }
}
