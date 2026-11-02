// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
// © 2026 Maciej Grajczyk
//! `galion-mimi`: Mimi neural audio codec path (agent A6).
//!
//! STATUS: STUB (`IS_STUB = true`). There are **no weights** in this crate and none may be added
//! (FINAL-BUILD-SPEC §5: "no weights / no invented Mimi").
//!
//! Wave 2: the SEANet encoder/decoder, the 25⇄12.5 Hz down/upsample convs and the split RVQ are
//! **real kernels** on galion-tensor's streaming ops (A5: `CausalConv1dStream`,
//! `StridedConv1dStream`, `ConvTranspose1dStream`), built from a `weights::WeightSource`. Without
//! a source (the default, and the only production state until A21 clears the licence) the codec
//! skips the kernels and returns zero codes / silence.
//!
//! Wave 20 (W20-15): the streaming transformer is now a **real kernel** too
//! ([`transformer::StreamingTransformer`]: LayerNorm → RoPE MHA over the A5 `KvRing` → LayerScale,
//! LayerNorm → GELU MLP → LayerScale), bit-identical to its batch reference and checked against an
//! f64 oracle — on synthetic in-test tensors only. With a weight source every stage of the codec is
//! a real kernel, but no real weights exist here, so output is still not Mimi audio.
//!
//! Op inventory (decode-first; see `docs/A06-mimi-plan.md` mirror in galion-voice-20/v4/agents):
//! * [`seanet`] — SEANet causal conv encoder/decoder (Conv1d, ConvTranspose1d, ELU, residual units).
//! * [`transformer`] — causal streaming transformer bottleneck (RoPE, MHA w/ KV ring, LayerScale, GELU MLP); real kernel + batch reference.
//! * [`rvq`] — split residual vector quantizer (1 semantic + N acoustic codebooks).
//! * [`codec`] — the 80 ms frame API ([`codec::MimiCodec`], alias `MimiStub`) tying the above together.
//! * [`weights`] — weight contract (names, shapes, licence gate), `WeightSource` seam, `load_codec`
//!   (refuses until A21).
//!
//! Numbers in [`config`] follow Kyutai's published Mimi description (24 kHz, 12.5 Hz, 512-d latent,
//! 2048-entry codebooks). They are marked `VERIFY` until checked against the real
//! config shipped with the CC-BY weights once A21 clears the licence path.

pub mod codec;
pub mod config;
pub mod rvq;
pub mod seanet;
pub mod tensor;
pub mod transformer;
pub mod weights;

pub use codec::{MimiCodec, MimiStub, StubError};
pub use config::MimiConfig;

/// True for this whole crate until real, licence-cleared weights load through `weights::load_codec`
/// and golden frames match upstream. (Since W20-15 all kernels incl. the transformer are real, but
/// they have only seen synthetic tensors; the `VERIFY` hyper-parameters / conventions are unchecked.) Callers (gates, demo) must check this
/// and never present stub output as real audio.
pub const IS_STUB: bool = true;
