// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! # galion-live-demo (W21-01)
//!
//! One-command **live** demo on the real 80 ms path:
//!
//! ```text
//! you WAV (16-bit PCM, any rate → 24 kHz) or synthetic
//!   → 1920-sample frames (80 ms @ 24 kHz)
//!   → RikaReplayBackend::step_live  (Heartbeat::step + Rika + Mock turn-taking, per frame)
//!   → MeVoice::render               (me band; pluggable — W21-02 Mimi / W21-03 cascade)
//!   → FrameSink::on_frame           (console capture; pluggable — W21-04 render/console)
//!   → out/live_output_stereo.wav (L = you, R = me) + console_capture.txt + SHA256SUMS
//! ```
//!
//! Hot-path contract kept: `Heartbeat::step` (inside `step_live`) never blocks and never
//! allocates; the frame loop renders into pre-allocated buffers. Wall-clock pacing
//! (`--realtime`) sleeps **between** frames, never inside `step`. A frame that starts more
//! than one heartbeat behind its wall-clock deadline is passed as `late` → a pause, never a
//! glitch (no catch-up).
//!
//! Resampling: inputs not at 24 kHz (e.g. 16 kHz) are linearly interpolated to 24 kHz
//! once, before the frame loop (demo quality; the Mimi path will own real resampling).

#[cfg(feature = "dot-viz")]
pub mod dot;
pub mod mimi;
pub mod play;
pub mod sha256;

use galion_emotion::band::consent_bits;
use galion_emotion::{Cue, OffReason, Purpose, RikaState};
use galion_heartbeat::presence::RikaReplayBackend;
use galion_heartbeat::symphony::{BandView, DotMark, NanoPcmRing, SymphonyWire};
use galion_heartbeat::{HeartbeatConfig, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};
use galion_presence::backend::{AffectIn, AffectOut, FrameIn as PIn};
use galion_presence::spec::YouState;
use galion_presence::wav;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Reply peak level of the default tone voice.
pub const REPLY_LEVEL: f32 = 0.25;
/// Opener hum peak level: 20·log10(0.25/0.05) = 14 dB below the reply (spec ≥ 12 dB).
pub const HUM_LEVEL: f32 = 0.05;

/// Per-frame context handed to the me-band voice. Plain `Copy` data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeCtx {
    pub index: u32,
    /// Turn-taking says the me band is voiced this frame.
    pub voiced: bool,
    /// First voiced frame of a reply.
    pub onset: bool,
    /// Rika flush (user onset while armed): me band goes silent this frame.
    pub yield_now: bool,
    /// Late frame: pause — hold, start nothing new.
    pub paused: bool,
    /// Linear me-band gain from Rika (1.0 neutral, never > 1).
    pub gain: f32,
    /// Bounded lean-in amount applied this frame (0 = none).
    pub lean_in: f32,
    /// Feeling-band cue applied this frame (never lexical).
    pub cue: Cue,
    /// E-out declared by the backend for this frame.
    pub affect: AffectOut,
}

/// Me-band renderer hook. W21-02 (Mimi decode) / W21-03 (EN cascade) plug in here.
/// `render` is called once per 80 ms frame with `out.len() == FRAME_SAMPLES`, must fill
/// `out` completely, must not block and should not allocate.
pub trait MeVoice {
    fn name(&self) -> &'static str;
    fn reset(&mut self) {}
    /// Per-frame UI label (e.g. "MIMI synthetic (IS_STUB) BYPASS"); empty = none.
    fn tag(&self) -> &'static str {
        ""
    }
    fn render(&mut self, ctx: &MeCtx, out: &mut [f32]);
    /// FP-02: the voice still has reply audio queued (e.g. spoken words mid-sentence).
    /// Fed to turn-taking as `me_planned`, so the heartbeat's `me_voiced` (and with it the
    /// dot's **speaking** state and barge-in yield) lasts exactly as long as the words.
    fn planned(&self) -> bool {
        false
    }
}

