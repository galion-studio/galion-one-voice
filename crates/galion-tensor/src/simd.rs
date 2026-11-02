// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Micro-kernels used by GEMV / matmul. **Scalar is the oracle**; SIMD kernels must match it
//! within the stated tolerance (f32 dot) or bit-exactly (int8 dot, f32 axpy).
//!
//! | Kernel        | Scalar | AVX2 (+FMA)            | AVX-512                         | NEON (aarch64)          | vs scalar |
//! |---------------|--------|------------------------|---------------------------------|-------------------------|-----------|
//! | `dot` f32     | seq.   | 4×8 `fmadd` accs       | 4×16 `vfmadd231ps` (inline asm) | 4×4 `vfmaq_f32`         | tolerance: reassociated + fused |
//! | `dot_i8` →i32 | seq.   | `cvtepi8_epi16`+`madd` | `vpmovsxbw`+`vpmaddwd` (asm, BW) | `vmull_s8`+`vpadalq_s16` | **bit-exact** (integer) |
//! | `axpy` f32    | seq.   | `mul`+`add` (no FMA)   | `vmulps`+`vaddps` (asm)         | `vmulq`+`vaddq`         | **bit-exact** (same per-lane op order) |
//!
//! AVX-512 uses stable `asm!` because the `_mm512_*` intrinsics / `avx512f` target feature are
//! not stable on the pinned toolchain (rustc 1.85). The asm only touches xmm/ymm/zmm 0–5
//! (declared clobbered) and ends with `vzeroupper`.
//!
//! Safety model: the `*_unchecked` adapters are private and only handed out by
//! [`dot_for`] / [`dot_i8_for`] / [`axpy_for`] after the runtime CPU check succeeded, so every
//! public entry point is safe to call on any CPU (unsupported levels resolve to scalar).
//!
//! f32 dot tolerance (G-T2): for length `k`, `|simd - scalar| ≤ 2·k·ε·Σ|aᵢbᵢ|` (ε = 2⁻²⁴),
//! the standard recursive-summation bound applied to both sides. See [`dot_tolerance`].

use crate::features::{cpu_supports, SimdLevel};

/// Function-pointer type for an f32 dot kernel.
pub type DotFn = fn(&[f32], &[f32]) -> f32;
/// Function-pointer type for an int8 dot kernel (exact i32 accumulation).
pub type DotI8Fn = fn(&[i8], &[i8]) -> i32;
/// Function-pointer type for `y[j] += alpha * x[j]` (non-fused, per-lane identical to scalar).
pub type AxpyFn = fn(f32, &[f32], &mut [f32]);

/// Largest `k` for which an i8·i8 dot cannot overflow i32 (`k · 128² < 2³¹`).
pub const DOT_I8_MAX_K: usize = (1usize << 31) / (128 * 128) - 1;

// ----------------------------------------------------------------------------------------------
// Scalar oracles
// ----------------------------------------------------------------------------------------------

/// Reference f32 dot: strictly sequential accumulation `acc += a[i]*b[i]` (no FMA, no reassociation),
/// so it is bit-identical to a naive `for` loop.
#[inline]
pub fn dot_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0f32;
    for (x, y) in a.iter().zip(b) {
        acc += x * y;
    }
    acc
}

/// Reference int8 dot with i32 accumulation. Exact for `len ≤ DOT_I8_MAX_K`.
#[inline]
pub fn dot_i8_scalar(a: &[i8], b: &[i8]) -> i32 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0i32;
    for (&x, &y) in a.iter().zip(b) {
        acc = acc.wrapping_add(x as i32 * y as i32);
    }
    acc
}

/// Reference axpy: `y[j] += alpha * x[j]` (multiply, round, add, round — never fused).
#[inline]
pub fn axpy_scalar(alpha: f32, x: &[f32], y: &mut [f32]) {
    debug_assert_eq!(x.len(), y.len());
    for (o, &v) in y.iter_mut().zip(x) {
        *o += alpha * v;
    }
}

