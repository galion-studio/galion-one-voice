// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! SIMD feature gates (compile-time cargo features + runtime CPU detection).
//!
//! A kernel runs only when the cargo feature is compiled, the target implements it, and the
//! running CPU has the flags that kernel actually uses. Those three switches are per kernel:
//!
//! | Kernel   | AVX2              | AVX-512                         | NEON          |
//! |----------|-------------------|---------------------------------|---------------|
//! | f32 `dot`  | `avx2` + `fma`  | `avx512f`                       | `neon`        |
//! | `dot_i8`   | `avx2` + `fma`  | `avx512f` **and** `avx512bw`    | `neon`        |
//! | f32 `axpy` | `avx2` + `fma`  | `avx512f`                       | `neon`        |
//!
//! [`selected`] is the highest level the f32 kernels can run, capped by `GALION_SIMD`.
//! [`kernel_level`] is the level a specific kernel runs at for a requested level. f32 `dot` and
//! `axpy` do not step down: an unavailable request is scalar. `dot_i8` does: an AVX-512 request
//! without BW uses AVX2 when that kernel is compiled and the CPU has it. [`report`] and
//! [`startup_line`] name the level each kernel really runs, so `selected=avx512` is not read as
//! "the int8 kernel is AVX-512".
//!
//! All SIMD features are opt-in (not in `default`), so a default build stays on the scalar
//! oracle. Scalar-vs-SIMD tolerance / bit-exact tests live in `simd.rs`.
//!
//! `GALION_SIMD` is read once and cached: `scalar` / `off` / `0` force scalar; `avx2` and `neon`
//! cap the level; anything else, including unset, caps at AVX-512, which still has to pass the
//! three switches. No `target-cpu=native`.

use std::sync::OnceLock;

/// Vector ISA level. Higher means a wider register file, not "every kernel can use it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SimdLevel {
    Scalar,
    Neon,
    Avx2,
    Avx512,
}

impl SimdLevel {
    /// Cargo feature compiled in for this level on this build.
    pub const fn compiled(self) -> bool {
        match self {
            SimdLevel::Scalar => true,
            SimdLevel::Avx2 => cfg!(feature = "simd-avx2"),
            SimdLevel::Avx512 => cfg!(feature = "simd-avx512"),
            SimdLevel::Neon => cfg!(feature = "simd-neon"),
        }
    }
    /// A real kernel exists for the current target arch (still needs the cargo feature and CPU flags).
    pub const fn is_implemented(self) -> bool {
        match self {
            SimdLevel::Scalar => true,
            SimdLevel::Avx2 => cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            SimdLevel::Avx512 => cfg!(target_arch = "x86_64"),
            SimdLevel::Neon => cfg!(target_arch = "aarch64"),
        }
    }
    /// f32 lanes per vector register.
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

/// Which micro-kernel a level is being chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kernel {
    /// f32 dot. AVX-512 needs `avx512f` only.
    Dot,
    /// int8 dot. AVX-512 needs `avx512f` and `avx512bw`.
    DotI8,
    /// f32 axpy. AVX-512 needs `avx512f` only.
    Axpy,
}

impl Kernel {
    pub const ALL: [Kernel; 3] = [Kernel::Dot, Kernel::DotI8, Kernel::Axpy];

    pub const fn name(self) -> &'static str {
        match self {
            Kernel::Dot => "dot",
            Kernel::DotI8 => "dot_i8",
            Kernel::Axpy => "axpy",
        }
    }
}

/// CPU flags that gate a kernel. Independent of cargo features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuIsa {
    avx2: bool,
    fma: bool,
    avx512f: bool,
    avx512bw: bool,
    neon: bool,
}

impl CpuIsa {
    fn host() -> Self {
        static ISA: OnceLock<CpuIsa> = OnceLock::new();
        *ISA.get_or_init(Self::read)
    }

    fn read() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            Self {
                avx2: std::arch::is_x86_feature_detected!("avx2"),
                fma: std::arch::is_x86_feature_detected!("fma"),
                avx512f: std::arch::is_x86_feature_detected!("avx512f"),
                avx512bw: std::arch::is_x86_feature_detected!("avx512bw"),
                neon: false,
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self {
                avx2: false,
                fma: false,
                avx512f: false,
                avx512bw: false,
                neon: std::arch::is_aarch64_feature_detected!("neon"),
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        {
            Self {
                avx2: false,
                fma: false,
                avx512f: false,
                avx512bw: false,
                neon: false,
            }
        }
    }
}

/// What this binary compiled in, including the arch the kernels exist for.
/// A feature flag on the wrong arch does not make the kernel available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompiledIsa {
    avx2: bool,
    avx512: bool,
    neon: bool,
    x86: bool,
    x86_64: bool,
    aarch64: bool,
}

