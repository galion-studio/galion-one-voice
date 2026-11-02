// Galion Voice — Perpetual Open MIT License
//! Live wiring (A11): console ↔ heartbeat / tensor / emotion **stubs**.
//!
//! This is the dev/operator console on (stub) real-time data — not the user UI
//! (the user sees one dot, A12). Data still flows one way:
//! `LiveFeed -> LiveSource (MetricsSource) -> ConsoleState.live -> ui::draw`.
//!
//! Locked behaviour implemented here:
//! * **One latency number per stage.** [`Stage`] has a fixed set of stages; each
//!   tick a feed reports at most one `f32` ms per stage ([`StageLatencies`]). The
//!   console shows exactly that number (no averages, no percentiles on screen).
//! * **"no reading" when SENSE is off.** If the SENSE consent bit is not granted,
//!   the feed must not produce affect, and the console renders `no reading` for
//!   both the affect line and the `sense` stage — never a zero, never a guess.
//! * **Persisted heartbeat answers.** [`AnswerLog`] appends every committed
//!   [`HeartbeatEvent`] as one JSONL line and restores them on start.
//! * **`NO_COLOR`.** [`color_enabled`] follows <https://no-color.org>; when off,
//!   [`strip_color`] removes every fg/bg from the frame and keeps emphasis via
//!   `REVERSED`/`BOLD`, so the YES/NO highlight stays visible in monochrome.
//!
//! The stubs ([`StubFeed`]) are deterministic (tiny LCG, no wall clock) so
//! headless snapshots are reproducible.
//!
//! **Real feeds (A11 follow-up).** A live tick is split into two feed traits that
//! are always compiled (no extra deps): [`TimingFeed`] (capture → think → speak
//! wall-clock timing + op counters, and the captured mic PCM) and [`AffectFeed`]
//! (SENSE state + one affect reading computed from that PCM). [`ComposedFeed`]
//! joins them into a [`LiveFeed`]. The real implementations live in
//! [`crate::feeds`] behind cargo features `tensor-feed` (galion-tensor kernels)
//! and `emotion-feed` (galion-emotion `AffectDetector` → A9 `AffectStore`);
//! with `--no-default-features` the console builds stub-only. In-process
//! [`StubTiming`] / [`StubAffect`] / [`NoAffect`] go through the same traits.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::heartbeat::{Answer, HeartbeatEvent, FRAME_MS};
use crate::model::{BenchSample, ConsoleState};
use crate::source::MetricsSource;

// ---------------------------------------------------------------------------
// Stages + latency
// ---------------------------------------------------------------------------

/// Pipeline stages inside one 80 ms heartbeat. Order is the render order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Mic frame in (capture + codec encode).
    Capture,
    /// Affect detection (SENSE). Absent when SENSE consent is off.
    Sense,
    /// Model step (galion-tensor forward for this frame).
    Think,
    /// Speech frame out (decode + playout enqueue).
    Speak,
}

impl Stage {
    pub const ALL: [Stage; 4] = [Stage::Capture, Stage::Sense, Stage::Think, Stage::Speak];

    pub fn label(self) -> &'static str {
        match self {
            Stage::Capture => "capture",
            Stage::Sense => "sense",
            Stage::Think => "think",
            Stage::Speak => "speak",
        }
    }

    fn idx(self) -> usize {
        self as usize
    }
}

/// Exactly one latency number (ms) per stage, or `None` = no reading this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StageLatencies([Option<f32>; 4]);

impl StageLatencies {
    pub fn set(&mut self, s: Stage, ms: f32) {
        self.0[s.idx()] = Some(ms.max(0.0));
    }
    pub fn clear(&mut self, s: Stage) {
        self.0[s.idx()] = None;
    }
    pub fn get(&self, s: Stage) -> Option<f32> {
        self.0[s.idx()]
    }
    /// Sum of reported stages (the frame budget check is `total <= FRAME_MS`).
    pub fn total(&self) -> f32 {
        self.0.iter().flatten().sum()
    }
}

/// Wall-clock helper for real feeds: `timer.time(Stage::Think, || step())`.
#[derive(Debug, Default)]
pub struct StageTimer {
    pub lat: StageLatencies,
}

