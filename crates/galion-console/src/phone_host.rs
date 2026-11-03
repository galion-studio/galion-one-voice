// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Screen-demo glue for the phone path (feature `phone-turn`).
//!
//! The 80 ms loop calls [`PhoneHost::drive_you`] and [`PhoneHost::drive_me`]
//! *outside* `Heartbeat::step`. Neither waits on ASR, the reply model, or TTS.
//! [`crate::turn_worker::TurnWorker`] owns that work. Levels, utterances, barge-in,
//! and published replies go through [`crate::bus::ProcessBus`] (`try_send` only).
//! Reply PCM is only a memory queue drained one heartbeat frame at a time into
//! the me band. Nothing here writes audio to disk.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use galion_heartbeat::{FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};
use galion_render::backends::web::{MicTap, ReplyBox, SayInbox, SpokenLine, Turn, TurnInbox, WebSurface};
use galion_render::v2_persona::Floor;

use crate::auto_workflow::{proceed, Proceed, ProceedSource};
use galion_emotion::VoicePersona;

use crate::bus::{pcm16_to_bytes, BusMsg, ProcessBus};
use crate::turn_worker::{TurnJob, TurnReply, TurnWorker};
use crate::workflow::{next_step, Next};

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
    says: SayInbox,
    replies: ReplyBox,
    floor: Arc<Mutex<Floor>>,
    bus: ProcessBus,
    worker: TurnWorker,
    /// Remaining reply samples at 24 kHz.
    play: VecDeque<i16>,
    /// A turn the worker could not accept yet (channel full). Newest wins.
    pending: Option<TurnJob>,
    /// A typed line the speak slot could not accept yet.
    pending_line: Option<SpokenLine>,
    phase: f32,
    /// Heartbeat frames observed on [`Self::drive_you`]. `t_ms = frame * 80`.
    frame: u64,
    t_ms: u64,
    /// Last proceed for an accepted mic take. Starts as the auto program.
    last_proceed: Proceed,
    /// Accepted mic takes handed to `auto_workflow::proceed`. Not incremented on the 80 ms tick.
    proceeded: u64,
    /// Last workflow step. The turn id is 0 until an utterance is recorded.
    last_next: Next,
    last_next_turn: u64,
}

impl PhoneHost {
    /// Spawn the turn worker and clone the HTTP handles. Call before `open` returns
    /// the surface into a `Box<dyn RenderSurface>`.
    pub fn from_surface(web: &WebSurface) -> Self {
        let bus = ProcessBus::new();
        Self {
            mic: web.mic_tap(),
            turns: web.turn_inbox(),
            says: web.say_inbox(),
            replies: web.reply_box(),
            floor: web.persona_floor(),
            worker: TurnWorker::spawn_on(bus.clone()),
            bus,
            play: VecDeque::new(),
            pending: None,
            pending_line: None,
            phase: 0.0,
            frame: 0,
            t_ms: 0,
            last_proceed: Proceed::Program,
            proceeded: 0,
            last_next: Next::Speak,
            last_next_turn: 0,
        }
    }

    /// Proceed chosen for the newest accepted mic take.
    pub fn last_proceed(&self) -> &Proceed {
        &self.last_proceed
    }

    /// How many accepted mic takes have proceeded. Quiet frames do not count.
    pub fn proceeded(&self) -> u64 {
        self.proceeded
    }

    /// Register the one-dot floor is speaking in. Read outside `Heartbeat::step`.
    pub fn voice_persona(&self) -> VoicePersona {
        match self.floor.lock() {
            Ok(floor) if floor.id() == "jarvis" => VoicePersona::Jarvis,
            _ => VoicePersona::Samantha,
        }
    }

    /// Turn id and the step chosen for it. No transcript is kept.
    pub fn workflow(&self) -> (u64, Next) {
        (self.last_next_turn, self.last_next)
    }

    pub fn bus(&self) -> &ProcessBus {
        &self.bus
    }

