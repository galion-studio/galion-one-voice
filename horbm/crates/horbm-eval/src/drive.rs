// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Drive `Duplex::tick` on the virtual clock. One scenario, one loop.
//!
//! User audio is the stand-in from [`crate::speech`]. The recognizer follows
//! the same script, one partial per voiced frame. The mouth on a conversation
//! is [`NativeMouth`]. The echo probe uses a generated far-end so the room
//! output clears the ear floor.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use horbm::{CardHit, CardLookup};
use horbm_duplex::{
    ConfiguredPersona, CoreAffect, Duplex, Floor, FrameOut, Hypothesis, HypothesisKind, LoopEvent,
    NativeMouth, Parts, RecogError, Recognizer, SpeakingBreath, FRAME_SAMPLES,
};
use horbm_voice::Mouth;

use crate::echo::{self, Far, FarMouth, Room};
use crate::scenario::{Kind, Phase, Scenario, Seg};
use crate::speech;

struct Cards;

impl CardLookup for Cards {
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

struct Script {
    pending: Rc<RefCell<String>>,
    frames: Cell<u32>,
}

impl Script {
    fn new(pending: Rc<RefCell<String>>) -> Self {
        Self {
            pending,
            frames: Cell::new(0),
        }
    }
}

impl Recognizer for Script {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        self.frames.set(self.frames.get().saturating_add(1));
        let text = self.pending.borrow().trim().to_string();
        if text.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Hypothesis {
            kind: HypothesisKind::Partial,
            text,
            start_ms: 0,
            end_ms: self.audio_ms(),
        }])
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.get().saturating_mul(horbm_duplex::FRAME_MS)
    }

    fn name(&self) -> &str {
        "script"
    }
}

pub struct Obs {
    pub t_ms: u64,
    pub you: bool,
    pub me: bool,
    pub event: LoopEvent,
    pub floor: Floor,
    pub phase: Phase,
}

pub struct Trial {
    pub id: &'static str,
    pub kind: Kind,
    pub obs: Vec<Obs>,
    pub samples: usize,
    pub opened: bool,
    /// The echo canceller accepted `set_aec(true)` on this trial.
    pub echo_live: bool,
}

pub fn run_all(scenarios: &[Scenario]) -> Vec<Trial> {
    scenarios.iter().map(run_one).collect()
}

fn run_one(scenario: &Scenario) -> Trial {
    match scenario.kind {
        Kind::EchoTone => run_echo(scenario, Far::Tone),
        Kind::EchoNoise => run_echo(scenario, Far::Noise),
        _ => run_script(scenario),
    }
}

fn seated() -> (CoreAffect, ConfiguredPersona) {
    let persona = horbm::Persona::load_development()
        .unwrap_or_else(|err| panic!("configured persona is not loaded: {err}"));
    let affect = CoreAffect::from_baseline(persona.baseline());
    (affect, ConfiguredPersona::from_persona(persona))
}

fn run_script(scenario: &Scenario) -> Trial {
    let pending = Rc::new(RefCell::new(String::new()));
    let (emotion, persona) = seated();
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Script::new(Rc::clone(&pending)),
            emotion,
            breath: SpeakingBreath,
            persona,
            mouth: NativeMouth::new(),
        },
        Cards,
    )
    .unwrap_or_else(|_| panic!("{} refused the configured persona", scenario.id));
    arm_conversation(&mut duplex);
    let mut trial = Trial {
        id: scenario.id,
        kind: scenario.kind,
        obs: Vec::new(),
        samples: 0,
        opened: true,
        echo_live: false,
    };
    for segment in &scenario.segments {
        match *segment {
            Seg::Say {
                words,
                hold,
                falling,
                phase,
            } => say(
                &mut duplex,
                &pending,
                &mut trial,
                words,
                hold,
                falling,
                phase,
            ),
            Seg::Quiet { frames, phase } => {
                for _ in 0..frames {
                    step(
                        &mut duplex,
                        &pending,
                        &mut trial,
                        "",
                        &speech::silence(),
                        phase,
                    );
                }
            }
            Seg::Ack { text, frames } => {
                for _ in 0..frames {
                    step(
                        &mut duplex,
                        &pending,
                        &mut trial,
                        text,
                        &speech::tone(160.0),
                        Phase::Listen,
                    );
                }
            }
            Seg::Barge { words } => {
                let mut partial = String::new();
                for word in words {
                    push_word(&mut partial, word);
                    step(
                        &mut duplex,
                        &pending,
                        &mut trial,
                        &partial,
                        &speech::tone(140.0),
                        Phase::Barge,
                    );
                }
            }
            Seg::Tap { text } => {
                step(
                    &mut duplex,
                    &pending,
                    &mut trial,
                    text,
                    &speech::tone(140.0),
                    Phase::Tap,
                );
            }
        }
    }
    trial
}

