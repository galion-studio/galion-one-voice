// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One closed run. Scripted 16 kHz frames, no file, no weight, no socket.
//!
//! The stand-ins below are the recognizer, emotion, breath, and persona
//! ports. PR 279, PR 263, PR 278, and PR 276 replace them later.

use std::collections::VecDeque;

use horbm::{CardHit, CardLookup};
use horbm_duplex::{
    Affect, Breath, BreathCue, BreathPhase, Budgets, Duplex, Emotion, Floor, FrameOut, Hypothesis,
    HypothesisKind, LoopError, LoopEvent, Parts, Persona, RecogError, Recognizer, Style,
    DEV_PERSONA, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE,
};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine};

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        1
    }
}

struct Scripted {
    lines: VecDeque<Vec<String>>,
    words: Vec<String>,
    shown: usize,
    frames: u32,
    open: bool,
}

impl Scripted {
    fn new(lines: &[&str]) -> Self {
        Self {
            lines: lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect(),
            words: Vec::new(),
            shown: 0,
            frames: 0,
            open: false,
        }
    }

    fn snapshot(&self, kind: HypothesisKind) -> Hypothesis {
        let shown = &self.words[..self.shown];
        Hypothesis {
            kind,
            text: shown.join(" "),
            start_ms: 0,
            end_ms: self.audio_ms(),
        }
    }
}

impl Recognizer for Scripted {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        if !self.open {
            self.words = self.lines.pop_front().unwrap_or_default();
            self.shown = 0;
            self.frames = 0;
            self.open = true;
        }
        self.frames = self.frames.saturating_add(1);
        if self.shown < self.words.len() {
            self.shown += 1;
            return Ok(vec![self.snapshot(HypothesisKind::Partial)]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        if !self.open || self.shown == 0 {
            self.open = false;
            self.shown = 0;
            self.frames = 0;
            return Ok(Vec::new());
        }
        let done = self.snapshot(HypothesisKind::Final);
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        Ok(vec![done])
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.words.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "stand-in"
    }
}

struct SamanthaAffect;

impl Emotion for SamanthaAffect {
    fn step(&mut self, _t_ms: u32) -> Affect {
        Affect {
            valence: 0.28,
            arousal: 0.36,
            dominance: 0.44,
        }
    }
}

struct ExhaleBreath;

impl Breath for ExhaleBreath {
    fn cue(&mut self, _t_ms: u32, speaking: bool) -> BreathCue {
        if speaking {
            BreathCue {
                phase: BreathPhase::Exhale,
                speaking: true,
            }
        } else {
            BreathCue {
                phase: BreathPhase::Inhale,
                speaking: false,
            }
        }
    }
}

struct Samantha;

impl Persona for Samantha {
    fn id(&self) -> &str {
        DEV_PERSONA
    }

    fn voice(&self) -> &str {
        DEV_PERSONA
    }

    fn style(&self) -> Style {
        Style::from_brevity(0.55, 0.8, 2)
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

struct Aria;

impl Persona for Aria {
    fn id(&self) -> &str {
        "aria"
    }

    fn voice(&self) -> &str {
        "aria"
    }

    fn style(&self) -> Style {
        Style::from_brevity(0.4, 0.7, 2)
    }

    fn unknown_line(&self) -> &str {
        horbm::UNKNOWN_REPLY
    }
}

struct ToneMouth {
    span: u32,
    left: u32,
    tone: i16,
    pending_first: bool,
    spoken: Vec<String>,
    rates: Vec<f32>,
    cancels: u32,
}

impl ToneMouth {
    fn new(span: u32) -> Self {
        Self {
            span,
            left: 0,
            tone: 0,
            pending_first: false,
            spoken: Vec::new(),
            rates: Vec::new(),
            cancels: 0,
        }
    }

    fn spoken(&self) -> &[String] {
        &self.spoken
    }

    fn rates(&self) -> &[f32] {
        &self.rates
    }

    fn cancels(&self) -> u32 {
        self.cancels
    }
}

impl Mouth for ToneMouth {
    fn id(&self) -> &str {
        "tone"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.spoken.push(line.text.clone());
        self.rates.push(line.rate);
        if line.text.trim().is_empty() {
            self.left = 0;
            self.pending_first = false;
            return;
        }
        self.tone = 400;
        self.left = self.span;
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let mut samples = vec![0i16; FRAME_SAMPLES];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = if index % 2 == 0 {
                self.tone
            } else {
                -self.tone
            };
        }
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        if self.left > 0 {
            self.cancels = self.cancels.saturating_add(1);
        }
        self.left = 0;
        self.pending_first = false;
    }

    fn speaking(&self) -> bool {
        self.left > 0
    }
}

fn tone() -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        *sample = if index % 2 == 0 { 800 } else { -800 };
    }
    pcm
}

