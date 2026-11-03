//! # galion-render
//!
//! Hybrid rendering for Galion Voice (Perpetual Open MIT License).
//!
//! One data model ([`VizFrame`]) and one trait ([`RenderSurface`]); many surfaces:
//!
//! | feature    | surface               | deps        | status |
//! |------------|-----------------------|-------------|--------|
//! | `terminal` | [`TerminalSurface`]   | std only    | working (default) |
//! | `window`   | [`WindowSurface`]     | none (stub) | stub, returns `Unsupported` on open |
//! | `web`      | [`WebSurface`]        | std only    | JSON frames + optional tiny HTTP endpoint |
//! | (always)   | [`PngSeqSurface`]     | std only    | headless framebuffer → PNG (`latest.png` + sequence), W21-04 |
//!
//! The voice pipeline never knows which surface is attached: it produces `VizFrame`s
//! (dual-band levels, emotion, transcript tails, latency) and a [`Fanout`] pushes them to
//! every surface. Rendering is strictly *best-effort and non-blocking for audio*: a slow
//! or broken surface is dropped, never allowed to stall the duplex loop.
//!
//! **User surface = one dot** ([`dot`]): `#F5F5F7` on obsidian, amber→violet on the
//! background only. The discrete golden ([`DotParams`]) is still a solid fill
//! scaled by brightness, and that is what G4 hashes. The phone page paints the
//! locked pigment itself (a near-white gradient whose body stop is `#F5F5F7`)
//! and uses brightness for glow and a little size, not as a multiply on the fill.
//! [`NullSurface`] records [`DotParams`] and does not see the phone gradient.
//!
//! [`OrbEngine`] is the display-rate spring between those discrete targets
//! (60 / 120 / 240 Hz, variable `dt`), on the [`Luti`] easing profile.
//! Aspect stays 1 — a circle. PNG, the terminal, and [`NullSurface`] do not
//! call it, so the G4 golden stays [`DotParams`]. The web `/dot` object may
//! carry a one-step Luti account beside that golden. The engine must not run
//! inside `Heartbeat::step`.
//!
//! `galion-console` owns *what* is shown (layout, panels, colors); `galion-render` owns
//! *where* it is shown. Console builds a `VizFrame`, render delivers it.

#![forbid(unsafe_code)]

pub mod frame;
pub mod palette;
pub mod dot;
pub mod spring;
pub mod orb_engine;
pub mod surface;
pub mod backends;
pub mod png;
/// Std-only WAV helpers for the phone path (mono PCM16, 16 kHz ↔ 24 kHz).
pub mod wav;
/// Instruction-layer floor for the one circle page. `GET /v2` redirects to `/`.
pub mod v2_persona;

pub use frame::{Band, BandState, Emotion, VizFrame};
pub use surface::{Fanout, NullSurface, RenderError, RenderSurface, SurfaceCaps, SurfaceKind};
pub use dot::{DotParams, DotState};
pub use spring::{Spring, SpringGain, SpringStep};
pub use orb_engine::{DisplayHz, Luti, OrbAccount, OrbEngine, OrbMotion, OrbSample};
pub use palette::Rgb;
pub use backends::png_seq::{PngSeqConfig, PngSeqSurface};

#[cfg(feature = "terminal")]
pub use backends::terminal::{TerminalDotSurface, TerminalSurface};
#[cfg(feature = "window")]
pub use backends::window::{FrameMailbox, WindowSurface};
#[cfg(feature = "web")]
pub use backends::web::{MicTap, ReplyBox, Turn, TurnFormat, TurnInbox, WebSurface};

/// Returns the surface kinds compiled into this build, in preference order.
#[allow(unused_mut)]
pub fn compiled_surfaces() -> Vec<SurfaceKind> {
    let mut v = Vec::new();
    #[cfg(feature = "terminal")]
    v.push(SurfaceKind::Terminal);
    #[cfg(feature = "window")]
    v.push(SurfaceKind::Window);
    #[cfg(feature = "web")]
    v.push(SurfaceKind::Web);
    v
}
