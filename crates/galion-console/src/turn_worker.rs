// Galion Voice — Perpetual Open MIT License
//! S7 · phone turn worker (feature `phone-turn`): one long-lived thread off the 80 ms
//! heartbeat (TURN-WORKER.md). The heartbeat `try_submit`s a finished utterance and
//! `try_recv`s the reply each tick; it never blocks on model work.
//!
//! Pipeline: [`galion_cascade::PhoneLane`] (S4 ASR → S5 LLM → S6 TTS), built once from env
//! inside the worker thread (`GALION_ASR`, `GALION_LLM`, `GALION_SPEECH`). S4 is Vosk when
//! `GALION_ASR=vosk` and the small-en model is on disk (`asr=vosk`). The length-bucket
//! placeholder is not selected by the environment. Errors (`AsrUnratified`, `AsrUnavailable`,
//! `NoSpeech`) are reported in [`TurnReply::error`] and a clear two-tone WAV is published
//! instead, so the phone still hears that its turn arrived. The tone is not a transcript.
//! `GALION_SPEECH=off` is that path: two-tone WAV, `backend=tone`, `error` set.
//!
//! The job slot is the V2-S1 bus lane ([`crate::bus::JOB_DEPTH`] = 1, `try_send` only).
//! Cancel is [`crate::bus::ProcessBus::supersede`]. A stale `turn_id` is dropped before
//! ASR and again before the reply is handed back. The same token drops Piper clauses
//! that have not started. The thread and the Piper process are not killed.

use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::time::Instant;

use std::sync::{Arc, Mutex};

use galion_cascade::en_lane::{wav_bytes, LANE_RATE_HZ};
use galion_cascade::phone::FrameProsody;
use galion_cascade::{PhoneConfig, PhoneError, PhoneLane};

use crate::bus::{
    bytes_to_pcm16, pcm16_to_bytes, BusBody, BusGate, BusMsg, ProcessBus, WORKER_REPLY_DEPTH,
};

/// Latest 80 ms prosody, written by the screen loop and read when a turn speaks.
/// The heartbeat does not lock this. Identity until the host stores a frame.
#[derive(Clone)]
pub struct ProsodySlot {
    inner: Arc<Mutex<FrameProsody>>,
}

impl ProsodySlot {
    pub fn identity() -> Self {
        Self { inner: Arc::new(Mutex::new(FrameProsody::identity())) }
    }

    pub fn store(&self, prosody: FrameProsody) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = prosody;
        }
    }

    pub fn load(&self) -> FrameProsody {
        self.inner.lock().map(|slot| *slot).unwrap_or_else(|_| FrameProsody::identity())
    }
}

/// One utterance handed to the worker: mono PCM16 at `rate` (the lane resamples to 16 kHz).
pub struct TurnJob {
    pub id: u64,
    pub rate: u32,
    pub pcm: Vec<i16>,
}

impl TurnJob {
    /// Overwrite the utterance. Also run from [`Drop`].
    pub fn wipe(&mut self) {
        for s in &mut self.pcm {
            *s = 0;
        }
        std::hint::black_box(self.pcm.as_slice());
    }
}

impl Drop for TurnJob {
    fn drop(&mut self) {
        self.wipe();
    }
}

/// Finished reply: mono PCM16 at [`LANE_RATE_HZ`] (24 kHz) + its WAV bytes.
pub struct TurnReply {
    pub id: u64,
    /// S4 transcript (placeholder when `asr == "interim-placeholder"`).
    pub transcript: String,
    pub text: String,
    /// 24 kHz mono PCM16 of `wav` (drives the me band).
    pub pcm: Vec<i16>,
    pub wav: Vec<u8>,
    pub ms: u32,
    /// `a21-weights` | `system-flite` | `tone`.
    pub backend: &'static str,
    /// `stub-fixture` | `interim-placeholder` | `none` (error).
    pub asr: &'static str,
    pub llm: &'static str,
    /// `"asr=.. llm=.. tts=.."` stage timings.
    pub stages: String,
    /// Register in force when the turn was admitted. Not drawn, not a film voice.
    pub persona: &'static str,
    /// Set when the lane failed and the tone was published instead.
    pub error: Option<String>,
    pub in_ms: u32,
    pub took_ms: u32,
}

