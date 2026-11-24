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
}
