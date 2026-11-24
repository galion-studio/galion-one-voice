// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Native mouth. The in-process tone from `horbm_voice`.
//!
//! The default build links this mouth and no Piper process. `--features piper`
//! adds [`crate::PiperMouth`] beside it. [`NativeMouth::last`] is the frame
//! the tick just pulled, so a host can hand it to a speaker without the tick
//! growing a second buffer type.

use horbm_voice::{FakeMouth, Mouth, MouthPull, SpokenLine};

/// Eight frames of generated tone. No file and no process.
pub struct NativeMouth {
    inner: FakeMouth,
    last: Vec<i16>,
}

impl Default for NativeMouth {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeMouth {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: FakeMouth::new(),
            last: Vec::new(),
        }
    }

    /// Samples from the last [`Mouth::pull`] that returned a frame.
    #[must_use]
    pub fn last(&self) -> &[i16] {
        &self.last
    }
}

impl Mouth for NativeMouth {
    fn id(&self) -> &str {
        "native"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.last.clear();
        self.inner.start(line);
    }

    fn pull(&mut self) -> MouthPull {
        match self.inner.pull() {
            MouthPull::Frame(frame) => {
                self.last.clone_from(&frame.samples);
                MouthPull::Frame(frame)
            }
            other => {
                self.last.clear();
                other
            }
        }
    }

    fn cancel(&mut self) {
        self.last.clear();
        self.inner.cancel();
    }

    fn speaking(&self) -> bool {
        self.inner.speaking()
    }
}