/// FP-02 · me band = a pre-rendered spoken reply (EN cascade WAV: A21 Piper CPU
/// voice, flite only as fallback — any 24 kHz mono reply WAV). Each me-turn onset plays the reply from
/// the top; a yield / barge-in stops it; a late frame holds it (no catch-up). While no words
/// play, the inner [`ToneVoice`] renders only Rika's opt-in soft "mm" hum (never the tone reply).
#[derive(Debug, Clone, Default)]
pub struct ReplyVoice {
    reply: Vec<f32>,
    pos: usize,
    playing: bool,
    hum: ToneVoice,
}

impl ReplyVoice {
    /// `reply` is 24 kHz mono (use [`resample_to_24k`] first).
    pub fn new(reply: Vec<f32>) -> Self {
        Self { reply, ..Self::default() }
    }
    pub fn reply_len(&self) -> usize {
        self.reply.len()
    }
}

impl MeVoice for ReplyVoice {
    fn name(&self) -> &'static str {
        "reply-wav (EN cascade words)"
    }
    fn reset(&mut self) {
        self.pos = 0;
        self.playing = false;
        self.hum.reset();
    }
    fn planned(&self) -> bool {
        self.playing && self.pos < self.reply.len()
    }
    fn render(&mut self, c: &MeCtx, out: &mut [f32]) {
        if c.onset && !c.yield_now {
            self.pos = 0;
            self.playing = !self.reply.is_empty();
        }
        if self.playing && (c.yield_now || !c.voiced) && !c.paused {
            self.playing = false; // barge-in / Rika flush: the words stop, never resume mid-sentence
        }
        if self.playing && !c.paused {
            let g = c.gain.clamp(0.0, 1.0);
            let n = out.len().min(self.reply.len() - self.pos);
            for (o, x) in out[..n].iter_mut().zip(&self.reply[self.pos..self.pos + n]) {
                *o = x * g;
            }
            out[n..].fill(0.0);
            self.pos += n;
            if self.pos >= self.reply.len() {
                self.playing = false;
            }
        } else {
            // Not speaking words: only the soft hum (ToneVoice with voiced=false).
            let quiet = MeCtx { voiced: false, onset: false, ..*c };
            self.hum.render(&quiet, out);
        }
    }
}

/// Default me voice: wordless harmonic tone (f0 shaped by E-out arousal) with a
/// syllable-rate envelope, plus the opt-in soft "mm" hum for Rika's `SoftBreath` cue.
/// Click-free: gain ramps linearly across each frame.
#[derive(Debug, Clone, Default)]
pub struct ToneVoice {
    phase: f32,
    syl: f32,
    hum_phase: f32,
    level: f32,
}

impl MeVoice for ToneVoice {
    fn name(&self) -> &'static str {
        "tone"
    }
    fn reset(&mut self) {
        *self = Self::default();
    }
    fn render(&mut self, c: &MeCtx, out: &mut [f32]) {
        let sr = SAMPLE_RATE as f32;
        let speak = c.voiced && !c.yield_now;
        let target = if speak { REPLY_LEVEL * c.gain.clamp(0.0, 1.0) } else { 0.0 };
        let f0 = 150.0 + 80.0 * c.affect.arousal.clamp(0.0, 1.0) + 20.0 * c.lean_in.clamp(0.0, 1.0);
        let hum = !speak && c.cue == Cue::SoftBreath;
        let n = out.len().max(1) as f32;
        let start = self.level;
        let tau = core::f32::consts::TAU;
        for (i, x) in out.iter_mut().enumerate() {
            let lvl = start + (target - start) * (i as f32 + 1.0) / n;
            self.phase = (self.phase + f0 / sr) % 1.0;
            self.syl = (self.syl + 4.0 / sr) % 1.0;
            let env = 0.55 + 0.45 * (tau * self.syl).sin();
            let p = tau * self.phase;
            let mut s = lvl * env * (0.6 * p.sin() + 0.25 * (2.0 * p).sin() + 0.15 * (3.0 * p).sin());
            if hum {
                self.hum_phase = (self.hum_phase + 220.0 / sr) % 1.0;
                let w = (core::f32::consts::PI * (i as f32 + 0.5) / n).sin(); // one-frame Hann
                s += HUM_LEVEL * w * (tau * self.hum_phase).sin();
            }
            *x = s;
        }
        self.level = target;
    }
}

