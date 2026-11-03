// Galion Voice — Perpetual Open MIT License
//! S7 · phone turn worker (feature `phone-turn`): one long-lived thread off the 80 ms
//! heartbeat (TURN-WORKER.md). The heartbeat `try_submit`s a finished utterance and
//! `try_recv`s the reply each tick; it never blocks on model work.
//!
//! Pipeline: [`galion_cascade::PhoneLane`] (S4 ASR → S5 LLM → S6 TTS), built once from env
//! inside the worker thread (`GALION_ASR`, `GALION_LLM`, `GALION_SPEECH`). S4 English ASR is
//! **not ratified**: `GALION_ASR=interim` is a length-based placeholder transcript, labelled
//! `asr=interim-placeholder` in every reply. Errors (e.g. `AsrUnratified` when `GALION_ASR` is
//! unset/stub/off, `NoSpeech`) are reported in [`TurnReply::error`] and a clear two-tone WAV is
//! published instead, so the phone still hears that its turn arrived.
//! `GALION_SPEECH=off` is that path: two-tone WAV, `backend=tone`, `error` set.
//! A superseded turn drops Piper clauses that have not started and does not kill the worker.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::time::Instant;

use galion_cascade::en_lane::{wav_bytes, LANE_RATE_HZ};
use galion_cascade::{PhoneConfig, PhoneError, PhoneLane};

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

/// Handle owned by the heartbeat loop.
pub struct TurnWorker {
    tx: SyncSender<TurnJob>,
    rx: Receiver<TurnReply>,
    /// Latest id the heartbeat cares about; the worker skips jobs older than this.
    current: Arc<AtomicU64>,
}

impl TurnWorker {
    /// Env-configured lane (`GALION_ASR`, `GALION_LLM`).
    pub fn spawn() -> Self { Self::spawn_with(PhoneConfig::from_env()) }

    /// Same worker with an explicit lane config.
    /// The presence-eval harness uses this so a barge-in probe does not mutate `GALION_ASR`.
    ///
    /// Returns only after the lane exists. Piper pre-warm runs there, so a turn
    /// submitted as soon as `spawn` returns is not stuck behind model load in
    /// the one-slot queue. The 80 ms loop never calls this.
    pub fn spawn_with(cfg: PhoneConfig) -> Self {
        let (tx, jrx) = sync_channel::<TurnJob>(1);
        let (rtx, rx) = sync_channel::<TurnReply>(4);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        let current = Arc::new(AtomicU64::new(0));
        let cur = current.clone();
        std::thread::Builder::new()
            .name("galion-turn-worker".into())
            .spawn(move || {
                let mut lane = PhoneLane::new(cfg);
                let _ = ready_tx.send(());
                while let Ok(job) = jrx.recv() {
                    if job.id < cur.load(Ordering::Acquire) {
                        continue; // superseded (barge-in) before we started
                    }
                    let Some(r) = respond(&mut lane, job, &cur) else { continue };
                    if r.id < cur.load(Ordering::Acquire) {
                        continue; // superseded while synthesizing
                    }
                    if rtx.try_send(r).is_err() {
                        // heartbeat gone or backed up: drop, never block
                    }
                }
            })
            .expect("spawn turn worker");
        ready_rx.recv().expect("turn worker stopped during lane start");
        Self { tx, rx, current }
    }

    /// Mark `id` as the only turn whose reply is still wanted (cancels older in-flight work).
    pub fn supersede(&self, id: u64) {
        self.current.store(id, Ordering::Release);
    }

    /// Non-blocking submit. `Err(job)` when the worker is busy with another turn.
    pub fn try_submit(&self, job: TurnJob) -> Result<(), TurnJob> {
        self.supersede(job.id);
        match self.tx.try_send(job) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(j)) | Err(TrySendError::Disconnected(j)) => Err(j),
        }
    }

    /// Non-blocking poll for a finished reply that is still current.
    pub fn try_recv(&self) -> Option<TurnReply> {
        loop {
            match self.rx.try_recv() {
                Ok(r) if r.id >= self.current.load(Ordering::Acquire) => return Some(r),
                Ok(_) => continue, // stale
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

fn respond(lane: &mut PhoneLane, mut job: TurnJob, current: &Arc<AtomicU64>) -> Option<TurnReply> {
    if job.id < current.load(Ordering::Acquire) { return None; }
    lane.arm_cancel(Arc::clone(current), job.id);
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
                id: job.id, transcript: t.transcript, text: t.reply_text, pcm, wav: t.wav_bytes, ms,
                backend: t.speech_backend, asr: t.asr.id(), llm: t.llm,
                stages: format!("asr={} llm={} tts={} tts_first={}", t.stage_ms.asr, t.stage_ms.llm, t.stage_ms.tts, t.stage_ms.tts_first_pcm),
                error: None, in_ms, took_ms: t0.elapsed().as_millis() as u32,
            })
        }
        Err(PhoneError::Superseded) => None,
        Err(e) => {
            let kind = match &e {
                PhoneError::AsrUnratified(_) => "AsrUnratified (set GALION_ASR=interim for the placeholder lane; real EN ASR awaits ratification)",
                PhoneError::NoSpeech(_) => "NoSpeech (GALION_SPEECH / Piper weights)",
                PhoneError::BadWav(_) => "BadWav",
                PhoneError::Empty => "Empty",
                _ => "Cascade",
            };
            let pcm = tone();
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            Some(TurnReply {
                id: job.id, transcript: String::new(), text: String::new(), wav: wav_bytes(&pcm, LANE_RATE_HZ), pcm, ms,
                backend: "tone", asr: "none", llm: "none", stages: String::new(),
                error: Some(format!("{kind}: {e}")), in_ms, took_ms: t0.elapsed().as_millis() as u32,
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
            return d.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
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

    fn voice(secs: f32) -> Vec<i16> {
        let n = (secs * 16_000.0) as usize;
        (0..n).map(|i| {
            let t = i as f32 / 16_000.0;
            (0.2 * (t * 180.0 * std::f32::consts::TAU).sin() * (t * 3.0 * std::f32::consts::TAU).sin().abs() * 32767.0) as i16
        }).collect()
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
        assert!(w.try_submit(TurnJob { id: 1, rate: 16_000, pcm: vec![0; 160] }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 5 { got = w.try_recv(); std::thread::sleep(std::time::Duration::from_millis(5)); }
        let r = got.expect("reply");
        assert_eq!((r.id, r.backend), (1, "tone"));
        assert!(r.error.as_deref().is_some_and(|e| e.starts_with("AsrUnratified")));
        assert_eq!(reply_pcm(&r.wav), r.pcm);
        assert!(r.wav.starts_with(b"RIFF"));
        w.supersede(9); // barge-in: anything older than 9 is stale
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
        let _env = EnvGuard::set(&[("GALION_SPEECH", "off"), ("GALION_ASR", "interim")]);
        let w = TurnWorker::spawn();
        assert!(w.try_submit(TurnJob { id: 3, rate: 16_000, pcm: voice(1.2) }).is_ok());
        let t0 = Instant::now();
        let mut got = None;
        while got.is_none() && t0.elapsed().as_secs() < 8 {
            got = w.try_recv();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let r = got.expect("tone reply");
        assert_eq!((r.id, r.backend), (3, "tone"));
        let err = r.error.expect("error label");
        assert!(err.contains("NoSpeech"), "{err}");
        assert!(err.contains("GALION_SPEECH=off"), "{err}");
        assert!(r.wav.starts_with(b"RIFF"));
        assert!(r.pcm.iter().any(|&s| s.unsigned_abs() > 1000));
    }
}