impl StageTimer {
    pub fn time<T>(&mut self, s: Stage, f: impl FnOnce() -> T) -> T {
        let t0 = Instant::now();
        let out = f();
        self.lat.set(s, t0.elapsed().as_secs_f32() * 1000.0);
        out
    }
}

// ---------------------------------------------------------------------------
// Affect (mirror of galion-emotion's dimensional frame, console-side only)
// ---------------------------------------------------------------------------

/// What the console needs from an emotion frame. Mirrors the VAD-first
/// `galion_emotion::EmotionFrame` without depending on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffectReading {
    /// -1..=1
    pub valence: f32,
    /// 0..=1
    pub arousal: f32,
    /// 0..=1
    pub confidence: f32,
}

/// One live tick from a feed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiveSnapshot {
    pub lat: StageLatencies,
    /// SENSE consent bit as seen by the pipeline this tick.
    pub sense: bool,
    /// Must be `None` whenever `sense == false` (enforced by [`LiveView::apply`]).
    pub affect: Option<AffectReading>,
    /// Tensor stats for the frame (ops, GFLOP/s).
    pub ops: u64,
    pub gflops: f64,
}

/// Anything that can produce one [`LiveSnapshot`] per heartbeat tick.
pub trait LiveFeed {
    fn sample(&mut self, tick: u64) -> LiveSnapshot;
    /// Short source name for the footer (`source: live <name>`).
    fn name(&self) -> &'static str {
        "stub"
    }
}

impl<F: LiveFeed + ?Sized> LiveFeed for Box<F> {
    fn sample(&mut self, tick: u64) -> LiveSnapshot {
        (**self).sample(tick)
    }
    fn name(&self) -> &'static str {
        (**self).name()
    }
}

// ---------------------------------------------------------------------------
// Feed traits for real sources (tensor timing + emotion affect)
// ---------------------------------------------------------------------------

/// One heartbeat of pipeline timing from a tensor/codec runtime.
/// `None` = the stage did not run this tick (renders `no reading`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StageTiming {
    pub capture: Option<f32>,
    pub think: Option<f32>,
    pub speak: Option<f32>,
    /// Arithmetic ops (2 × MACs) executed this tick.
    pub ops: u64,
    pub gflops: f64,
}

/// Capture → think → speak timing source (e.g. galion-tensor kernels).
pub trait TimingFeed {
    /// Run (or observe) one 80 ms heartbeat. Afterwards [`TimingFeed::pcm`]
    /// holds the captured mic frame for the SENSE stage.
    fn step(&mut self, tick: u64) -> StageTiming;
    /// Mono f32 PCM in [-1, 1] of the frame captured by the last `step`.
    fn pcm(&self) -> &[f32];
    fn sample_rate(&self) -> u32;
    fn name(&self) -> &'static str {
        "timing"
    }
}

/// What an affect source reports for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AffectSample {
    /// Wall-clock ms of detect + store for this tick (the `sense` stage).
    pub latency_ms: Option<f32>,
    pub reading: Option<AffectReading>,
}

/// SENSE-gated affect source (e.g. galion-emotion `AffectStore`).
pub trait AffectFeed {
    /// SENSE consent as of the last `read` (after any per-heartbeat revalidation).
    fn sense(&self) -> bool;
    /// Analyse the captured frame. Must return no reading when SENSE is off.
    fn read(&mut self, tick: u64, pcm: &[f32], sample_rate: u32) -> AffectSample;
    fn name(&self) -> &'static str {
        "affect"
    }
}

/// SENSE permanently off: never touches PCM, never produces a reading.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAffect;

impl AffectFeed for NoAffect {
    fn sense(&self) -> bool {
        false
    }
    fn read(&mut self, _: u64, _: &[f32], _: u32) -> AffectSample {
        AffectSample::default()
    }
    fn name(&self) -> &'static str {
        "none"
    }
}

/// Timing + affect → one [`LiveSnapshot`] per tick. The affect feed sees the PCM
/// the timing feed just captured, so SENSE runs on the same frame as THINK.
pub struct ComposedFeed<T: TimingFeed, A: AffectFeed> {
    pub timing: T,
    pub affect: A,
    pub label: &'static str,
}

impl<T: TimingFeed, A: AffectFeed> ComposedFeed<T, A> {
    pub fn new(timing: T, affect: A, label: &'static str) -> Self {
        Self { timing, affect, label }
    }
}

