// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-04 · heartbeat → one dot + live console bars (feature `dot-viz`).
//!
//! The bridge W21-01 attaches to its live loop. Per 80 ms frame:
//!
//! ```text
//! FrameOut (galion-heartbeat) ─┐
//! you/me PCM levels ───────────┼─► viz_from_heartbeat() ─► VizFrame ─► Fanout (galion-render)
//! LiveSnapshot (timing+affect) ┘                                       ├─ PngSeqSurface (headless PNG)
//!                                                                      ├─ TerminalDotSurface / WebSurface
//!                                                                      └─ WindowSurface (stub → falls back)
//! ```
//!
//! [`DemoPump`] pumps any [`TimingFeed`] (e.g. `TensorTimingFeed`) and any
//! [`AffectFeed`] (e.g. `EmotionAffectFeed`) from the caller's loop, with the
//! affect feed reading the **same** you-band PCM the heartbeat stepped on.
//! [`DotConsole`] owns the fanout plus a Rika glyph history and draws the
//! developer panel (timing / emotion bars) as plain ANSI text — no TTY needed.
//!
//! Nothing here runs inside `Heartbeat::step()`; it is called after it, on the
//! host's loop, and every surface is best-effort (Fanout drops a failing one).

use std::collections::VecDeque;

use galion_heartbeat::{FrameOut, FRAME_MS};
use galion_render::{dot, Emotion, Fanout, RenderSurface, VizFrame};

use crate::live::{AffectFeed, LiveSnapshot, Stage, StageLatencies, TimingFeed};

/// V1-01 single screen: the one dot, fullscreen, nothing else. Served at `/` and
/// `/screen` by `galion_render::WebSurface::with_page` (feature `dot-viz-web`); polls
/// `/dot` every 80 ms. See docs/agent-v1-demo/V1-01-STATUS.md.
pub const SCREEN_HTML: &str = include_str!("../assets/v1-screen.html");

/// Root-mean-square of a PCM slice (0 for empty).
pub fn rms(pcm: &[f32]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len() as f32).sqrt()
}

/// RMS above which a band counts as voicing (room tone ≈ 0.001, breath ≈ 0.003).
pub const ACTIVE_RMS: f32 = 0.01;
/// RMS that maps to level 1.0.
pub const FULL_RMS: f32 = 0.15;

/// RMS → 0..=1 display level.
pub fn level(rms: f32) -> f32 {
    if rms.is_finite() { (rms / FULL_RMS).clamp(0.0, 1.0) } else { 0.0 }
}

/// Coarse label from valence/arousal (display only; never fed back into audio).
pub fn affect_label(valence: f32, arousal: f32) -> &'static str {
    match (valence > 0.15, valence < -0.15, arousal > 0.55) {
        (true, _, true) => "bright",
        (true, _, false) => "warm",
        (_, true, true) => "tense",
        (_, true, false) => "low",
        (false, false, true) => "energised",
        _ => "calm",
    }
}

/// Build the render frame for heartbeat frame `out`. `me_voiced` = the me band
/// actually produced sound this frame (after `yield_now` / pause handling).
pub fn viz_from_heartbeat(out: &FrameOut, you_pcm: &[f32], me_pcm: &[f32], me_voiced: bool, snap: &LiveSnapshot) -> VizFrame {
    let (you_rms, me_rms) = (rms(you_pcm), rms(me_pcm));
    let mut f = VizFrame {
        seq: out.index as u64,
        t_ms: out.index as u64 * FRAME_MS as u64,
        latency_ms: snap.lat.total(),
        rtf: snap.lat.total() / FRAME_MS as f32,
        ..Default::default()
    };
    f.listen.level = level(you_rms);
    f.listen.active = you_rms > ACTIVE_RMS;
    if let Some(a) = snap.affect {
        f.listen.emotion = Emotion { label: affect_label(a.valence, a.arousal), intensity: a.arousal.clamp(0.0, 1.0), valence: a.valence.clamp(-1.0, 1.0) };
    }
    f.speak.level = level(me_rms);
    f.speak.active = me_voiced && !out.paused;
    let lean = out.applied.lean_in.clamp(0.0, 1.0);
    f.speak.emotion = Emotion { label: if lean > 0.0 { "lean-in" } else { "neutral" }, intensity: lean, valence: 0.0 };
    f
}

/// Pumps a timing feed and an affect feed from the host loop.
pub struct DemoPump<T: TimingFeed, A: AffectFeed> {
    pub timing: T,
    pub affect: A,
}

