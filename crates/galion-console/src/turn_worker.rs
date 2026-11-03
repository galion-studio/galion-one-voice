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
    pub fn spawn_with(cfg: PhoneConfig) -> Self {
        let (tx, jrx) = sync_channel::<TurnJob>(1);
        let (rtx, rx) = sync_channel::<TurnReply>(4);
        let current = Arc::new(AtomicU64::new(0));
        let cur = current.clone();
        std::thread::Builder::new()
            .name("galion-turn-worker".into())
            .spawn(move || {
                let mut lane = PhoneLane::new(cfg);
                while let Ok(job) = jrx.recv() {
                    if job.id < cur.load(Ordering::Acquire) {
                        continue; // superseded (barge-in) before we started
                    }
                    let r = respond(&mut lane, job);
                    if r.id < cur.load(Ordering::Acquire) {
                        continue; // superseded while synthesizing
                    }
                    if rtx.try_send(r).is_err() {
                        // heartbeat gone or backed up: drop, never block
                    }
                }
            })
            .expect("spawn turn worker");
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

fn respond(lane: &mut PhoneLane, job: TurnJob) -> TurnReply {
    let t0 = Instant::now();
    let in_ms = (job.pcm.len() as u64 * 1000 / job.rate.max(1) as u64) as u32;
    let wav_in = wav_bytes(&job.pcm, job.rate);
    match lane.turn_wav(&wav_in) {
        Ok(t) => {
            let pcm = reply_pcm(&t.wav_bytes);
            let ms = (pcm.len() as u64 * 1000 / LANE_RATE_HZ as u64) as u32;
            TurnReply {
                id: job.id, transcript: t.transcript, text: t.reply_text, pcm, wav: t.wav_bytes, ms,
                backend: t.speech_backend, asr: t.asr.id(), llm: t.llm,
                stages: format!("asr={} llm={} tts={}", t.stage_ms.asr, t.stage_ms.llm, t.stage_ms.tts),
                error: None, in_ms, took_ms: t0.elapsed().as_millis() as u32,
            }
        }
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
            TurnReply {
                id: job.id, transcript: String::new(), text: String::new(), wav: wav_bytes(&pcm, LANE_RATE_HZ), pcm, ms,
                backend: "tone", asr: "none", llm: "none", stages: String::new(),
                error: Some(format!("{kind}: {e}")), in_ms, took_ms: t0.elapsed().as_millis() as u32,
            }
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
    #[test]
    fn tone_is_audible_and_bounded() {
        let t = tone();
        assert_eq!(t.len(), 12_000 + 2_400);
        assert!(t.iter().any(|&s| s.unsigned_abs() > 10_000));
    }
    #[test]
    fn stale_reply_dropped_after_supersede() {
        std::env::set_var("GALION_ASR", "off"); // -> AsrUnratified -> tone fallback
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
}
