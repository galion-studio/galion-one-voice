// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Screen-demo glue for the phone path (feature `phone-turn`).
//!
//! The 80 ms loop calls [`PhoneHost::drive_you`] and [`PhoneHost::drive_me`].
//! Neither waits on ASR, the reply model, or TTS. [`crate::turn_worker::TurnWorker`]
//! owns that work. Reply PCM is only a memory queue drained one heartbeat frame
//! at a time into the me band. Nothing here writes audio to disk.

use std::collections::VecDeque;
use std::time::Duration;

use galion_heartbeat::SAMPLE_RATE;
use galion_render::backends::web::{MicTap, ReplyBox, TurnInbox, WebSurface};

use crate::turn_worker::{TurnJob, TurnWorker};

/// What the phone path overrode on this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhoneDrive {
    /// Browser mic RMS was fresh enough to replace the synthetic you band.
    pub you_live: bool,
    /// A reply is still draining into the me band.
    pub me_live: bool,
}

/// Handles cloned off [`WebSurface`] before it is boxed into the render fanout.
pub struct PhoneHost {
    mic: MicTap,
    turns: TurnInbox,
    replies: ReplyBox,
    worker: TurnWorker,
    /// Remaining reply samples at 24 kHz.
    play: VecDeque<i16>,
    /// A turn the worker could not accept yet (channel full). Newest wins.
    pending: Option<TurnJob>,
    phase: f32,
}

impl PhoneHost {
    /// Spawn the turn worker and clone the HTTP handles. Call before `open` returns
    /// the surface into a `Box<dyn RenderSurface>`.
    pub fn from_surface(web: &WebSurface) -> Self {
        Self {
            mic: web.mic_tap(),
            turns: web.turn_inbox(),
            replies: web.reply_box(),
            worker: TurnWorker::spawn(),
            play: VecDeque::new(),
            pending: None,
            phase: 0.0,
        }
    }

    /// Replace `you` when the browser posted a fresh RMS above the room floor.
    pub fn drive_you(&mut self, you: &mut [f32]) -> bool {
        let Some(rms) = self.mic.latest(Duration::from_millis(320)) else {
            return false;
        };
        if rms < 0.012 {
            return false;
        }
        let amp = (rms * 3.5).clamp(0.02, 0.22);
        let sr = SAMPLE_RATE as f32;
        for x in you.iter_mut() {
            self.phase += 190.0 * std::f32::consts::TAU / sr;
            if self.phase > std::f32::consts::TAU {
                self.phase -= std::f32::consts::TAU;
            }
            *x = amp * self.phase.sin();
        }
        true
    }

    /// Take at most the newest utterance, publish any finished reply, drain one
    /// frame of reply PCM into `me`.
    pub fn drive_me(&mut self, me: &mut [f32]) -> bool {
        self.flush_submit();
        if let Some(t) = self.turns.take_latest() {
            // New utterance cancels whatever the speakers were playing (barge-in).
            wipe_play(&mut self.play);
            if let Some((rate, ch, pcm)) = t.pcm16() {
                self.enqueue(TurnJob { id: t.id, rate, pcm: downmix(&pcm, ch) });
            }
        }
        self.flush_submit();
        if let Some(r) = self.worker.try_recv() {
            let _ = self.replies.set_reply(r.id.to_string(), r.wav.clone(), r.ms);
            wipe_play(&mut self.play);
            self.play.extend(r.pcm.iter().copied());
            // Lengths and labels only. The helper cannot take PCM or the transcript.
            eprintln!(
                "{}",
                phone_reply_log_line(r.id, r.asr, r.backend, r.ms, r.in_ms, r.took_ms, r.error.as_deref())
            );
        }
        if self.play.is_empty() {
            return false;
        }
        for x in me.iter_mut() {
            *x = pop_pcm(&mut self.play).map(|s| s as f32 / 32768.0).unwrap_or(0.0);
        }
        true
    }
}

impl Drop for PhoneHost {
    fn drop(&mut self) {
        wipe_play(&mut self.play);
        self.pending.take();
    }
}

