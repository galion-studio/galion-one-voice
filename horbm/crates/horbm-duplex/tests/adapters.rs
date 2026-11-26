// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Each real adapter on synthetic signals. No recording and no model file.
//!
//! The virtual-clock gate stays in `duplex.rs`. These tests are the port
//! contract: the same traits, on generated frames.

use galion_emotion_core::{IntentMsg, AROUSAL};
use horbm::{CardHit, CardLookup, Pad};
use horbm_duplex::{
    open_persona, Breath, BreathPhase, ConfiguredPersona, CoreAffect, Duplex, EarSense, Emotion,
    FramePump, LoopError, NativeMouth, Parts, Persona, ProsodyEar, Recognizer, SilentRecognizer,
    SpeakingBreath, FRAME_SAMPLES, SAMPLE_RATE,
};
use horbm_voice::{Mouth, MouthPull, SpokenLine, VoiceControls};

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }

    fn len(&self) -> usize {
        0
    }
}

fn sine(hz: f32, amp: f32) -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        let t = index as f32 / SAMPLE_RATE as f32;
        let x = (t * hz * std::f32::consts::TAU).sin() * amp;
        *sample = x as i16;
    }
    pcm
}

fn seated() -> (CoreAffect, ConfiguredPersona) {
    let persona = horbm::Persona::load_development().expect("configured persona");
    let affect = CoreAffect::from_baseline(persona.baseline());
    (affect, ConfiguredPersona::from_persona(persona))
}

fn harness() -> Duplex<SilentRecognizer, CoreAffect, SpeakingBreath, ConfiguredPersona, NativeMouth>
{
    let (emotion, persona) = seated();
    let mut duplex = Duplex::new(
        Parts {
            recognizer: SilentRecognizer::new(),
            emotion,
            breath: SpeakingBreath,
            persona,
            mouth: NativeMouth::new(),
        },
        NoCards,
    )
    .expect("configured persona");
    duplex.set_aec(false);
    duplex
}

#[test]
fn emotion_core_satisfies_the_port_and_leaves_the_baseline() {
    let (mut emotion, persona) = seated();
    let base = emotion.baseline();
    assert_eq!(base.persona_id(), persona.id());
    assert_eq!(base.voice_id(), persona.voice());
    let bits = base.identity_bits();
    let rejected = emotion.step(80);
    assert_eq!(emotion.frames(), 0);
    assert_eq!(rejected.valence.to_bits(), base.valence().to_bits());
    assert_eq!(rejected.arousal.to_bits(), base.arousal().to_bits());
    assert_eq!(rejected.dominance.to_bits(), base.dominance().to_bits());
    let first = emotion.step(0);
    assert_eq!(emotion.frames(), 1);
    assert_eq!(first.valence.to_bits(), base.valence().to_bits());
    assert_eq!(first.arousal.to_bits(), base.arousal().to_bits());
    assert_eq!(first.dominance.to_bits(), base.dominance().to_bits());
    let stuck = emotion.step(0);
    assert_eq!(emotion.frames(), 1);
    assert_eq!(stuck, first);
    let mut intent = IntentMsg::steady(80);
    intent.strength = 1.0;
    intent.set_pull(AROUSAL, 0.5).expect("pull");
    let moved = emotion.step_intent(&intent);
    assert!(
        moved.arousal > first.arousal,
        "arousal {} did not leave {}",
        moved.arousal,
        first.arousal
    );
    assert!(moved.valence.is_finite() && moved.dominance.is_finite());
    assert_eq!(emotion.frames(), 2);
    assert_eq!(emotion.baseline().identity_bits(), bits);
    let view = emotion.view();
    assert_eq!(view.baseline.persona_id(), persona.id());
}

