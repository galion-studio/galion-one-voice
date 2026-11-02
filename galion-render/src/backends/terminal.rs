//! Default surface: ANSI terminal, std only. Works over SSH on a cheap VPS.
//! Two bars (listen / speak), emotion tag, transcript tails, latency + RTF footer.
//! Uses the alternate screen + cursor-home redraw (no full clears => no flicker).

use crate::frame::{BandState, VizFrame};
use crate::surface::{RenderError, RenderSurface, SurfaceCaps, SurfaceKind};
use std::io::Write;

pub struct TerminalSurface {
    out: Box<dyn Write + Send>,
    width: u16,
    color: bool,
    alt_screen: bool,
    open: bool,
}

impl TerminalSurface {
    /// stdout, width from $COLUMNS (fallback 80), color unless $NO_COLOR is set.
    pub fn stdout() -> Self {
        let width = std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()).unwrap_or(80);
        let color = std::env::var_os("NO_COLOR").is_none();
        Self { out: Box::new(std::io::stdout()), width, color, alt_screen: true, open: false }
    }

    /// Any writer (tests, log files). No alt screen.
    pub fn with_writer(out: Box<dyn Write + Send>, width: u16, color: bool) -> Self {
        Self { out, width, color, alt_screen: false, open: false }
    }

    fn bar(&self, b: &BandState, name: &str, rgb: (u8, u8, u8)) -> String {
        let label_w = 8usize;
        let tail_w = 18usize;
        let bar_w = (self.width as usize).saturating_sub(label_w + tail_w + 4).clamp(8, 120);
        let lvl = b.level.clamp(0.0, 1.0);
        let filled = (lvl * bar_w as f32).round() as usize;
        let mut s = String::with_capacity(bar_w * 4 + 64);
        s.push_str(&format!("{name:<label_w$}"));
        if self.color {
            s.push_str(&format!("\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2));
        }
        for i in 0..bar_w { s.push(if i < filled { '█' } else { '·' }); }
        if self.color { s.push_str("\x1b[0m"); }
        let dot = if b.active { '●' } else { '○' };
        s.push_str(&format!(" {dot} {:<9} {:>3.0}%", b.emotion.label, b.emotion.intensity * 100.0));
        s
    }

    fn tail(&self, s: &str) -> String {
        let max = (self.width as usize).saturating_sub(10).max(10);
        let n = s.chars().count();
        if n <= max { s.to_string() } else { s.chars().skip(n - max).collect() }
    }
}

impl RenderSurface for TerminalSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Terminal }
    fn caps(&self) -> SurfaceCaps {
        SurfaceCaps { color: self.color, pixels: false, max_fps: 15.0, size: (self.width, 6) }
    }
    fn open(&mut self) -> Result<(), RenderError> {
        if self.alt_screen { self.out.write_all(b"\x1b[?1049h\x1b[?25l")?; }
        self.out.flush()?;
        self.open = true;
        Ok(())
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        if !self.open { return Err(RenderError::Closed); }
        let mut buf = String::with_capacity(1024);
        if self.alt_screen { buf.push_str("\x1b[H"); }
        let eol = if self.alt_screen { "\x1b[K\n" } else { "\n" };
        buf.push_str(&format!("GALION VOICE  t={:>7.2}s  #{}", f.t_ms as f64 / 1000.0, f.seq)); buf.push_str(eol);
        buf.push_str(&self.bar(&f.listen, "LISTEN", (80, 180, 255))); buf.push_str(eol);
        buf.push_str(&format!("  you> {}", self.tail(&f.listen.transcript_tail))); buf.push_str(eol);
        buf.push_str(&self.bar(&f.speak, "SPEAK", (255, 150, 90))); buf.push_str(eol);
        buf.push_str(&format!("  gal> {}", self.tail(&f.speak.transcript_tail))); buf.push_str(eol);
        buf.push_str(&format!("latency {:>6.0} ms   rtf {:>5.2}", f.latency_ms, f.rtf)); buf.push_str(eol);
        self.out.write_all(buf.as_bytes())?;
        self.out.flush()?;
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> {
        if self.open {
            self.open = false;
            if self.alt_screen { self.out.write_all(b"\x1b[?25h\x1b[?1049l")?; }
            self.out.flush()?;
        }
        Ok(())
    }
}

impl Drop for TerminalSurface {
    fn drop(&mut self) { let _ = self.close(); }
}