/// One reply line for the demo log. It records id, ASR label, backend, and
/// durations. It does not accept PCM or transcript text. An error that looks
/// like audio or a transcript is replaced with `redacted`.
pub fn phone_reply_log_line(
    id: u64,
    asr: &str,
    backend: &str,
    audio_ms: u32,
    in_ms: u32,
    took_ms: u32,
    err: Option<&str>,
) -> String {
    format!(
        "phone reply id={id} asr={asr} backend={backend} audio_ms={audio_ms} in_ms={in_ms} took_ms={took_ms} err={}",
        safe_log_err(err)
    )
}

fn safe_log_err(err: Option<&str>) -> &str {
    match err {
        None | Some("") => "-",
        Some(e) if log_err_is_unsafe(e) => "redacted",
        Some(e) => e,
    }
}

fn log_err_is_unsafe(e: &str) -> bool {
    let lower = e.to_ascii_lowercase();
    e.contains("RIFF") || lower.contains("pcm") || lower.contains("transcript")
}

fn pop_pcm(play: &mut VecDeque<i16>) -> Option<i16> {
    let sample = {
        let front = play.front_mut()?;
        let sample = *front;
        *front = 0;
        sample
    };
    let _ = play.pop_front();
    Some(sample)
}

fn wipe_play(play: &mut VecDeque<i16>) {
    for s in play.iter_mut() {
        *s = 0;
    }
    std::hint::black_box(play.make_contiguous());
    play.clear();
}

impl PhoneHost {
    fn enqueue(&mut self, job: TurnJob) {
        if let Some(old) = self.pending.take() {
            if old.id > job.id {
                self.pending = Some(old);
                return;
            }
        }
        self.pending = Some(job);
    }

    fn flush_submit(&mut self) {
        if let Some(job) = self.pending.take() {
            if let Err(job) = self.worker.try_submit(job) {
                self.pending = Some(job);
            }
        }
    }
}

/// Average interleaved PCM down to mono. `channels == 1` clones.
pub fn downmix(pcm: &[i16], channels: u16) -> Vec<i16> {
    let ch = (channels as usize).max(1);
    if ch == 1 {
        return pcm.to_vec();
    }
    pcm.chunks(ch)
        .map(|f| {
            let s: i32 = f.iter().map(|&s| s as i32).sum();
            (s / ch as i32) as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_stereo() {
        assert_eq!(downmix(&[1, 3, 10, 30], 2), vec![2, 20]);
        assert_eq!(downmix(&[4, 5], 1), vec![4, 5]);
    }

    #[test]
    fn phone_log_has_no_pcm_or_transcript() {
        let transcript = "SECRET_TRANSCRIPT_do_not_log";
        let line = phone_reply_log_line(7, "interim-placeholder", "tone", 80, 100, 40, Some("AsrUnratified"));
        assert!(line.contains("id=7") && line.contains("audio_ms=80") && line.contains("err=AsrUnratified"), "{line}");
        assert!(!line.contains(transcript), "{line}");
        assert!(!line.contains("RIFF"), "{line}");
        assert!(!line.to_ascii_lowercase().contains("pcm"), "{line}");
        let redacted = phone_reply_log_line(
            1,
            "none",
            "tone",
            1,
            1,
            1,
            Some("boom RIFF pcm transcript SECRET_TRANSCRIPT_do_not_log"),
        );
        assert!(redacted.contains("err=redacted"), "{redacted}");
        assert!(!redacted.contains("RIFF") && !redacted.contains(transcript), "{redacted}");
        assert!(!redacted.to_ascii_lowercase().contains("pcm"), "{redacted}");
    }

    #[test]
    fn reply_queue_is_zeroised_on_wipe() {
        let mut q = VecDeque::from(vec![1i16, -2, 3000]);
        assert_eq!(pop_pcm(&mut q), Some(1));
        wipe_play(&mut q);
        assert!(q.is_empty());
    }
}
