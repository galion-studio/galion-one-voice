//! Nebula Galion Voice V2 library. The window and the browser are optional.

pub mod demo;
pub mod frame;
pub mod noise;
pub mod quality;
pub mod render;
pub mod shade;
pub mod sim;

#[cfg(target_arch = "wasm32")]
mod wasm_api;