impl CompiledIsa {
    const fn this_build() -> Self {
        Self {
            avx2: cfg!(feature = "simd-avx2"),
            avx512: cfg!(feature = "simd-avx512"),
            neon: cfg!(feature = "simd-neon"),
            x86: cfg!(target_arch = "x86"),
            x86_64: cfg!(target_arch = "x86_64"),
            aarch64: cfg!(target_arch = "aarch64"),
        }
    }
}

#[derive(Clone, Copy)]
enum Cap {
    /// `GALION_SIMD` unset. Ceiling is AVX-512; the label stays `none`.
    None,
    Scalar,
    Neon,
    Avx2,
    Avx512,
}

fn normalize_cap(cap: Option<&str>) -> Cap {
    match cap.map(|c| c.trim().to_ascii_lowercase()).as_deref() {
        None => Cap::None,
        Some("scalar") | Some("off") | Some("0") => Cap::Scalar,
        Some("neon") => Cap::Neon,
        Some("avx2") => Cap::Avx2,
        Some(_) => Cap::Avx512,
    }
}

fn cap_label(cap: Option<&str>) -> &'static str {
    match normalize_cap(cap) {
        Cap::None => "none",
        Cap::Scalar => "scalar",
        Cap::Neon => "neon",
        Cap::Avx2 => "avx2",
        Cap::Avx512 => "avx512",
    }
}

fn cap_ceiling(cap: Cap) -> SimdLevel {
    match cap {
        Cap::Scalar => SimdLevel::Scalar,
        Cap::Neon => SimdLevel::Neon,
        Cap::Avx2 => SimdLevel::Avx2,
        Cap::None | Cap::Avx512 => SimdLevel::Avx512,
    }
}

fn compiled_on(level: SimdLevel, build: &CompiledIsa) -> bool {
    match level {
        SimdLevel::Scalar => true,
        SimdLevel::Avx2 => build.avx2,
        SimdLevel::Avx512 => build.avx512,
        SimdLevel::Neon => build.neon,
    }
}

fn implemented_on(level: SimdLevel, build: &CompiledIsa) -> bool {
    match level {
        SimdLevel::Scalar => true,
        SimdLevel::Avx2 => build.x86 || build.x86_64,
        SimdLevel::Avx512 => build.x86_64,
        SimdLevel::Neon => build.aarch64,
    }
}

/// Flags the level needs, ignoring cargo features and arch. AVX-512 here is `avx512f` only.
fn cpu_flag(level: SimdLevel, isa: &CpuIsa) -> bool {
    match level {
        SimdLevel::Scalar => true,
        SimdLevel::Avx2 => isa.avx2 && isa.fma,
        SimdLevel::Avx512 => isa.avx512f,
        SimdLevel::Neon => isa.neon,
    }
}

fn ready(level: SimdLevel, kernel: Kernel, isa: &CpuIsa, build: &CompiledIsa) -> bool {
    match level {
        SimdLevel::Scalar => true,
        SimdLevel::Avx2 => {
            compiled_on(level, build) && implemented_on(level, build) && isa.avx2 && isa.fma
        }
        SimdLevel::Avx512 => {
            let f = compiled_on(level, build) && implemented_on(level, build) && isa.avx512f;
            match kernel {
                Kernel::DotI8 => f && isa.avx512bw,
                Kernel::Dot | Kernel::Axpy => f,
            }
        }
        SimdLevel::Neon => compiled_on(level, build) && implemented_on(level, build) && isa.neon,
    }
}

fn detected_on(isa: &CpuIsa) -> SimdLevel {
    if isa.avx512f {
        SimdLevel::Avx512
    } else if isa.avx2 && isa.fma {
        SimdLevel::Avx2
    } else if isa.neon {
        SimdLevel::Neon
    } else {
        SimdLevel::Scalar
    }
}

