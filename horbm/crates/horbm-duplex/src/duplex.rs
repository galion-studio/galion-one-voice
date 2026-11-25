// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One tick, both bands.
//!
//! Order, fixed: echo, ear, recognizer, emotion, brain prefetch, conductor,
//! breath, mouth. The conductor owns the floor. The mouth is the me band.
//! The you band is the frame that just arrived, after echo removal.

use galion_conductor::{
    is_listed_backchannel, Conductor, Ear as ConductorEar, Floor as ConductorFloor, ReactionKind,
    SideBandView, TickEvent as ConductorEvent,
};
use horbm::UNKNOWN_REPLY;
use horbm_voice::{Mouth, MouthPull, SpokenLine};

use crate::brain::BrainHandle;
use crate::echo::PlaybackEcho;
use crate::ports::{
    Affect, Breath, BreathCue, EarSense, Emotion, Hypothesis, Persona, ProsodyCue, Recognizer,
    Style, DEV_PERSONA,
};
use crate::prosody::ProsodyEar;
use crate::report::LatencyReport;
use crate::{LoopError, FRAME_MS, FRAME_SAMPLES};

/// The five seats. Cards for the brain are a separate argument.
pub struct Parts<R, E, B, P, M> {
    pub recognizer: R,
    pub emotion: E,
    pub breath: B,
    pub persona: P,
    pub mouth: M,
}

/// What this frame decided, after the conductor and the mouth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopEvent {
    None,
    EagerEndOfTurn,
    Committed,
    FirstAudio,
    Backchannel,
    /// Samantha nodded while the user kept the floor. Not a barge-in.
    ListenBackchannel,
    BargeIn,
    TurnResumed,
}

/// Floor names, copied from the conductor so a test can read them
/// without taking that crate as its own dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floor {
    Listening,
    Speculating,
    Speaking,
    Ducked,
    Paused,
}

/// One frame of both bands. `partial` is the hypothesis the conductor saw.
/// The gate line does not print it.
pub struct FrameOut {
    pub t_ms: u64,
    pub event: LoopEvent,
    pub floor: Floor,
    pub epoch: u64,
    pub cancel: bool,
    pub me_silent: bool,
    pub yielded: bool,
    pub you_samples: usize,
    pub you_voiced: bool,
    pub partial: String,
    pub me_samples: usize,
    pub me_audible: bool,
    pub me_first: bool,
    pub affect: Affect,
    pub breath: BreathCue,
    pub reaction: bool,
    pub reaction_peak: i16,
    pub side_idle: bool,
    pub research_open: bool,
}

impl std::fmt::Debug for FrameOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameOut")
            .field("t_ms", &self.t_ms)
            .field("event", &self.event)
            .field("floor", &self.floor)
            .field("epoch", &self.epoch)
            .field("cancel", &self.cancel)
            .field("you_voiced", &self.you_voiced)
            .field("you_samples", &self.you_samples)
            .field("you_chars", &self.partial.chars().count())
            .field("me_audible", &self.me_audible)
            .field("me_samples", &self.me_samples)
            .field("me_first", &self.me_first)
            .field("yielded", &self.yielded)
            .field("reaction", &self.reaction)
            .field("side_idle", &self.side_idle)
            .finish()
    }
}

/// Ear, recognizer, brain, conductor, and mouth on one virtual clock.
pub struct Duplex<R, E, B, P, M> {
    recognizer: R,
    emotion: E,
    breath: B,
    persona: P,
    style: Style,
    mouth: M,
    brain: BrainHandle,
    conductor: Conductor<BrainHandle>,
    partial: String,
    you_open: bool,
    me_started: bool,
    yielded: bool,
    skip_pull: bool,
    /// Quiet frames since the mouth queue emptied. Sixteen frames is
    /// 1.28 s, past the end-to-end smoke's 800 ms tail, so a finished
    /// clip does not drop the floor before that script's barge.
    mouth_quiet: u32,
    /// Clear `partial` after this frame's output has copied it.
    release_partial: bool,
    first_voiced_ms: Option<u64>,
    last_voiced_ms: Option<u64>,
    eager_ms: Option<u64>,
    onset_ms: Option<u64>,
    sense: ProsodyEar,
    last_cue: ProsodyCue,
    echo: PlaybackEcho,
    report: LatencyReport,
}

