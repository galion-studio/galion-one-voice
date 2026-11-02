// Galion Voice — Perpetual Open MIT License
use crate::{Result, Tensor, TensorError};

#[derive(Debug, Clone, Copy)]
pub struct Conv1dParams { pub stride: usize, pub padding: usize, pub dilation: usize, pub groups: usize }
impl Default for Conv1dParams { fn default() -> Self { Self { stride: 1, padding: 0, dilation: 1, groups: 1 } } }

/// Naive scalar conv1d. `x`: `[c_in, len]`, `w`: `[c_out, c_in/groups, k]`, `bias`: `[c_out]`.
/// Reference only — im2col/SIMD and the streaming causal variant (codec encoders) are TODO.
pub fn conv1d(x: &Tensor, w: &Tensor, bias: Option<&Tensor>, p: Conv1dParams) -> Result<Tensor> {
    if x.shape().rank() != 2 || w.shape().rank() != 3 || p.groups == 0 || p.stride == 0 {
        return Err(TensorError::InvalidShape("conv1d: x [c_in,len], w [c_out,c_in/g,k]".into()));
    }
    let (c_in, len) = (x.dims()[0], x.dims()[1]);
    let (c_out, cpg, k) = (w.dims()[0], w.dims()[1], w.dims()[2]);
    if c_in % p.groups != 0 || c_out % p.groups != 0 || cpg != c_in / p.groups {
        return Err(TensorError::ShapeMismatch { op: "conv1d", lhs: x.dims().to_vec(), rhs: w.dims().to_vec() });
    }
    let span = p.dilation * (k - 1) + 1;
    if len + 2 * p.padding < span { return Err(TensorError::InvalidShape("conv1d: input too short".into())); }
    let out_len = (len + 2 * p.padding - span) / p.stride + 1;
    let (xv, wv) = (x.to_vec_f32(), w.to_vec_f32());
    let bv = bias.map(|b| b.to_vec_f32());
    let opg = c_out / p.groups;
    let mut out = vec![0f32; c_out * out_len];
    for oc in 0..c_out {
        let g = oc / opg;
        for t in 0..out_len {
            let mut acc = bv.as_ref().map_or(0.0, |b| b[oc]);
            for ic in 0..cpg {
                let xc = g * cpg + ic;
                for kk in 0..k {
                    let pos = (t * p.stride + kk * p.dilation) as isize - p.padding as isize;
                    if pos >= 0 && (pos as usize) < len {
                        acc += xv[xc * len + pos as usize] * wv[(oc * cpg + ic) * k + kk];
                    }
                }
            }
            out[oc * out_len + t] = acc;
        }
    }
    Tensor::from_vec(out, [c_out, out_len])
}

/// Parameters for [`conv_transpose1d`] (PyTorch `ConvTranspose1d` semantics).
#[derive(Debug, Clone, Copy)]
pub struct ConvTranspose1dParams { pub stride: usize, pub padding: usize, pub output_padding: usize, pub dilation: usize, pub groups: usize }
impl Default for ConvTranspose1dParams {
    fn default() -> Self { Self { stride: 1, padding: 0, output_padding: 0, dilation: 1, groups: 1 } }
}

/// Naive scalar transposed conv1d (upsampling). `x`: `[c_in, len]`, `w`: `[c_in, c_out/groups, k]`
/// (PyTorch layout), `bias`: `[c_out]`. Output length
/// `(len-1)*stride - 2*padding + dilation*(k-1) + output_padding + 1`.
///
/// Computed in **gather form** (each output sums its contributing inputs) with a fixed order:
/// bias, then `ic` (within the group), then `kk`. This order is the bit-exact contract shared with
/// [`crate::ConvTranspose1dStream`]. Versus a scatter/overlap-add implementation (e.g. PyTorch)
/// results agree to f32 rounding, not bit-for-bit.
pub fn conv_transpose1d(x: &Tensor, w: &Tensor, bias: Option<&Tensor>, p: ConvTranspose1dParams) -> Result<Tensor> {
    if x.shape().rank() != 2 || w.shape().rank() != 3 || p.groups == 0 || p.stride == 0 || p.dilation == 0 {
        return Err(TensorError::InvalidShape("conv_transpose1d: x [c_in,len], w [c_in,c_out/g,k]".into()));
    }
    let (c_in, len) = (x.dims()[0], x.dims()[1]);
    let (wc_in, opg, k) = (w.dims()[0], w.dims()[1], w.dims()[2]);
    if wc_in != c_in || c_in % p.groups != 0 || k == 0 || opg == 0 || len == 0 {
        return Err(TensorError::ShapeMismatch { op: "conv_transpose1d", lhs: x.dims().to_vec(), rhs: w.dims().to_vec() });
    }
    let c_out = opg * p.groups;
    if let Some(b) = bias {
        if b.dims() != [c_out] {
            return Err(TensorError::ShapeMismatch { op: "conv_transpose1d bias", lhs: vec![c_out], rhs: b.dims().to_vec() });
        }
    }
    let full = (len - 1) * p.stride + p.dilation * (k - 1) + p.output_padding + 1;
    if full <= 2 * p.padding { return Err(TensorError::InvalidShape("conv_transpose1d: padding too large".into())); }
    let out_len = full - 2 * p.padding;
    let (xv, wv) = (x.to_vec_f32(), w.to_vec_f32());
    let bv = bias.map(|b| b.to_vec_f32());
    let cpg = c_in / p.groups;
    let mut out = vec![0f32; c_out * out_len];
    for oc in 0..c_out {
        let (g, ol) = (oc / opg, oc % opg);
        for j in 0..out_len {
            let mut acc = bv.as_ref().map_or(0.0, |b| b[oc]);
            for ic in 0..cpg {
                let xc = g * cpg + ic;
                for kk in 0..k {
                    let r = (j + p.padding) as isize - (kk * p.dilation) as isize;
                    if r >= 0 && r as usize % p.stride == 0 && (r as usize / p.stride) < len {
                        acc += xv[xc * len + r as usize / p.stride] * wv[(xc * opg + ol) * k + kk];
                    }
                }
            }
            out[oc * out_len + j] = acc;
        }
    }
    Tensor::from_vec(out, [c_out, out_len])
}