impl<T: TimingFeed, A: AffectFeed> DemoPump<T, A> {
    pub fn new(timing: T, affect: A) -> Self {
        Self { timing, affect }
    }
    /// One heartbeat tick. `you_pcm` is the frame the heartbeat just stepped on
    /// (24 kHz); SENSE reads it only when the affect feed has consent.
    pub fn pump(&mut self, tick: u64, you_pcm: &[f32], sample_rate: u32) -> LiveSnapshot {
        let t = self.timing.step(tick);
        let mut lat = StageLatencies::default();
        for (st, v) in [(Stage::Capture, t.capture), (Stage::Think, t.think), (Stage::Speak, t.speak)] {
            if let Some(ms) = v {
                lat.set(st, ms);
            }
        }
        let sense = self.affect.sense();
        let a = self.affect.read(tick, you_pcm, sample_rate);
        if let (true, Some(ms)) = (sense, a.latency_ms) {
            lat.set(Stage::Sense, ms);
        }
        LiveSnapshot { lat, sense, affect: if sense { a.reading } else { None }, ops: t.ops, gflops: t.gflops }
    }
    pub fn names(&self) -> (&'static str, &'static str) {
        (self.timing.name(), self.affect.name())
    }
}

/// Panel options.
#[derive(Clone, Copy, Debug)]
pub struct PanelOpts {
    pub color: bool,
    /// Dot area in terminal cells (0 rows = no dot, panel only).
    pub dot_cols: u16,
    pub dot_rows: u16,
    pub bar_width: usize,
}

impl Default for PanelOpts {
    fn default() -> Self {
        Self { color: true, dot_cols: 48, dot_rows: 12, bar_width: 32 }
    }
}

/// Running stats for the summary line.
#[derive(Clone, Copy, Debug, Default)]
pub struct DotStats {
    pub frames: u64,
    pub paused: u64,
    pub lean_in_frames: u64,
    pub yields: u64,
    pub sum_total_ms: f64,
    pub max_total_ms: f32,
    pub over_budget: u64,
    pub max_step_us: f32,
}

/// Fanout + glyph history + panel renderer.
pub struct DotConsole {
    pub fanout: Fanout,
    glyphs: VecDeque<char>,
    pub stats: DotStats,
    pub last: Option<VizFrame>,
}

/// Width of the Rika glyph history row.
pub const GLYPH_HISTORY: usize = 48;
/// Compute budget inside the 80 ms frame (LOCKS `frame_budget_p95_ms`).
pub const BUDGET_MS: f32 = 60.0;

impl Default for DotConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl DotConsole {
    pub fn new() -> Self {
        Self { fanout: Fanout::new(), glyphs: VecDeque::with_capacity(GLYPH_HISTORY), stats: DotStats::default(), last: None }
    }

    /// Attach a surface; on failure returns the reason (caller logs and continues).
    pub fn attach(&mut self, s: Box<dyn RenderSurface>) -> Result<(), String> {
        self.fanout.attach(s).map_err(|e| e.to_string())
    }

    /// Feed one heartbeat frame: builds the `VizFrame`, presents it on every surface,
    /// updates glyph history and stats. `step_us` = measured `Heartbeat::step` time.
    pub fn frame(&mut self, out: &FrameOut, you_pcm: &[f32], me_pcm: &[f32], me_voiced: bool, snap: &LiveSnapshot, step_us: f32) -> &VizFrame {
        let viz = viz_from_heartbeat(out, you_pcm, me_pcm, me_voiced, snap);
        self.fanout.present(&viz);
        if self.glyphs.len() == GLYPH_HISTORY {
            self.glyphs.pop_front();
        }
        let g = out.glyph();
        self.glyphs.push_back(if g == ' ' { '_' } else { g });
        let s = &mut self.stats;
        s.frames += 1;
        s.paused += out.paused as u64;
        s.lean_in_frames += out.lean_in_audible() as u64;
        s.yields += out.yield_now() as u64;
        let total = snap.lat.total();
        s.sum_total_ms += total as f64;
        s.max_total_ms = s.max_total_ms.max(total);
        s.over_budget += (total > BUDGET_MS) as u64;
        s.max_step_us = s.max_step_us.max(step_us);
        self.last = Some(viz);
        self.last.as_ref().unwrap()
    }

    pub fn glyph_row(&self) -> String {
        self.glyphs.iter().collect()
    }