impl<T: TimingFeed, A: AffectFeed> LiveFeed for ComposedFeed<T, A> {
    fn sample(&mut self, tick: u64) -> LiveSnapshot {
        let t = self.timing.step(tick);
        let mut lat = StageLatencies::default();
        for (st, v) in [(Stage::Capture, t.capture), (Stage::Think, t.think), (Stage::Speak, t.speak)] {
            if let Some(ms) = v {
                lat.set(st, ms);
            }
        }
        let sr = self.timing.sample_rate();
        let a = self.affect.read(tick, self.timing.pcm(), sr);
        let sense = self.affect.sense();
        let affect = if sense { a.reading } else { None };
        if let (true, Some(ms)) = (sense, a.latency_ms) {
            lat.set(Stage::Sense, ms);
        }
        LiveSnapshot { lat, sense, affect, ops: t.ops, gflops: t.gflops }
    }
    fn name(&self) -> &'static str {
        self.label
    }
}

/// Deterministic in-process [`TimingFeed`] (no wall clock): fixed-shape stage
/// times + a 220 Hz tone whose loudness swells over ~2 s.
#[derive(Debug)]
pub struct StubTiming {
    pcm: Vec<f32>,
}

impl Default for StubTiming {
    fn default() -> Self {
        Self { pcm: vec![0.0; 1920] }
    }
}

impl TimingFeed for StubTiming {
    fn step(&mut self, tick: u64) -> StageTiming {
        let amp = 0.05 + 0.25 * ((tick as f32 * 0.25).sin() * 0.5 + 0.5);
        let n = self.pcm.len();
        for (i, x) in self.pcm.iter_mut().enumerate() {
            let t = (tick as usize * n + i) as f32 / 24_000.0;
            *x = amp * (t * 220.0 * std::f32::consts::TAU).sin();
        }
        let think = 20.0 + (tick % 5) as f32;
        StageTiming { capture: Some(2.0), think: Some(think), speak: Some(4.0), ops: 1_000_000, gflops: 1.0 }
    }
    fn pcm(&self) -> &[f32] {
        &self.pcm
    }
    fn sample_rate(&self) -> u32 {
        24_000
    }
    fn name(&self) -> &'static str {
        "stub-timing"
    }
}

/// Deterministic in-process [`AffectFeed`]: arousal = frame RMS / 0.3. SENSE
/// can be revoked mid-run with [`StubAffect::revoke`].
#[derive(Debug, Default)]
pub struct StubAffect {
    pub sense: bool,
    pub reads_with_pcm: u64,
}

impl StubAffect {
    pub fn new(sense: bool) -> Self {
        Self { sense, reads_with_pcm: 0 }
    }
    pub fn revoke(&mut self) {
        self.sense = false;
    }
}

impl AffectFeed for StubAffect {
    fn sense(&self) -> bool {
        self.sense
    }
    fn read(&mut self, _: u64, pcm: &[f32], _: u32) -> AffectSample {
        if !self.sense || pcm.is_empty() {
            return AffectSample::default();
        }
        self.reads_with_pcm += 1;
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len() as f32).sqrt();
        AffectSample {
            latency_ms: Some(0.5),
            reading: Some(AffectReading { valence: 0.0, arousal: (rms / 0.3).min(1.0), confidence: 0.3 }),
        }
    }
    fn name(&self) -> &'static str {
        "stub-affect"
    }
}

/// Deterministic heartbeat / tensor / emotion stub.
pub struct StubFeed {
    seed: u64,
    pub sense: bool,
}

impl StubFeed {
    pub fn new(sense: bool) -> Self {
        Self { seed: 0xA11_C0DE, sense }
    }
    fn next(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.seed >> 40) as f32) / ((1u64 << 24) as f32)
    }
}

