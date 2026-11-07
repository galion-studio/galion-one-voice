// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Public helpers on small in-memory buffers. CPU only: no weight files, no device.

use galion_tensor::features::{cpu_supports, report, select_with_cap, selected};
use galion_tensor::gemv_i8::gemv_i8_scalar_ref;
use galion_tensor::gq::{parse_sha256sums, sha256, to_hex};
use galion_tensor::ops::{
    add, conv1d, conv1d_causal_streaming, conv_transpose1d, embedding, gelu, matmul, mul, rms_norm, rope_interleaved, rope_neox,
    silu, softmax, top_k, Conv1dParams, ConvTranspose1dParams,
};
use galion_tensor::{
    attention_reference, gemv, gemv_f32_into, gemv_i8_f32x_into, gemv_t_f32_into, quantize_row_i8, CausalConv1dStream,
    ConvTranspose1dStream, DType, Exec, KvRing, QuantRowsI8, Shape, SimdLevel, Storage, StreamingAttention, StridedConv1dStream,
    Tensor, TensorError,
};

fn close(got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len());
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        let tol = 1e-5 * (1.0 + b.abs());
        assert!((a - b).abs() <= tol, "idx {i}: {a} vs {b}");
    }
}

#[test]
fn tiny_buffers_views_storage_and_dtypes() {
    let t = Tensor::from_vec(vec![1.0, -0.5, 0.0, 1.5], [2, 2]).unwrap();
    assert_eq!(t.dtype(), DType::F32);
    assert_eq!(t.storage().nbytes(), 16);
    assert_eq!(t.numel(), 4);
    assert_eq!(t.get(&[1, 0]).unwrap(), 0.0);
    assert!(matches!(t.get(&[2, 0]), Err(TensorError::IndexOutOfBounds { index: 2, bound: 2 })));
    assert!(Tensor::from_vec(vec![1.0], [2, 2]).is_err());

    let col = t.transpose(0, 1).unwrap();
    assert_eq!(col.dims(), &[2, 2]);
    assert!(!col.is_contiguous());
    assert_eq!(col.to_vec_f32(), vec![1.0, 0.0, -0.5, 1.5]);
    assert_eq!(col.reshape([4]).unwrap().to_vec_f32(), vec![1.0, 0.0, -0.5, 1.5]);

    let bf = t.to_bf16();
    assert_eq!(bf.dtype(), DType::BF16);
    assert_eq!(bf.storage().nbytes(), 8);
    assert_eq!(bf.to_vec_f32(), t.to_vec_f32());

    let q = Tensor::from_vec(vec![127.0, -127.0], [2]).unwrap().to_i8_symmetric();
    assert_eq!(q.dtype(), DType::I8);
    assert_eq!(q.to_vec_f32(), vec![127.0, -127.0]);
    assert_eq!(Tensor::zeros([2]).to_i8_symmetric().to_vec_f32(), vec![0.0, 0.0]);

    let s = Tensor::scalar(3.0);
    assert_eq!(s.dims(), &[] as &[usize]);
    assert_eq!(s.get(&[]).unwrap(), 3.0);
    assert_eq!(Tensor::full([2, 1], 4.0).to_vec_f32(), vec![4.0, 4.0]);

    assert_eq!(Shape::from([2, 1]).broadcast(&Shape::from([1, 3])).unwrap(), Shape::from([2, 3]));
    assert!(Shape::from([2]).broadcast(&Shape::from([3])).is_err());
    assert!(matches!(t.storage(), Storage::F32(_)));
}

