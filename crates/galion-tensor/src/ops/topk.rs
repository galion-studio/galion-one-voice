// Galion Voice — Perpetual Open MIT License
use crate::{Result, Tensor, TensorError};

/// Top-k over the last dim, sorted descending (ties -> lower index first).
/// Returns `(values [.., k], indices [.., k])`. O(n log n) per row; partial-select is a TODO.
pub fn top_k(x: &Tensor, k: usize) -> Result<(Tensor, Vec<u32>)> {
    let d = x.shape().last();
    if k == 0 || k > d { return Err(TensorError::InvalidShape(format!("top_k: k={k} for last dim {d}"))); }
    let v = x.to_vec_f32();
    let rows = v.len() / d;
    let (mut vals, mut idxs) = (Vec::with_capacity(rows * k), Vec::with_capacity(rows * k));
    let mut order: Vec<u32> = Vec::with_capacity(d);
    for r in 0..rows {
        let row = &v[r * d..(r + 1) * d];
        order.clear();
        order.extend(0..d as u32);
        order.sort_by(|&a, &b| row[b as usize].total_cmp(&row[a as usize]).then(a.cmp(&b)));
        for &i in &order[..k] { vals.push(row[i as usize]); idxs.push(i); }
    }
    let mut dims = x.dims().to_vec();
    if dims.is_empty() { dims.push(1); }
    *dims.last_mut().unwrap() = k;
    Ok((Tensor::from_vec(vals, dims)?, idxs))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topk_rows() {
        let x = Tensor::from_vec(vec![0.1, 0.9, 0.5, 0.9, -1., 3., 2., 1.], [2, 4]).unwrap();
        let (v, i) = top_k(&x, 2).unwrap();
        assert_eq!(v.dims(), &[2, 2]);
        assert_eq!(v.to_vec_f32(), vec![0.9, 0.9, 3., 2.]);
        assert_eq!(i, vec![1, 3, 1, 2]);
        assert!(top_k(&x, 5).is_err());
    }
}