/// Error bound used by tests for a reassociated/fused f32 dot vs the scalar oracle.
pub fn dot_tolerance(a: &[f32], b: &[f32]) -> f32 {
    let abs: f64 = a.iter().zip(b).map(|(x, y)| (*x as f64 * *y as f64).abs()).sum();
    (2.0 * a.len().max(1) as f64 * f32::EPSILON as f64 * abs) as f32 + f32::MIN_POSITIVE
}

// ----------------------------------------------------------------------------------------------
// AVX2 + FMA (std::arch intrinsics, stable)
// ----------------------------------------------------------------------------------------------

#[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
mod avx2 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn hsum256(v: __m256) -> f32 {
        let s = _mm_add_ps(_mm256_castps256_ps128(v), _mm256_extractf128_ps(v, 1));
        let s = _mm_add_ps(s, _mm_movehl_ps(s, s));
        let s = _mm_add_ss(s, _mm_movehdup_ps(s));
        _mm_cvtss_f32(s)
    }

    /// # Safety
    /// CPU must support AVX2 and FMA.
    #[target_feature(enable = "avx2,fma")]
    pub unsafe fn dot(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        let (pa, pb) = (a.as_ptr(), b.as_ptr());
        let (mut c0, mut c1, mut c2, mut c3) = (_mm256_setzero_ps(), _mm256_setzero_ps(), _mm256_setzero_ps(), _mm256_setzero_ps());
        let mut i = 0;
        while i + 32 <= n {
            c0 = _mm256_fmadd_ps(_mm256_loadu_ps(pa.add(i)), _mm256_loadu_ps(pb.add(i)), c0);
            c1 = _mm256_fmadd_ps(_mm256_loadu_ps(pa.add(i + 8)), _mm256_loadu_ps(pb.add(i + 8)), c1);
            c2 = _mm256_fmadd_ps(_mm256_loadu_ps(pa.add(i + 16)), _mm256_loadu_ps(pb.add(i + 16)), c2);
            c3 = _mm256_fmadd_ps(_mm256_loadu_ps(pa.add(i + 24)), _mm256_loadu_ps(pb.add(i + 24)), c3);
            i += 32;
        }
        while i + 8 <= n {
            c0 = _mm256_fmadd_ps(_mm256_loadu_ps(pa.add(i)), _mm256_loadu_ps(pb.add(i)), c0);
            i += 8;
        }
        let mut s = hsum256(_mm256_add_ps(_mm256_add_ps(c0, c1), _mm256_add_ps(c2, c3)));
        while i < n {
            s += *pa.add(i) * *pb.add(i);
            i += 1;
        }
        s
    }

    /// # Safety
    /// CPU must support AVX2.
    #[target_feature(enable = "avx2")]
    pub unsafe fn dot_i8(a: &[i8], b: &[i8]) -> i32 {
        let n = a.len().min(b.len());
        let (pa, pb) = (a.as_ptr(), b.as_ptr());
        let (mut c0, mut c1) = (_mm256_setzero_si256(), _mm256_setzero_si256());
        let mut i = 0;
        while i + 32 <= n {
            let a0 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pa.add(i) as *const __m128i));
            let b0 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pb.add(i) as *const __m128i));
            let a1 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pa.add(i + 16) as *const __m128i));
            let b1 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pb.add(i + 16) as *const __m128i));
            c0 = _mm256_add_epi32(c0, _mm256_madd_epi16(a0, b0));
            c1 = _mm256_add_epi32(c1, _mm256_madd_epi16(a1, b1));
            i += 32;
        }
        while i + 16 <= n {
            let a0 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pa.add(i) as *const __m128i));
            let b0 = _mm256_cvtepi8_epi16(_mm_loadu_si128(pb.add(i) as *const __m128i));
            c0 = _mm256_add_epi32(c0, _mm256_madd_epi16(a0, b0));
            i += 16;
        }
        let c = _mm256_add_epi32(c0, c1);
        let s = _mm_add_epi32(_mm256_castsi256_si128(c), _mm256_extracti128_si256(c, 1));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32(s, 0x4E));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32(s, 0xB1));
        let mut acc = _mm_cvtsi128_si32(s);
        while i < n {
            acc = acc.wrapping_add(*pa.add(i) as i32 * *pb.add(i) as i32);
            i += 1;
        }
        acc
    }

    /// # Safety
    /// CPU must support AVX2. Non-fused mul then add => bit-exact vs `axpy_scalar`.
    #[target_feature(enable = "avx2")]
    pub unsafe fn axpy(alpha: f32, x: &[f32], y: &mut [f32]) {
        let n = x.len().min(y.len());
        let (px, py) = (x.as_ptr(), y.as_mut_ptr());
        let va = _mm256_set1_ps(alpha);
        let mut i = 0;
        while i + 8 <= n {
            let p = _mm256_mul_ps(va, _mm256_loadu_ps(px.add(i)));
            _mm256_storeu_ps(py.add(i), _mm256_add_ps(_mm256_loadu_ps(py.add(i)), p));
            i += 8;
        }
        while i < n {
            *py.add(i) += alpha * *px.add(i);
            i += 1;
        }
    }
}