    /// One-line summary (headless logs).
    pub fn summary_line(&self, out: &FrameOut, snap: &LiveSnapshot) -> String {
        let v = self.last.clone().unwrap_or_default();
        let d = dot::DotParams::from_frame(&v);
        let aff = match snap.affect {
            Some(a) => format!("v{:+.2} a{:.2} c{:.2} {}", a.valence, a.arousal, a.confidence, v.listen.emotion.label),
            None => "no reading".into(),
        };
        format!(
            "t={:>6.2}s f{:<4} dot={:<9} r={:.3} b={:.2} | you {:.2} me {:.2} | rika {} {:?} | total {:>5.1}ms (cap {:.1} sense {} think {:.1} speak {:.1}) | affect {}",
            v.t_ms as f32 / 1000.0,
            out.index,
            d.state.as_str(),
            d.radius,
            d.brightness,
            v.listen.level,
            v.speak.level,
            out.glyph(),
            out.decided.state,
            snap.lat.total(),
            snap.lat.get(Stage::Capture).unwrap_or(0.0),
            snap.lat.get(Stage::Sense).map(|x| format!("{x:.2}")).unwrap_or_else(|| "-".into()),
            snap.lat.get(Stage::Think).unwrap_or(0.0),
            snap.lat.get(Stage::Speak).unwrap_or(0.0),
            aff
        )
    }

