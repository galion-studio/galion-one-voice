// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Optional Vosk ear. Compiled only with `--features vosk`.
//!
//! [`VoskEar`] wraps [`galion_sam_asr::Asr`]. `VoskAsr` dlopens libvosk and
//! does not call `vosk_gpu_init`. The feature test uses `FakeAsr` and does
//! not open a model. The host calls [`VoskEar::mark_end`] when the turn ends.

use galion_sam_asr::{Asr, AsrEvent, AsrKind};

use crate::ear::{Ear, EarEvent};

/// One [`Asr`] polled once per conductor frame.
pub struct VoskEar<A> {
    inner: A,
    end: bool,
}

impl<A> VoskEar<A> {
    pub fn new(inner: A) -> Self {
        Self { inner, end: false }
    }

    /// The next [`Ear::poll`] closes the utterance with [`Asr::finish`].
    pub fn mark_end(&mut self) {
        self.end = true;
    }
}

impl<A: Asr> Ear for VoskEar<A> {
    fn id(&self) -> &str {
        self.inner.name()
    }

    fn poll(&mut self, pcm: &[i16]) -> EarEvent {
        let result = if self.end {
            self.end = false;
            self.inner.finish()
        } else {
            self.inner.push_pcm(pcm)
        };
        match result {
            Ok(events) => map_last(&events),
            Err(err) => EarEvent::Fault(err.message().to_string()),
        }
    }
}

/// Energy endpointer in front of a streaming [`Asr`].
///
/// The gate decides the turn. [`Asr::finish`] runs on that turn-end, not on a
/// separate host signal. Echo frames are not pushed. Compiled only with
/// `--features vosk`. `FakeAsr` is the test double and does not load a model.
pub struct StreamingEar<A> {
    energy: crate::energy::EnergyEar,
    asr: A,
}

impl<A> StreamingEar<A> {
    pub fn new(asr: A) -> Self {
        Self {
            energy: crate::energy::EnergyEar::new(),
            asr,
        }
    }
}

impl<A: Asr> Ear for StreamingEar<A> {
    fn id(&self) -> &str {
        self.asr.name()
    }

    fn note_playback(&mut self, pcm: &[i16]) {
        self.energy.note_playback(pcm);
    }

    fn poll(&mut self, pcm: &[i16]) -> EarEvent {
        let event = self.energy.poll(pcm);
        let open = self.energy.user_turn_open();
        match event {
            EarEvent::Onset => push_keep(&mut self.asr, pcm, EarEvent::Onset),
            EarEvent::End(_) => finish_turn(&mut self.asr, pcm),
            EarEvent::Quiet if open => match self.asr.push_pcm(pcm) {
                Ok(events) => match map_last(&events) {
                    EarEvent::Quiet => EarEvent::Quiet,
                    other => other,
                },
                Err(err) => EarEvent::Fault(err.message().to_string()),
            },
            EarEvent::User | EarEvent::Barge => push_keep(&mut self.asr, pcm, event),
            other => other,
        }
    }
}

fn push_keep<A: Asr>(asr: &mut A, pcm: &[i16], event: EarEvent) -> EarEvent {
    match asr.push_pcm(pcm) {
        Ok(_) => event,
        Err(err) => EarEvent::Fault(err.message().to_string()),
    }
}

fn finish_turn<A: Asr>(asr: &mut A, pcm: &[i16]) -> EarEvent {
    if let Err(err) = asr.push_pcm(pcm) {
        return EarEvent::Fault(err.message().to_string());
    }
    match asr.finish() {
        Ok(events) => match map_last(&events) {
            EarEvent::Quiet => EarEvent::end(""),
            EarEvent::Partial(text) | EarEvent::End(text) => EarEvent::End(text),
            other => other,
        },
        Err(err) => EarEvent::Fault(err.message().to_string()),
    }
}

fn map_last(events: &[AsrEvent]) -> EarEvent {
    let Some(last) = events.last() else {
        return EarEvent::Quiet;
    };
    let text = last.text.trim();
    if text.is_empty() {
        return EarEvent::Quiet;
    }
    match last.kind {
        AsrKind::Final => EarEvent::End(text.to_string()),
        AsrKind::Partial => EarEvent::Partial(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::kettle_turn;
    use crate::clock::{FIRST_AUDIO_BUDGET_MS, FRAME_MS};
    use crate::ear::Ear;
    use crate::mouth::{FakeMouth, Mouth};
    use crate::turn::{TickEvent, VoiceLoop};
    use galion_sam_asr::{FakeAsr, HEARTBEAT_SAMPLES};

    #[test]
    fn fake_asr_reaches_the_brain_and_the_first_frame() {
        let mut voice = VoiceLoop::new(
            VoskEar::new(FakeAsr::new("where is the kettle")),
            FakeMouth::new(),
            kettle_turn(),
        );
        let frame = vec![0i16; HEARTBEAT_SAMPLES];
        for _ in 0..4 {
            let out = voice.tick(&frame);
            assert!(out.audio.is_none());
            assert!(out.reply.is_none());
        }
        voice.ear_mut().mark_end();
        let eot = voice.tick(&[]);
        assert_eq!(eot.event, TickEvent::EndOfSpeech);
        assert_eq!(eot.reply.as_deref(), Some("The kettle is on the hob."));
        let audio = voice.tick(&[]);
        assert_eq!(audio.event, TickEvent::FirstAudio);
        let waited = voice.first_audio_after_eot_ms().expect("first audio");
        assert!(waited <= u64::from(FIRST_AUDIO_BUDGET_MS));
        assert_eq!(waited, u64::from(FRAME_MS));
        assert_eq!(voice.mouth().id(), "fake");
        assert_eq!(voice.ear_mut().id(), "fake");
    }

    #[test]
    fn the_energy_endpointer_closes_the_stream_without_a_host_mark() {
        let mut ear = StreamingEar::new(FakeAsr::new("where is the kettle"));
        let quiet = vec![0i16; HEARTBEAT_SAMPLES];
        let loud = (0..HEARTBEAT_SAMPLES)
            .map(|index| ((index as f64 * 0.2).sin() * 8000.0) as i16)
            .collect::<Vec<_>>();
        assert_eq!(ear.poll(&quiet), EarEvent::Quiet);
        assert_eq!(ear.poll(&loud), EarEvent::Onset);
        let mid = ear.poll(&loud);
        assert!(matches!(mid, EarEvent::Partial(_)), "{mid}");
        assert_eq!(ear.poll(&quiet), EarEvent::end("where is the kettle"));
    }
}
