// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! SIMD feature gates (compile-time cargo features + runtime CPU detection).
//!
//! Two independent switches must BOTH be on before a SIMD kernel is selected:
//!
//! | Level   | Cargo feature  | Runtime check (target)                         | Kernel status |
//! |---------|----------------|------------------------------------------------|---------------|
//! | Scalar  | (always)       | —                                              | reference, source of truth |
//! | Avx2    | `simd-avx2`    | `avx2` + `fma` (x86/x86_64)                    | real (intrinsics) |
//! | Avx512  | `simd-avx512`  | `avx512f` (+`avx512bw` for int8) (x86_64 only) | real (stable `asm!`) |
//! | Neon    | `simd-neon`    | `neon` (aarch64)                               | real (intrinsics; untested on x86 CI host) |
//!
//! All SIMD features are opt-in (not in `default`), so a default build stays on the scalar
//! oracle. [`SimdLevel::is_implemented`] is per-target-arch. Scalar-vs-SIMD tolerance /
//! bit-exact tests live in `simd.rs`, `gemv.rs`, `gemv_i8.rs` (G-T2).
//! Runtime override via env `GALION_SIMD` (read once, cached): `scalar` forces scalar;
//! `avx2` / `neon` cap the level (e.g. avoid AVX-512 frequency throttling on some VPS CPUs).

/// Vector ISA level for f32 kernels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SimdLevel {
    Scalar,
    Neon,
    Avx2,
    Avx512,
}

impl SimdLevel {
    /// Cargo feature compiled in for this level.
    pub const fn compiled(self) -> bool {
        match self {
            SimdLevel::Scalar => true,
            SimdLevel::Avx2 => cfg!(feature = "simd-avx2"),
            SimdLevel::Avx512 => cfg!(feature = "simd-avx512"),
            SimdLevel::Neon => cfg!(feature = "simd-neon"),
        }
    }
    /// Real (non-stub) kernel exists for the current target arch.
    pub const fn is_implemented(self) -> bool {
        match self {
            SimdLevel::Scalar => true,
            SimdLevel::Avx2 => cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            SimdLevel::Avx512 => cfg!(target_arch = "x86_64"),
            SimdLevel::Neon => cfg!(target_arch = "aarch64"),
        }
    }
    /// f32 lanes per vector register (for future blocking decisions).
    pub const fn f32_lanes(self) -> usize {
        match self {
            SimdLevel::Scalar => 1,
            SimdLevel::Neon => 4,
            SimdLevel::Avx2 => 8,
            SimdLevel::Avx512 => 16,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            SimdLevel::Scalar => "scalar",
            SimdLevel::Neon => "neon",
            SimdLevel::Avx2 => "avx2",
            SimdLevel::Avx512 => "avx512",
        }
    }
}

/// Runtime CPU support for a level (independent of cargo features).
pub fn cpu_supports(level: SimdLevel) -> bool {
    match level {
        SimdLevel::Scalar => true,
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        SimdLevel::Avx2 => std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma"),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        SimdLevel::Avx512 => std::arch::is_x86_feature_detected!("avx512f"),
        #[cfg(target_arch = "aarch64")]
        SimdLevel::Neon => std::arch::is_aarch64_feature_detected!("neon"),
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// Highest level the CPU supports (ignores cargo features and stub status). Diagnostic only.
pub fn detected() -> SimdLevel {
    [SimdLevel::Avx512, SimdLevel::Avx2, SimdLevel::Neon]
        .into_iter()
        .find(|&l| cpu_supports(l))
        .unwrap_or(SimdLevel::Scalar)
}

/// Level kernels will actually dispatch to: compiled AND cpu-supported AND implemented,
/// capped by `GALION_SIMD` (see module docs). Computed once and cached (no per-call env read /
/// allocation — safe to call in the frame loop).
pub fn selected() -> SimdLevel {
    static SEL: std::sync::OnceLock<SimdLevel> = std::sync::OnceLock::new();
    *SEL.get_or_init(|| select_with_cap(std::env::var("GALION_SIMD").ok().as_deref()))
}

/// Uncached selection with an explicit cap string (`None` = no cap). Testable without env.
pub fn select_with_cap(cap: Option<&str>) -> SimdLevel {
    let cap = match cap.map(|c| c.trim().to_ascii_lowercase()) {
        Some(c) if c == "scalar" || c == "off" || c == "0" => return SimdLevel::Scalar,
        Some(c) if c == "neon" => SimdLevel::Neon,
        Some(c) if c == "avx2" => SimdLevel::Avx2,
        _ => SimdLevel::Avx512,
    };
    [SimdLevel::Avx512, SimdLevel::Avx2, SimdLevel::Neon]
        .into_iter()
        .filter(|&l| l <= cap)
        .find(|&l| l.compiled() && cpu_supports(l) && l.is_implemented())
        .unwrap_or(SimdLevel::Scalar)
}

/// One-line diagnostic, e.g. for probes/gates logs.
pub fn report() -> String {
    let lv = [SimdLevel::Avx512, SimdLevel::Avx2, SimdLevel::Neon];
    let parts: Vec<String> = lv
        .iter()
        .map(|&l| format!("{}(cpu={},feat={},impl={})", l.name(), cpu_supports(l), l.compiled(), l.is_implemented()))
        .collect();
    format!("detected={} selected={} {}", detected().name(), selected().name(), parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_respects_gates() {
        assert!(cpu_supports(SimdLevel::Scalar));
        let s = selected();
        assert!(s.is_implemented() && s.compiled() && cpu_supports(s));
        if !cfg!(any(feature = "simd-avx2", feature = "simd-avx512", feature = "simd-neon")) {
            assert_eq!(s, SimdLevel::Scalar, "default build must stay on the scalar oracle");
        }
        assert_eq!(select_with_cap(Some("scalar")), SimdLevel::Scalar);
        assert!(select_with_cap(Some("avx2")) <= SimdLevel::Avx2);
        assert!(select_with_cap(Some("neon")) <= SimdLevel::Neon);
        assert!(SimdLevel::Avx2.f32_lanes() == 8 && SimdLevel::Avx512.f32_lanes() == 16);
        assert!(report().contains(&format!("selected={}", s.name())));
    }
}