// ----------------------------------------------------------------------------------------------
// AVX-512 (stable inline asm; x86_64 only)
// ----------------------------------------------------------------------------------------------

#[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
mod avx512 {
    use std::arch::asm;

    /// # Safety
    /// CPU must support AVX-512F.
    pub unsafe fn dot(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        let body = n & !15;
        let mut s: f32;
        asm!(
            "vxorps xmm0, xmm0, xmm0",
            "vxorps xmm1, xmm1, xmm1",
            "vxorps xmm2, xmm2, xmm2",
            "vxorps xmm3, xmm3, xmm3",
            "mov {c}, {n}",
            "shr {c}, 6",
            "jz 3f",
            "2:",
            "vmovups zmm4, [{a}]",
            "vfmadd231ps zmm0, zmm4, [{b}]",
            "vmovups zmm5, [{a} + 64]",
            "vfmadd231ps zmm1, zmm5, [{b} + 64]",
            "vmovups zmm4, [{a} + 128]",
            "vfmadd231ps zmm2, zmm4, [{b} + 128]",
            "vmovups zmm5, [{a} + 192]",
            "vfmadd231ps zmm3, zmm5, [{b} + 192]",
            "add {a}, 256",
            "add {b}, 256",
            "dec {c}",
            "jnz 2b",
            "3:",
            "mov {c}, {n}",
            "and {c}, 63",
            "shr {c}, 4",
            "jz 5f",
            "4:",
            "vmovups zmm4, [{a}]",
            "vfmadd231ps zmm0, zmm4, [{b}]",
            "add {a}, 64",
            "add {b}, 64",
            "dec {c}",
            "jnz 4b",
            "5:",
            "vaddps zmm0, zmm0, zmm1",
            "vaddps zmm2, zmm2, zmm3",
            "vaddps zmm0, zmm0, zmm2",
            "vextractf32x8 ymm1, zmm0, 1",
            "vaddps ymm0, ymm0, ymm1",
            "vextractf128 xmm1, ymm0, 1",
            "vaddps xmm0, xmm0, xmm1",
            "vmovhlps xmm1, xmm1, xmm0",
            "vaddps xmm0, xmm0, xmm1",
            "vmovshdup xmm1, xmm0",
            "vaddss xmm0, xmm0, xmm1",
            "vzeroupper",
            a = inout(reg) a.as_ptr() => _,
            b = inout(reg) b.as_ptr() => _,
            n = in(reg) body,
            c = out(reg) _,
            out("xmm0") s, out("xmm1") _, out("xmm2") _, out("xmm3") _, out("xmm4") _, out("xmm5") _,
            options(nostack, readonly),
        );
        for i in body..n {
            s += a[i] * b[i];
        }
        s
    }