/// User surface for the terminal: **the one dot only**, centred, in block glyphs on an
/// obsidian background. No header, no bars, no text (those live in [`TerminalSurface`],
/// the developer console). Redraws with cursor-home on the alternate screen.
pub struct TerminalDotSurface {
    out: Box<dyn Write + Send>,
    cols: u16,
    rows: u16,
    opts: crate::dot::TerminalDotOpts,
    alt_screen: bool,
    open: bool,
}

impl TerminalDotSurface {
    /// stdout; size from $COLUMNS/$LINES (fallback 80x24); colour unless $NO_COLOR;
    /// background wash if $GALION_DOT_GRADIENT=1.
    pub fn stdout() -> Self {
        let env = |k: &str, d: u16| std::env::var(k).ok().and_then(|c| c.parse().ok()).unwrap_or(d);
        let opts = crate::dot::TerminalDotOpts {
            color: std::env::var_os("NO_COLOR").is_none(),
            gradient_bg: std::env::var("GALION_DOT_GRADIENT").map(|v| v == "1").unwrap_or(false),
        };
        Self { out: Box::new(std::io::stdout()), cols: env("COLUMNS", 80), rows: env("LINES", 24), opts, alt_screen: true, open: false }
    }

    /// Any writer (tests). No alt screen; one frame per `present`, separated by `\n\n`.
    pub fn with_writer(out: Box<dyn Write + Send>, cols: u16, rows: u16, opts: crate::dot::TerminalDotOpts) -> Self {
        Self { out, cols, rows, opts, alt_screen: false, open: false }
    }
}

impl RenderSurface for TerminalDotSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Terminal }
    fn caps(&self) -> SurfaceCaps {
        SurfaceCaps { color: self.opts.color, pixels: false, max_fps: 12.5, size: (self.cols, self.rows) }
    }
    fn open(&mut self) -> Result<(), RenderError> {
        if self.alt_screen { self.out.write_all(b"\x1b[?1049h\x1b[?25l")?; }
        self.out.flush()?;
        self.open = true;
        Ok(())
    }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        if !self.open { return Err(RenderError::Closed); }
        let d = crate::dot::DotParams::from_frame(f);
        let body = crate::dot::render_terminal(&d, self.cols, self.rows, f.t_ms, self.opts);
        if self.alt_screen { self.out.write_all(b"\x1b[H")?; }
        self.out.write_all(body.as_bytes())?;
        if !self.alt_screen { self.out.write_all(b"\n\n")?; }
        self.out.flush()?;
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> {
        if self.open {
            self.open = false;
            if self.alt_screen { self.out.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l")?; }
            self.out.flush()?;
        }
        Ok(())
    }
}

impl Drop for TerminalDotSurface {
    fn drop(&mut self) { let _ = self.close(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(b); Ok(b.len()) }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }

    #[test]
    fn renders_plain_frame() {
        let sink = Shared::default();
        let mut t = TerminalSurface::with_writer(Box::new(sink.clone()), 60, false);
        t.open().unwrap();
        let mut f = VizFrame::default();
        f.listen.level = 0.5;
        f.speak.transcript_tail = "hello world".into();
        t.present(&f).unwrap();
        let s = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(s.contains("LISTEN") && s.contains("SPEAK") && s.contains("gal> hello world"));
        assert!(!s.contains('\x1b'));
    }

    #[test]
    fn dot_surface_draws_only_the_dot_fast() {
        let sink = Shared::default();
        let opts = crate::dot::TerminalDotOpts { color: false, gradient_bg: false };
        let mut t = TerminalDotSurface::with_writer(Box::new(sink.clone()), 80, 24, opts);
        t.open().unwrap();
        let mut f = VizFrame::default();
        f.listen.transcript_tail = "secret words".into();
        f.listen.active = true;
        let n = 200u32;
        let t0 = std::time::Instant::now();
        for i in 0..n { f.t_ms = i as u64 * 80; t.present(&f).unwrap(); }
        let mean = t0.elapsed() / n;
        eprintln!("TerminalDotSurface 80x24 present mean {mean:?}");
        let s = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(!s.contains("secret") && !s.contains("LISTEN") && !s.contains('\x1b'));
        assert!(s.chars().all(|c| matches!(c, '█' | '▀' | '▄' | ' ' | '\n')));
        assert!(mean < std::time::Duration::from_millis(1), "mean {mean:?}");
    }
}