/// One frame as seen by observers (console, render). Plain `Copy` data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRecord {
    pub index: u32,
    pub t_ms: u32,
    pub you: YouState,
    pub you_dbfs: f32,
    pub me_dbfs: f32,
    pub me_voiced: bool,
    pub state: RikaState,
    pub glyph: char,
    pub cue: Cue,
    pub gain: f32,
    pub lean_in_audible: bool,
    pub yield_now: bool,
    pub paused: bool,
    pub late: bool,
    pub off: Option<OffReason>,
    pub affect_out: AffectOut,
    /// Me-voice label for this frame ([`MeVoice::tag`]).
    pub voice_tag: &'static str,
    /// One-dot mark from [`SymphonyWire`] for this frame.
    pub dot: DotMark,
}

/// End-of-run summary.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub frames: u32,
    pub paused_frames: u32,
    pub late_frames: u32,
    pub me_voiced_frames: u32,
    pub me_onsets: u32,
    pub yields: u32,
    pub lean_in_frames: u32,
    pub soft_hum_frames: u32,
    pub lean_in_ms: Option<u32>,
    pub rika_enabled: bool,
    pub explain: &'static str,
    pub backend: &'static str,
    pub voice: &'static str,
}

/// Frame observer hook. W21-04 (one-dot window / live console feeds) plugs in here.
pub trait FrameSink {
    fn on_frame(&mut self, r: &FrameRecord);
    /// Raw per-frame view for renderers (W21-04 `DotConsole::frame`): the heartbeat output,
    /// this frame's you/me PCM, and the measured `step_live` time. Called after the step,
    /// never inside it.
    fn on_heartbeat(&mut self, _hb: &galion_heartbeat::FrameOut, _you: &[f32], _me: &[f32], _me_voiced: bool, _step_us: f32) {}
    fn finish(&mut self, _s: &Summary) {}
}

/// Consent setting from `--consent none|sense|sense,adapt`.
pub fn parse_consent(s: &str) -> Result<u8, String> {
    let mut bits = 0u8;
    for p in s.split(',').map(str::trim) {
        match p {
            "none" | "" => {}
            "sense" => bits |= consent_bits::SENSE,
            "adapt" => bits |= consent_bits::ADAPT,
            o => return Err(format!("--consent: unknown bit '{o}' (none|sense|adapt, comma-separated)")),
        }
    }
    Ok(bits)
}

pub fn parse_purpose(s: &str) -> Result<Purpose, String> {
    Ok(match s {
        "personal" => Purpose::PersonalAssistant,
        "accessibility" => Purpose::Accessibility,
        "medical" => Purpose::MedicalOrSafety,
        "workplace" => Purpose::Workplace,
        "education" => Purpose::Education,
        o => return Err(format!("--purpose: unknown '{o}' (personal|accessibility|medical|workplace|education)")),
    })
}

fn prohibited(p: Purpose) -> bool {
    matches!(p, Purpose::Workplace | Purpose::Education)
}

/// Settings of one frame-loop run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunCfg {
    pub rika_opener: bool,
    pub consent_bits: u8,
    pub purpose: Purpose,
    /// Pace frames to the wall clock (sleep between frames).
    pub realtime: bool,
}

impl Default for RunCfg {
    fn default() -> Self {
        Self { rika_opener: false, consent_bits: 0, purpose: Purpose::PersonalAssistant, realtime: false }
    }
}

/// Heartbeat config for the demo: Rika OFF unless the opener is requested (Q14 / Q11 opt-in);
/// with the opener, v0 thresholds + soft acknowledgment ("mm" hum, never words).
pub fn heartbeat_config(rika_opener: bool) -> HeartbeatConfig {
    if rika_opener {
        let mut c = HeartbeatConfig::rika_v0();
        c.rika.soft_ack = true;
        c
    } else {
        HeartbeatConfig::default()
    }
}

/// The frame loop. `you` is 24 kHz mono; it is processed in 80 ms frames (a trailing partial
/// frame is zero-padded). Returns the me band (same length as the padded you band).
pub fn run_frames(you: &[f32], cfg: &RunCfg, voice: &mut dyn MeVoice, sinks: &mut [&mut dyn FrameSink]) -> (Vec<f32>, Summary) {
    run_frames_draining(you, cfg, voice, sinks, None)
}