impl LiveFeed for StubFeed {
    fn sample(&mut self, tick: u64) -> LiveSnapshot {
        let mut lat = StageLatencies::default();
        lat.set(Stage::Capture, 1.5 + 2.0 * self.next());
        let think = 25.0 + 30.0 * self.next();
        lat.set(Stage::Think, think);
        lat.set(Stage::Speak, 3.0 + 4.0 * self.next());
        // Draw the affect randoms even when SENSE is off so the other stages are
        // identical across sense on/off runs (comparable snapshots).
        let (s_lat, v, a) = (0.8 + 1.2 * self.next(), self.next(), self.next());
        let affect = if self.sense {
            lat.set(Stage::Sense, s_lat);
            // Slow-moving arousal so it reads like a voice, not noise.
            let phase = (tick as f32 * FRAME_MS as f32 / 2000.0).sin() * 0.5 + 0.5;
            Some(AffectReading { valence: (v - 0.5) * 0.4, arousal: 0.7 * phase + 0.3 * a, confidence: 0.25 })
        } else {
            None
        };
        LiveSnapshot { lat, sense: self.sense, affect, ops: (think * 400.0) as u64, gflops: 2.0 + think as f64 / 10.0 }
    }
}

// ---------------------------------------------------------------------------
// View state + MetricsSource
// ---------------------------------------------------------------------------

/// What the console shows for the live strip. Lives in `ConsoleState.live`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveView {
    pub tick: u64,
    pub lat: StageLatencies,
    pub sense: bool,
    pub affect: Option<AffectReading>,
    /// Feed name for the footer (`""` = stub).
    pub source: &'static str,
}

impl LiveView {
    /// Apply one snapshot. SENSE off => affect and sense latency are dropped,
    /// even if a buggy feed produced them.
    pub fn apply(&mut self, tick: u64, mut s: LiveSnapshot) {
        if !s.sense {
            s.affect = None;
            s.lat.clear(Stage::Sense);
        }
        self.tick = tick;
        self.lat = s.lat;
        self.sense = s.sense;
        self.affect = s.affect;
    }

    /// Plain text of the live strip (also what `NO_COLOR` users read).
    pub fn text(&self) -> String {
        let mut out = String::from("live");
        for st in Stage::ALL {
            out.push_str(&format!("  {} {}", st.label(), fmt_lat(self.stage(st))));
        }
        out.push_str(&format!("  | affect {}", self.affect_text()));
        out
    }

    fn stage(&self, st: Stage) -> Option<f32> {
        if st == Stage::Sense && !self.sense {
            None
        } else {
            self.lat.get(st)
        }
    }

    pub fn affect_text(&self) -> String {
        match (self.sense, self.affect) {
            (true, Some(a)) => format!("v{:+.2} a{:.2} c{:.2}", a.valence, a.arousal, a.confidence),
            _ => NO_READING.to_string(),
        }
    }

    /// Styled line for the header strip. Over-budget stages render red.
    pub fn line(&self) -> Line<'static> {
        let dim = Style::default().fg(Color::DarkGray);
        let mut spans = vec![Span::styled(" live", Style::default().fg(Color::Cyan))];
        for st in Stage::ALL {
            let v = self.stage(st);
            let style = match v {
                None => dim,
                Some(ms) if ms > FRAME_MS as f32 => Style::default().fg(Color::Red),
                Some(_) => Style::default().fg(Color::Green),
            };
            spans.push(Span::raw(format!("  {} ", st.label())));
            spans.push(Span::styled(fmt_lat(v), style));
        }
        spans.push(Span::raw("  | affect "));
        let a = self.affect_text();
        let st = if a == NO_READING { dim } else { Style::default().fg(Color::Magenta) };
        spans.push(Span::styled(a, st));
        Line::from(spans)
    }
}

pub const NO_READING: &str = "no reading";

fn fmt_lat(v: Option<f32>) -> String {
    match v {
        // Sub-0.1 ms stages (e.g. real SENSE ≈ µs) keep a visible non-zero number.
        Some(ms) if ms >= 0.1 => format!("{ms:.1}ms"),
        Some(ms) if ms >= 0.01 => format!("{ms:.2}ms"),
        Some(_) => "<0.01ms".to_string(),
        None => NO_READING.to_string(),
    }
}

/// `MetricsSource` that wires a [`LiveFeed`] into the console. Wraps an inner
/// source (e.g. `SyntheticSource` for jobs) and overrides the tensor stats and
/// pushes the per-frame total as a bench sample.
pub struct LiveSource<F: LiveFeed, S: MetricsSource> {
    pub feed: F,
    pub inner: S,
}