fn select_with_cap_on(cap: Option<&str>, isa: &CpuIsa, build: &CompiledIsa) -> SimdLevel {
    let cap = normalize_cap(cap);
    if matches!(cap, Cap::Scalar) {
        return SimdLevel::Scalar;
    }
    let ceiling = cap_ceiling(cap);
    [SimdLevel::Avx512, SimdLevel::Avx2, SimdLevel::Neon]
        .into_iter()
        .filter(|&level| level <= ceiling)
        .find(|&level| ready(level, Kernel::Dot, isa, build))
        .unwrap_or(SimdLevel::Scalar)
}

/// Level `kernel` runs at when the caller asked for `requested`.
///
/// f32 [`Kernel::Dot`] and [`Kernel::Axpy`] use `requested` or scalar. [`Kernel::DotI8`] steps
/// from AVX-512 down to AVX2 when BW is missing or the AVX-512 kernel is not compiled.
pub(crate) fn kernel_level_on(
    kernel: Kernel,
    requested: SimdLevel,
    isa: &CpuIsa,
    build: &CompiledIsa,
) -> SimdLevel {
    match kernel {
        Kernel::Dot | Kernel::Axpy => {
            if ready(requested, kernel, isa, build) {
                requested
            } else {
                SimdLevel::Scalar
            }
        }
        Kernel::DotI8 => {
            if requested == SimdLevel::Avx512 && ready(SimdLevel::Avx512, Kernel::DotI8, isa, build)
            {
                SimdLevel::Avx512
            } else if matches!(requested, SimdLevel::Avx2 | SimdLevel::Avx512)
                && ready(SimdLevel::Avx2, Kernel::DotI8, isa, build)
            {
                SimdLevel::Avx2
            } else if requested == SimdLevel::Neon
                && ready(SimdLevel::Neon, Kernel::DotI8, isa, build)
            {
                SimdLevel::Neon
            } else {
                SimdLevel::Scalar
            }
        }
    }
}

fn format_startup(cap_label: &str, dot: SimdLevel, dot_i8: SimdLevel, axpy: SimdLevel) -> String {
    let body = format!(
        "galion-tensor dot={} dot_i8={} axpy={}",
        dot.name(),
        dot_i8.name(),
        axpy.name()
    );
    if cap_label == "none" {
        body
    } else {
        format!("{body} cap={cap_label}")
    }
}

fn format_report(
    cap_label: &str,
    selected: SimdLevel,
    isa: &CpuIsa,
    build: &CompiledIsa,
) -> String {
    let dot = kernel_level_on(Kernel::Dot, selected, isa, build);
    let dot_i8 = kernel_level_on(Kernel::DotI8, selected, isa, build);
    let axpy = kernel_level_on(Kernel::Axpy, selected, isa, build);
    let parts: Vec<String> = [SimdLevel::Avx512, SimdLevel::Avx2, SimdLevel::Neon]
        .into_iter()
        .map(|level| {
            let cpu = cpu_flag(level, isa);
            let feat = compiled_on(level, build);
            let implemented = implemented_on(level, build);
            if level == SimdLevel::Avx512 {
                format!(
                    "avx512(cpu={cpu},bw={},feat={feat},impl={implemented})",
                    isa.avx512bw
                )
            } else {
                format!("{}(cpu={cpu},feat={feat},impl={implemented})", level.name())
            }
        })
        .collect();
    format!(
        "detected={} selected={} dot={} dot_i8={} axpy={} cap={} {}",
        detected_on(isa).name(),
        selected.name(),
        dot.name(),
        dot_i8.name(),
        axpy.name(),
        cap_label,
        parts.join(" ")
    )
}

struct Selection {
    cap_label: &'static str,
    level: SimdLevel,
}

fn selection() -> &'static Selection {
    static SEL: OnceLock<Selection> = OnceLock::new();
    SEL.get_or_init(|| {
        let raw = std::env::var("GALION_SIMD").ok();
        let isa = CpuIsa::host();
        let build = CompiledIsa::this_build();
        Selection {
            cap_label: cap_label(raw.as_deref()),
            level: select_with_cap_on(raw.as_deref(), &isa, &build),
        }
    })
}

/// Runtime CPU support for a level (independent of cargo features).
///
/// [`SimdLevel::Avx512`] is `avx512f` only. The int8 kernel also needs `avx512bw`; use
/// [`kernel_level`] for the level a kernel will actually enter.
pub fn cpu_supports(level: SimdLevel) -> bool {
    cpu_flag(level, &CpuIsa::host())
}