    /// # Safety
    /// CPU must support AVX-512F + AVX-512BW.
    pub unsafe fn dot_i8(a: &[i8], b: &[i8]) -> i32 {
        let n = a.len().min(b.len());
        let body = n & !31;
        let mut acc: i32;
        asm!(
            "vpxord xmm0, xmm0, xmm0",
            "vpxord xmm3, xmm3, xmm3",
            "mov {c}, {n}",
            "shr {c}, 6",
            "jz 3f",
            "2:",
            "vpmovsxbw zmm1, [{a}]",
            "vpmovsxbw zmm2, [{b}]",
            "vpmaddwd zmm1, zmm1, zmm2",
            "vpaddd zmm0, zmm0, zmm1",
            "vpmovsxbw zmm4, [{a} + 32]",
            "vpmovsxbw zmm5, [{b} + 32]",
            "vpmaddwd zmm4, zmm4, zmm5",
            "vpaddd zmm3, zmm3, zmm4",
            "add {a}, 64",
            "add {b}, 64",
            "dec {c}",
            "jnz 2b",
            "3:",
            "test {n}, 32",
            "jz 4f",
            "vpmovsxbw zmm1, [{a}]",
            "vpmovsxbw zmm2, [{b}]",
            "vpmaddwd zmm1, zmm1, zmm2",
            "vpaddd zmm0, zmm0, zmm1",
            "4:",
            "vpaddd zmm0, zmm0, zmm3",
            "vextracti32x8 ymm1, zmm0, 1",
            "vpaddd ymm0, ymm0, ymm1",
            "vextracti128 xmm1, ymm0, 1",
            "vpaddd xmm0, xmm0, xmm1",
            "vpshufd xmm1, xmm0, 0x4E",
            "vpaddd xmm0, xmm0, xmm1",
            "vpshufd xmm1, xmm0, 0xB1",
            "vpaddd xmm0, xmm0, xmm1",
            "vmovd {r:e}, xmm0",
            "vzeroupper",
            a = inout(reg) a.as_ptr() => _,
            b = inout(reg) b.as_ptr() => _,
            n = in(reg) body,
            c = out(reg) _,
            r = out(reg) acc,
            out("xmm0") _, out("xmm1") _, out("xmm2") _, out("xmm3") _, out("xmm4") _, out("xmm5") _,
            options(nostack, readonly),
        );
        for i in body..n {
            acc = acc.wrapping_add(a[i] as i32 * b[i] as i32);
        }
        acc
    }

    /// # Safety
    /// CPU must support AVX-512F. Non-fused mul then add => bit-exact vs `axpy_scalar`.
    pub unsafe fn axpy(alpha: f32, x: &[f32], y: &mut [f32]) {
        let n = x.len().min(y.len());
        let body = n & !15;
        asm!(
            "vbroadcastss zmm0, {al:x}",
            "mov {c}, {n}",
            "shr {c}, 4",
            "jz 3f",
            "2:",
            "vmulps zmm1, zmm0, [{x}]",
            "vaddps zmm1, zmm1, [{y}]",
            "vmovups [{y}], zmm1",
            "add {x}, 64",
            "add {y}, 64",
            "dec {c}",
            "jnz 2b",
            "3:",
            "vzeroupper",
            al = in(xmm_reg) alpha,
            x = inout(reg) x.as_ptr() => _,
            y = inout(reg) y.as_mut_ptr() => _,
            n = in(reg) body,
            c = out(reg) _,
            out("xmm0") _, out("xmm1") _,
            options(nostack),
        );
        for i in body..n {
            y[i] += alpha * x[i];
        }
    }
}

// ----------------------------------------------------------------------------------------------
// NEON (aarch64, std::arch intrinsics, stable)
// ----------------------------------------------------------------------------------------------

#[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
mod neon {
    use std::arch::aarch64::*;