#[test]
fn elementwise_norm_and_linear_ops_on_tiny_buffers() {
    let a = Tensor::from_vec(vec![1.0, 2.0, 3.0], [3]).unwrap();
    assert_eq!(add(&a, &Tensor::scalar(1.0)).unwrap().to_vec_f32(), vec![2.0, 3.0, 4.0]);
    let row = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3]).unwrap();
    let scale = Tensor::from_vec(vec![2.0, 3.0], [2, 1]).unwrap();
    assert_eq!(mul(&row, &scale).unwrap().to_vec_f32(), vec![2.0, 4.0, 6.0, 12.0, 15.0, 18.0]);
    assert!(add(&a, &Tensor::zeros([2])).is_err());

    let z = Tensor::from_vec(vec![0.0, 0.0], [2]).unwrap();
    assert_eq!(gelu(&z, Exec::Scalar).unwrap().to_vec_f32(), vec![0.0, 0.0]);
    assert_eq!(silu(&z, Exec::Scalar).unwrap().to_vec_f32(), vec![0.0, 0.0]);
    let act = Tensor::from_vec(vec![-1.0, 0.0, 1.0], [3]).unwrap();
    assert_eq!(gelu(&act, Exec::Parallel).unwrap().to_vec_f32(), gelu(&act, Exec::Scalar).unwrap().to_vec_f32());
    assert_eq!(silu(&act, Exec::Parallel).unwrap().to_vec_f32(), silu(&act, Exec::Scalar).unwrap().to_vec_f32());

    let sm = softmax(&Tensor::from_vec(vec![0.0, 0.0], [2]).unwrap(), Exec::Scalar).unwrap();
    assert_eq!(sm.to_vec_f32(), vec![0.5, 0.5]);
    let x = Tensor::from_vec(vec![3.0, 4.0], [2]).unwrap();
    let w = Tensor::from_vec(vec![1.0, 2.0], [2]).unwrap();
    let inv = 1.0 / (12.5f32).sqrt();
    close(&rms_norm(&x, &w, 0.0, Exec::Scalar).unwrap().to_vec_f32(), &[3.0 * inv, 4.0 * inv * 2.0]);
    assert_eq!(
        rms_norm(&x, &w, 1e-5, Exec::Parallel).unwrap().to_vec_f32(),
        rms_norm(&x, &w, 1e-5, Exec::Scalar).unwrap().to_vec_f32()
    );

    let lhs = Tensor::from_vec(vec![1.0, 2.0, 3.0], [1, 3]).unwrap();
    let rhs = Tensor::from_vec(vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0], [3, 2]).unwrap();
    let y = matmul(&lhs, &rhs, Exec::Scalar).unwrap();
    assert_eq!(y.dims(), &[1, 2]);
    assert_eq!(y.to_vec_f32(), vec![14.0, 32.0]);
    let wt = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3]).unwrap();
    assert_eq!(gemv(&wt, &lhs.reshape([3]).unwrap(), None, Exec::Scalar).unwrap().to_vec_f32(), vec![14.0, 32.0]);
    assert_eq!(matmul(&lhs, &rhs, Exec::Parallel).unwrap().to_vec_f32(), y.to_vec_f32());

    let mut into = [0.0; 2];
    gemv_f32_into(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, 3, &[1.0, 2.0, 3.0], Some(&[0.5, -0.5]), &mut into, Exec::Scalar).unwrap();
    assert_eq!(into, [14.5, 31.5]);
    let mut into_t = [0.0; 2];
    gemv_t_f32_into(SimdLevel::Scalar, &[1.0, 2.0, 3.0], &[1.0, 4.0, 2.0, 5.0, 3.0, 6.0], 3, 2, None, &mut into_t, Exec::Scalar).unwrap();
    assert_eq!(into_t, [14.0, 32.0]);
    assert!(gemv_f32_into(&[1.0], 2, 2, &[1.0, 0.0], None, &mut into, Exec::Scalar).is_err());
}

