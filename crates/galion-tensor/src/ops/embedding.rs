// Galion Voice — Perpetual Open MIT License
use crate::{Result, Tensor, TensorError};

/// Row gather: `table[vocab, dim]`, `ids` -> `[ids.len(), dim]`.
pub fn embedding(table: &Tensor, ids: &[u32]) -> Result<Tensor> {
    if table.shape().rank() != 2 { return Err(TensorError::InvalidShape("embedding: table must be [vocab, dim]".into())); }
    let (vocab, dim) = (table.dims()[0], table.dims()[1]);
    let tv = table.contiguous();
    let td = tv.as_f32_slice().unwrap();
    let mut out = Vec::with_capacity(ids.len() * dim);
    for &id in ids {
        let id = id as usize;
        if id >= vocab { return Err(TensorError::IndexOutOfBounds { index: id, bound: vocab }); }
        out.extend_from_slice(&td[id * dim..(id + 1) * dim]);
    }
    Tensor::from_vec(out, [ids.len(), dim])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup() {
        let t = Tensor::from_vec((0..12).map(|x| x as f32).collect(), [4, 3]).unwrap();
        assert_eq!(embedding(&t, &[2, 0, 2]).unwrap().to_vec_f32(), vec![6., 7., 8., 0., 1., 2., 6., 7., 8.]);
        assert!(embedding(&t, &[4]).is_err());
        // works on bf16 storage too (upcast)
        assert_eq!(embedding(&t.to_bf16(), &[1]).unwrap().to_vec_f32(), vec![3., 4., 5.]);
    }
}