impl<R, E, B, P, M> Duplex<R, E, B, P, M>
where
    R: Recognizer,
    E: Emotion,
    B: Breath,
    P: Persona,
    M: Mouth,
{
    /// Samantha, a local brain, and an offline side band.
    ///
    /// `cards` is the only lookup. Nothing is downloaded and nothing is dialed.
    pub fn new(
        parts: Parts<R, E, B, P, M>,
        cards: impl horbm::CardLookup + Send + 'static,
    ) -> Result<Self, LoopError> {
        let style = accept_samantha(&parts.persona)?;
        let brain = BrainHandle::with_cards(cards);
        let conductor = Conductor::with_reaction(brain.clone(), ReactionKind::Silence);
        Ok(Self {
            recognizer: parts.recognizer,
            emotion: parts.emotion,
            breath: parts.breath,
            persona: parts.persona,
            style,
            mouth: parts.mouth,
            brain,
            conductor,
            partial: String::new(),
            you_open: false,
            me_started: false,
            yielded: false,
            skip_pull: false,
            mouth_quiet: 0,
            release_partial: false,
            first_voiced_ms: None,
            last_voiced_ms: None,
            eager_ms: None,
            onset_ms: None,
            sense: ProsodyEar::new(),
            last_cue: ProsodyCue::disabled(),
            echo: PlaybackEcho::new(),
            report: LatencyReport::fresh(),
        })
    }

    /// Prosody and ear sensing. On when the feature is compiled.
    /// `false` is the runtime off switch: a non-zero sample is voiced again.
    pub fn set_prosody(&mut self, on: bool) {
        self.sense.set_enabled(on);
    }

    #[must_use]
    pub fn prosody_enabled(&self) -> bool {
        self.sense.enabled()
    }

    /// The cue from the last accepted frame. Disabled before the first tick.
    #[must_use]
    pub fn prosody(&self) -> ProsodyCue {
        self.last_cue
    }

    /// Echo removal. On when the feature is compiled.
    /// `false` is the bypass: the microphone frame is the ear frame.
    pub fn set_aec(&mut self, on: bool) {
        self.echo.set_enabled(on);
    }

    #[must_use]
    pub fn aec_enabled(&self) -> bool {
        self.echo.enabled()
    }

    #[must_use]
    pub fn report(&self) -> &LatencyReport {
        &self.report
    }

    #[must_use]
    pub fn t_ms(&self) -> u64 {
        self.conductor.t_ms()
    }

    #[must_use]
    pub fn persona(&self) -> &P {
        &self.persona
    }

    #[must_use]
    pub fn mouth(&self) -> &M {
        &self.mouth
    }

    /// The emotion port. The soak reads the moment from here.
    #[must_use]
    pub fn emotion(&self) -> &E {
        &self.emotion
    }

    /// One 80 ms frame. A short buffer leaves the clock where it was.
    pub fn tick(&mut self, pcm: &[i16]) -> Result<FrameOut, LoopError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(LoopError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        let t_ms = self.conductor.t_ms();
        let t32 = u32::try_from(t_ms).unwrap_or(u32::MAX);
        let mut cleaned = [0i16; FRAME_SAMPLES];
        self.echo.cancel_into(pcm, &mut cleaned)?;
        let cue = self.sense.hear(&cleaned);
        self.last_cue = cue;
        let voiced = if cue.enabled {
            cue.voiced
        } else {
            cleaned.iter().any(|sample| *sample != 0)
        };
        if voiced && self.first_voiced_ms.is_none() {
            self.first_voiced_ms = Some(t_ms);
        }
        if voiced {
            self.last_voiced_ms = Some(t_ms);
        }
        self.release_mouth_if_quiet(voiced);
        self.take_hypothesis(&cleaned, voiced, t_ms)?;
        let affect = self.emotion.step(t32);
        self.prefetch_while_listening();
        self.note_onset(voiced, t_ms);

        let heard = self.partial.clone();
        if self.release_partial {
            self.partial.clear();
            self.release_partial = false;
        }
        let ear = if voiced && !heard.is_empty() {
            ConductorEar::Speech(&heard)
        } else {
            ConductorEar::Quiet
        };
        let writes = self.brain.writes();
        let reads = self.brain.reads();
        let out = self.conductor.tick(ear, None);
        self.note_conductor(&out, writes, reads);

        let mut event = map_event(out.event);
        if out.event == ConductorEvent::Committed && !self.yielded && !self.me_started {
            self.start_mouth(out.clause.as_ref());
            self.me_started = true;
            self.skip_pull = true;
        }

        let mut me_samples = 0usize;
        let mut me_audible = false;
        let mut me_first = false;
        let mut noted = false;
        if out.me_silent || out.event == ConductorEvent::BargeIn {
            self.mouth.cancel();
            self.recognizer.cancel()?;
            self.you_open = false;
            self.yielded = true;
            self.skip_pull = false;
            self.note_barge(out.t_ms, voiced);
            event = LoopEvent::BargeIn;
        } else if self.skip_pull {
            self.skip_pull = false;
        } else if self.mouth.speaking() && !self.yielded {
            if let Some(pulled) = self.pull_mouth() {
                noted = true;
                me_samples = pulled.samples;
                me_audible = pulled.audible;
                me_first = pulled.first;
                if me_first {
                    event = LoopEvent::FirstAudio;
                    self.note_voice(out.t_ms);
                }
            }
        }
        if !noted {
            self.echo.note_silence();
        }

        if event == LoopEvent::Backchannel {
            self.report.backchannel_frames = self.report.backchannel_frames.saturating_add(1);
            if out.cancel || out.me_silent {
                self.report.backchannel_cancels = self.report.backchannel_cancels.saturating_add(1);
            }
        }

        let research_open = research_is_open(&out);
        if research_open {
            self.report.research_frames = self.report.research_frames.saturating_add(1);
        }
        if voiced && me_audible {
            self.report.overlap_live_frames = self.report.overlap_live_frames.saturating_add(1);
        }
        if me_audible {
            self.report.me_audible_frames = self.report.me_audible_frames.saturating_add(1);
        }
        self.report.you_frames = self.report.you_frames.saturating_add(1);

        let speaking = me_audible || (self.mouth.speaking() && !self.yielded);
        let breath = self.breath.cue(t32, speaking);
        if self.yielded && matches!(out.floor, ConductorFloor::Listening) {
            self.yielded = false;
            self.me_started = false;
            self.skip_pull = false;
            self.partial.clear();
        }
        Ok(FrameOut {
            t_ms: out.t_ms,
            event,
            floor: map_floor(out.floor),
            epoch: out.epoch,
            cancel: out.cancel,
            me_silent: out.me_silent,
            yielded: self.yielded,
            you_samples: pcm.len(),
            you_voiced: voiced,
            partial: heard,
            me_samples,
            me_audible,
            me_first,
            affect,
            breath,
            reaction: out.reaction.is_some(),
            reaction_peak: out.reaction.as_ref().map(|item| item.peak).unwrap_or(0),
            side_idle: matches!(out.side_band, SideBandView::Idle),
            research_open,
        })
    }

    fn release_mouth_if_quiet(&mut self, voiced: bool) {
        if self.yielded || !self.me_started || self.mouth.speaking() {
            self.mouth_quiet = 0;
            return;
        }
        if voiced {
            self.mouth_quiet = 0;
            return;
        }
        self.mouth_quiet = self.mouth_quiet.saturating_add(1);
        // 16 frames is 1.28 s, past the end-to-end smoke's 800 ms tail.
        if self.mouth_quiet < 16 {
            return;
        }
        self.conductor.mouth_idle();
        self.me_started = false;
        self.skip_pull = false;
        self.partial.clear();
        self.mouth_quiet = 0;
    }

    fn take_hypothesis(&mut self, pcm: &[i16], voiced: bool, t_ms: u64) -> Result<(), LoopError> {
        if voiced {
            let hyps = self.recognizer.push_frame(pcm).map_err(LoopError::from)?;
            self.you_open = true;
            if let Some(text) = latest_text(&hyps) {
                if self.report.recognizer_frames.is_none() {
                    let origin = self.first_voiced_ms.unwrap_or(t_ms);
                    self.report.recognizer_frames = Some(frames_between(t_ms, origin));
                }
                if text != self.partial {
                    self.partial = text.to_string();
                }
            }
            return Ok(());
        }
        if self.you_open {
            let _final = self.recognizer.finish().map_err(LoopError::from)?;
            self.you_open = false;
            // The conductor already stored this partial. The frame output
            // still needs it; `tick` clears the field after the copy.
            self.release_partial = true;
        }
        Ok(())
    }

    fn prefetch_while_listening(&mut self) {
        if self.partial.is_empty() {
            return;
        }
        if !matches!(
            self.conductor.floor(),
            ConductorFloor::Listening | ConductorFloor::Speculating
        ) {
            return;
        }
        self.brain.prefetch(&self.partial);
    }

    fn note_onset(&mut self, voiced: bool, t_ms: u64) {
        if self.onset_ms.is_some() || !voiced || !self.me_started || self.yielded {
            return;
        }
        if self.partial.is_empty() || is_listed_backchannel(&self.partial) {
            return;
        }
        self.onset_ms = Some(t_ms);
    }

    fn note_conductor(&mut self, out: &galion_conductor::TickOut, writes: u32, reads: u32) {
        if out.event != ConductorEvent::EagerEndOfTurn {
            if out.event == ConductorEvent::Committed {
                if let Some(eager) = self.eager_ms {
                    self.report.commit_frames = Some(frames_between(out.t_ms, eager));
                }
            }
            if out.reaction.is_some() && self.report.reaction_frames.is_none() {
                let eager = self.eager_ms.unwrap_or(out.t_ms);
                self.report.reaction_frames = Some(frames_between(out.t_ms, eager));
            }
            return;
        }
        self.eager_ms = Some(out.t_ms);
        self.report.brain_frames = Some(0);
        self.report.brain_filled_before_eager = writes > 0 && self.brain.writes() == writes;
        self.report.brain_read_on_eager = self.brain.reads() > reads;
        let voiced_at = self.last_voiced_ms.unwrap_or(out.t_ms);
        self.report.conductor_frames = Some(frames_between(out.t_ms, voiced_at));
        if out.reaction.is_some() {
            self.report.reaction_frames = Some(0);
        }
    }

    fn start_mouth(&mut self, clause: Option<&galion_conductor::PickedClause>) {
        let Some(clause) = clause else {
            return;
        };
        if clause.clause.is_empty() {
            return;
        }
        let line = self.brain.mouth_line(&clause.clause);
        self.report.brain_line_matched = line.matched;
        self.report.first_clause_words = Some(clause.lead.split_whitespace().count());
        self.mouth.start(&SpokenLine {
            text: clause.clause.clone(),
            mood: line.mood,
            rate: self.style.rate,
            energy: line.energy,
            pitch_st: line.pitch_st,
            pause_ms: line.pause_ms,
        });
    }

    fn pull_mouth(&mut self) -> Option<Pulled> {
        match self.mouth.pull() {
            MouthPull::Frame(frame) => {
                let audible = frame.samples.iter().any(|sample| *sample != 0);
                self.echo.note_playback(&frame.samples);
                Some(Pulled {
                    samples: frame.samples.len(),
                    audible,
                    first: frame.first,
                })
            }
            MouthPull::Done | MouthPull::Idle => None,
        }
    }

    fn note_voice(&mut self, t_ms: u64) {
        let Some(eager) = self.eager_ms else {
            return;
        };
        self.report.voice_ms = Some(t_ms.saturating_sub(eager));
        self.report.affect_frames = Some(frames_between(t_ms, eager));
    }

    fn note_barge(&mut self, t_ms: u64, voiced: bool) {
        let Some(onset) = self.onset_ms else {
            return;
        };
        self.report.barge_ms = Some(t_ms.saturating_sub(onset));
        self.report.barge_frames = Some(frames_between(t_ms, onset));
        if voiced {
            self.report.barge_while_you_live = true;
        }
    }
}

