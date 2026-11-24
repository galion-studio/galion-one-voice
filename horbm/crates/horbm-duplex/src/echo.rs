// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Acoustic echo removal in front of the ear.
//!
//! The canceller is `fdaf_aec::FdafAec` (crate `fdaf-aec` 0.1.0, MIT),
//! used as published. This module only slices the 80 ms duplex frame into
//! that crate's blocks, keeps the mouth frame from the previous tick as
//! the far-end reference, and can pass the microphone through unchanged.
//!
//! The reference is one clock behind the microphone. The live loop plays
//! a mouth frame at the end of a tick, and the next captured frame is what
//! the microphone heard. The crate's filter is 512 samples (32 ms) at
//! 16 kHz, so the acoustic delay and the short reverb have to sit inside
//! that tail. A longer room is outside this component.

use crate::{LoopError, FRAME_SAMPLES};

/// Published crate behind the `aec` feature.
pub const AEC_CRATE: &str = "fdaf-aec";
/// Licence of [`AEC_CRATE`] 0.1.0.
pub const AEC_LICENSE: &str = "MIT";
/// Published version this crate is built against.
pub const AEC_VERSION: &str = "0.1.0";

/// Half of the FDAF length. Five of these fill one 80 ms frame.
#[cfg(feature = "aec")]
const BLOCK: usize = 256;
#[cfg(feature = "aec")]
const FFT_SIZE: usize = BLOCK * 2;
/// Step size passed to [`fdaf_aec::FdafAec::new`]. Stable on a tone and on
/// noise for the synthetic room in the echo tests. Not a new filter.
#[cfg(feature = "aec")]
const STEP: f32 = 0.08;

#[cfg(feature = "aec")]
const _: () = assert!(FRAME_SAMPLES % BLOCK == 0);
#[cfg(feature = "aec")]
const _: () = assert!(FFT_SIZE == 512);

/// Mouth playback versus the microphone, on the 80 ms clock.
pub struct PlaybackEcho {
    enabled: bool,
    #[cfg(feature = "aec")]
    queued: Vec<i16>,
    #[cfg(feature = "aec")]
    far_buf: Vec<f32>,
    #[cfg(feature = "aec")]
    mic_buf: Vec<f32>,
    #[cfg(feature = "aec")]
    inner: fdaf_aec::FdafAec,
}

impl Default for PlaybackEcho {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackEcho {
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: cfg!(feature = "aec"),
            #[cfg(feature = "aec")]
            queued: vec![0; FRAME_SAMPLES],
            #[cfg(feature = "aec")]
            far_buf: vec![0.0; BLOCK],
            #[cfg(feature = "aec")]
            mic_buf: vec![0.0; BLOCK],
            #[cfg(feature = "aec")]
            inner: fdaf_aec::FdafAec::new(FFT_SIZE, STEP),
        }
    }

    /// The feature is in this build. The runtime bypass is separate.
    #[must_use]
    pub const fn compiled() -> bool {
        cfg!(feature = "aec")
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled && Self::compiled()
    }

    /// `false` copies the microphone through and does not step the filter.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on && Self::compiled();
    }

    /// Remove echo from one microphone frame.
    ///
    /// The far end is whatever [`Self::note_playback`] stored on the
    /// previous tick. A disabled filter returns the same samples.
    pub fn cancel(&mut self, mic: &[i16]) -> Result<Vec<i16>, LoopError> {
        let mut out = vec![0i16; FRAME_SAMPLES];
        self.cancel_into(mic, &mut out)?;
        Ok(out)
    }

    /// Same as [`Self::cancel`], writing into `out`.
    ///
    /// `out` must already be one frame. This does not allocate that frame.
    /// When echo removal is on, each block still comes back from `fdaf-aec`
    /// as a new buffer. That crate's `process` returns a `Vec`.
    pub fn cancel_into(&mut self, mic: &[i16], out: &mut [i16]) -> Result<(), LoopError> {
        if mic.len() != FRAME_SAMPLES || out.len() != FRAME_SAMPLES {
            let got = if mic.len() != FRAME_SAMPLES {
                mic.len()
            } else {
                out.len()
            };
            return Err(LoopError::Frame {
                got,
                expected: FRAME_SAMPLES,
            });
        }
        if self.enabled() {
            #[cfg(feature = "aec")]
            {
                self.filter_into(mic, out);
                return Ok(());
            }
        }
        out.copy_from_slice(mic);
        Ok(())
    }

    /// Far-end frame that pairs with the next microphone frame.
    ///
    /// A short buffer is stored as silence so the clock still advances.
    pub fn note_playback(&mut self, far: &[i16]) {
        #[cfg(feature = "aec")]
        {
            if far.len() != FRAME_SAMPLES {
                self.note_silence();
                return;
            }
            self.queued.clear();
            self.queued.extend_from_slice(far);
        }
        #[cfg(not(feature = "aec"))]
        {
            let _ = far;
        }
    }

    /// No mouth samples on this tick. The next microphone frame is paired
    /// with silence.
    pub fn note_silence(&mut self) {
        #[cfg(feature = "aec")]
        {
            if self.queued.len() != FRAME_SAMPLES {
                self.queued.resize(FRAME_SAMPLES, 0);
            }
            self.queued.fill(0);
        }
    }

    #[cfg(feature = "aec")]
    fn filter_into(&mut self, mic: &[i16], out: &mut [i16]) {
        let mut offset = 0;
        while offset < FRAME_SAMPLES {
            for index in 0..BLOCK {
                let sample = offset + index;
                self.far_buf[index] = f32::from(self.queued[sample]) / 32768.0;
                self.mic_buf[index] = f32::from(mic[sample]) / 32768.0;
            }
            let cleaned = self.inner.process(&self.far_buf, &self.mic_buf);
            for (index, sample) in cleaned.iter().enumerate() {
                out[offset + index] = unit_to_pcm(*sample);
            }
            offset += BLOCK;
        }
    }
}

#[cfg(feature = "aec")]
fn unit_to_pcm(sample: f32) -> i16 {
    if !sample.is_finite() {
        return 0;
    }
    let scaled = (f64::from(sample) * 32768.0).round();
    if scaled >= 32767.0 {
        32767
    } else if scaled <= -32768.0 {
        -32768
    } else {
        scaled as i16
    }
}
