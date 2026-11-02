// Galion Voice — Perpetual Open MIT License
//! Reference-counted, CPU-resident buffers. Future: mmap-backed `.gq` blocks.
use crate::{dtype::Bf16, DType};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Storage {
    F32(Arc<Vec<f32>>),
    BF16(Arc<Vec<Bf16>>),
    /// Symmetric per-tensor i8 placeholder: real = q * scale.
    I8 { data: Arc<Vec<i8>>, scale: f32 },
}

impl Storage {
    pub fn dtype(&self) -> DType {
        match self {
            Storage::F32(_) => DType::F32,
            Storage::BF16(_) => DType::BF16,
            Storage::I8 { .. } => DType::I8,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Storage::F32(v) => v.len(),
            Storage::BF16(v) => v.len(),
            Storage::I8 { data, .. } => data.len(),
        }
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    /// Read element `i` upcast to f32 (dequant path for placeholders).
    #[inline]
    pub fn get_f32(&self, i: usize) -> f32 {
        match self {
            Storage::F32(v) => v[i],
            Storage::BF16(v) => v[i].to_f32(),
            Storage::I8 { data, scale } => data[i] as f32 * *scale,
        }
    }
    pub fn nbytes(&self) -> usize { self.len() * self.dtype().size_of() }
}
