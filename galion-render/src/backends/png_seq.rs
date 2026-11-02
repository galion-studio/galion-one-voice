// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Headless framebuffer surface (W21-04): rasterises the one dot with the same CPU
//! path a window / `/dev/fb0` backend uses ([`crate::dot::rasterize`]) and writes PNGs.
//! No display server, no deps. std-only, always compiled.
//!
//! * `latest.png` is rewritten every presented frame (a viewer can poll it live).
//! * With `sequence = true`, `frame_000000.png`, … are also written (one per presented
//!   frame after `max_fps` decimation) for turning into a GIF/MP4 later.
//!
//! `present()` does file I/O, so attach it via [`crate::Fanout`] with a modest
//! `max_fps` (default 12.5 = one per 80 ms frame) and keep frames small.

use std::path::PathBuf;

use crate::dot::{rasterize, DotParams};
use crate::frame::VizFrame;
use crate::surface::{RenderError, RenderSurface, SurfaceCaps, SurfaceKind};

#[derive(Debug, Clone)]
pub struct PngSeqConfig {
    pub dir: PathBuf,
    pub width: u32,
    pub height: u32,
    /// Also keep the numbered sequence (otherwise only `latest.png`).
    pub sequence: bool,
    /// Paint the amber→violet background wash (background only; never the dot).
    pub gradient_bg: bool,
    pub max_fps: f32,
}

impl Default for PngSeqConfig {
    fn default() -> Self {
        Self { dir: PathBuf::from("target/dot-frames"), width: 320, height: 180, sequence: true, gradient_bg: true, max_fps: 12.5 }
    }
}

pub struct PngSeqSurface {
    cfg: PngSeqConfig,
    buf: Vec<u32>,
    written: u64,
    last_dot: Option<DotParams>,
}

impl PngSeqSurface {
    pub fn new(cfg: PngSeqConfig) -> Self {
        let n = cfg.width as usize * cfg.height as usize;
        Self { cfg, buf: vec![0; n], written: 0, last_dot: None }
    }
    /// Numbered frames written so far.
    pub fn written(&self) -> u64 { self.written }
    pub fn last_dot(&self) -> Option<DotParams> { self.last_dot }
    pub fn dir(&self) -> &std::path::Path { &self.cfg.dir }
    /// The current framebuffer (`0x00RRGGBB`, `width × height`).
    pub fn framebuffer(&self) -> &[u32] { &self.buf }
}

impl RenderSurface for PngSeqSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Window }
    fn caps(&self) -> SurfaceCaps {
        SurfaceCaps {
            color: true,
            pixels: true,
            max_fps: self.cfg.max_fps,
            size: (self.cfg.width.min(u16::MAX as u32) as u16, self.cfg.height.min(u16::MAX as u32) as u16),
        }
    }
    fn open(&mut self) -> Result<(), RenderError> {
        std::fs::create_dir_all(&self.cfg.dir)?;
        Ok(())
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        let d = DotParams::from_frame(f);
        rasterize(&mut self.buf, self.cfg.width, self.cfg.height, &d, f.t_ms, self.cfg.gradient_bg);
        let (w, h) = (self.cfg.width, self.cfg.height);
        if self.cfg.sequence {
            crate::png::write_rgb(self.cfg.dir.join(format!("frame_{:06}.png", self.written)), w, h, &self.buf)?;
        }
        crate::png::write_rgb(self.cfg.dir.join("latest.png"), w, h, &self.buf)?;
        self.written += 1;
        self.last_dot = Some(d);
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_latest_and_sequence() {
        let dir = std::env::temp_dir().join(format!("galion-pngseq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = PngSeqSurface::new(PngSeqConfig { dir: dir.clone(), width: 32, height: 18, ..Default::default() });
        s.open().unwrap();
        let mut f = VizFrame::default();
        f.speak.active = true;
        s.present(&f).unwrap();
        f.t_ms = 80;
        s.present(&f).unwrap();
        assert_eq!(s.written(), 2);
        assert!(dir.join("latest.png").exists() && dir.join("frame_000001.png").exists());
        let d = s.last_dot().unwrap();
        assert_eq!(s.framebuffer()[9 * 32 + 16], d.color().to_u32());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