fn silence() -> Vec<i16> {
    vec![0i16; FRAME_SAMPLES]
}

fn samantha() -> Duplex<Scripted, SamanthaAffect, ExhaleBreath, Samantha, ToneMouth> {
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Scripted::new(&["where is the kettle", "yeah", "stop now"]),
            emotion: SamanthaAffect,
            breath: ExhaleBreath,
            persona: Samantha,
            mouth: ToneMouth::new(24),
        },
        Kettle,
    )
    .expect("samantha");
    // The virtual-clock gate is not an acoustic path. The mouth tone and the
    // scripted mic are the same square, so a live canceller would treat the
    // person as echo.
    duplex.set_aec(false);
    duplex
}

fn tick(
    duplex: &mut Duplex<Scripted, SamanthaAffect, ExhaleBreath, Samantha, ToneMouth>,
    pcm: &[i16],
) -> FrameOut {
    duplex.tick(pcm).expect("frame")
}

#[test]
fn full_duplex_samantha_both_bands() {
    let mut duplex = samantha();
    assert_eq!(duplex.persona().id(), DEV_PERSONA);
    assert_eq!(duplex.persona().voice(), DEV_PERSONA);
    let cap = Budgets::locked();

    for index in 0..4 {
        let frame = tick(&mut duplex, &tone());
        assert_eq!(frame.you_samples, FRAME_SAMPLES, "word {index}");
        assert!(frame.you_voiced);
        assert!(!frame.me_audible);
        assert!(frame.side_idle);
        assert!(!frame.research_open);
    }
    let hold = tick(&mut duplex, &tone());
    assert_eq!(hold.event, LoopEvent::None);
    assert_eq!(hold.partial, "where is the kettle");

    let eager = tick(&mut duplex, &silence());
    assert_eq!(eager.event, LoopEvent::EagerEndOfTurn);
    assert!(eager.reaction);
    assert_eq!(eager.reaction_peak, 0);
    assert_eq!(eager.t_ms - hold.t_ms, u64::from(FRAME_MS));
    assert_eq!(eager.affect.valence, 0.28);
    assert_eq!(eager.affect.arousal, 0.36);
    assert_eq!(eager.affect.dominance, 0.44);
    assert!(eager.side_idle);
    assert!(!eager.research_open);

    let commit = tick(&mut duplex, &silence());
    assert_eq!(commit.event, LoopEvent::Committed);
    assert_eq!(commit.floor, Floor::Speaking);
    assert!(!commit.me_audible, "the commit frame arms the mouth");
    assert_eq!(commit.t_ms - eager.t_ms, u64::from(FRAME_MS));
    assert_eq!(
        duplex.mouth().spoken(),
        &["The kettle is on the hob.".to_string()]
    );
    let rate = duplex.mouth().rates()[0];
    let expect = Style::from_brevity(0.55, 0.8, 2).rate;
    assert!((rate - expect).abs() < 1.0e-5);

    let audio = tick(&mut duplex, &silence());
    assert_eq!(audio.event, LoopEvent::FirstAudio);
    assert!(audio.me_first);
    assert!(audio.me_audible);
    assert_eq!(audio.me_samples, FRAME_SAMPLES);
    assert_eq!(audio.breath.phase, BreathPhase::Exhale);
    assert!(audio.breath.speaking);
    assert_eq!(audio.t_ms - eager.t_ms, 160);
    assert!(audio.t_ms - eager.t_ms <= u64::from(cap.voice_p50_ms));
    let epoch = audio.epoch;

    for index in 0..6 {
        let frame = tick(&mut duplex, &tone());
        assert_eq!(frame.event, LoopEvent::Backchannel, "yeah {index}");
        assert!(!frame.cancel);
        assert!(!frame.me_silent);
        assert!(frame.me_audible);
        assert!(frame.you_voiced);
        assert_eq!(frame.epoch, epoch);
        assert_eq!(frame.partial, "yeah");
    }
    assert_eq!(duplex.mouth().cancels(), 0);

    let between = tick(&mut duplex, &silence());
    assert!(!between.cancel);
    assert!(between.me_audible);
    assert_eq!(between.epoch, epoch);
    assert_ne!(between.event, LoopEvent::BargeIn);

    let onset = tick(&mut duplex, &tone());
    assert_eq!(onset.partial, "stop");
    assert!(onset.me_audible, "the arming frame still plays");
    assert!(!onset.cancel);
    assert_ne!(onset.event, LoopEvent::Backchannel);

    let arm = tick(&mut duplex, &tone());
    assert_eq!(arm.partial, "stop now");
    assert!(arm.me_audible);
    assert!(!arm.me_silent);

    let stop = tick(&mut duplex, &tone());
    assert_eq!(stop.event, LoopEvent::BargeIn);
    assert!(stop.cancel);
    assert!(stop.me_silent);
    assert!(!stop.me_audible);
    assert!(stop.you_voiced);
    assert_eq!(stop.you_samples, FRAME_SAMPLES);
    assert_eq!(stop.floor, Floor::Paused);
    assert!(stop.epoch > epoch);
    let barge_ms = stop.t_ms - onset.t_ms;
    assert_eq!(barge_ms, 160);
    assert!(barge_ms <= u64::from(cap.barge_ms));
    assert!(barge_ms <= u64::from(cap.barge_lock_ms));
    assert_eq!(duplex.mouth().cancels(), 1);

    for _ in 0..2 {
        let later = tick(&mut duplex, &silence());
        assert!(!later.me_audible);
        assert!(later.yielded);
        assert_eq!(later.you_samples, FRAME_SAMPLES);
        assert!(!later.research_open);
    }

    let report = duplex.report();
    let line = report.gate_line();
    assert!(!line.to_ascii_lowercase().contains("kettle"));
    assert!(!line.contains("hob"));
    println!("{line}");
    let misses = report.budget_misses();
    assert!(misses.is_empty(), "{line}\n{}", misses.join("\n"));
    assert_eq!(report.ear_frames, cap.ear_frames);
    assert_eq!(report.recognizer_frames, Some(0));
    assert_eq!(report.brain_frames, Some(0));
    assert!(report.brain_filled_before_eager);
    assert!(report.brain_read_on_eager);
    assert_eq!(report.conductor_frames, Some(1));
    assert_eq!(report.commit_frames, Some(1));
    assert_eq!(report.reaction_frames, Some(0));
    assert_eq!(report.voice_ms, Some(160));
    assert_eq!(report.affect_frames, Some(2));
    assert_eq!(report.barge_frames, Some(2));
    assert_eq!(report.barge_ms, Some(160));
    assert!(report.barge_while_you_live);
    assert_eq!(report.first_clause_words, Some(2));
    assert_eq!(report.backchannel_frames, 6);
    assert_eq!(report.backchannel_cancels, 0);
    assert!(report.overlap_live_frames >= 6);
    assert_eq!(report.research_frames, 0);
    let voice = report.turn_text("voice");
    let barge = report.turn_text("barge");
    assert_eq!(
        voice,
        "duplex s28 event=voice clock=frame eager_to_voice_ms=160 barge_to_silence_ms=160 backchannel_cancels=0"
    );
    assert_eq!(
        barge,
        "duplex s28 event=barge clock=frame eager_to_voice_ms=160 barge_to_silence_ms=160 backchannel_cancels=0"
    );
    assert!(!voice.to_ascii_lowercase().contains("kettle"));
    assert!(report
        .turn_json("voice")
        .contains("\"backchannel_cancels\":0"));
}

