//! Backends, each behind its own cargo feature.
#[cfg(feature = "terminal")]
pub mod terminal;
#[cfg(feature = "window")]
pub mod window;
#[cfg(feature = "web")]
mod edge;
#[cfg(feature = "web")]
pub mod web;
/// Headless PNG framebuffer surface (std-only, always compiled; W21-04).
pub mod png_seq;