impl TurnReply {
    /// Overwrite PCM, the WAV bytes, and the transcript. Also run from [`Drop`].
    pub fn wipe(&mut self) {
        for s in &mut self.pcm {
            *s = 0;
        }
        for b in &mut self.wav {
            *b = 0;
        }
        std::hint::black_box(self.pcm.as_slice());
        std::hint::black_box(self.wav.as_slice());
        wipe_string(&mut self.transcript);
        wipe_string(&mut self.text);
    }
}

impl Drop for TurnReply {
    fn drop(&mut self) {
        self.wipe();
    }
}

fn wipe_string(s: &mut String) {
    let mut v = std::mem::take(s).into_bytes();
    for b in &mut v {
        *b = 0;
    }
    std::hint::black_box(v.as_slice());
}

/// Persona reply text to speak as-is (no ASR, no canned table).
pub struct SpeakJob {
    pub id: u64,
    pub text: String,
}

impl Drop for SpeakJob {
    fn drop(&mut self) {
        wipe_string(&mut self.text);
    }
}

/// Handle owned by the heartbeat loop. The loop only calls [`Self::try_submit`],
/// [`Self::try_speak`], and [`Self::try_recv`]; all three return immediately.
pub struct TurnWorker {
    bus: ProcessBus,
    reply_rx: Receiver<TurnReply>,
    speak_tx: SyncSender<SpeakJob>,
}

impl TurnWorker {
    /// Env-configured lane (`GALION_ASR`, `GALION_LLM`).
    pub fn spawn() -> Self {
        Self::spawn_with(phone_config_from_env())
    }

    /// Same worker with an explicit lane config.
    /// The presence-eval harness uses this so a barge-in probe does not mutate `GALION_ASR`.
    ///
    /// Returns only after the lane exists. Piper pre-warm runs there, so a turn
    /// submitted as soon as `spawn` returns is not stuck behind model load in
    /// the one-slot queue. The 80 ms loop never calls this.
    pub fn spawn_with(cfg: PhoneConfig) -> Self {
        Self::spawn_on_with(ProcessBus::new(), cfg, ProsodySlot::identity())
    }

    /// `bus` is shared with the host. The lane is read from the environment.
    /// The worker thread holds only the cancel gate, so dropping this handle
    /// disconnects the job lane and the thread exits.
    pub fn spawn_on(bus: ProcessBus) -> Self {
        Self::spawn_linked(bus, ProsodySlot::identity())
    }

    /// Same as [`spawn_on`] with the host's prosody slot. The 80 ms loop writes it.
    pub fn spawn_linked(bus: ProcessBus, prosody: ProsodySlot) -> Self {
        Self::spawn_on_with(bus, phone_config_from_env(), prosody)
    }

