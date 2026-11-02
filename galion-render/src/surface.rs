//! The shared `RenderSurface` trait and a fan-out that isolates failures.

use crate::dot::DotParams;
use crate::frame::VizFrame;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Terminal,
    Window,
    Web,
    /// Test / headless sink.
    Null,
}

impl SurfaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SurfaceKind::Terminal => "terminal",
            SurfaceKind::Window => "window",
            SurfaceKind::Web => "web",
            SurfaceKind::Null => "null",
        }
    }
}

/// What a surface can do; lets galion-console pick a layout without knowing the backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCaps {
    pub color: bool,
    /// Can show smooth (sub-cell) waveforms.
    pub pixels: bool,
    /// Max useful frame rate; Fanout decimates above this.
    pub max_fps: f32,
    /// Columns x rows (terminal) or logical px (window); (0,0) if unknown/remote.
    pub size: (u16, u16),
}

#[derive(Debug)]
pub enum RenderError {
    /// Feature compiled in but backend not implemented / not available on this host.
    Unsupported(&'static str),
    Io(std::io::Error),
    /// Surface was closed by the user (window closed, client gone).
    Closed,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Unsupported(w) => write!(f, "surface unsupported: {w}"),
            RenderError::Io(e) => write!(f, "io: {e}"),
            RenderError::Closed => write!(f, "surface closed"),
        }
    }
}
impl std::error::Error for RenderError {}
impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
    }
}

/// One rendering target. Implementations MUST NOT block for long in `present`:
/// it is called from a viz thread fed by the audio loop, and anything over a few ms
/// is a bug. Surfaces that need an event loop (window) run it on their own thread.
pub trait RenderSurface: Send {
    fn kind(&self) -> SurfaceKind;
    fn caps(&self) -> SurfaceCaps;
    /// Acquire resources (enter alt screen, open window, bind socket).
    fn open(&mut self) -> Result<(), RenderError>;
    /// Draw one frame.
    fn present(&mut self, frame: &VizFrame) -> Result<(), RenderError>;
    /// Release resources. Must be idempotent; also called from Drop by well-behaved impls.
    fn close(&mut self) -> Result<(), RenderError>;
}

/// Headless sink for CI and gates (G4): counts frames and records the dot that a real
/// surface would draw, so goldens can be checked without a terminal, window or socket.
/// `present()` is O(1), allocation-free and well under the 1 ms budget.
#[derive(Debug, Default)]
pub struct NullSurface {
    pub frames: u64,
    /// Dot params of the most recent frame (None before the first `present`).
    pub last_dot: Option<DotParams>,
}
impl NullSurface {
    pub fn new() -> Self { Self::default() }
}
impl RenderSurface for NullSurface {
    fn kind(&self) -> SurfaceKind { SurfaceKind::Null }
    fn caps(&self) -> SurfaceCaps { SurfaceCaps { color: false, pixels: false, max_fps: f32::INFINITY, size: (0, 0) } }
    fn open(&mut self) -> Result<(), RenderError> { Ok(()) }
    fn present(&mut self, f: &VizFrame) -> Result<(), RenderError> {
        self.frames += 1;
        self.last_dot = Some(DotParams::from_frame(f));
        Ok(())
    }
    fn close(&mut self) -> Result<(), RenderError> { Ok(()) }
}

struct Slot {
    surface: Box<dyn RenderSurface>,
    min_dt_ms: u64,
    last_t_ms: Option<u64>,
}

/// Pushes each frame to every attached surface; decimates per surface `max_fps`;
/// a surface that errors is closed and detached (render never stalls voice).
#[derive(Default)]
pub struct Fanout {
    slots: Vec<Slot>,
    dropped: Vec<(SurfaceKind, String)>,
}

impl Fanout {
    pub fn new() -> Self { Self::default() }