struct Pulled {
    samples: usize,
    audible: bool,
    first: bool,
}

fn accept_samantha(persona: &impl Persona) -> Result<Style, LoopError> {
    if persona.id() != DEV_PERSONA || persona.voice() != DEV_PERSONA {
        return Err(LoopError::Persona(
            "dev phase speaks as samantha and with the samantha voice",
        ));
    }
    if persona.unknown_line() != UNKNOWN_REPLY {
        return Err(LoopError::Persona(
            "the spoken miss has to be the stored persona line",
        ));
    }
    let style = persona.style();
    if !style.brevity.is_finite() || !(0.0..=1.0).contains(&style.brevity) {
        return Err(LoopError::Persona("brevity is a number from 0 to 1"));
    }
    if !style.warmth.is_finite() || !(0.0..=1.0).contains(&style.warmth) {
        return Err(LoopError::Persona("warmth is a number from 0 to 1"));
    }
    if !(1..=4).contains(&style.max_sentences) {
        return Err(LoopError::Persona("max sentences is 1 to 4"));
    }
    let expect = Style::from_brevity(style.brevity, style.warmth, style.max_sentences);
    if (style.rate - expect.rate).abs() > 1.0e-5 {
        return Err(LoopError::Persona("rate is 1 minus 0.15 times brevity"));
    }
    Ok(style)
}