    /// # Safety
    /// CPU must support NEON (baseline on aarch64).
    #[target_feature(enable = "neon")]
    pub unsafe fn dot(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        let (pa, pb) = (a.as_ptr(), b.as_ptr());
        let (mut c0, mut c1, mut c2, mut c3) = (vdupq_n_f32(0.0), vdupq_n_f32(0.0), vdupq_n_f32(0.0), vdupq_n_f32(0.0));
        let mut i = 0;
        while i + 16 <= n {
            c0 = vfmaq_f32(c0, vld1q_f32(pa.add(i)), vld1q_f32(pb.add(i)));
            c1 = vfmaq_f32(c1, vld1q_f32(pa.add(i + 4)), vld1q_f32(pb.add(i + 4)));
            c2 = vfmaq_f32(c2, vld1q_f32(pa.add(i + 8)), vld1q_f32(pb.add(i + 8)));
            c3 = vfmaq_f32(c3, vld1q_f32(pa.add(i + 12)), vld1q_f32(pb.add(i + 12)));
            i += 16;
        }
        while i + 4 <= n {
            c0 = vfmaq_f32(c0, vld1q_f32(pa.add(i)), vld1q_f32(pb.add(i)));
            i += 4;
        }
        let mut s = vaddvq_f32(vaddq_f32(vaddq_f32(c0, c1), vaddq_f32(c2, c3)));
        while i < n {
            s += *pa.add(i) * *pb.add(i);
            i += 1;
        }
        s
    }

    /// # Safety
    /// CPU must support NEON.
    #[target_feature(enable = "neon")]
    pub unsafe fn dot_i8(a: &[i8], b: &[i8]) -> i32 {
        let n = a.len().min(b.len());
        let (pa, pb) = (a.as_ptr(), b.as_ptr());
        let (mut c0, mut c1) = (vdupq_n_s32(0), vdupq_n_s32(0));
        let mut i = 0;
        while i + 16 <= n {
            let va = vld1q_s8(pa.add(i));
            let vb = vld1q_s8(pb.add(i));
            // |i8·i8| ≤ 16384 fits i16; pairwise-accumulate into i32 lanes.
            c0 = vpadalq_s16(c0, vmull_s8(vget_low_s8(va), vget_low_s8(vb)));
            c1 = vpadalq_s16(c1, vmull_high_s8(va, vb));
            i += 16;
        }
        let mut acc = vaddvq_s32(vaddq_s32(c0, c1));
        while i < n {
            acc = acc.wrapping_add(*pa.add(i) as i32 * *pb.add(i) as i32);
            i += 1;
        }
        acc
    }

