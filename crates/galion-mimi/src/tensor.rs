// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Fake-zero tensor: a shape plus a flat `[c, n]` f32 buffer. Used for frame-loop activations that
//! the real kernels write into (`data`) and for the transformer placeholder. Weights themselves are
//! `galion_tensor::Tensor` (via `weights::WeightSource`). `Default` is the empty, non-allocating value.

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FakeTensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl FakeTensor {
    /// Allocate a zero tensor. Allowed only at construction time, never in the frame loop.
    pub fn zeros(shape: &[usize]) -> Self {
        let n = shape.iter().product();
        Self { shape: shape.to_vec(), data: vec![0.0; n] }
    }
    pub fn numel(&self) -> usize {
        self.data.len()
    }
    pub fn is_all_zero(&self) -> bool {
        self.data.iter().all(|v| *v == 0.0)
    }
    /// Zero in place (frame-loop safe, no allocation).
    pub fn fill_zero(&mut self) {
        self.data.iter_mut().for_each(|v| *v = 0.0);
    }
}