    /// Replace `you` when the browser posted a fresh RMS above the room floor.
    pub fn drive_you(&mut self, you: &mut [f32]) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.t_ms = self.frame.saturating_mul(u64::from(FRAME_MS));
        let Some(rms) = self.mic.latest(Duration::from_millis(320)) else {
            return false;
        };
        let _ = self
            .bus
            .try_send(BusMsg::mic_level(self.bus.alloc_id(), self.t_ms, rms));
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

    /// Take at most the newest utterance and the newest typed line, publish any
    /// finished reply, drain one heartbeat frame of reply PCM into `me`.
    pub fn drive_me(&mut self, me: &mut [f32]) -> bool {
        self.poll_me(me, true)
    }

    /// Poll the worker, then optionally copy one [`FRAME_SAMPLES`] frame.
    /// `emit == false` still polls and does not pop PCM: a late heartbeat is a pause.
    /// A buffer longer than one frame does not catch up.
    pub fn poll_me(&mut self, me: &mut [f32], emit: bool) -> bool {
        self.flush_submit();
        let turn = self.turns.take_latest();
        let say = self.pending_line.take().or_else(|| self.says.take_latest());
        // Submit the older id first so the newer one keeps the cancel token.
        match (say, turn) {
            (Some(s), Some(t)) if s.id > t.id => {
                self.submit_turn(t);
                self.submit_say(s);
            }
            (Some(s), Some(t)) => {
                self.submit_say(s);
                self.submit_turn(t);
            }
            (Some(s), None) => self.submit_say(s),
            (None, Some(t)) => self.submit_turn(t),
            (None, None) => {}
        }
        self.flush_submit();
        if let Some(r) = self.worker.try_recv() {
            self.publish(r);
        }
        if !emit || self.play.is_empty() {
            return false;
        }
        copy_one_frame(&mut self.play, me)
    }

    /// Drop queued reply PCM. Call this on a yield, before the next poll,
    /// so a reply published on that same tick is not wiped.
    pub fn release_me(&mut self) {
        wipe_play(&mut self.play);
    }
}