#[test]
fn the_tick_hands_the_configured_moment_to_the_mouth() {
    struct Remember {
        inner: NativeMouth,
        felt: u32,
        parked: bool,
    }

    impl Mouth for Remember {
        fn id(&self) -> &str {
            "remember"
        }

        fn start(&mut self, line: &SpokenLine) {
            self.inner.start(line);
        }

        fn pull(&mut self) -> MouthPull {
            self.inner.pull()
        }

        fn cancel(&mut self) {
            self.inner.cancel();
        }

        fn speaking(&self) -> bool {
            self.inner.speaking()
        }

        fn feel(&mut self, controls: VoiceControls) {
            self.felt = self.felt.saturating_add(1);
            self.parked = controls.parked;
        }
    }

    let (emotion, persona) = seated();
    let mut duplex = Duplex::new(
        Parts {
            recognizer: SilentRecognizer::new(),
            emotion,
            breath: SpeakingBreath,
            persona,
            mouth: Remember {
                inner: NativeMouth::new(),
                felt: 0,
                parked: true,
            },
        },
        NoCards,
    )
    .expect("configured persona");
    duplex
        .tick(&vec![0i16; FRAME_SAMPLES])
        .expect("one quiet frame");
    assert_eq!(duplex.mouth().felt, 1);
    assert!(!duplex.mouth().parked);
}

#[test]
fn the_configured_persona_loads_and_any_other_id_is_refused() {
    let configured = horbm::Persona::load_development().expect("configured persona");
    let persona = open_persona(configured.id()).expect("open");
    assert_eq!(persona.id(), configured.id());
    assert_eq!(persona.voice(), configured.voice());
    assert_eq!(persona.unknown_line(), horbm::UNKNOWN_REPLY);
    let style = persona.style();
    let hook = configured.style();
    assert!((style.brevity - hook.brevity).abs() < 1.0e-5);
    assert!((style.warmth - hook.warmth).abs() < 1.0e-5);
    assert_eq!(style.max_sentences, hook.max_sentences);
    assert!((style.rate - hook.rate).abs() < 1.0e-5);
    let missed = open_persona("other").expect_err("other id");
    let also = open_persona("jarvis").expect_err("jarvis");
    assert_eq!(
        missed,
        LoopError::Persona("the id is not the configured persona")
    );
    assert_eq!(also, missed);
}

#[test]
fn the_native_mouth_satisfies_the_port_on_a_generated_tone() {
    let mut mouth = NativeMouth::new();
    assert_eq!(mouth.id(), "native");
    mouth.start(&SpokenLine {
        text: "The kettle is on the hob.".into(),
        mood: Pad::RESTING,
        rate: 0.9175,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
    });
    assert!(mouth.speaking());
    match mouth.pull() {
        MouthPull::Frame(frame) => {
            assert_eq!(frame.samples.len(), FRAME_SAMPLES);
            assert_eq!(frame.sample_rate, SAMPLE_RATE);
            assert!(frame.first);
            assert!(frame.samples.iter().any(|sample| *sample != 0));
            assert_eq!(mouth.last(), frame.samples.as_slice());
        }
        MouthPull::Idle | MouthPull::Done => panic!("the first pull was empty"),
    }
    mouth.cancel();
    assert!(!mouth.speaking());
    assert!(mouth.last().is_empty());
    match mouth.pull() {
        MouthPull::Done | MouthPull::Idle => {}
        MouthPull::Frame(_) => panic!("cancel still had samples"),
    }
}