fn say<M: Mouth>(
    duplex: &mut Duplex<Script, CoreAffect, SpeakingBreath, ConfiguredPersona, M>,
    pending: &Rc<RefCell<String>>,
    trial: &mut Trial,
    words: &[&str],
    hold: u16,
    falling: bool,
    phase: Phase,
) {
    let mut partial = String::new();
    for (index, word) in words.iter().enumerate() {
        push_word(&mut partial, word);
        let last = index + 1 == words.len() && hold == 0;
        let pcm = if falling && last {
            speech::falling()
        } else {
            speech::tone(180.0)
        };
        step(duplex, pending, trial, &partial, &pcm, phase);
    }
    for index in 0..hold {
        let pcm = if falling && index + 1 == hold {
            speech::falling()
        } else {
            speech::tone(180.0)
        };
        step(duplex, pending, trial, &partial, &pcm, phase);
    }
}

fn push_word(partial: &mut String, word: &str) {
    if !partial.is_empty() {
        partial.push(' ');
    }
    partial.push_str(word);
}

fn run_echo(scenario: &Scenario, far: Far) -> Trial {
    let pending = Rc::new(RefCell::new(String::new()));
    let (emotion, persona) = seated();
    let mut duplex = Duplex::new(
        Parts {
            recognizer: Script::new(Rc::clone(&pending)),
            emotion,
            breath: SpeakingBreath,
            persona,
            mouth: FarMouth::new(far),
        },
        Cards,
    )
    .unwrap_or_else(|_| panic!("{} refused the configured persona", scenario.id));
    let echo_live = arm_echo(&mut duplex);
    let mut trial = Trial {
        id: scenario.id,
        kind: scenario.kind,
        obs: Vec::new(),
        samples: 0,
        opened: false,
        echo_live,
    };
    let words = ["where", "is", "the", "kettle"];
    let mut partial = String::new();
    for word in words {
        push_word(&mut partial, word);
        step(
            &mut duplex,
            &pending,
            &mut trial,
            &partial,
            &echo::user_tone(),
            Phase::User,
        );
    }
    step(
        &mut duplex,
        &pending,
        &mut trial,
        &partial,
        &echo::user_tone(),
        Phase::User,
    );
    for _ in 0..8 {
        step(
            &mut duplex,
            &pending,
            &mut trial,
            "",
            &speech::silence(),
            Phase::AfterTurn,
        );
        if trial
            .obs
            .iter()
            .any(|obs| obs.event == LoopEvent::FirstAudio)
        {
            trial.opened = true;
            break;
        }
    }
    let mut playing = if duplex.mouth().last().len() == FRAME_SAMPLES {
        duplex.mouth().last().to_vec()
    } else {
        speech::silence()
    };
    let mut room = Room::new();
    for _ in 0..echo::GRACE_FRAMES {
        let mic = room.render(&playing);
        step(
            &mut duplex,
            &pending,
            &mut trial,
            "yeah",
            &mic,
            Phase::EchoGrace,
        );
        playing = next_far(duplex.mouth());
    }
    for _ in 0..echo::SCORE_FRAMES {
        let mic = room.render(&playing);
        step(
            &mut duplex,
            &pending,
            &mut trial,
            "stop now",
            &mic,
            Phase::EchoScore,
        );
        if trial
            .obs
            .iter()
            .any(|obs| obs.phase == Phase::EchoScore && obs.event == LoopEvent::BargeIn)
        {
            break;
        }
        playing = next_far(duplex.mouth());
    }
    trial
}

fn next_far(mouth: &FarMouth) -> Vec<i16> {
    if mouth.last().len() == FRAME_SAMPLES {
        mouth.last().to_vec()
    } else {
        speech::silence()
    }
}

fn step<M: Mouth>(
    duplex: &mut Duplex<Script, CoreAffect, SpeakingBreath, ConfiguredPersona, M>,
    pending: &Rc<RefCell<String>>,
    trial: &mut Trial,
    text: &str,
    pcm: &[i16],
    phase: Phase,
) {
    *pending.borrow_mut() = text.to_string();
    trial.samples = trial.samples.saturating_add(pcm.len());
    let frame = duplex
        .tick(pcm)
        .unwrap_or_else(|_| panic!("{} frame", trial.id));
    note(trial, frame, phase);
}

fn note(trial: &mut Trial, frame: FrameOut, phase: Phase) {
    trial.obs.push(Obs {
        t_ms: frame.t_ms,
        you: frame.you_voiced,
        me: frame.me_audible,
        event: frame.event,
        floor: frame.floor,
        phase,
    });
}

fn arm_conversation<M: Mouth>(
    duplex: &mut Duplex<Script, CoreAffect, SpeakingBreath, ConfiguredPersona, M>,
) {
    duplex.set_prosody(true);
    #[cfg(duplex_aec)]
    {
        duplex.set_aec(false);
    }
}

/// `true` when the canceller is in this build and accepted the on switch.
fn arm_echo<M: Mouth>(
    duplex: &mut Duplex<Script, CoreAffect, SpeakingBreath, ConfiguredPersona, M>,
) -> bool {
    duplex.set_prosody(true);
    #[cfg(duplex_aec)]
    {
        duplex.set_aec(true);
        duplex.aec_enabled()
    }
    #[cfg(not(duplex_aec))]
    {
        false
    }
}