fn latest_text(hyps: &[Hypothesis]) -> Option<&str> {
    hyps.iter().rev().find_map(|hyp| {
        let text = hyp.text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

fn map_event(event: ConductorEvent) -> LoopEvent {
    match event {
        ConductorEvent::None => LoopEvent::None,
        ConductorEvent::EagerEndOfTurn => LoopEvent::EagerEndOfTurn,
        ConductorEvent::Committed => LoopEvent::Committed,
        ConductorEvent::Backchannel => LoopEvent::Backchannel,
        ConductorEvent::ListenBackchannel => LoopEvent::ListenBackchannel,
        ConductorEvent::BargeIn => LoopEvent::BargeIn,
        ConductorEvent::TurnResumed => LoopEvent::TurnResumed,
    }
}

fn map_floor(floor: ConductorFloor) -> Floor {
    match floor {
        ConductorFloor::Listening => Floor::Listening,
        ConductorFloor::Speculating => Floor::Speculating,
        ConductorFloor::Speaking => Floor::Speaking,
        ConductorFloor::Ducked => Floor::Ducked,
        ConductorFloor::Paused => Floor::Paused,
    }
}

fn research_is_open(out: &galion_conductor::TickOut) -> bool {
    if out.research_clause.is_some() {
        return true;
    }
    matches!(
        out.side_band,
        SideBandView::Pending { .. } | SideBandView::Merged { .. }
    )
}

fn frames_between(later: u64, earlier: u64) -> u32 {
    let delta = later.saturating_sub(earlier);
    u32::try_from(delta / u64::from(FRAME_MS)).unwrap_or(u32::MAX)
}