/// Streaming causal conv1d with carried state — stub (needed for frame-by-frame codec on CPU).
pub fn conv1d_causal_streaming(_x: &Tensor, _w: &Tensor, _state: &mut Vec<f32>) -> Result<Tensor> {
    Err(TensorError::Unimplemented("conv1d_causal_streaming"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conv1d_basic_and_padding() {
        let x = Tensor::from_vec(vec![1., 2., 3., 4., 5.], [1, 5]).unwrap();
        let w = Tensor::from_vec(vec![1., 0., -1.], [1, 1, 3]).unwrap();
        let y = conv1d(&x, &w, None, Conv1dParams::default()).unwrap();
        assert_eq!(y.dims(), &[1, 3]);
        assert_eq!(y.to_vec_f32(), vec![-2., -2., -2.]);
        let b = Tensor::from_vec(vec![10.], [1]).unwrap();
        let y = conv1d(&x, &w, Some(&b), Conv1dParams { padding: 1, stride: 2, ..Default::default() }).unwrap();
        assert_eq!(y.to_vec_f32(), vec![8., 8., 14.]); // [-2, -2, 4] + 10 at t=0,2,4
    }
    #[test]
    fn conv1d_depthwise() {
        let x = Tensor::from_vec(vec![1., 1., 1., 2., 2., 2.], [2, 3]).unwrap();
        let w = Tensor::from_vec(vec![1., 1., 3., 3.], [2, 1, 2]).unwrap();
        let y = conv1d(&x, &w, None, Conv1dParams { groups: 2, ..Default::default() }).unwrap();
        assert_eq!(y.to_vec_f32(), vec![2., 2., 12., 12.]);
    }
    #[test]
    fn conv_transpose1d_hand_example_and_scatter_agreement() {
        // x=[1,2], w=[1,10,100], s=2: scatter gives [1,10,102,20,200].
        let x = Tensor::from_vec(vec![1., 2.], [1, 2]).unwrap();
        let w = Tensor::from_vec(vec![1., 10., 100.], [1, 1, 3]).unwrap();
        let y = conv_transpose1d(&x, &w, None, ConvTranspose1dParams { stride: 2, ..Default::default() }).unwrap();
        assert_eq!(y.to_vec_f32(), vec![1., 10., 102., 20., 200.]);
        let y = conv_transpose1d(&x, &w, Some(&Tensor::from_vec(vec![0.5], [1]).unwrap()),
            ConvTranspose1dParams { stride: 2, padding: 1, output_padding: 1, ..Default::default() }).unwrap();
        assert_eq!(y.to_vec_f32(), vec![10.5, 102.5, 20.5, 200.5]);
        // Grouped + dilated vs an independent scatter (overlap-add) implementation.
        let (c_in, len, g, opg, k, s, d) = (4usize, 7usize, 2usize, 3usize, 5usize, 3usize, 2usize);
        let xv: Vec<f32> = (0..c_in * len).map(|i| ((i * 37 % 11) as f32 - 5.0) * 0.25).collect();
        let wv: Vec<f32> = (0..c_in * opg * k).map(|i| ((i * 13 % 7) as f32 - 3.0) * 0.5).collect();
        let x = Tensor::from_vec(xv.clone(), [c_in, len]).unwrap();
        let w = Tensor::from_vec(wv.clone(), [c_in, opg, k]).unwrap();
        let y = conv_transpose1d(&x, &w, None, ConvTranspose1dParams { stride: s, dilation: d, groups: g, ..Default::default() }).unwrap();
        let ol = (len - 1) * s + d * (k - 1) + 1;
        let c_out = opg * g;
        let mut want = vec![0f32; c_out * ol];
        for ic in 0..c_in { for t in 0..len { for o in 0..opg { for kk in 0..k {
            let oc = (ic / (c_in / g)) * opg + o;
            want[oc * ol + t * s + kk * d] += xv[ic * len + t] * wv[(ic * opg + o) * k + kk];
        }}}}
        assert_eq!(y.dims(), &[c_out, ol]);
        crate::ops::assert_close(&y.to_vec_f32(), &want, 1e-6);
    }
}
