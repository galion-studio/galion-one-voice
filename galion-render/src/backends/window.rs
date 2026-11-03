//! Optional native window surface — STUB (offscreen one-dot rasteriser is real).
//!
//! Contract is final; backend is not. `open()` returns `Unsupported` so callers fall back
//! to terminal. Planned backend (added as an optional dep under this feature only):
//! `softbuffer` + `winit` (pure-Rust CPU framebuffer, no GPU/wgpu), event loop on its own
//! thread, frames handed over via a single-slot mailbox (latest wins) so `present` never
//! blocks. That thread is where [`crate::OrbEngine`] steps, at the vsync `dt`
//! (60 / 120 / 240 Hz when the display allows). `present()` only stores the latest
//! [`VizFrame`]. It does not integrate the spring, and the spring does not run
//! inside `Heartbeat::step`. Galion OS builds can swap in a direct `/dev/fb0` /
//! DRM dumb-buffer writer behind the same trait without any windowing system.

use crate::frame::VizFrame;
use crate::surface::{RenderError, RenderSurface, SurfaceCaps, SurfaceKind};

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u16,
    pub height: u16,
    pub max_fps: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "Galion Voice".into(), width: 960, height: 320, max_fps: 30.0 }
    }
}

pub struct WindowSurface {
    cfg: WindowConfig,
    last: Option<VizFrame>,
}

impl WindowSurface {
    pub fn new(cfg: WindowConfig) -> Self { Self { cfg, last: None } }
    /// Render the latest frame (the one dot over the amber→violet background) into a
    /// caller-owned `0x00RRGGBB` buffer of `width × height`. This is what the future
    /// softbuffer/winit or `/dev/fb0` thread will call; it is deliberately *not* run inside
    /// `present()` so `present()` stays O(1). Returns false if no frame yet.
    pub fn render_offscreen(&self, buf: &mut [u32]) -> bool {
        let Some(f) = &self.last else { return false };
        let d = crate::dot::DotParams::from_frame(f);
        crate::dot::rasterize(buf, self.cfg.width as u32, self.cfg.height as u32, &d, f.t_ms, true);
        true
    }
    /// Latest frame handed to the (future) event-loop thread.
    pub fn last_frame(&self) -> Option<&VizFrame> { self.last.as_ref() }
}

impl RenderSurface for WindowSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Window }
    fn caps(&self) -> SurfaceCaps {
        SurfaceCaps { color: true, pixels: true, max_fps: self.cfg.max_fps, size: (self.cfg.width, self.cfg.height) }
    }
    fn open(&mut self) -> Result<(), RenderError> {
        Err(RenderError::Unsupported("window backend not yet implemented (stub); use terminal or web"))
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        self.last = Some(f.clone());
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> { Ok(()) }
}

/// Single-slot, latest-wins frame mailbox between `present()` (voice thread) and the
/// future window / `/dev/fb0` thread. std-only. `put` never blocks on the reader beyond a
/// short mutex hold; `take` returns the newest frame once (older frames are dropped).
#[derive(Clone, Default)]
pub struct FrameMailbox {
    inner: std::sync::Arc<std::sync::Mutex<(Option<VizFrame>, u64)>>,
}

impl FrameMailbox {
    pub fn new() -> Self { Self::default() }
    /// Replace any pending frame. Returns true if an unread frame was overwritten.
    pub fn put(&self, f: VizFrame) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let dropped = g.0.is_some();
        if dropped { g.1 += 1; }
        g.0 = Some(f);
        dropped
    }
    /// Take the newest frame, if any.
    pub fn take(&self) -> Option<VizFrame> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).0.take()
    }
    /// Frames overwritten before being read (decimation counter).
    pub fn overwritten(&self) -> u64 { self.inner.lock().unwrap_or_else(|e| e.into_inner()).1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mailbox_latest_wins() {
        let m = FrameMailbox::new();
        assert!(m.take().is_none());
        assert!(!m.put(VizFrame { t_ms: 80, ..Default::default() }));
        assert!(m.put(VizFrame { t_ms: 160, ..Default::default() }));
        let m2 = m.clone();
        assert_eq!(m2.take().unwrap().t_ms, 160);
        assert!(m.take().is_none());
        assert_eq!(m.overwritten(), 1);
    }
    #[test]
    fn offscreen_dot() {
        let mut w = WindowSurface::new(WindowConfig { width: 96, height: 64, ..Default::default() });
        let mut fb = vec![0u32; 96 * 64];
        assert!(!w.render_offscreen(&mut fb));
        w.present(&VizFrame::default()).unwrap();
        assert!(w.render_offscreen(&mut fb));
        let d = crate::dot::DotParams::from_frame(&VizFrame::default());
        assert_eq!(fb[32 * 96 + 48], d.color().to_u32());
        assert_eq!(fb[0], crate::palette::OBSIDIAN.to_u32());
    }
}