impl<F: LiveFeed, S: MetricsSource> MetricsSource for LiveSource<F, S> {
    fn poll(&mut self, state: &mut ConsoleState) {
        self.inner.poll(state);
        let snap = self.feed.sample(state.tick);
        state.tensor.ops = snap.ops;
        state.tensor.gflops = snap.gflops;
        let name = self.feed.name();
        let view = state.live.get_or_insert_with(LiveView::default);
        view.apply(state.tick, snap);
        view.source = name;
        let total = view.lat.total() as f64;
        if state.tick % 4 == 0 {
            state.push_bench(BenchSample {
                label: format!("frame (live {name})"),
                rtf: total / FRAME_MS as f64,
                latency_ms: total,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Persisted answers (JSONL)
// ---------------------------------------------------------------------------

/// Append-only JSONL log of committed heartbeat answers. One line per event:
/// `{"tick":6,"answer":"NO","at_ms":480,"presses":1}`. Hand-rolled (no serde dep).
#[derive(Debug)]
pub struct AnswerLog {
    path: PathBuf,
    file: File,
}

impl AnswerLog {
    /// Open (create parents + file) for append.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, ev: &HeartbeatEvent) -> io::Result<()> {
        writeln!(self.file, "{}", encode_event(ev))?;
        self.file.flush()
    }

    /// Read all valid events from `path` (missing file = empty; bad lines skipped).
    pub fn load(path: impl AsRef<Path>) -> io::Result<Vec<HeartbeatEvent>> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for line in BufReader::new(f).lines() {
            if let Some(ev) = decode_event(&line?) {
                out.push(ev);
            }
        }
        Ok(out)
    }
}

pub fn encode_event(ev: &HeartbeatEvent) -> String {
    format!(
        "{{\"tick\":{},\"answer\":\"{}\",\"at_ms\":{},\"presses\":{}}}",
        ev.tick,
        ev.kind.label(),
        ev.at_ms,
        ev.presses
    )
}

pub fn decode_event(line: &str) -> Option<HeartbeatEvent> {
    fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let pat = format!("\"{key}\":");
        let rest = &line[line.find(&pat)? + pat.len()..];
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().trim_matches('"'))
    }
    let kind = match field(line, "answer")? {
        "YES" => Answer::Yes,
        "NO" => Answer::No,
        _ => return None,
    };
    Some(HeartbeatEvent {
        tick: field(line, "tick")?.parse().ok()?,
        kind,
        at_ms: field(line, "at_ms")?.parse().ok()?,
        presses: field(line, "presses")?.parse().ok()?,
    })
}

/// Default answers path: `$GALION_CONSOLE_ANSWERS`, else
/// `$XDG_STATE_HOME/galion-console/answers.jsonl`, else
/// `$HOME/.local/state/galion-console/answers.jsonl`.
pub fn default_answers_path() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(p) = env("GALION_CONSOLE_ANSWERS") {
        return Some(p);
    }
    let base = env("XDG_STATE_HOME").or_else(|| env("HOME").map(|h| h.join(".local/state")))?;
    Some(base.join("galion-console").join("answers.jsonl"))
}

// ---------------------------------------------------------------------------
// NO_COLOR
// ---------------------------------------------------------------------------

/// `NO_COLOR` present and non-empty => colour off (https://no-color.org).
pub fn color_enabled() -> bool {
    color_enabled_from(std::env::var_os("NO_COLOR").as_deref().map(|v| v.to_string_lossy().into_owned()))
}

/// Testable core of [`color_enabled`].
pub fn color_enabled_from(no_color: Option<String>) -> bool {
    !matches!(no_color, Some(v) if !v.is_empty())
}