/// [`run_frames`] with the clone ring attached.
/// `None` keeps the me-slot path. A full ring frame replaces that frame's
/// audible me buffer after `step`. The model call is not made here.
pub fn run_frames_draining(
    you: &[f32],
    cfg: &RunCfg,
    voice: &mut dyn MeVoice,
    sinks: &mut [&mut dyn FrameSink],
    mut nano: Option<&mut NanoPcmRing>,
) -> (Vec<f32>, Summary) {
    let frames = you.len().div_ceil(FRAME_SAMPLES) as u32;
    let mut backend = RikaReplayBackend::with_config(heartbeat_config(cfg.rika_opener));
    voice.reset();
    let mut me = vec![0.0f32; frames as usize * FRAME_SAMPLES];
    let mut you_buf = [0.0f32; FRAME_SAMPLES];
    let gate = wav::Gate::default();
    let sense = cfg.consent_bits & consent_bits::SENSE != 0 && !prohibited(cfg.purpose);
    let adapt = cfg.consent_bits & consent_bits::ADAPT != 0 && !prohibited(cfg.purpose);
    let mut s = Summary {
        frames,
        paused_frames: 0,
        late_frames: 0,
        me_voiced_frames: 0,
        me_onsets: 0,
        yields: 0,
        lean_in_frames: 0,
        soft_hum_frames: 0,
        lean_in_ms: None,
        rika_enabled: backend.heartbeat().config().rika.enabled,
        explain: "",
        backend: "mock+heartbeat-rika (step_live)",
        voice: voice.name(),
    };
    let frame_dur = Duration::from_millis(FRAME_MS as u64);
    let t0 = Instant::now();
    let mut prev_voiced = false;
    let mut wire = SymphonyWire::new();
    let mut scratch = [0.0f32; FRAME_SAMPLES];
    for i in 0..frames {
        let a = i as usize * FRAME_SAMPLES;
        let b = (a + FRAME_SAMPLES).min(you.len());
        you_buf[..b - a].copy_from_slice(&you[a..b]);
        you_buf[b - a..].fill(0.0);
        // Late = this frame starts more than one whole heartbeat behind its slot.
        let late = cfg.realtime && t0.elapsed() > frame_dur * (i + 1);
        let you_dbfs = wav::rms_dbfs(&you_buf);
        let you_state = if you_dbfs >= gate.speech_dbfs {
            YouState::Speech
        } else if you_dbfs >= gate.breath_dbfs {
            YouState::Breath
        } else {
            YouState::Silence
        };
        // E-in proxy for the mock's E-out (only with SENSE, never for prohibited purposes).
        let affect_in = sense.then(|| {
            let ar = ((you_dbfs + 50.0) / 40.0).clamp(0.0, 1.0);
            AffectIn { valence: 0.5, arousal: ar }
        });
        let pin = PIn { index: i, you: you_state, affect_in, adapt, late, me_planned: voice.planned() };
        let ts = Instant::now(); // clock read outside Heartbeat::step (timing for the console)
        let (po, ho) = backend.step_live(&pin, &you_buf, cfg.consent_bits, cfg.purpose);
        let step_us = ts.elapsed().as_secs_f32() * 1e6;
        let ctx = MeCtx {
            index: i,
            voiced: po.me_voiced,
            onset: po.me_voiced && !prev_voiced,
            yield_now: ho.yield_now(),
            paused: ho.paused,
            gain: ho.me_gain(),
            lean_in: ho.applied.lean_in,
            cue: ho.applied.cue,
            affect: po.affect_out,
        };
        let out = &mut me[a..a + FRAME_SAMPLES];
        voice.render(&ctx, out);
        let sector = {
            let emit = wire.me_emit(&ho, out.len());
            let me_slot: &[f32] = if emit && ctx.voiced { &*out } else { &[] };
            wire.step(
                &ho,
                BandView {
                    you_active: you_state == YouState::Speech,
                    feeling_e: if sense { po.affect_out.arousal } else { 0.0 },
                    sense,
                    me_slot,
                },
                &mut scratch,
                nano.as_deref_mut(),
            )
        };
        if sector.release_slot {
            if let Some(ring) = nano.as_deref_mut() {
                ring.release();
            }
        }
        if sector.nano_frame {
            out.copy_from_slice(&scratch);
        }
        prev_voiced = po.me_voiced;
        s.paused_frames += ho.paused as u32;
        s.late_frames += late as u32;
        s.me_voiced_frames += po.me_voiced as u32;
        s.me_onsets += ctx.onset as u32;
        s.yields += ctx.yield_now as u32;
        s.lean_in_frames += ho.lean_in_audible() as u32;
        s.soft_hum_frames += (ctx.cue == Cue::SoftBreath && !(ctx.voiced && !ctx.yield_now)) as u32;
        let rec = FrameRecord {
            index: i,
            t_ms: i * FRAME_MS,
            you: you_state,
            you_dbfs,
            me_dbfs: wav::rms_dbfs(out),
            me_voiced: po.me_voiced,
            state: ho.decided.state,
            glyph: ho.glyph(),
            cue: ho.applied.cue,
            gain: ctx.gain,
            lean_in_audible: ho.lean_in_audible(),
            yield_now: ctx.yield_now,
            paused: ho.paused,
            late,
            off: ho.decided.off,
            affect_out: po.affect_out,
            voice_tag: voice.tag(),
            dot: sector.dot,
        };
        for k in sinks.iter_mut() {
            k.on_frame(&rec);
            k.on_heartbeat(&ho, &you_buf, out, po.me_voiced, step_us);
        }
        if cfg.realtime {
            // Pace to the wall clock between frames — never inside Heartbeat::step.
            let deadline = frame_dur * (i + 1);
            let now = t0.elapsed();
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
    }
    s.lean_in_ms = backend.heartbeat().lean_in_ms();
    s.explain = backend.heartbeat().explain();
    for k in sinks.iter_mut() {
        k.finish(&s);
    }
    (me, s)
}

fn bar(db: f32) -> String {
    let n = (((db + 60.0) / 60.0) * 20.0).round().clamp(0.0, 20.0) as usize;
    let mut b = "#".repeat(n);
    b.push_str(&".".repeat(20 - n));
    b
}

/// Text console capture (the `console_capture.txt` format; deterministic unless `--realtime`).
#[derive(Debug, Default)]
pub struct CaptureSink {
    pub text: String,
    /// Echo each line to stdout as it happens (the "live" view).
    pub echo: bool,
}

impl CaptureSink {
    pub fn new(header: &str, echo: bool) -> Self {
        let mut text = String::with_capacity(64 * 1024);
        text.push_str(header);
        text.push_str("t(s)   YOU band             ME band              you     rika gain  cue        flags\n");
        if echo {
            print!("{text}");
        }
        Self { text, echo }
    }
    fn push(&mut self, line: &str) {
        if self.echo {
            println!("{line}");
        }
        self.text.push_str(line);
        self.text.push('\n');
    }
}

impl FrameSink for CaptureSink {
    fn on_frame(&mut self, r: &FrameRecord) {
        let you = match r.you {
            YouState::Silence => "silence",
            YouState::Breath => "breath ",
            YouState::Speech => "speech ",
        };
        let cue = match r.cue {
            Cue::None => "-        ",
            Cue::LeanIn => "lean-in  ",
            Cue::SoftBreath => "soft-hum ",
        };
        let mut flags = String::new();
        if r.me_voiced {
            flags.push_str("ME ");
        }
        if r.lean_in_audible {
            flags.push_str("LEAN ");
        }
        if r.yield_now {
            flags.push_str("YIELD ");
        }
        if r.paused {
            flags.push_str("PAUSE ");
        }
        if r.late {
            flags.push_str("LATE ");
        }
        match r.off {
            Some(o) => flags.push_str(&format!("OFF({o:?})")),
            None => flags.push_str(&format!("{:?}", r.state)),
        }
        if !r.voice_tag.is_empty() && r.me_dbfs > -90.0 {
            flags.push_str(" | ");
            flags.push_str(r.voice_tag);
        }
        let line = format!(
            "{:5.2}  {} {} {} {}    {:4.2}  {} {}",
            r.t_ms as f32 / 1000.0,
            bar(r.you_dbfs),
            bar(r.me_dbfs),
            you,
            if r.off.is_some() { '-' } else { r.glyph },
            r.gain,
            cue,
            flags.trim_end()
        );
        self.push(&line);
    }
    fn finish(&mut self, s: &Summary) {
        let lean = s.lean_in_ms.map_or("n/a".to_string(), |m| format!("{m} ms"));
        let line = format!(
            "--\nsummary: frames={} ({} ms) paused={} late={} me_voiced={} me_onsets={} yields={} lean_in_frames={} soft_hum_frames={} rika={} lean_in_latency={}\n\
             rika explain: \"{}\"\nbackend={} voice={}",
            s.frames,
            s.frames * FRAME_MS,
            s.paused_frames,
            s.late_frames,
            s.me_voiced_frames,
            s.me_onsets,
            s.yields,
            s.lean_in_frames,
            s.soft_hum_frames,
            if s.rika_enabled { "ON(v0+soft_ack)" } else { "OFF(default)" },
            lean,
            s.explain,
            s.backend,
            s.voice
        );
        self.push(&line);
    }
}

/// Deterministic LCG noise in [-1, 1].
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// Synthetic you band (24 kHz mono, deterministic, 202 frames = 16.16 s): room tone, a breath
/// before speaking (Rika R1 moment), two user turns, a mid-session hesitant breath (soft-hum
/// opener when opted in), and a barge-in into the reply.
/// No voices, no weights — tones and seeded noise only.
pub fn synth_you() -> Vec<f32> {
    #[derive(Clone, Copy)]
    enum K {
        Room,
        Breath,
        Voice,
    }
    let script: &[(K, u32)] = &[
        (K::Room, 13),
        (K::Breath, 5), // breath before speaking → Rika R1 lean-in (160 ms), caught on the voice
        (K::Voice, 20), // turn 1
        (K::Room, 94),  // me replies, then quiet room until Rika's 8 s refractory ends
        (K::Breath, 16), // hesitant breath mid-session → armed → (opener) one soft "mm" hum
        (K::Voice, 18), // turn 2
        (K::Room, 6),   // me reply starts ...
        (K::Voice, 6),  // ... barge-in: me yields
        (K::Room, 24),
    ];
    let mut rng = Lcg(13);
    let mut v = Vec::with_capacity(script.iter().map(|s| s.1 as usize).sum::<usize>() * FRAME_SAMPLES);
    let sr = SAMPLE_RATE as f32;
    let tau = core::f32::consts::TAU;
    for &(k, n) in script {
        let len = n as usize * FRAME_SAMPLES;
        for j in 0..len {
            let t = j as f32 / sr;
            let x = match k {
                K::Room => 0.001 * 3f32.sqrt() * rng.next(),
                K::Breath => 0.003 * 3f32.sqrt() * rng.next(),
                K::Voice => {
                    let f0 = 180.0 + 15.0 * (tau * 3.0 * t).sin();
                    let p = tau * f0 * t;
                    let env = 0.6 + 0.4 * (tau * 4.0 * t).sin().abs();
                    let edge = (j.min(len - 1 - j) as f32 / 240.0).min(1.0);
                    0.12 * env * edge * (0.7 * p.sin() + 0.2 * (2.0 * p).sin() + 0.1 * (3.0 * p).sin())
                }
            };
            v.push(x);
        }
    }
    v
}

/// Linear resample to 24 kHz (identity at 24 kHz).
pub fn resample_to_24k(x: &[f32], sr: u32) -> Vec<f32> {
    if sr == SAMPLE_RATE || x.is_empty() {
        return x.to_vec();
    }
    let n = (x.len() as u64 * SAMPLE_RATE as u64 / sr as u64) as usize;
    let step = sr as f64 / SAMPLE_RATE as f64;
    (0..n)
        .map(|i| {
            let p = i as f64 * step;
            let k = p as usize;
            let f = (p - k as f64) as f32;
            let a = x[k.min(x.len() - 1)];
            let b = x[(k + 1).min(x.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

fn to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// Full demo config (the CLI).
#[derive(Debug, Clone)]
pub struct Config {
    pub input: Option<PathBuf>,
    pub out_dir: PathBuf,
    pub run: RunCfg,
    /// Echo console lines to stdout while running.
    pub echo: bool,
}

/// Paths written by [`run_demo`].
#[derive(Debug, Clone)]
pub struct Artifacts {
    pub input_wav: PathBuf,
    pub stereo_wav: PathBuf,
    pub capture: PathBuf,
    pub sums: PathBuf,
    pub summary: Summary,
    pub stereo_frames: usize,
}

fn consent_label(bits: u8) -> String {
    let mut v = Vec::new();
    if bits & consent_bits::SENSE != 0 {
        v.push("sense");
    }
    if bits & consent_bits::ADAPT != 0 {
        v.push("adapt");
    }
    if v.is_empty() {
        "none".into()
    } else {
        v.join(",")
    }
}

/// Read/synthesise → frame loop → write stereo WAV, capture and SHA256SUMS.
pub fn run_demo(cfg: &Config, voice: &mut dyn MeVoice, extra: &mut [&mut dyn FrameSink]) -> Result<Artifacts, String> {
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| format!("cannot create {}: {e}", cfg.out_dir.display()))?;
    let (you, src) = match &cfg.input {
        Some(p) => {
            let pcm = wav::read_wav(p)?;
            let note = if pcm.sample_rate == SAMPLE_RATE {
                format!("{} ({} Hz, {} ch, {} bit)", p.display(), pcm.sample_rate, pcm.channels, pcm.bits_per_sample)
            } else {
                format!("{} ({} Hz → 24000 Hz linear resample, {} ch, {} bit)", p.display(), pcm.sample_rate, pcm.channels, pcm.bits_per_sample)
            };
            (resample_to_24k(&pcm.samples, pcm.sample_rate), note)
        }
        None => (synth_you(), "synthetic (room/breath/tone script, seed 13)".to_string()),
    };
    if you.is_empty() {
        return Err("input has no samples".into());
    }
    let input_wav = cfg.out_dir.join("live_input_24k.wav");
    let stereo_wav = cfg.out_dir.join("live_output_stereo.wav");
    let capture = cfg.out_dir.join("console_capture.txt");
    let sums = cfg.out_dir.join("SHA256SUMS");
    let r = &cfg.run;
    let header = format!(
        "GALION VOICE live demo (W21-01) | backend=mock+heartbeat-rika (Heartbeat::step, 80 ms) | voice={}\n\
         in={src}\nsr=24000 frame_ms={FRAME_MS} frame_samples={FRAME_SAMPLES} | consent={} purpose={:?} | rika opener: {} | pacing: {}\n",
        voice.name(),
        consent_label(r.consent_bits),
        r.purpose,
        if r.rika_opener { "requested" } else { "off (default)" },
        if r.realtime { "realtime (wall clock)" } else { "offline (as fast as possible)" }
    );
    let mut cap = CaptureSink::new(&header, cfg.echo);
    let (me, summary) = {
        let mut sinks: Vec<&mut dyn FrameSink> = Vec::with_capacity(1 + extra.len());
        sinks.push(&mut cap);
        for e in extra.iter_mut() {
            sinks.push(&mut **e);
        }
        run_frames(&you, r, voice, &mut sinks)
    };
    let mut inter = Vec::with_capacity(me.len() * 2);
    for (i, m) in me.iter().enumerate() {
        inter.push(to_i16(you.get(i).copied().unwrap_or(0.0)));
        inter.push(to_i16(*m));
    }
    let mono: Vec<i16> = you.iter().map(|x| to_i16(*x)).collect();
    let files: [(&Path, Vec<u8>); 3] = [
        (&input_wav, wav::encode_pcm16(SAMPLE_RATE, 1, &mono)),
        (&stereo_wav, wav::encode_pcm16(SAMPLE_RATE, 2, &inter)),
        (&capture, cap.text.clone().into_bytes()),
    ];
    let mut sum_txt = String::new();
    for (p, bytes) in &files {
        std::fs::write(p, bytes).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        sum_txt.push_str(&format!("{}  {}\n", sha256::hex(bytes), p.file_name().unwrap().to_string_lossy()));
    }
    std::fs::write(&sums, sum_txt).map_err(|e| format!("cannot write {}: {e}", sums.display()))?;
    Ok(Artifacts { input_wav, stereo_wav, capture, sums, summary, stereo_frames: me.len() })
}