#[test]
fn prosody_hears_a_synthetic_sine_and_the_off_switch_is_explicit() {
    let mut sense = ProsodyEar::new();
    if !ProsodyEar::compiled() {
        assert!(!sense.enabled());
        sense.set_enabled(true);
        assert!(!sense.enabled());
        return;
    }
    assert!(sense.enabled());
    let tone = sine(200.0, 8000.0);
    let cue = sense.hear(&tone);
    assert!(cue.enabled);
    assert!(cue.voiced, "rms {}", cue.rms_db);
    assert!(
        (cue.f0_hz - 200.0).abs() < 8.0,
        "f0 {} rms {}",
        cue.f0_hz,
        cue.rms_db
    );
    assert_eq!(sense.hear(&tone), cue);
    let quiet = sense.hear(&vec![0i16; FRAME_SAMPLES]);
    assert!(!quiet.voiced);
    assert_eq!(quiet.f0_hz, 0.0);
    let tiny = sense.hear(&vec![1i16; FRAME_SAMPLES]);
    assert!(
        !tiny.voiced,
        "a unit sample is under the clamp, rms {}",
        tiny.rms_db
    );

    let mut duplex = harness();
    assert!(duplex.prosody_enabled());
    let heard = duplex.tick(&tone).expect("loud frame");
    assert!(heard.you_voiced);
    assert!(heard.you_samples == FRAME_SAMPLES);
    let mut whisper = vec![0i16; FRAME_SAMPLES];
    whisper[0] = 1;
    let ignored = duplex.tick(&whisper).expect("quiet frame");
    assert!(!ignored.you_voiced);
    duplex.set_prosody(false);
    assert!(!duplex.prosody_enabled());
    let restored = duplex.tick(&whisper).expect("nonzero frame");
    assert!(restored.you_voiced);
}

#[test]
fn a_silent_recognizer_does_not_invent_a_line() {
    let mut ear = SilentRecognizer::new();
    let loud = sine(180.0, 6000.0);
    let heard = ear.push_frame(&loud).expect("frame");
    assert!(heard.is_empty());
    assert_eq!(ear.audio_ms(), 80);
    assert_eq!(ear.name(), "stand-in");
    ear.cancel().expect("cancel");
    assert_eq!(ear.audio_ms(), 0);
    assert!(ear.finish().expect("finish").is_empty());
    let err = ear.push_frame(&[0i16; 10]).expect_err("short");
    let text = err.to_string();
    assert!(text.contains("10"));
    assert!(!text.to_ascii_lowercase().contains("kettle"));
}

#[test]
fn four_capture_slices_are_one_duplex_frame() {
    let mut pump = FramePump::new();
    assert_eq!(FRAME_SAMPLES, 320 * 4);
    let slice = vec![7i16; 320];
    assert!(pump.push(&slice).is_empty());
    assert!(pump.push(&slice).is_empty());
    assert!(pump.push(&slice).is_empty());
    assert_eq!(pump.pending(), 960);
    let frames = pump.push(&slice);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].len(), FRAME_SAMPLES);
    assert!(frames[0].iter().all(|sample| *sample == 7));
    assert_eq!(pump.pending(), 0);
    assert!(pump.push(&[1i16; 100]).is_empty());
    assert_eq!(pump.pending(), 100);
}

#[test]
fn the_breath_stand_in_exhales_only_while_speaking() {
    let mut breath = SpeakingBreath;
    let speech = breath.cue(0, true);
    assert_eq!(speech.phase, BreathPhase::Exhale);
    assert!(speech.speaking);
    let rest = breath.cue(80, false);
    assert_eq!(rest.phase, BreathPhase::Inhale);
    assert!(!rest.speaking);
}

#[cfg(feature = "vosk")]
mod vosk_port {
    use super::*;
    use galion_sam_asr::{Asr, AsrError, AsrEvent, AsrKind, FakeAsr, VoskConfig, Word};
    use horbm_duplex::{AsrRecognizer, HypothesisKind, RecogError, Recognizer};

