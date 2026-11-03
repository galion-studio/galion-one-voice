// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-04 hook (feature `dot-viz`): feed the one-dot visualizer from the live frame loop.
//! Pure presentation — runs after `step_live`, never inside `Heartbeat::step`. Writes a PNG
//! sequence (+ `latest.png`) under `<out>/dot/`. Window surface is unsupported upstream.
use crate::{FrameRecord, FrameSink, Summary};
use galion_console::dotviz::{DemoPump, DotConsole};
use galion_console::live::{StubAffect, StubTiming};
use galion_heartbeat::SAMPLE_RATE;
use galion_render::{PngSeqConfig, PngSeqSurface};
use std::path::Path;

pub struct DotSink {
    dc: DotConsole,
    pump: DemoPump<StubTiming, StubAffect>,
    /// FP-02: frames shown in earlier runs (`--loops`). The dot clock keeps counting across
    /// turns; otherwise the 12.5 fps decimator sees t_ms jump back to 0 and freezes the feed.
    base: u32,
}

impl DotSink {
    /// `png_dir = Some(dir)`: write `<dir>/dot/` PNG frames (`sequence` = keep numbered frames).
    /// FP-02: the V1 web path passes `None` — the screen only needs `/dot`, no disk churn.
    pub fn new(png_dir: Option<&Path>, sense_on: bool, sequence: bool) -> Result<Self, String> {
        let mut dc = DotConsole::new();
        if let Some(d) = png_dir {
            dc.attach(Box::new(PngSeqSurface::new(PngSeqConfig { dir: d.join("dot"), sequence, ..Default::default() })))?;
        }
        Ok(Self { dc, pump: DemoPump::new(StubTiming::default(), StubAffect::new(sense_on)), base: 0 })
    }

    /// V1-02 hook (feature `dot-viz-web`): also serve the live dot at `addr` (`/`, `/dot`,
    /// `/health`; loopback only — galion-render refuses public binds). This is the single-screen
    /// feed: the V1 screen polls `/dot` from the *real* Heartbeat + Rika loop, not the synthetic pump.
    #[cfg(feature = "dot-viz-web")]
    pub fn attach_web(&mut self, addr: std::net::SocketAddr) -> Result<(), String> {
        // V1-01 owns the page (galion_console::dotviz::SCREEN_HTML); we only feed it the live loop.
        self.dc.attach(Box::new(
            galion_render::WebSurface::serve(addr)
                .with_page(galion_console::dotviz::SCREEN_HTML)
                .with_v2_page(galion_console::dotviz::V2_SCREEN_HTML),
        ))
    }
}

impl FrameSink for DotSink {
    fn on_frame(&mut self, _r: &FrameRecord) {}
    fn on_heartbeat(&mut self, hb: &galion_heartbeat::FrameOut, you: &[f32], me: &[f32], me_voiced: bool, step_us: f32) {
        let hb = galion_heartbeat::FrameOut { index: hb.index + self.base, ..*hb };
        let snap = self.pump.pump(hb.index as u64, you, SAMPLE_RATE);
        self.dc.frame(&hb, you, me, me_voiced, &snap, step_us);
    }
    fn finish(&mut self, s: &Summary) {
        self.base += s.frames;
        println!("dot: {:?}", self.dc.stats);
    }
}