/// Remove all colours from a rendered frame. Cells that had a background
/// (highlights, gauges, the header badge) become `REVERSED` so emphasis survives.
pub fn strip_color(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            let had_bg = !matches!(cell.bg, Color::Reset);
            cell.fg = Color::Reset;
            cell.bg = Color::Reset;
            if had_bg {
                cell.modifier.insert(Modifier::REVERSED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ui, SyntheticSource};
    use ratatui::{backend::TestBackend, Terminal};

    fn run(sense: bool, ticks: u64) -> ConsoleState {
        let mut s = ConsoleState::default();
        let mut src = LiveSource { feed: StubFeed::new(sense), inner: SyntheticSource::default() };
        for _ in 0..ticks {
            src.poll(&mut s);
            s.advance();
        }
        s
    }

    fn render(s: &mut ConsoleState) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(120, 24)).unwrap();
        term.draw(|f| ui::draw(f, s)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()).collect()
    }

    #[test]
    fn one_latency_number_per_stage() {
        let s = run(true, 5);
        let v = s.live.as_ref().unwrap();
        for st in Stage::ALL {
            assert!(v.lat.get(st).is_some(), "{st:?}");
        }
        let t = v.text();
        for st in Stage::ALL {
            assert_eq!(t.matches(&format!(" {} ", st.label())).count(), 1, "{t}");
        }
        assert_eq!(t.matches("ms").count(), 4, "exactly one number per stage: {t}");
    }

    #[test]
    fn sense_off_shows_no_reading() {
        let mut s = run(false, 5);
        let v = s.live.as_ref().unwrap();
        assert!(v.affect.is_none());
        assert!(v.lat.get(Stage::Sense).is_none());
        assert!(v.text().contains("sense no reading"));
        assert!(v.text().contains("affect no reading"));
        let rows = render(&mut s);
        assert!(rows.iter().any(|r| r.contains("affect no reading")), "{rows:#?}");
    }

    #[test]
    fn sense_on_shows_affect() {
        let mut s = run(true, 5);
        assert!(!s.live.as_ref().unwrap().affect_text().contains(NO_READING));
        let rows = render(&mut s);
        assert!(rows.iter().any(|r| r.contains("affect v")), "{rows:#?}");
    }

    #[test]
    fn apply_drops_affect_from_buggy_feed_when_sense_off() {
        let mut v = LiveView::default();
        let mut lat = StageLatencies::default();
        lat.set(Stage::Sense, 1.0);
        v.apply(1, LiveSnapshot { lat, sense: false, affect: Some(AffectReading { valence: 0.1, arousal: 0.2, confidence: 0.3 }), ops: 0, gflops: 0.0 });
        assert!(v.affect.is_none() && v.lat.get(Stage::Sense).is_none());
    }

    #[test]
    fn other_stages_identical_across_sense() {
        let (a, b) = (run(true, 7), run(false, 7));
        let (a, b) = (a.live.unwrap(), b.live.unwrap());
        for st in [Stage::Capture, Stage::Think, Stage::Speak] {
            assert_eq!(a.lat.get(st), b.lat.get(st));
        }
    }

    #[test]
    fn answers_roundtrip_and_restore() {
        let dir = std::env::temp_dir().join(format!("galion-a11-{}", std::process::id()));
        let path = dir.join("sub/answers.jsonl");
        let _ = fs::remove_dir_all(&dir);
        let evs = [
            HeartbeatEvent { tick: 3, kind: Answer::Yes, at_ms: 240, presses: 1 },
            HeartbeatEvent { tick: 9, kind: Answer::No, at_ms: 720, presses: 4 },
        ];
        {
            let mut log = AnswerLog::open(&path).unwrap();
            for e in &evs {
                log.append(e).unwrap();
            }
        }
        // Garbage lines are skipped, not fatal.
        OpenOptions::new().append(true).open(&path).unwrap().write_all(b"not json\n").unwrap();
        let back = AnswerLog::load(&path).unwrap();
        assert_eq!(back, evs);

        let mut s = ConsoleState::default();
        for e in &back {
            s.yes_no.restore(*e);
        }
        assert_eq!(s.yes_no.last(), Some(evs[1]));
        assert_eq!(s.yes_no.log().len(), 2);
        assert_eq!(s.yes_no.highlighted(s.tick), None, "restored answers are not hot");
        assert!(AnswerLog::load(dir.join("missing.jsonl")).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    fn run_feed<F: LiveFeed>(feed: F, ticks: u64) -> (ConsoleState, LiveSource<F, SyntheticSource>) {
        let mut s = ConsoleState::default();
        let mut src = LiveSource { feed, inner: SyntheticSource::default() };
        for _ in 0..ticks {
            src.poll(&mut s);
            s.advance();
        }
        (s, src)
    }

    fn assert_one_number_per_stage(v: &LiveView, expect_ms: usize) {
        let t = v.text();
        for st in Stage::ALL {
            assert_eq!(t.matches(&format!(" {} ", st.label())).count(), 1, "{t}");
        }
        assert_eq!(t.matches("ms").count(), expect_ms, "{t}");
    }

    #[test]
    fn composed_feed_one_number_per_stage_and_affect_from_pcm() {
        let (mut s, src) = run_feed(ComposedFeed::new(StubTiming::default(), StubAffect::new(true), "composed"), 6);
        let v = s.live.clone().unwrap();
        assert_one_number_per_stage(&v, 4);
        assert_eq!(v.lat.get(Stage::Sense), Some(0.5));
        let a = v.affect.expect("affect when SENSE on");
        assert!(a.arousal > 0.0, "arousal derived from captured PCM");
        assert_eq!(src.feed.affect.reads_with_pcm, 6, "affect sees the timing feed's PCM each tick");
        assert_eq!(s.tensor.ops, 1_000_000);
        assert_eq!(v.source, "composed");
        let rows = render(&mut s);
        assert!(rows.iter().any(|r| r.contains("source: live composed")), "{rows:#?}");
    }

    #[test]
    fn composed_feed_sense_off_no_reading() {
        for (name, feed) in [
            ("stub-off", Box::new(ComposedFeed::new(StubTiming::default(), StubAffect::new(false), "x")) as Box<dyn LiveFeed>),
            ("none", Box::new(ComposedFeed::new(StubTiming::default(), NoAffect, "x"))),
        ] {
            let (mut s, _) = run_feed(feed, 4);
            let v = s.live.clone().unwrap();
            assert!(v.affect.is_none() && v.lat.get(Stage::Sense).is_none(), "{name}");
            assert!(v.text().contains("sense no reading") && v.text().contains("affect no reading"), "{name}");
            assert_one_number_per_stage(&v, 3);
            let rows = render(&mut s);
            assert!(rows.iter().any(|r| r.contains("affect no reading")), "{name}");
        }
    }

    #[test]
    fn composed_feed_sense_revoked_mid_run() {
        let (mut s, mut src) = run_feed(ComposedFeed::new(StubTiming::default(), StubAffect::new(true), "x"), 3);
        assert!(s.live.as_ref().unwrap().affect.is_some());
        src.feed.affect.revoke();
        src.poll(&mut s);
        let v = s.live.as_ref().unwrap();
        assert!(v.affect.is_none() && v.text().contains("sense no reading"));
    }

    #[test]
    fn missing_stage_renders_no_reading_not_zero() {
        struct Half;
        impl TimingFeed for Half {
            fn step(&mut self, _: u64) -> StageTiming {
                StageTiming { capture: Some(1.0), think: None, speak: Some(2.0), ops: 0, gflops: 0.0 }
            }
            fn pcm(&self) -> &[f32] {
                &[]
            }
            fn sample_rate(&self) -> u32 {
                24_000
            }
        }
        let (s, _) = run_feed(ComposedFeed::new(Half, StubAffect::new(true), "x"), 2);
        let v = s.live.unwrap();
        assert!(v.text().contains("think no reading"), "{}", v.text());
        assert!(v.affect.is_none(), "empty PCM => no affect reading");
    }

    #[test]
    fn tiny_latency_is_one_nonzero_number() {
        assert_eq!(fmt_lat(Some(12.34)), "12.3ms");
        assert_eq!(fmt_lat(Some(0.04)), "0.04ms");
        assert_eq!(fmt_lat(Some(0.0)), "<0.01ms");
        assert_eq!(fmt_lat(None), NO_READING);
    }

    #[test]
    fn no_color_env_semantics() {
        assert!(color_enabled_from(None));
        assert!(color_enabled_from(Some(String::new())));
        assert!(!color_enabled_from(Some("1".into())));
    }

    #[test]
    fn no_color_strips_every_cell_but_keeps_highlight() {
        let mut s = run(true, 3);
        s.color = false;
        s.yes_no.press(Answer::Yes);
        s.advance();
        let mut term = Terminal::new(TestBackend::new(120, 24)).unwrap();
        term.draw(|f| ui::draw(f, &mut s)).unwrap();
        let buf = term.backend().buffer();
        let mut reversed = 0;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let c = &buf[(x, y)];
                assert_eq!((c.fg, c.bg), (Color::Reset, Color::Reset), "colour at {x},{y}");
                if c.modifier.contains(Modifier::REVERSED) {
                    reversed += 1;
                }
            }
        }
        assert!(reversed > 0, "YES highlight must survive as REVERSED");
    }
}