/// Highest level the CPU supports (ignores cargo features and BW). Diagnostic only.
pub fn detected() -> SimdLevel {
    detected_on(&CpuIsa::host())
}

/// Highest level f32 `dot` and `axpy` will run, capped by `GALION_SIMD`.
///
/// Computed once and cached (no per-call env read after the first call, no allocation on the
/// steady path). [`Kernel::DotI8`] may be lower than this; see [`kernel_level`].
pub fn selected() -> SimdLevel {
    selection().level
}

/// Uncached selection with an explicit cap string (`None` = no cap). Testable without env.
pub fn select_with_cap(cap: Option<&str>) -> SimdLevel {
    select_with_cap_on(cap, &CpuIsa::host(), &CompiledIsa::this_build())
}

/// Level `kernel` runs at when the caller asked for `requested` on this CPU and this build.
pub fn kernel_level(kernel: Kernel, requested: SimdLevel) -> SimdLevel {
    kernel_level_on(
        kernel,
        requested,
        &CpuIsa::host(),
        &CompiledIsa::this_build(),
    )
}

/// One-line diagnostic. Names the level each kernel runs at under [`selected`].
///
/// `dot` and `axpy` match `selected`. `dot_i8` matches `selected` only when that level's int8
/// kernel is really available (AVX-512 needs BW). `cap` is `none` when `GALION_SIMD` is unset.
pub fn report() -> String {
    let sel = selection();
    format_report(
        sel.cap_label,
        sel.level,
        &CpuIsa::host(),
        &CompiledIsa::this_build(),
    )
}