#[test]
fn i8_gemv_matches_scalar_oracle_on_a_2x2() {
    let a = [1.0f32, 0.0, 0.0, 2.0];
    let w = QuantRowsI8::quantize(&a, 2, 2).unwrap();
    assert_eq!(w.m, 2);
    assert_eq!(w.k, 2);
    assert_eq!(w.nbytes(), 2 * 2 + 4 * 2);
    let mut row = [0.0; 2];
    w.dequantize_row(1, &mut row);
    close(&row, &[0.0, 2.0]);

    let x = [1.0f32, 1.0];
    let mut scratch = [0i8; 2];
    let mut y = [0.0; 2];
    gemv_i8_f32x_into(&w, &x, &mut scratch, Some(&[0.5, -0.5]), &mut y, Exec::Scalar).unwrap();
    assert_eq!(y, [1.5, 1.5]);
    let mut y_par = [0.0; 2];
    gemv_i8_f32x_into(&w, &x, &mut scratch, Some(&[0.5, -0.5]), &mut y_par, Exec::Parallel).unwrap();
    assert_eq!(y_par, y);
    let mut oracle_q = [0i8; 2];
    let xs = quantize_row_i8(&x, &mut oracle_q);
    assert_eq!(gemv_i8_scalar_ref(&w, &oracle_q, xs, Some(&[0.5, -0.5])), y.to_vec());

    let mut zeros = [9i8; 2];
    assert_eq!(quantize_row_i8(&[0.0, 0.0], &mut zeros), 1.0);
    assert_eq!(zeros, [0, 0]);
    assert!(QuantRowsI8::from_parts(vec![1, 2, 3], vec![1.0], 2, 2).is_err());
}

#[test]
fn embedding_topk_rope_and_conv_on_tiny_buffers() {
    let table = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], [2, 2]).unwrap();
    assert_eq!(embedding(&table, &[1, 0]).unwrap().to_vec_f32(), vec![3.0, 4.0, 1.0, 2.0]);
    assert!(embedding(&table, &[2]).is_err());

    let scores = Tensor::from_vec(vec![0.2, 1.5, -0.4, 1.5], [4]).unwrap();
    let (vals, idx) = top_k(&scores, 2).unwrap();
    assert_eq!(vals.to_vec_f32(), vec![1.5, 1.5]);
    assert_eq!(idx, vec![1, 3]);
    assert!(top_k(&scores, 0).is_err());

    let x = Tensor::from_vec(vec![0.25, -0.5, 1.0, 0.5], [1, 1, 4]).unwrap();
    assert_eq!(rope_neox(&x, 0, 10_000.0).unwrap().to_vec_f32(), x.to_vec_f32());
    assert!(rope_neox(&Tensor::zeros([1, 1, 3]), 0, 10_000.0).is_err());
    assert!(matches!(rope_interleaved(&x, 0, 10_000.0), Err(TensorError::Unimplemented(_))));

    let signal = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0], [1, 5]).unwrap();
    let kernel = Tensor::from_vec(vec![1.0, 0.0, -1.0], [1, 1, 3]).unwrap();
    let y = conv1d(&signal, &kernel, None, Conv1dParams::default()).unwrap();
    assert_eq!(y.to_vec_f32(), vec![-2.0, -2.0, -2.0]);

    let up_x = Tensor::from_vec(vec![1.0, 2.0], [1, 2]).unwrap();
    let up_w = Tensor::from_vec(vec![1.0, 10.0, 100.0], [1, 1, 3]).unwrap();
    let up = conv_transpose1d(&up_x, &up_w, None, ConvTranspose1dParams { stride: 2, ..Default::default() }).unwrap();
    assert_eq!(up.to_vec_f32(), vec![1.0, 10.0, 102.0, 20.0, 200.0]);

    let mut stream = CausalConv1dStream::new(&kernel, None, 1, 1, 5).unwrap();
    let mut streamed = vec![0.0; 5];
    stream.step(&signal.to_vec_f32(), 5, &mut streamed).unwrap();
    let padded = conv1d(&signal, &kernel, None, Conv1dParams { padding: 2, ..Default::default() }).unwrap();
    assert_eq!(streamed, padded.to_vec_f32()[..5]);

    let mut down = StridedConv1dStream::new(&kernel, None, 2, 1, 1, 4).unwrap();
    assert_eq!(down.left_padding(), 1);
    let frame = [1.0f32, 2.0, 3.0, 4.0];
    assert_eq!(down.outputs_for(frame.len()), 2);
    let mut down_out = vec![0.0; 2];
    let m = down.step(&frame, frame.len(), &mut down_out).unwrap();
    assert_eq!(m, 2);
    let whole = Tensor::from_vec(frame.to_vec(), [1, 4]).unwrap();
    let reference = conv1d(&whole, &kernel, None, Conv1dParams { stride: 2, padding: 1, ..Default::default() }).unwrap();
    assert_eq!(down_out, reference.to_vec_f32());

    let mut up_stream = ConvTranspose1dStream::new(&up_w, None, 2, 1, 1, 2).unwrap();
    let mut up_out = [0.0; 4];
    up_stream.step(&[1.0, 2.0], 2, &mut up_out).unwrap();
    assert_eq!(up_out, [1.0, 10.0, 102.0, 20.0]);

    let mut state = Vec::new();
    assert!(matches!(conv1d_causal_streaming(&signal, &kernel, &mut state), Err(TensorError::Unimplemented(_))));
}