    /// Open and attach. On open failure the surface is not attached and the error returned
    /// (caller typically logs and falls back to terminal).
    pub fn attach(&mut self, mut s: Box<dyn RenderSurface>) -> Result<(), RenderError> {
        s.open()?;
        let fps = s.caps().max_fps;
        let min_dt_ms = if fps.is_finite() && fps > 0.0 { (1000.0 / fps) as u64 } else { 0 };
        self.slots.push(Slot { surface: s, min_dt_ms, last_t_ms: None });
        Ok(())
    }

    pub fn len(&self) -> usize { self.slots.len() }
    pub fn is_empty(&self) -> bool { self.slots.is_empty() }
    pub fn kinds(&self) -> Vec<SurfaceKind> { self.slots.iter().map(|s| s.surface.kind()).collect() }
    /// Surfaces detached due to errors, with reason.
    pub fn dropped(&self) -> &[(SurfaceKind, String)] { &self.dropped }

    pub fn present(&mut self, frame: &VizFrame) {
        let dropped = &mut self.dropped;
        self.slots.retain_mut(|slot| {
            if let Some(last) = slot.last_t_ms {
                if frame.t_ms.saturating_sub(last) < slot.min_dt_ms {
                    return true; // decimated
                }
            }
            match slot.surface.present(frame) {
                Ok(()) => { slot.last_t_ms = Some(frame.t_ms); true }
                Err(e) => {
                    let _ = slot.surface.close();
                    dropped.push((slot.surface.kind(), e.to_string()));
                    false
                }
            }
        });
    }

    pub fn close_all(&mut self) {
        for s in &mut self.slots { let _ = s.surface.close(); }
        self.slots.clear();
    }
}

impl Drop for Fanout {
    fn drop(&mut self) { self.close_all(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Failing;
    impl RenderSurface for Failing {
        fn kind(&self) -> SurfaceKind { SurfaceKind::Window }
        fn caps(&self) -> SurfaceCaps { SurfaceCaps { color: true, pixels: true, max_fps: 60.0, size: (0, 0) } }
        fn open(&mut self) -> Result<(), RenderError> { Ok(()) }
        fn present(&mut self, _: &VizFrame) -> Result<(), RenderError> { Err(RenderError::Closed) }
        fn close(&mut self) -> Result<(), RenderError> { Ok(()) }
    }

    #[test]
    fn failing_surface_is_detached_others_continue() {
        let mut f = Fanout::new();
        f.attach(Box::new(NullSurface::default())).unwrap();
        f.attach(Box::new(Failing)).unwrap();
        f.present(&VizFrame::default());
        assert_eq!(f.kinds(), vec![SurfaceKind::Null]);
        assert_eq!(f.dropped().len(), 1);
    }

    #[test]
    fn null_surface_records_dot() {
        use crate::dot::DotState;
        let mut n = NullSurface::new();
        assert!(n.last_dot.is_none());
        let mut f = VizFrame::default();
        f.speak.active = true;
        n.present(&f).unwrap();
        assert_eq!(n.frames, 1);
        assert_eq!(n.last_dot.unwrap().state, DotState::Speaking);
    }

    /// G4 timing note: `present()` must stay < 1 ms (FINAL-BUILD-SPEC S5). Asserted on the
    /// mean over 10k frames (robust to CI jitter) even in debug builds; the worst case is
    /// printed (`cargo test -- --nocapture`) but not asserted, since a shared CI host can
    /// preempt any single call.
    #[test]
    fn null_present_under_1ms() {
        let mut fan = Fanout::new();
        fan.attach(Box::new(NullSurface::new())).unwrap();
        let mut f = VizFrame::default();
        let n = 10_000u64;
        let mut worst = std::time::Duration::ZERO;
        let t0 = std::time::Instant::now();
        for i in 0..n {
            f.seq = i;
            f.t_ms = i * 80;
            f.speak.active = i % 3 == 0;
            f.listen.active = i % 5 == 0;
            let s = std::time::Instant::now();
            fan.present(&f);
            worst = worst.max(s.elapsed());
        }
        let mean = t0.elapsed() / n as u32;
        eprintln!("NullSurface via Fanout: mean {mean:?}, worst {worst:?} over {n} frames");
        assert!(mean < std::time::Duration::from_millis(1), "mean {mean:?}");
    }
}