    /// Full developer panel: the dot (top) + timing / emotion bars (below).
    pub fn panel(&self, out: &FrameOut, snap: &LiveSnapshot, step_us: f32, o: PanelOpts) -> String {
        let v = self.last.clone().unwrap_or_default();
        let d = dot::DotParams::from_frame(&v);
        let mut s = String::with_capacity(8 * 1024);
        if o.dot_rows > 0 {
            s.push_str(&dot::render_terminal(&d, o.dot_cols, o.dot_rows, v.t_ms, dot::TerminalDotOpts { color: o.color, gradient_bg: false }));
            s.push('\n');
        }
        let (dim, amber, violet, red, rst) = if o.color {
            ("\x1b[2m", "\x1b[38;2;245;165;36m", "\x1b[38;2;124;58;237m", "\x1b[31m", "\x1b[0m")
        } else {
            ("", "", "", "", "")
        };
        let w = o.bar_width;
        let bar = |x: f32, max: f32| -> String {
            let k = if max > 0.0 && x.is_finite() { (x / max).clamp(0.0, 1.0) } else { 0.0 };
            let n = (k * w as f32).round() as usize;
            format!("{}{}", "█".repeat(n), "░".repeat(w - n))
        };
        let centred = |x: f32| -> String {
            // -1..=1 around a centre tick.
            let half = w / 2;
            let k = if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 };
            let n = (k.abs() * half as f32).round() as usize;
            let mut b = vec!['░'; w];
            b[half] = '│';
            for i in 0..n {
                if k < 0.0 { b[half - 1 - i.min(half - 1)] = '█' } else if half + 1 + i < w { b[half + 1 + i] = '█' }
            }
            b.into_iter().collect()
        };
        s.push_str(&format!(
            "{dim}── developer console · W21-04 · frame {} · t {:.2}s · dot {} r{:.3} b{:.2}{}{rst}\x1b[K\n",
            out.index,
            v.t_ms as f32 / 1000.0,
            d.state.as_str(),
            d.radius,
            d.brightness,
            if out.paused { " · PAUSED (late frame)" } else { "" }
        ));
        s.push_str(&format!("{amber}TIMING{rst}  (ms, bar = 0..80 ms frame){}\x1b[K\n", if snap.lat.total() > BUDGET_MS { format!(" {red}OVER 60ms BUDGET{rst}") } else { String::new() }));
        for st in Stage::ALL {
            match snap.lat.get(st) {
                Some(ms) => s.push_str(&format!("  {:<8}▕{}▏{:>7.2}\x1b[K\n", st.label(), bar(ms, FRAME_MS as f32), ms)),
                None => s.push_str(&format!("  {:<8}▕{}▏ {dim}no reading{rst}\x1b[K\n", st.label(), "·".repeat(w))),
            }
        }
        let total = snap.lat.total();
        s.push_str(&format!("  {:<8}▕{}▏{:>7.2}  rtf {:.2}  hb.step {:.0}µs  {:.2} GFLOP/s\x1b[K\n", "total", bar(total, FRAME_MS as f32), total, total / FRAME_MS as f32, step_us, snap.gflops));
        s.push_str(&format!("{violet}EMOTION{rst} (you band, SENSE {}){}\x1b[K\n", if snap.sense { "on" } else { "off" }, match snap.affect {
            Some(_) => format!(" · {}", v.listen.emotion.label),
            None => String::new(),
        }));
        match snap.affect {
            Some(a) => {
                s.push_str(&format!("  {:<8}▕{}▏{:>+7.2}\x1b[K\n", "valence", centred(a.valence), a.valence));
                s.push_str(&format!("  {:<8}▕{}▏{:>7.2}\x1b[K\n", "arousal", bar(a.arousal, 1.0), a.arousal));
                s.push_str(&format!("  {:<8}▕{}▏{:>7.2}\x1b[K\n", "confid.", bar(a.confidence, 1.0), a.confidence));
            }
            None => {
                for l in ["valence", "arousal", "confid."] {
                    s.push_str(&format!("  {:<8}▕{}▏ {dim}no reading{rst}\x1b[K\n", l, "·".repeat(w)));
                }
            }
        }
        s.push_str(&format!("BANDS\x1b[K\n  {:<8}▕{}▏{:>7.2}{}\x1b[K\n", "you", bar(v.listen.level, 1.0), v.listen.level, if v.listen.active { "  voicing" } else { "" }));
        s.push_str(&format!("  {:<8}▕{}▏{:>7.2}{}\x1b[K\n", "me", bar(v.speak.level, 1.0), v.speak.level, if v.speak.active { "  voicing" } else { "" }));
        s.push_str(&format!(
            "  {:<8}▕{}▏ lean-in {:.2}  gain {:.2}{}{}\x1b[K\n",
            "feeling",
            bar(out.applied.lean_in, 1.0),
            out.applied.lean_in,
            out.me_gain(),
            if out.yield_now() { "  YIELD" } else { "" },
            if !out.me_onset_allowed { "  hold" } else { "" }
        ));
        s.push_str(&format!("RIKA    {:?} {}  {dim}{}{rst}\x1b[K\n", out.decided.state, out.glyph(), self.glyph_row()));
        let st = &self.stats;
        s.push_str(&format!(
            "{dim}frames {}  paused {}  lean-in {}  yields {}  mean {:.1}ms  max {:.1}ms  over-budget {}  surfaces {:?}{rst}\x1b[K\n",
            st.frames,
            st.paused,
            st.lean_in_frames,
            st.yields,
            if st.frames > 0 { st.sum_total_ms / st.frames as f64 } else { 0.0 },
            st.max_total_ms,
            st.over_budget,
            self.fanout.kinds()
        ));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{StubAffect, StubTiming};
    use galion_emotion::{DefaultConsentGate, Purpose};
    use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_SAMPLES, RIKA_BITS, SAMPLE_RATE};
    use galion_render::{dot::DotState, NullSurface};

    fn tone(amp: f32) -> Vec<f32> {
        (0..FRAME_SAMPLES).map(|i| amp * (i as f32 * 0.05).sin()).collect()
    }

    #[test]
    fn heartbeat_drives_dot_states() {
        let mut gate = DefaultConsentGate::default();
        gate.grant(0, "en", Purpose::PersonalAssistant);
        let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), gate);
        let mut pump = DemoPump::new(StubTiming::default(), StubAffect::new(true));
        let mut dc = DotConsole::new();
        dc.attach(Box::new(NullSurface::new())).unwrap();
        let (quiet, voice, me) = (tone(0.0005), tone(0.1), tone(0.08));
        let silent = vec![0.0f32; FRAME_SAMPLES];
        let mut states = Vec::new();
        for i in 0..30u64 {
            let (you, me_on) = match i { 0..=9 => (&quiet, false), 10..=19 => (&voice, false), _ => (&quiet, true) };
            let inp = FrameIn { you_pcm: you, affect: None, tension: None, consent_bits: RIKA_BITS, purpose: Purpose::PersonalAssistant, session_open: true, me_speaking: me_on, late: false };
            let out = hb.step(&inp);
            let snap = pump.pump(i, you, SAMPLE_RATE);
            let viz = dc.frame(&out, you, if me_on { &me } else { &silent }, me_on, &snap, 1.0);
            states.push(dot::DotParams::from_frame(viz).state);
        }
        assert_eq!(states[5], DotState::Idle);
        assert_eq!(states[15], DotState::Listening);
        assert_eq!(states[25], DotState::Speaking);
        assert_eq!(dc.stats.frames, 30);
        let p = dc.panel(&hb.step(&FrameIn { you_pcm: &quiet, affect: None, tension: None, consent_bits: RIKA_BITS, purpose: Purpose::PersonalAssistant, session_open: true, me_speaking: false, late: false }), &pump.pump(30, &quiet, SAMPLE_RATE), 1.0, PanelOpts { color: false, ..Default::default() });
        assert!(p.contains("TIMING") && p.contains("EMOTION") && p.contains("arousal") && p.contains("RIKA"));
        assert!(!p.contains("\x1b[3"), "no colour escapes when color=false");
    }

    #[test]
    fn sense_off_shows_no_reading() {
        let mut pump = DemoPump::new(StubTiming::default(), StubAffect::new(false));
        let snap = pump.pump(0, &tone(0.1), SAMPLE_RATE);
        assert!(snap.affect.is_none() && snap.lat.get(Stage::Sense).is_none());
    }
}