/// Copy at most one heartbeat frame. Does not allocate.
fn copy_one_frame(play: &mut VecDeque<i16>, me: &mut [f32]) -> bool {
    if play.is_empty() {
        return false;
    }
    let n = me.len().min(FRAME_SAMPLES);
    for (i, x) in me.iter_mut().enumerate() {
        if i < n {
            *x = pop_pcm(play).map(|s| s as f32 / 32768.0).unwrap_or(0.0);
        } else {
            *x = 0.0;
        }
    }
    true
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
    fn publish(&mut self, r: TurnReply) {
        let speech = BusMsg::speech_pcm(
            self.bus.alloc_id(),
            r.id,
            self.t_ms,
            self.bus.cancel(),
            pcm16_to_bytes(&r.pcm),
            r.ms,
        );
        let text = BusMsg::reply_text(
            self.bus.alloc_id(),
            r.id,
            self.t_ms,
            self.bus.cancel(),
            r.text.clone(),
            None,
            false,
        );
        let Some(speech) = self.bus.before_publish(speech) else {
            return;
        };
        let _ = self.bus.try_send_reply(speech);
        if let Some(text) = self.bus.before_publish(text) {
            let _ = self.bus.try_send_reply(text);
        }
        if !r.transcript.trim().is_empty() {
            self.note_transcript(r.id, &r.transcript);
            if let Ok(mut floor) = self.floor.lock() {
                let _ = floor.say(&r.transcript, false, 0.0, false);
            }
        }
        if let Some(err) = r.error.as_deref() {
            let _ = self.bus.try_send(BusMsg::fault(
                self.bus.alloc_id(),
                r.id,
                self.t_ms,
                self.bus.cancel(),
                "phone",
                fault_kind(err),
            ));
        }
        let _ = self.replies.set_reply(r.id.to_string(), r.wav.clone(), r.ms);
        wipe_play(&mut self.play);
        self.play.extend(r.pcm.iter().copied());
        // Lengths and labels only. The helper cannot take PCM or the transcript.
        eprintln!(
            "{}",
            phone_reply_log_line(
                r.id,
                r.asr,
                r.backend,
                r.ms,
                r.in_ms,
                r.took_ms,
                r.error.as_deref()
            )
        );
    }

    fn submit_say(&mut self, mut line: SpokenLine) {
        if line.barge {
            wipe_play(&mut self.play);
        }
        if line.silence || line.reply.trim().is_empty() {
            return;
        }
        let id = line.id;
        let text = std::mem::take(&mut line.reply);
        if let Err(text) = self.worker.try_speak(id, text) {
            line.reply = text;
            self.pending_line = Some(line);
        }
    }

    fn submit_turn(&mut self, t: Turn) {
        // New utterance cancels whatever the speakers were playing (barge-in).
        wipe_play(&mut self.play);
        if let Some((rate, ch, pcm)) = t.pcm16() {
            let pcm = downmix(&pcm, ch);
            let turn_id = t.id;
            // The page does not send a skill token. Live path is the auto program.
            self.last_proceed = proceed(ProceedSource::Mic, None);
            self.proceeded = self.proceeded.saturating_add(1);
            let _ = self
                .bus
                .try_send(BusMsg::barge_in(self.bus.alloc_id(), turn_id, self.t_ms));
            let _ = self.bus.try_send(BusMsg::utterance(
                self.bus.alloc_id(),
                turn_id,
                self.t_ms,
                self.bus.cancel(),
                rate,
                pcm16_to_bytes(&pcm),
            ));
            self.enqueue(TurnJob {
                id: turn_id,
                rate,
                pcm,
            });
            self.note_recorded(turn_id);
        }
    }

    fn note_recorded(&mut self, turn_id: u64) {
        self.last_next = Next::Recorded;
        self.last_next_turn = turn_id;
    }

    fn note_transcript(&mut self, turn_id: u64, transcript: &str) {
        if transcript.trim().is_empty() {
            return;
        }
        self.last_next = next_step(transcript);
        self.last_next_turn = turn_id;
    }

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

/// First token of a lane error (`AsrUnratified`, `Empty`, …). No audio, no transcript.
fn fault_kind(err: &str) -> String {
    err.split(':')
        .next()
        .unwrap_or("error")
        .trim()
        .chars()
        .take(64)
        .collect()
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
        let line = phone_reply_log_line(
            7,
            "interim-placeholder",
            "tone",
            80,
            100,
            40,
            Some("AsrUnratified"),
        );
        assert!(
            line.contains("id=7") && line.contains("audio_ms=80") && line.contains("err=AsrUnratified"),
            "{line}"
        );
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
    fn drain_takes_one_heartbeat_frame_and_leaves_the_tail() {
        let mut play = VecDeque::from(vec![16_000i16; FRAME_SAMPLES + 4]);
        let mut me = vec![0.0f32; FRAME_SAMPLES + 4];
        assert!(copy_one_frame(&mut play, &mut me));
        assert_eq!(play.len(), 4, "the next frame stays queued");
        assert!(me[..FRAME_SAMPLES].iter().all(|s| (*s - 16_000.0 / 32_768.0).abs() < 1e-6));
        assert!(me[FRAME_SAMPLES..].iter().all(|s| *s == 0.0), "no second frame in this tick");
        let mut short_play = VecDeque::from(vec![1_000i16; 100]);
        let mut short = [0.0f32; 8];
        assert!(copy_one_frame(&mut short_play, &mut short));
        assert_eq!(short_play.len(), 92, "a short buffer still takes only its own length");
    }

    #[test]
    fn reply_queue_is_zeroised_on_wipe() {
        let mut q = VecDeque::from(vec![1i16, -2, 3000]);
        assert_eq!(pop_pcm(&mut q), Some(1));
        wipe_play(&mut q);
        assert!(q.is_empty());
    }

    #[test]
    fn drive_posts_mic_level_and_utterance_on_the_bus() {
        use crate::bus::BusBody;
        use galion_render::backends::web::TurnFormat;

        std::env::set_var("GALION_ASR", "off");
        let web = WebSurface::mailbox();
        web.turn_inbox().push(
            TurnFormat::Pcm16 {
                rate: 16_000,
                channels: 1,
            },
            vec![0, 0, 1, 0],
        );
        let mut host = PhoneHost::from_surface(&web);
        // Post the level after the host (and its turn worker) exist: `drive_you`
        // only trusts a level newer than 320 ms, and spawning the worker can take
        // longer than that on a loaded CPU.
        web.mic_tap().set(0.25);
        let mut you = vec![0.0f32; 8];
        assert!(host.drive_you(&mut you));
        match host.bus().try_recv_mic().unwrap().body {
            BusBody::MicLevel { rms } => assert!((rms - 0.25).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
        let mut me = vec![0.0f32; 8];
        let _ = host.drive_me(&mut me);
        assert_eq!(host.workflow(), (1, crate::workflow::Next::Recorded));
        assert_eq!(host.bus().cancel(), 1);
        let utt = host
            .bus()
            .try_recv_utterance()
            .expect("utterance on the bus");
        assert_eq!(host.proceeded(), 1);
        assert_eq!(host.last_proceed(), &crate::auto_workflow::Proceed::Program);
        assert_eq!(utt.turn_id, 1);
        assert_eq!(utt.t_ms, u64::from(galion_heartbeat::FRAME_MS));
        match utt.body {
            BusBody::Utterance { rate, pcm } => {
                assert_eq!(rate, 16_000);
                assert_eq!(pcm, vec![0, 0, 1, 0]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn typed_line_is_spoken_and_grief_stays_on_the_floor() {
        let prev_speech = std::env::var("GALION_SPEECH").ok();
        let prev_asr = std::env::var("GALION_ASR").ok();
        std::env::set_var("GALION_ASR", "off");
        std::env::set_var("GALION_SPEECH", "off");
        let web = WebSurface::mailbox();
        {
            let said = web
                .persona_floor()
                .lock()
                .unwrap()
                .say("My mother died this morning.", false, 0.0, false);
            assert_eq!(said.mood, "grief");
        }
        web.say_inbox()
            .push("Your mother died this morning.".into(), false, false);
        let mut host = PhoneHost::from_surface(&web);
        match prev_speech {
            Some(v) => std::env::set_var("GALION_SPEECH", v),
            None => std::env::remove_var("GALION_SPEECH"),
        }
        match prev_asr {
            Some(v) => std::env::set_var("GALION_ASR", v),
            None => std::env::remove_var("GALION_ASR"),
        }
        let mut me = vec![0.0f32; 8];
        let t0 = std::time::Instant::now();
        let mut got = false;
        while !got && t0.elapsed().as_secs() < 5 {
            let _ = host.drive_you(&mut [0.0; 8]);
            let _ = host.drive_me(&mut me);
            got = web.reply_box().latest_json().is_some();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(got, "persona line was not published for the page to play");
        let floor = web.persona_floor();
        let floor = floor.lock().unwrap();
        assert!(floor.mood_from_turn());
        assert_eq!(floor.mood(), "grief");
    }

    #[test]
    fn words_choose_a_program_without_a_second_screen() {
        std::env::set_var("GALION_ASR", "off");
        let web = WebSurface::mailbox();
        let mut host = PhoneHost::from_surface(&web);
        assert_eq!(host.voice_persona(), galion_emotion::VoicePersona::Samantha);
        assert_eq!(host.workflow(), (0, crate::workflow::Next::Speak));
        host.note_transcript(4, "please start the heartbeat");
        assert_eq!(host.workflow(), (4, crate::workflow::Next::Program("heartbeat")));
        host.note_transcript(5, "I hear you");
        assert_eq!(host.workflow().1, crate::workflow::Next::Speak);
        host.note_transcript(6, "skill presence");
        assert_eq!(host.workflow(), (6, crate::workflow::Next::Skill("presence")));
    }
}