    fn spawn_on_with(bus: ProcessBus, cfg: PhoneConfig, prosody: ProsodySlot) -> Self {
        let job_rx = bus.take_job_rx().expect("job receiver taken once");
        let gate = bus.gate();
        let (rtx, reply_rx) = sync_channel::<TurnReply>(WORKER_REPLY_DEPTH);
        let (speak_tx, speak_rx) = sync_channel::<SpeakJob>(1);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("galion-turn-worker".into())
            .spawn(move || {
                let mut lane = PhoneLane::new(cfg);
                let _ = ready_tx.send(());
                loop {
                    match speak_rx.try_recv() {
                        Ok(job) => {
                            let turn_id = job.id;
                            if !gate.admit_asr(turn_id) {
                                continue;
                            }
                            let Some(reply) = respond_speak(&mut lane, job, &gate, &prosody) else {
                                let _ = gate.admit_publish(turn_id);
                                continue;
                            };
                            if !gate.admit_publish(reply.id) {
                                continue;
                            }
                            if rtx.try_send(reply).is_err() {
                                gate.note_worker_reply_dropped();
                            }
                            continue;
                        }
                        Err(TryRecvError::Disconnected) => break,
                        Err(TryRecvError::Empty) => {}
                    }
                    match job_rx.recv_timeout(std::time::Duration::from_millis(25)) {
                        Ok(msg) => {
                            let turn_id = msg.turn_id;
                            if !gate.admit_asr(turn_id) {
                                continue; // superseded (barge-in) before ASR
                            }
                            let job = turn_job_from(msg);
                            let Some(reply) = respond(&mut lane, job, &gate, &prosody) else {
                                let _ = gate.admit_publish(turn_id);
                                continue; // superseded while synthesizing, before publish
                            };
                            if !gate.admit_publish(reply.id) {
                                continue;
                            }
                            if rtx.try_send(reply).is_err() {
                                // depth 4 and full, or the host is gone: drop, never block
                                gate.note_worker_reply_dropped();
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("spawn turn worker");
        ready_rx.recv().expect("turn worker stopped during lane start");
        Self { bus, reply_rx, speak_tx }
    }

    pub fn bus(&self) -> &ProcessBus {
        &self.bus
    }

    /// Mark `id` as the newest turn whose reply is still wanted.
    /// Older in-flight work is dropped. The thread keeps running.
    pub fn supersede(&self, id: u64) {
        self.bus.supersede(id);
    }

    /// Speak `text` as the persona already wrote it. `Err(text)` when the slot is full.
    /// An older id does not rewind the cancel token.
    pub fn try_speak(&self, id: u64, text: String) -> Result<(), String> {
        self.supersede(id);
        match self.speak_tx.try_send(SpeakJob { id, text }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(mut job) | TrySendError::Disconnected(mut job)) => {
                Err(std::mem::take(&mut job.text))
            }
        }
    }

    /// Non-blocking submit. `Err(job)` when the depth-1 job lane is full.
    /// An older id does not rewind the cancel token.
    pub fn try_submit(&self, job: TurnJob) -> Result<(), TurnJob> {
        self.supersede(job.id);
        let msg = BusMsg::utterance(
            self.bus.alloc_id(),
            job.id,
            0,
            self.bus.cancel(),
            job.rate,
            pcm16_to_bytes(&job.pcm),
        );
        match self.bus.try_send_job(msg) {
            Ok(()) => Ok(()),
            Err(msg) => Err(turn_job_from(msg)),
        }
    }

    /// Non-blocking poll. A reply that went stale after it was queued is dropped.
    pub fn try_recv(&self) -> Option<TurnReply> {
        loop {
            match self.reply_rx.try_recv() {
                Ok(r) if self.bus.admit_publish(r.id) => return Some(r),
                Ok(_) => continue,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

fn phone_config_from_env() -> PhoneConfig {
    PhoneConfig::from_env().unwrap_or_else(|e| panic!("phone lane: {e}"))
}

fn turn_job_from(msg: BusMsg) -> TurnJob {
    let (rate, pcm) = match msg.body {
        BusBody::Utterance { rate, pcm } => (rate, bytes_to_pcm16(&pcm)),
        _ => (16_000, Vec::new()),
    };
    TurnJob {
        id: msg.turn_id,
        rate,
        pcm,
    }
}

fn respond_speak(lane: &mut PhoneLane, mut job: SpeakJob, gate: &BusGate, prosody: &ProsodySlot) -> Option<TurnReply> {
    let persona = gate.persona_id().as_str();
    let id = job.id;
    lane.set_delivery(persona, prosody.load());
    lane.arm_cancel(gate.cancel_token(), id);
    let t0 = Instant::now();
    let mut text = std::mem::take(&mut job.text);
    let result = lane.speak_fixed(&text);
    wipe_string(&mut text);
    drop(job);
    match result {
        Ok(t) => {
            let pcm = reply_pcm(&t.wav_bytes);
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id,
                transcript: String::new(),
                text: t.reply_text,
                pcm,
                wav: t.wav_bytes,
                ms,
                backend: t.speech_backend,
                asr: t.asr.id(),
                llm: "persona-floor",
                stages: format!("tts={} tts_first={}", t.stage_ms.tts, t.stage_ms.tts_first_pcm),
                error: None,
                in_ms: 0,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
        Err(PhoneError::Superseded) => None,
        Err(PhoneError::Empty) => None,
        Err(e) => {
            let pcm = tone();
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id,
                transcript: String::new(),
                text: String::new(),
                wav: wav_bytes(&pcm, LANE_RATE_HZ),
                pcm,
                ms,
                backend: "tone",
                asr: "text-inject",
                llm: "persona-floor",
                stages: String::new(),
                error: Some(format!("NoSpeech: {e}")),
                in_ms: 0,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
    }
}

fn respond(lane: &mut PhoneLane, mut job: TurnJob, gate: &BusGate, prosody: &ProsodySlot) -> Option<TurnReply> {
    let persona = gate.persona_id().as_str();
    lane.set_delivery(persona, prosody.load());
    lane.arm_cancel(gate.cancel_token(), job.id);
    let t0 = Instant::now();
    let in_ms = (job.pcm.len() as u64 * 1000 / job.rate.max(1) as u64) as u32;
    let mut wav_in = wav_bytes(&job.pcm, job.rate);
    let result = lane.turn_wav(&wav_in);
    for b in &mut wav_in {
        *b = 0;
    }
    std::hint::black_box(wav_in.as_slice());
    job.wipe();
    match result {
        Ok(t) => {
            let pcm = reply_pcm(&t.wav_bytes);
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id: job.id,
                transcript: t.transcript,
                text: t.reply_text,
                pcm,
                wav: t.wav_bytes,
                ms,
                backend: t.speech_backend,
                asr: t.asr.id(),
                llm: t.llm,
                stages: format!(
                    "asr={} llm={} tts={} tts_first={}",
                    t.stage_ms.asr, t.stage_ms.llm, t.stage_ms.tts, t.stage_ms.tts_first_pcm
                ),
                error: None,
                in_ms,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
        Err(PhoneError::Superseded) => None,
        Err(e) => {
            let kind = match &e {
                PhoneError::AsrUnratified(_) => "AsrUnratified (set GALION_ASR=vosk and place the small-en model; docs/asr/VOSK.md)",
                PhoneError::AsrUnavailable(_) => "AsrUnavailable (Vosk model or worker missing; typed text still works; no transcript invented)",
                PhoneError::NoSpeech(_) => "NoSpeech (GALION_SPEECH / Piper weights)",
                PhoneError::BadWav(_) => "BadWav",
                PhoneError::Empty => "Empty",
                _ => "Cascade",
            };
            let pcm = tone();
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id: job.id,
                transcript: String::new(),
                text: String::new(),
                wav: wav_bytes(&pcm, LANE_RATE_HZ),
                pcm,
                ms,
                backend: "tone",
                asr: "none",
                llm: "none",
                stages: String::new(),
                error: Some(format!("{kind}: {e}")),
                in_ms,
                took_ms: t0.elapsed().as_millis() as u32,
                persona,
            })
        }
    }
}

/// 24 kHz mono samples of a PCM16 WAV (data chunk scan; empty on malformed input).
fn reply_pcm(b: &[u8]) -> Vec<i16> {
    let mut i = 12;
    while i + 8 <= b.len() {
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        if &b[i..i + 4] == b"data" {
            let d = &b[i + 8..(i + 8 + n).min(b.len())];
            return d
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
        }
        i += 8 + n + (n & 1);
    }
    Vec::new()
}

/// Clear two-tone "ding-dong" (660 Hz then 440 Hz, 250 ms each, 10 ms ramps) @ 24 kHz.
pub fn tone() -> Vec<i16> {
    let sr = LANE_RATE_HZ as usize;
    let seg = sr / 4;
    let ramp = sr / 100;
    let mut out = Vec::with_capacity(seg * 2 + sr / 10);
    for f in [660.0f32, 440.0] {
        for i in 0..seg {
            let env = (i.min(seg - 1 - i) as f32 / ramp as f32).min(1.0);
            let ph = std::f32::consts::TAU * f * i as f32 / sr as f32;
            out.push((0.35 * env * ph.sin() * i16::MAX as f32) as i16);
        }
    }
    out.extend(std::iter::repeat(0).take(sr / 10));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    struct EnvGuard { saved: Vec<(String, Option<String>)> }
    impl EnvGuard {
        fn set(pairs: &[(&str, &str)]) -> Self {
            let saved = pairs.iter().map(|(k, v)| {
                let prev = std::env::var(k).ok();
                std::env::set_var(k, v);
                ((*k).to_string(), prev)
            }).collect();
            Self { saved }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, prev) in self.saved.drain(..) {
                match prev { Some(v) => std::env::set_var(k, v), None => std::env::remove_var(k) }
            }
        }
    }

    #[test]
    fn tone_is_audible_and_bounded() {
        let t = tone();
        assert_eq!(t.len(), 12_000 + 2_400);
        assert!(t.iter().any(|&s| s.unsigned_abs() > 10_000));
    }
    #[test]
    fn stale_reply_dropped_after_supersede() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]); // -> AsrUnratified -> tone fallback
        let w = TurnWorker::spawn();
        assert!(w
            .try_submit(TurnJob {
                id: 1,
                rate: 16_000,
                pcm: vec![0; 160]
            })
            .is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 5 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let r = got.expect("reply");
        assert_eq!((r.id, r.backend, r.persona), (1, "tone", "samantha"));
        assert!(r
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with("AsrUnratified")));
        assert_eq!(reply_pcm(&r.wav), r.pcm);
        assert!(r.wav.starts_with(b"RIFF"));
        w.supersede(9); // barge-in: anything older than 9 is stale
        assert_eq!(w.bus().cancel(), 9);
        assert!(w.try_recv().is_none());
    }

    #[test]
    fn explicit_jarvis_is_on_the_reply_and_survives_barge_in() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]);
        let w = TurnWorker::spawn();
        w.bus()
            .try_send(crate::bus::BusMsg::persona_explicit(w.bus().alloc_id(), 0, crate::bus::PersonaId::Jarvis))
            .unwrap();
        assert!(w.try_submit(TurnJob { id: 1, rate: 16_000, pcm: vec![0; 160] }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 5 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.expect("reply").persona, "jarvis");
        w.supersede(9);
        assert_eq!(w.bus().persona_id(), crate::bus::PersonaId::Jarvis);
    }

    #[test]
    fn older_submit_does_not_rewind_cancel_and_is_dropped_before_asr() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_ASR", "off")]);
        let w = TurnWorker::spawn();
        w.supersede(9);
        assert!(w
            .try_submit(TurnJob {
                id: 3,
                rate: 16_000,
                pcm: vec![1, 0]
            })
            .is_ok());
        assert_eq!(
            w.bus().cancel(),
            9,
            "an older submit must not rewind cancel"
        );
        let t0 = Instant::now();
        while w.bus().dropped_before_asr() == 0 && t0.elapsed().as_secs() < 5 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            w.bus().dropped_before_asr() >= 1,
            "turn 3 was not dropped before ASR"
        );
        assert!(w.try_recv().is_none());
    }

    #[test]
    fn reply_and_job_pcm_are_zeroised() {
        let mut job = TurnJob { id: 1, rate: 16_000, pcm: vec![9, -9, 12345] };
        job.wipe();
        assert!(job.pcm.iter().all(|&s| s == 0));
        let mut reply = TurnReply {
            id: 1,
            transcript: "SECRET_TRANSCRIPT".into(),
            text: "hello".into(),
            pcm: vec![42, -42],
            wav: b"RIFFPCM".to_vec(),
            ms: 1,
            backend: "tone",
            asr: "none",
            llm: "none",
            stages: String::new(),
            error: None,
            persona: "samantha",
            in_ms: 1,
            took_ms: 1,
        };
        reply.wipe();
        assert!(reply.pcm.iter().all(|&s| s == 0));
        assert!(reply.wav.iter().all(|&b| b == 0));
        assert!(reply.transcript.is_empty() && reply.text.is_empty());
    }

    #[test]
    fn speech_off_publishes_tone_and_error_label() {
        let _lock = env_lock();
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "stub")]);
        let w = TurnWorker::spawn();
        assert!(w.try_submit(TurnJob { id: 3, rate: 16_000, pcm: galion_cascade::stub_fixture_pcm16("hello") }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 8 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let r = got.expect("tone reply");
        assert_eq!((r.id, r.backend), (3, "tone"));
        let err = r.error.as_deref().unwrap_or("");
        assert!(err.contains("NoSpeech"), "{err}");
        assert!(err.contains("GALION_SPEECH=off"), "{err}");
        assert!(r.wav.starts_with(b"RIFF"));
        assert!(r.pcm.iter().any(|&s| s.unsigned_abs() > 1000));
    }
}