#[test]
fn a_short_frame_does_not_move_the_clock() {
    let mut duplex = samantha();
    let err = match duplex.tick(&[0i16; 10]) {
        Ok(_) => panic!("a short frame was accepted"),
        Err(err) => err,
    };
    assert_eq!(
        err,
        LoopError::Frame {
            got: 10,
            expected: FRAME_SAMPLES,
        }
    );
    assert_eq!(duplex.t_ms(), 0);
    let frame = tick(&mut duplex, &silence());
    assert_eq!(frame.t_ms, 0);
    assert_eq!(duplex.t_ms(), u64::from(FRAME_MS));
}

#[test]
fn a_persona_other_than_samantha_is_refused() {
    match Duplex::new(
        Parts {
            recognizer: Scripted::new(&[]),
            emotion: SamanthaAffect,
            breath: ExhaleBreath,
            persona: Aria,
            mouth: ToneMouth::new(4),
        },
        Kettle,
    ) {
        Ok(_) => panic!("aria was accepted"),
        Err(err) => assert!(matches!(err, LoopError::Persona(_))),
    }
}

#[test]
fn stand_in_cancel_drops_the_open_hypothesis() {
    let mut ear = Scripted::new(&["where is the kettle"]);
    let frame = silence();
    let first = ear.push_frame(&frame).expect("partial");
    assert_eq!(first[0].kind, HypothesisKind::Partial);
    assert_eq!(first[0].text, "where");
    ear.cancel().expect("cancel");
    assert_eq!(ear.audio_ms(), 0);
    assert!(ear.finish().expect("empty").is_empty());
}
