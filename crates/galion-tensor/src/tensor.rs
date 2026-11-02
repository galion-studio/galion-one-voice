// Galion Voice — Perpetual Open MIT License
use crate::{dtype::Bf16, DType, Result, Shape, Storage, TensorError};
use std::sync::Arc;

/// Strided view over shared storage. Views (reshape/transpose) are zero-copy.
#[derive(Debug, Clone)]
pub struct Tensor {
    storage: Storage,
    shape: Shape,
    strides: Vec<usize>,
    offset: usize,
}

impl Tensor {
    pub fn from_vec(data: Vec<f32>, shape: impl Into<Shape>) -> Result<Self> {
        let shape = shape.into();
        if data.len() != shape.numel() {
            return Err(TensorError::InvalidShape(format!("{} elems for shape {:?}", data.len(), shape.dims())));
        }
        let strides = shape.contiguous_strides();
        Ok(Tensor { storage: Storage::F32(Arc::new(data)), shape, strides, offset: 0 })
    }
    pub fn zeros(shape: impl Into<Shape>) -> Self {
        let shape = shape.into();
        Self::from_vec(vec![0.0; shape.numel()], shape).expect("zeros")
    }
    pub fn full(shape: impl Into<Shape>, v: f32) -> Self {
        let shape = shape.into();
        Self::from_vec(vec![v; shape.numel()], shape).expect("full")
    }
    pub fn scalar(v: f32) -> Self { Self::from_vec(vec![v], Shape(vec![])).expect("scalar") }

    /// Store as bf16 (placeholder storage dtype).
    pub fn to_bf16(&self) -> Self {
        let v: Vec<Bf16> = self.to_vec_f32().into_iter().map(Bf16::from_f32).collect();
        Tensor { storage: Storage::BF16(Arc::new(v)), shape: self.shape.clone(), strides: self.shape.contiguous_strides(), offset: 0 }
    }
    /// Symmetric per-tensor i8 quantization (placeholder; not the `.gq` scheme).
    pub fn to_i8_symmetric(&self) -> Self {
        let v = self.to_vec_f32();
        let amax = v.iter().fold(0f32, |m, x| m.max(x.abs()));
        let scale = if amax == 0.0 { 1.0 } else { amax / 127.0 };
        let q: Vec<i8> = v.iter().map(|x| (x / scale).round().clamp(-127.0, 127.0) as i8).collect();
        Tensor { storage: Storage::I8 { data: Arc::new(q), scale }, shape: self.shape.clone(), strides: self.shape.contiguous_strides(), offset: 0 }
    }

    pub fn shape(&self) -> &Shape { &self.shape }
    pub fn dims(&self) -> &[usize] { self.shape.dims() }
    pub fn strides(&self) -> &[usize] { &self.strides }
    pub fn dtype(&self) -> DType { self.storage.dtype() }
    pub fn numel(&self) -> usize { self.shape.numel() }
    pub fn storage(&self) -> &Storage { &self.storage }
    /// Element offset of this view into its storage (needed for zero-copy strided fast paths).
    pub fn offset(&self) -> usize { self.offset }

    pub fn is_contiguous(&self) -> bool { self.strides == self.shape.contiguous_strides() }

    /// Fast path: borrow contiguous f32 data.
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match &self.storage {
            Storage::F32(v) if self.is_contiguous() => Some(&v[self.offset..self.offset + self.numel()]),
            _ => None,
        }
    }

    /// Element at multi-index, upcast to f32.
    pub fn get(&self, idx: &[usize]) -> Result<f32> {
        if idx.len() != self.shape.rank() {
            return Err(TensorError::InvalidShape(format!("index rank {} != {}", idx.len(), self.shape.rank())));
        }
        let mut off = self.offset;
        for (i, (&ix, &d)) in idx.iter().zip(self.dims()).enumerate() {
            if ix >= d { return Err(TensorError::IndexOutOfBounds { index: ix, bound: d }); }
            off += ix * self.strides[i];
        }
        Ok(self.storage.get_f32(off))
    }

    /// Materialize logical (row-major) order as f32, handling any strides/dtype.
    pub fn to_vec_f32(&self) -> Vec<f32> {
        if let Some(s) = self.as_f32_slice() { return s.to_vec(); }
        let n = self.numel();
        let rank = self.shape.rank();
        let mut out = Vec::with_capacity(n);
        let mut idx = vec![0usize; rank];
        for _ in 0..n {
            let off = self.offset + idx.iter().zip(&self.strides).map(|(i, s)| i * s).sum::<usize>();
            out.push(self.storage.get_f32(off));
            for d in (0..rank).rev() {
                idx[d] += 1;
                if idx[d] < self.dims()[d] { break; }
                idx[d] = 0;
            }
        }
        out
    }

    pub fn contiguous(&self) -> Tensor {
        if self.is_contiguous() && self.offset == 0 && self.dtype() == DType::F32 { return self.clone(); }
        Tensor::from_vec(self.to_vec_f32(), self.shape.clone()).expect("contiguous")
    }

    pub fn reshape(&self, shape: impl Into<Shape>) -> Result<Tensor> {
        let shape = shape.into();
        if shape.numel() != self.numel() {
            return Err(TensorError::ShapeMismatch { op: "reshape", lhs: self.dims().to_vec(), rhs: shape.0 });
        }
        let base = if self.is_contiguous() { self.clone() } else { self.contiguous() };
        let strides = shape.contiguous_strides();
        Ok(Tensor { storage: base.storage, shape, strides, offset: base.offset })
    }

    /// Zero-copy swap of two axes.
    pub fn transpose(&self, a: usize, b: usize) -> Result<Tensor> {
        let r = self.shape.rank();
        if a >= r || b >= r { return Err(TensorError::IndexOutOfBounds { index: a.max(b), bound: r }); }
        let mut t = self.clone();
        t.shape.0.swap(a, b);
        t.strides.swap(a, b);
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn views_and_dtypes() {
        let t = Tensor::from_vec((0..6).map(|x| x as f32).collect(), [2, 3]).unwrap();
        let tt = t.transpose(0, 1).unwrap();
        assert_eq!(tt.dims(), &[3, 2]);
        assert!(!tt.is_contiguous());
        assert_eq!(tt.to_vec_f32(), vec![0., 3., 1., 4., 2., 5.]);
        assert_eq!(tt.get(&[2, 1]).unwrap(), 5.0);
        assert_eq!(tt.reshape([6]).unwrap().to_vec_f32(), vec![0., 3., 1., 4., 2., 5.]);
        assert_eq!(t.to_bf16().to_vec_f32(), t.to_vec_f32());
        let q = t.to_i8_symmetric();
        assert_eq!(q.dtype(), DType::I8);
        for (a, b) in q.to_vec_f32().iter().zip(t.to_vec_f32()) { assert!((a - b).abs() < 0.03); }
        assert!(Tensor::from_vec(vec![1.0; 5], [2, 3]).is_err());
    }
}