/// Short line other crates can log once at startup. Cached for the process.
///
/// ```text
/// galion-tensor dot=avx512 dot_i8=avx2 axpy=avx512
/// galion-tensor dot=scalar dot_i8=scalar axpy=scalar cap=scalar
/// ```
///
/// The second form is `GALION_SIMD=scalar` (also `off` or `0`). Unset omits `cap`.
pub fn startup_line() -> &'static str {
    static LINE: OnceLock<String> = OnceLock::new();
    LINE.get_or_init(|| {
        let sel = selection();
        let isa = CpuIsa::host();
        let build = CompiledIsa::this_build();
        format_startup(
            sel.cap_label,
            kernel_level_on(Kernel::Dot, sel.level, &isa, &build),
            kernel_level_on(Kernel::DotI8, sel.level, &isa, &build),
            kernel_level_on(Kernel::Axpy, sel.level, &isa, &build),
        )
    })
    .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu(avx2: bool, fma: bool, avx512f: bool, avx512bw: bool, neon: bool) -> CpuIsa {
        CpuIsa {
            avx2,
            fma,
            avx512f,
            avx512bw,
            neon,
        }
    }

    fn build(
        avx2: bool,
        avx512: bool,
        neon: bool,
        x86: bool,
        x86_64: bool,
        aarch64: bool,
    ) -> CompiledIsa {
        CompiledIsa {
            avx2,
            avx512,
            neon,
            x86,
            x86_64,
            aarch64,
        }
    }

    fn x64(avx2: bool, avx512: bool) -> CompiledIsa {
        build(avx2, avx512, false, true, true, false)
    }

    fn check(
        name: &str,
        build: CompiledIsa,
        isa: CpuIsa,
        requested: SimdLevel,
        dot: SimdLevel,
        dot_i8: SimdLevel,
        axpy: SimdLevel,
    ) {
        assert_eq!(
            kernel_level_on(Kernel::Dot, requested, &isa, &build),
            dot,
            "{name} dot"
        );
        assert_eq!(
            kernel_level_on(Kernel::DotI8, requested, &isa, &build),
            dot_i8,
            "{name} dot_i8"
        );
        assert_eq!(
            kernel_level_on(Kernel::Axpy, requested, &isa, &build),
            axpy,
            "{name} axpy"
        );
    }

    #[test]
    fn selected_respects_gates() {
        assert!(cpu_supports(SimdLevel::Scalar));
        let s = selected();
        assert!(s.is_implemented() && s.compiled() && cpu_supports(s));
        assert_eq!(
            kernel_level(Kernel::Dot, s),
            s,
            "f32 dot runs at selected()"
        );
        assert_eq!(kernel_level(Kernel::Axpy, s), s, "axpy runs at selected()");
        assert!(kernel_level(Kernel::DotI8, s) <= s);
        if !cfg!(any(
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-neon"
        )) {
            assert_eq!(
                s,
                SimdLevel::Scalar,
                "default build must stay on the scalar oracle"
            );
            for kernel in Kernel::ALL {
                assert_eq!(
                    kernel_level(kernel, SimdLevel::Avx512),
                    SimdLevel::Scalar,
                    "{kernel:?}"
                );
            }
        }
        assert_eq!(select_with_cap(Some("scalar")), SimdLevel::Scalar);
        assert_eq!(select_with_cap(Some("off")), SimdLevel::Scalar);
        assert_eq!(select_with_cap(Some("0")), SimdLevel::Scalar);
        assert!(select_with_cap(Some("avx2")) <= SimdLevel::Avx2);
        assert!(select_with_cap(Some("neon")) <= SimdLevel::Neon);
        assert!(SimdLevel::Avx2.f32_lanes() == 8 && SimdLevel::Avx512.f32_lanes() == 16);
        let line = report();
        assert!(line.contains(&format!("selected={}", s.name())), "{line}");
        assert!(line.contains("cpu="), "{line}");
        assert!(line.contains("cap="), "{line}");
        for kernel in Kernel::ALL {
            let level = kernel_level(kernel, s);
            assert!(
                line.contains(&format!("{}={}", kernel.name(), level.name())),
                "{line}"
            );
            assert!(
                startup_line().contains(&format!("{}={}", kernel.name(), level.name())),
                "{}",
                startup_line()
            );
        }
        assert!(std::ptr::eq(startup_line(), startup_line()));
        assert!(startup_line().starts_with("galion-tensor "));
    }

    #[test]
    fn this_build_matches_cfg_consts() {
        let b = CompiledIsa::this_build();
        assert_eq!(b.avx2, SimdLevel::Avx2.compiled());
        assert_eq!(b.avx512, SimdLevel::Avx512.compiled());
        assert_eq!(b.neon, SimdLevel::Neon.compiled());
        assert_eq!(b.x86 || b.x86_64, SimdLevel::Avx2.is_implemented());
        assert_eq!(b.x86_64, SimdLevel::Avx512.is_implemented());
        assert_eq!(b.aarch64, SimdLevel::Neon.is_implemented());
        assert_eq!(b.x86, cfg!(target_arch = "x86"));
        assert_eq!(b.x86_64, cfg!(target_arch = "x86_64"));
    }

    #[test]
    fn host_report_is_the_pure_decision() {
        let isa = CpuIsa::host();
        let build = CompiledIsa::this_build();
        let raw = std::env::var("GALION_SIMD").ok();
        let sel = select_with_cap_on(raw.as_deref(), &isa, &build);
        assert_eq!(selected(), sel);
        assert_eq!(detected(), detected_on(&isa));
        assert_eq!(cpu_supports(SimdLevel::Avx2), isa.avx2 && isa.fma);
        assert_eq!(cpu_supports(SimdLevel::Avx512), isa.avx512f);
        assert_eq!(cpu_supports(SimdLevel::Neon), isa.neon);
        let label = cap_label(raw.as_deref());
        assert_eq!(report(), format_report(label, sel, &isa, &build));
        let expect = format_startup(
            label,
            kernel_level_on(Kernel::Dot, sel, &isa, &build),
            kernel_level_on(Kernel::DotI8, sel, &isa, &build),
            kernel_level_on(Kernel::Axpy, sel, &isa, &build),
        );
        assert_eq!(startup_line(), expect);
    }

    #[test]
    fn host_int8_level_follows_bw_on_this_feature_set() {
        let isa = CpuIsa::host();
        let build = CompiledIsa::this_build();
        let used = kernel_level_on(Kernel::DotI8, SimdLevel::Avx512, &isa, &build);
        if build.avx512 && build.x86_64 && isa.avx512f && isa.avx512bw {
            assert_eq!(used, SimdLevel::Avx512);
        } else if build.avx2 && (build.x86 || build.x86_64) && isa.avx2 && isa.fma {
            assert_eq!(used, SimdLevel::Avx2);
        } else {
            assert_eq!(used, SimdLevel::Scalar);
        }
        let dot = kernel_level_on(Kernel::Dot, SimdLevel::Avx512, &isa, &build);
        let axpy = kernel_level_on(Kernel::Axpy, SimdLevel::Avx512, &isa, &build);
        if build.avx512 && build.x86_64 && isa.avx512f {
            assert_eq!(dot, SimdLevel::Avx512);
            assert_eq!(axpy, SimdLevel::Avx512);
        } else {
            assert_eq!(dot, SimdLevel::Scalar);
            assert_eq!(axpy, SimdLevel::Scalar);
        }
    }

    #[test]
    fn cap_strings_and_their_labels() {
        assert_eq!(cap_label(None), "none");
        assert_eq!(cap_label(Some("scalar")), "scalar");
        assert_eq!(cap_label(Some(" OFF ")), "scalar");
        assert_eq!(cap_label(Some("0")), "scalar");
        assert_eq!(cap_label(Some("avx2")), "avx2");
        assert_eq!(cap_label(Some("AVX2")), "avx2");
        assert_eq!(cap_label(Some("neon")), "neon");
        assert_eq!(cap_label(Some("avx512")), "avx512");
        assert_eq!(cap_label(Some("nope")), "avx512");
        assert_eq!(cap_label(Some("")), "avx512");
        let full = cpu(true, true, true, true, false);
        let both = x64(true, true);
        assert_eq!(
            select_with_cap_on(Some("scalar"), &full, &both),
            SimdLevel::Scalar
        );
        assert_eq!(
            select_with_cap_on(Some("off"), &full, &both),
            SimdLevel::Scalar
        );
        assert_eq!(
            select_with_cap_on(Some("0"), &full, &both),
            SimdLevel::Scalar
        );
        assert_eq!(
            select_with_cap_on(Some("avx2"), &full, &both),
            SimdLevel::Avx2
        );
        assert_eq!(
            select_with_cap_on(Some("neon"), &full, &both),
            SimdLevel::Scalar
        );
        assert_eq!(select_with_cap_on(None, &full, &both), SimdLevel::Avx512);
        assert_eq!(
            select_with_cap_on(Some("nope"), &full, &both),
            SimdLevel::Avx512
        );
        let neon_box = build(false, false, true, false, false, true);
        let neon_cpu = cpu(false, false, false, false, true);
        assert_eq!(
            select_with_cap_on(None, &neon_cpu, &neon_box),
            SimdLevel::Neon
        );
        assert_eq!(
            select_with_cap_on(Some("avx2"), &neon_cpu, &neon_box),
            SimdLevel::Neon
        );
    }

    #[test]
    fn kernel_choice_covers_every_feature_set() {
        let wide = cpu(true, true, true, true, false);
        let no_bw = cpu(true, true, true, false, false);
        let avx2_only = cpu(true, true, false, false, false);
        let nothing = cpu(false, false, false, false, false);

        // The F3 gap: avx512f without avx512bw. f32 stays on AVX-512. int8 steps down to AVX2.
        check(
            "missing bw, both features",
            x64(true, true),
            no_bw,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Avx2,
            SimdLevel::Avx512,
        );
        // AVX-512 feature only, no BW, and no AVX2 kernel compiled: int8 is scalar.
        check(
            "missing bw, avx512 feature only",
            x64(false, true),
            no_bw,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Avx512,
        );
        // BW present: int8 is the AVX-512 kernel even when the AVX2 feature is off.
        check(
            "bw present, avx512 feature only",
            x64(false, true),
            wide,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
        );
        check(
            "bw present, both features",
            x64(true, true),
            wide,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
            SimdLevel::Avx512,
        );
        // No SIMD features: scalar oracle, whatever the CPU says.
        check(
            "no features",
            x64(false, false),
            wide,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        // f32 does not step down. int8 does, which is the existing dot_i8_level contract.
        check(
            "avx2 feature only, request avx512",
            x64(true, false),
            wide,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Scalar,
        );
        check(
            "avx2 feature only, request avx2",
            x64(true, false),
            wide,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
        );
        check(
            "no avx512 flags, request avx512",
            x64(true, true),
            avx2_only,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Scalar,
        );
        check(
            "no avx512 flags, request avx2",
            x64(true, true),
            avx2_only,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
        );
        check(
            "no flags",
            x64(true, true),
            nothing,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        check(
            "avx2 without fma",
            x64(true, false),
            cpu(true, false, false, false, false),
            SimdLevel::Avx2,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        check(
            "fma without avx2",
            x64(true, false),
            cpu(false, true, false, false, false),
            SimdLevel::Avx2,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        check(
            "scalar request",
            x64(true, true),
            wide,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        check(
            "bw without avx512f",
            x64(true, true),
            cpu(true, true, false, true, false),
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Scalar,
        );
        // i686 implements AVX2, not AVX-512.
        let i686 = build(true, true, false, true, false, false);
        check(
            "i686 request avx512",
            i686,
            wide,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Avx2,
            SimdLevel::Scalar,
        );
        check(
            "i686 request avx2",
            i686,
            avx2_only,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
        );
        // NEON.
        let arm = build(false, false, true, false, false, true);
        let neon = cpu(false, false, false, false, true);
        check(
            "aarch64 neon",
            arm,
            neon,
            SimdLevel::Neon,
            SimdLevel::Neon,
            SimdLevel::Neon,
            SimdLevel::Neon,
        );
        check(
            "aarch64 neon flag off",
            arm,
            nothing,
            SimdLevel::Neon,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        // An AVX-512 request does not fall through to NEON. selected() would have picked NEON.
        check(
            "aarch64 request avx512",
            arm,
            neon,
            SimdLevel::Avx512,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        assert_eq!(select_with_cap_on(None, &neon, &arm), SimdLevel::Neon);
        // NEON feature on x86_64 is not an implemented kernel.
        let neon_on_x64 = build(true, true, true, true, true, false);
        check(
            "neon feature on x64",
            neon_on_x64,
            cpu(true, true, true, true, true),
            SimdLevel::Neon,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
            SimdLevel::Scalar,
        );
        check(
            "cap stays on avx2",
            x64(true, true),
            no_bw,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
            SimdLevel::Avx2,
        );

        // Report text for the gap, including a build where the features are on.
        let sel = select_with_cap_on(None, &no_bw, &x64(true, true));
        assert_eq!(sel, SimdLevel::Avx512);
        let line = format_report("none", sel, &no_bw, &x64(true, true));
        assert!(line.contains("detected=avx512"), "{line}");
        assert!(line.contains("selected=avx512"), "{line}");
        assert!(line.contains("dot=avx512"), "{line}");
        assert!(line.contains("dot_i8=avx2"), "{line}");
        assert!(line.contains("axpy=avx512"), "{line}");
        assert!(line.contains("cap=none"), "{line}");
        assert!(line.contains("bw=false"), "{line}");
        assert_eq!(
            format_startup(
                "none",
                SimdLevel::Avx512,
                SimdLevel::Avx2,
                SimdLevel::Avx512
            ),
            "galion-tensor dot=avx512 dot_i8=avx2 axpy=avx512"
        );
        let capped = format_report("scalar", SimdLevel::Scalar, &wide, &x64(true, true));
        assert!(
            capped.contains("dot=scalar")
                && capped.contains("dot_i8=scalar")
                && capped.contains("axpy=scalar"),
            "{capped}"
        );
        assert_eq!(
            format_startup(
                "scalar",
                SimdLevel::Scalar,
                SimdLevel::Scalar,
                SimdLevel::Scalar
            ),
            "galion-tensor dot=scalar dot_i8=scalar axpy=scalar cap=scalar"
        );
        assert_eq!(
            format_startup("avx2", SimdLevel::Avx2, SimdLevel::Avx2, SimdLevel::Avx2),
            "galion-tensor dot=avx2 dot_i8=avx2 axpy=avx2 cap=avx2"
        );
        // Default-build simulation: CPU is wide, features are off, report stays scalar.
        let scalar_build = x64(false, false);
        let scalar_sel = select_with_cap_on(None, &wide, &scalar_build);
        assert_eq!(scalar_sel, SimdLevel::Scalar);
        let scalar_line = format_report("none", scalar_sel, &wide, &scalar_build);
        assert!(scalar_line.contains("detected=avx512"), "{scalar_line}");
        assert!(scalar_line.contains("selected=scalar"), "{scalar_line}");
        assert!(
            scalar_line.contains("dot=scalar")
                && scalar_line.contains("dot_i8=scalar")
                && scalar_line.contains("axpy=scalar"),
            "{scalar_line}"
        );
        assert!(scalar_line.contains("feat=false"), "{scalar_line}");
    }
}