#[test]
fn kv_ring_and_one_token_attention() {
    let mut ring = KvRing::new(2, 1, 2).unwrap();
    ring.push(&[1.0, 2.0], &[3.0, 4.0]).unwrap();
    ring.push(&[5.0, 6.0], &[7.0, 8.0]).unwrap();
    ring.push(&[9.0, 0.0], &[1.0, 1.0]).unwrap();
    assert!(ring.is_full());
    assert_eq!(ring.len(), 2);
    assert_eq!(ring.next_pos(), 3);
    assert_eq!(ring.first_pos(), 1);
    assert_eq!(ring.key(0, 0), &[5.0, 6.0]);
    assert_eq!(ring.value(1, 0), &[1.0, 1.0]);
    ring.clear();
    assert!(ring.is_empty());
    assert!(KvRing::new(0, 1, 2).is_err());

    let q = Tensor::from_vec(vec![1.0, 0.0], [1, 1, 2]).unwrap();
    let k = Tensor::from_vec(vec![1.0, 0.0], [1, 1, 2]).unwrap();
    let v = Tensor::from_vec(vec![4.0, 5.0], [1, 1, 2]).unwrap();
    let y = attention_reference(&q, &k, &v, 0, 10_000.0, None).unwrap();
    assert_eq!(y.to_vec_f32(), vec![4.0, 5.0]);

    let mut attn = StreamingAttention::new(1, 1, 2, 4, 10_000.0, 0).unwrap();
    let mut out = [0.0; 2];
    attn.step(&[1.0, 0.0], &[1.0, 0.0], &[4.0, 5.0], &mut out).unwrap();
    assert_eq!(out, [4.0, 5.0]);
    assert_eq!(attn.kv().len(), 1);
    attn.reset();
    assert!(attn.kv().is_empty());
    assert_eq!(attn.position(), 0);
    assert!(StreamingAttention::new(3, 2, 2, 4, 10_000.0, 0).is_err());
}

#[test]
fn simd_report_is_a_cpu_level_and_sha256_is_in_memory() {
    let level = selected();
    assert!(level.compiled() && level.is_implemented() && cpu_supports(level));
    assert!(matches!(level, SimdLevel::Scalar | SimdLevel::Neon | SimdLevel::Avx2 | SimdLevel::Avx512));
    assert_eq!(select_with_cap(Some("scalar")), SimdLevel::Scalar);
    let line = report();
    assert!(line.contains(&format!("selected={}", level.name())));
    assert!(line.contains("cpu="));

    let empty = sha256(b"");
    assert_eq!(to_hex(&empty), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    let abc = sha256(b"abc");
    assert_eq!(to_hex(&abc), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    let manifest = format!("{}  notes.txt\n# comment\n", to_hex(&abc));
    let lines = parse_sha256sums(&manifest).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].path, "notes.txt");
    assert_eq!(lines[0].sha256, abc);
    assert!(parse_sha256sums(&format!("{}  ../secret", to_hex(&abc))).is_err());
    assert!(parse_sha256sums(&format!("{}  /abs", to_hex(&abc))).is_err());
}
