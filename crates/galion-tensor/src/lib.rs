// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! `galion-tensor`: CPU/VPS-first tensor runtime skeleton for Galion Voice.
//!
//! Design rules:
//! * Scalar reference path is the source of truth (`ops::*` with `Exec::Scalar`).
//! * Optional rayon parallel path (feature `parallel`) must match the scalar path bit-for-bit
//!   or within a documented tolerance.
//! * No third-party inference runtimes. Weights will come from Galion's own `.gq` format (future).

pub mod attention;
pub mod conv_stream;
pub mod conv_resample;
pub mod dtype;
pub mod error;
pub mod features;
pub mod gemv;
pub mod gemv_i8;
pub mod kv;
pub mod gq;
pub mod ops;
pub mod shape;
pub mod simd;
pub mod storage;
pub mod tensor;
pub mod threading;

pub use attention::{attention_reference, RopeNeox, StreamingAttention};
pub use conv_stream::CausalConv1dStream;
pub use conv_resample::{ConvTranspose1dStream, StridedConv1dStream};
pub use dtype::{Bf16, DType};
pub use error::{Result, TensorError};
pub use features::SimdLevel;
pub use gemv::{gemv, gemv_f32_into, gemv_t_f32_into};
pub use gemv_i8::{gemv_i8_f32x_into, gemv_i8_into, quantize_row_i8, QuantRowsI8};
pub use kv::KvRing;
pub use gq::{GqDType, GqError, GqFile, Prefault};
pub use shape::Shape;
pub use storage::Storage;
pub use tensor::Tensor;
pub use threading::Exec;