    /// # Safety
    /// CPU must support NEON. Non-fused `vmulq`+`vaddq` => bit-exact vs `axpy_scalar`.
    #[target_feature(enable = "neon")]
    pub unsafe fn axpy(alpha: f32, x: &[f32], y: &mut [f32]) {
        let n = x.len().min(y.len());
        let (px, py) = (x.as_ptr(), y.as_mut_ptr());
        let va = vdupq_n_f32(alpha);
        let mut i = 0;
        while i + 4 <= n {
            let p = vmulq_f32(va, vld1q_f32(px.add(i)));
            vst1q_f32(py.add(i), vaddq_f32(vld1q_f32(py.add(i)), p));
            i += 4;
        }
        while i < n {
            *py.add(i) += alpha * *px.add(i);
            i += 1;
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Private unchecked adapters (only reachable after a runtime CPU check in *_for()).
// ----------------------------------------------------------------------------------------------

#[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
fn dot_avx2_unchecked(a: &[f32], b: &[f32]) -> f32 { unsafe { avx2::dot(a, b) } }
#[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
fn dot_i8_avx2_unchecked(a: &[i8], b: &[i8]) -> i32 { unsafe { avx2::dot_i8(a, b) } }
#[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
fn axpy_avx2_unchecked(al: f32, x: &[f32], y: &mut [f32]) { unsafe { avx2::axpy(al, x, y) } }

#[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
fn dot_avx512_unchecked(a: &[f32], b: &[f32]) -> f32 { unsafe { avx512::dot(a, b) } }
#[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
fn dot_i8_avx512_unchecked(a: &[i8], b: &[i8]) -> i32 { unsafe { avx512::dot_i8(a, b) } }
#[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
fn axpy_avx512_unchecked(al: f32, x: &[f32], y: &mut [f32]) { unsafe { avx512::axpy(al, x, y) } }

#[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
fn dot_neon_unchecked(a: &[f32], b: &[f32]) -> f32 { unsafe { neon::dot(a, b) } }
#[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
fn dot_i8_neon_unchecked(a: &[i8], b: &[i8]) -> i32 { unsafe { neon::dot_i8(a, b) } }
#[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
fn axpy_neon_unchecked(al: f32, x: &[f32], y: &mut [f32]) { unsafe { neon::axpy(al, x, y) } }

#[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
fn cpu_avx512bw() -> bool { std::arch::is_x86_feature_detected!("avx512bw") }

// ----------------------------------------------------------------------------------------------
// Public safe entry points (existing API) — runtime-checked, scalar fallback.
// ----------------------------------------------------------------------------------------------

/// AVX2+FMA dot (4×8-lane accumulators). Falls back to scalar if the CPU/arch lacks AVX2+FMA.
#[cfg(feature = "simd-avx2")]
#[inline]
pub fn dot_avx2(a: &[f32], b: &[f32]) -> f32 { dot_for(SimdLevel::Avx2)(a, b) }

/// AVX-512F dot (4×16-lane accumulators, asm). Falls back to scalar if unsupported.
#[cfg(feature = "simd-avx512")]
#[inline]
pub fn dot_avx512(a: &[f32], b: &[f32]) -> f32 { dot_for(SimdLevel::Avx512)(a, b) }

/// NEON dot (4×4-lane `vfmaq_f32`). Falls back to scalar off aarch64.
#[cfg(feature = "simd-neon")]
#[inline]
pub fn dot_neon(a: &[f32], b: &[f32]) -> f32 { dot_for(SimdLevel::Neon)(a, b) }

/// Resolve an f32 dot kernel for `level`. Levels not compiled in, not implemented for this
/// arch, or not supported by the running CPU resolve to [`dot_scalar`].
pub fn dot_for(level: SimdLevel) -> DotFn {
    match level {
        #[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
        SimdLevel::Avx2 if cpu_supports(SimdLevel::Avx2) => dot_avx2_unchecked,
        #[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
        SimdLevel::Avx512 if cpu_supports(SimdLevel::Avx512) => dot_avx512_unchecked,
        #[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
        SimdLevel::Neon if cpu_supports(SimdLevel::Neon) => dot_neon_unchecked,
        _ => {
            let _ = cpu_supports;
            dot_scalar
        }
    }
}

/// Resolve an int8 dot kernel for `level` (all are bit-exact vs [`dot_i8_scalar`]).
/// AVX-512 additionally needs AVX-512BW; otherwise it degrades to AVX2 (if compiled) or scalar.
pub fn dot_i8_for(level: SimdLevel) -> DotI8Fn {
    match level {
        #[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
        SimdLevel::Avx512 if cpu_supports(SimdLevel::Avx512) && cpu_avx512bw() => dot_i8_avx512_unchecked,
        #[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
        SimdLevel::Avx2 | SimdLevel::Avx512 if cpu_supports(SimdLevel::Avx2) => dot_i8_avx2_unchecked,
        #[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
        SimdLevel::Neon if cpu_supports(SimdLevel::Neon) => dot_i8_neon_unchecked,
        _ => dot_i8_scalar,
    }
}

/// Resolve an axpy kernel for `level` (all are bit-exact vs [`axpy_scalar`]).
pub fn axpy_for(level: SimdLevel) -> AxpyFn {
    match level {
        #[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
        SimdLevel::Avx2 if cpu_supports(SimdLevel::Avx2) => axpy_avx2_unchecked,
        #[cfg(all(feature = "simd-avx512", target_arch = "x86_64"))]
        SimdLevel::Avx512 if cpu_supports(SimdLevel::Avx512) => axpy_avx512_unchecked,
        #[cfg(all(feature = "simd-neon", target_arch = "aarch64"))]
        SimdLevel::Neon if cpu_supports(SimdLevel::Neon) => axpy_neon_unchecked,
        _ => axpy_scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: [SimdLevel; 4] = [SimdLevel::Scalar, SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon];

    fn vf(n: usize, s: usize) -> Vec<f32> {
        (0..n).map(|i| (((i * s + 5) % 37) as f32 - 18.0) * 0.0917 + (i as f32 * 1e-3).sin()).collect()
    }
    fn vi8(n: usize, s: usize) -> Vec<i8> {
        (0..n).map(|i| (((i * s + 11) % 256) as i32 - 128) as i8).collect()
    }

    #[test]
    fn dot_scalar_matches_naive() {
        let a: Vec<f32> = (0..103).map(|i| ((i * 7) % 17) as f32 * 0.25 - 2.0).collect();
        let b: Vec<f32> = (0..103).map(|i| ((i * 3) % 13) as f32 * 0.5 - 3.0).collect();
        let mut r = 0f32;
        for i in 0..a.len() { r += a[i] * b[i]; }
        assert_eq!(dot_scalar(&a, &b).to_bits(), r.to_bits());
        assert_eq!(dot_scalar(&[], &[]), 0.0);
        // Exactly-representable inputs (quarters × halves, small sums) => every level is exact.
        for l in LEVELS {
            assert_eq!(dot_for(l)(&a, &b), r, "{l:?}");
        }
    }

    #[test]
    fn dot_all_levels_within_tolerance_of_scalar_oracle() {
        for n in [0usize, 1, 3, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 100, 127, 128, 129, 255, 1000, 4096, 4099] {
            let (a, b) = (vf(n, 7), vf(n, 13));
            let r = dot_scalar(&a, &b);
            let tol = dot_tolerance(&a, &b);
            for l in LEVELS {
                let y = dot_for(l)(&a, &b);
                assert!((y - r).abs() <= tol, "n={n} {l:?}: {y} vs {r} (tol {tol})");
            }
        }
    }

    #[test]
    fn dot_handles_unaligned_subslices() {
        let (a, b) = (vf(300, 3), vf(300, 5));
        for off in 0..9 {
            let (sa, sb) = (&a[off..off + 200], &b[off + 1..off + 201]);
            let r = dot_scalar(sa, sb);
            for l in LEVELS {
                assert!((dot_for(l)(sa, sb) - r).abs() <= dot_tolerance(sa, sb), "off={off} {l:?}");
            }
        }
    }

    #[test]
    fn dot_i8_all_levels_bit_exact() {
        for n in [0usize, 1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 96, 127, 128, 1000, 4097] {
            let (a, b) = (vi8(n, 7), vi8(n, 13));
            let r: i64 = a.iter().zip(&b).map(|(&x, &y)| x as i64 * y as i64).sum();
            assert_eq!(dot_i8_scalar(&a, &b) as i64, r, "n={n}");
            for l in LEVELS {
                assert_eq!(dot_i8_for(l)(&a, &b) as i64, r, "n={n} {l:?}");
            }
        }
        // Worst-case magnitude: all -128.
        let a = vec![-128i8; 1024];
        for l in LEVELS { assert_eq!(dot_i8_for(l)(&a, &a), 1024 * 16384, "{l:?}"); }
        for off in 0..5 {
            let (a, b) = (vi8(200, 3), vi8(200, 5));
            let (sa, sb) = (&a[off..off + 150], &b[off + 2..off + 152]);
            for l in LEVELS { assert_eq!(dot_i8_for(l)(sa, sb), dot_i8_scalar(sa, sb), "{l:?}"); }
        }
    }

    #[test]
    fn axpy_all_levels_bit_exact() {
        for n in [0usize, 1, 7, 8, 9, 15, 16, 17, 33, 100, 1027] {
            let x = vf(n, 11);
            let y0 = vf(n, 17);
            let mut r = y0.clone();
            axpy_scalar(-0.731, &x, &mut r);
            for l in LEVELS {
                let mut y = y0.clone();
                axpy_for(l)(-0.731, &x, &mut y);
                let (yb, rb): (Vec<u32>, Vec<u32>) = (y.iter().map(|v| v.to_bits()).collect(), r.iter().map(|v| v.to_bits()).collect());
                assert_eq!(yb, rb, "n={n} {l:?}");
            }
        }
    }

    #[test]
    fn dot_i8_max_k_is_safe() {
        assert!((DOT_I8_MAX_K as i64) * 16384 < i32::MAX as i64);
    }
}