    #[test]
    fn fake_asr_satisfies_the_recognizer_contract_on_synthetic_frames() {
        let mut left = AsrRecognizer::new(FakeAsr::new("where is the kettle"));
        let mut right = AsrRecognizer::new(FakeAsr::new("where is the kettle"));
        let loud = sine(220.0, 4000.0);
        let quiet = vec![0i16; FRAME_SAMPLES];
        let a = left.push_frame(&loud).expect("loud");
        let b = right.push_frame(&quiet).expect("quiet");
        assert_eq!(a, b);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].kind, HypothesisKind::Partial);
        assert_eq!(a[0].text, "where");
        assert_eq!(a[0].end_ms, 80);
        assert_eq!(left.name(), "fake");
        let err = left.push_frame(&[0i16; 320]).expect_err("short");
        assert_eq!(
            err,
            RecogError::Frame {
                got: 320,
                expected: FRAME_SAMPLES,
            }
        );
        assert_eq!(left.audio_ms(), 80);
        for _ in 0..3 {
            left.push_frame(&quiet).expect("word");
        }
        let done = left.finish().expect("final");
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].kind, HypothesisKind::Final);
        assert_eq!(done[0].text, "where is the kettle");
        assert_eq!(done[0].end_ms, 320);
        assert_eq!(left.audio_ms(), 0);
        assert!(left.finish().expect("idle").is_empty());
    }

    #[test]
    fn cancel_drops_the_open_hypothesis_and_an_empty_final_is_not_a_line() {
        let mut ear = AsrRecognizer::new(FakeAsr::new("where is the kettle"));
        let frame = vec![0i16; FRAME_SAMPLES];
        ear.push_frame(&frame).expect("partial");
        ear.push_frame(&frame).expect("partial");
        ear.cancel().expect("cancel");
        assert_eq!(ear.audio_ms(), 0);
        assert!(ear.finish().expect("empty").is_empty());

        let mut blank = AsrRecognizer::new(BlankFinal);
        let out = blank.push_frame(&frame).expect("blank");
        assert!(out.is_empty());
        assert_eq!(blank.audio_ms(), 0);
    }

    #[test]
    fn an_engine_error_is_not_spoken_and_word_times_are_kept() {
        let mut boom = AsrRecognizer::new(Boom);
        match boom.push_frame(&vec![0i16; FRAME_SAMPLES]) {
            Ok(_) => panic!("the failure was a hypothesis"),
            Err(RecogError::Engine(message)) => {
                assert!(message.contains("decoder"));
                assert!(!message.to_ascii_lowercase().contains("kettle"));
            }
            Err(other) => panic!("{other}"),
        }

        let mut words = AsrRecognizer::new(WordsOnce::default());
        let done = words.push_frame(&vec![0i16; FRAME_SAMPLES]).expect("word");
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].kind, HypothesisKind::Final);
        assert_eq!(done[0].text, "hello");
        assert_eq!(done[0].start_ms, 120);
        assert_eq!(done[0].end_ms, 450);
        assert_eq!(words.audio_ms(), 0);
    }

    #[test]
    fn a_missing_model_is_an_error_and_not_a_transcript() {
        let missing = std::env::temp_dir().join("horbm-duplex-no-such-vosk-model");
        let opened = AsrRecognizer::open(VoskConfig {
            model_dir: missing,
            lib_path: None,
            threads: 1,
        });
        let err = match opened {
            Ok(_) => panic!("a missing model opened"),
            Err(err) => err,
        };
        match err {
            RecogError::Engine(message) => {
                assert!(message.contains("No transcript was invented"));
            }
            other => panic!("{other}"),
        }
    }

    struct BlankFinal;

    impl Asr for BlankFinal {
        fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            if pcm.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![AsrEvent {
                kind: AsrKind::Final,
                text: "   ".into(),
                audio_ms: 80,
                words: Vec::new(),
                why: "finish",
            }])
        }

        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }

        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }

        fn name(&self) -> &str {
            "blank"
        }
    }

    struct Boom;

    impl Asr for Boom {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            Err(AsrError::new("decoder failed"))
        }

        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Err(AsrError::new("decoder failed"))
        }

        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }

        fn name(&self) -> &str {
            "boom"
        }
    }

    #[derive(Default)]
    struct WordsOnce {
        done: bool,
    }

    impl Asr for WordsOnce {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            if self.done {
                return Ok(Vec::new());
            }
            self.done = true;
            Ok(vec![AsrEvent {
                kind: AsrKind::Final,
                text: "hello".into(),
                audio_ms: 80,
                words: vec![Word {
                    word: "hello".into(),
                    start: 0.12,
                    end: 0.45,
                    conf: 1.0,
                }],
                why: "finish",
            }])
        }

        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }

        fn reset(&mut self) -> Result<(), AsrError> {
            self.done = false;
            Ok(())
        }

        fn name(&self) -> &str {
            "words"
        }
    }
}
