# S57 — honest SIMD detection

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S57, honest hardware detection in `galion-tensor` |
| Written | 2026-11-25. No benchmark claim, no weight download, no voice audio, no paid API, no merge. |
| Base | started at `main` `5d14d7b6c589884691f1b9e001f00f7664a219ea`, then merged `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` |
| Gap | `docs/horbm/foundations/F3-compute-audit.md` §1 and §12.3. `cpu_supports(Avx512)` is `avx512f` only. The int8 kernel also needs `avx512bw`. `features::report()` could print `selected=avx512` while `dot_i8_for(selected())` ran AVX2. |
| This change | `crates/galion-tensor` and this note. `LOCKS.toml` was not edited. `SOUL.md` is not in this tree. |
| Not edited | `horbm/crates/horbm-brain` (another swarm), `crates/horbm-keys`, `LEDGER.md`, root `Cargo.toml` members, `.github/workflows/*`. |

## What it decides

Galion stays CPU-first. `galion-tensor` detects the CPU it is running on and dispatches each kernel to the widest level that kernel can actually execute. Scalar stays compiled in every build and stays the oracle: f32 `dot` within [`dot_tolerance`](../../../crates/galion-tensor/src/simd.rs), int8 `dot` and f32 `axpy` bit-exact.

There is no new SIMD crate and no `-C target-cpu=native`. The cargo features stay opt-in and off by default: `simd-avx2`, `simd-avx512`, `simd-neon`. A default build reports and runs scalar even when the CPU has AVX-512.

`cpu_supports(Avx512)` is still `avx512f` only. That flag is what the f32 kernels need. It is not a claim about the int8 kernel. Callers that want the level a kernel will enter use `kernel_level` (or `simd::dot_level`, `simd::dot_i8_level`, `simd::axpy_level`). `horbm-keys` and `horbm-brain` already call `dot_i8_level(selected())`. That function now reads the same decision as `report()`. Its result did not change.

## Per-kernel level

`selected()` is the highest level f32 `dot` and f32 `axpy` will run, after the cargo feature, the arch, and the CPU flags, capped by `GALION_SIMD`. `dot` and `axpy` at that level do not step down: `dot_for(selected())` is that kernel, and an explicit request for a level that is not available is scalar, the same as before.

`dot_i8` does step down, the same contract `dot_i8_level` already had. An AVX-512 request runs the AVX-512 int8 kernel only when `avx512f` and `avx512bw` are both present and `simd-avx512` is compiled for `x86_64`. Otherwise it uses AVX2 when `simd-avx2` is compiled and the CPU has `avx2` and `fma`, else scalar. AVX2 still requires `fma` because that is the gate for the AVX2 level, including the int8 kernel that does not itself emit an FMA.

| Kernel | AVX2 | AVX-512 | NEON |
|---|---|---|---|
| f32 `dot` | `avx2` + `fma`, feature `simd-avx2`, x86 or x86_64 | `avx512f`, feature `simd-avx512`, x86_64 | `neon`, feature `simd-neon`, aarch64 |
| `dot_i8` | same AVX2 gate; an AVX-512 request may land here | `avx512f` and `avx512bw` | same NEON gate |
| f32 `axpy` | same as f32 `dot` | `avx512f` only | same as f32 `dot` |

`detected()` is unchanged: the highest CPU flag, ignoring cargo features and ignoring BW. A CPU with `avx512f` and no `avx512bw` is still `detected=avx512`. The kernel fields are what say which function runs.

The pure function behind this is `kernel_level_on` in `features.rs`. It takes the kernel, the requested level, a `CpuIsa` (the five flags), and a `CompiledIsa` (which features this binary has, and which arch it is). `report()` formats that decision. Tests pass fake ISAs, including `avx512f` without `avx512bw`, so the gap is locked without depending on the host clearing a flag.

## What `report()` and `startup_line()` print

`report()` is the long line. One example, both SIMD features compiled, CPU has AVX-512F and AVX2+FMA, and no AVX-512BW:

```text
detected=avx512 selected=avx512 dot=avx512 dot_i8=avx2 axpy=avx512 cap=none avx512(cpu=true,bw=false,feat=true,impl=true) avx2(cpu=true,feat=true,impl=true) neon(cpu=false,feat=false,impl=false)
```

`cpu` on the `avx512(...)` term is `avx512f`. `bw` is `avx512bw`. `feat` is the cargo feature. `impl` is the arch. `selected` is the f32 level. `dot` and `axpy` match it. `dot_i8` is the int8 kernel.

`startup_line()` is the short line other crates log once. It is cached for the process (`&'static str`, the second call returns the same pointer). `GALION_SIMD` is read once, on the first of `selected`, `report`, or `startup_line`.

```text
galion-tensor dot=avx512 dot_i8=avx2 axpy=avx512
```

With `GALION_SIMD=scalar` (also `off` or `0`):

```text
galion-tensor dot=scalar dot_i8=scalar axpy=scalar cap=scalar
```

Unset omits `cap`. `avx2` and `neon` cap the ceiling (`cap=avx2`, `cap=neon`). Any other set value, including `avx512` and an unknown string, caps at AVX-512 and is labelled `cap=avx512`. A `neon` cap on x86_64 does not select NEON: the kernel is not implemented on that arch, so the level is scalar.

```rust
eprintln!("{}", galion_tensor::startup_line());
```

Levels in both lines are `scalar`, `neon`, `avx2`, or `avx512`.

## `GALION_SIMD`

Read once, trimmed, ASCII case-insensitive.

| Value | Ceiling | Label |
|---|---|---|
| unset | AVX-512, still subject to feature and CPU | `cap=none` (omitted on the startup line) |
| `scalar`, `off`, `0` | scalar, CPU not consulted | `cap=scalar` |
| `neon` | NEON | `cap=neon` |
| `avx2` | AVX2 (NEON remains eligible where it ranks below AVX2) | `cap=avx2` |
| `avx512` or anything else | AVX-512 | `cap=avx512` |

## Tests

`kernel_choice_covers_every_feature_set` states the level of each kernel for feature sets that are not necessarily the one in the current binary: no features, AVX2 only, AVX-512 only, both, NEON on aarch64, NEON feature on x86_64, i686 (AVX2 yes, AVX-512 no), missing FMA, BW without F, and the F3 gap (F without BW). It also checks the report text for that gap: `selected=avx512` together with `dot_i8=avx2`.

`reported_level_matches_the_kernel_that_runs` parses `report()` and `startup_line()`, and checks that the function pointer `dot_for` / `dot_i8_for` / `axpy_for` returned for `selected()` is the concrete kernel named by that level, not the scalar oracle unless the level is scalar. It does the same for every requested level (`Scalar`, `Avx2`, `Avx512`, `Neon`). It then runs that function on a short vector and compares it to the scalar oracle. The cfg arms in the test match the dispatch arms. A level whose kernel was not compiled fails the test if the report still names it.

`host_int8_level_follows_bw_on_this_feature_set` checks the host flags against that same decision for whatever features this binary compiled. On a CPU with AVX-512F and AVX-512BW and `simd-avx512` enabled, int8 is AVX-512. On the same CPU with only `simd-avx2`, an AVX-512 request for int8 is the AVX2 function, and `selected()` is AVX2, so the report says `dot_i8=avx2`.

The test host for this note is Linux x86_64. `/proc/cpuinfo` on that host lists `avx2`, `fma`, `avx512f`, and `avx512bw`. That is a flag list, not a timing. NEON is not executed here. The pure table covers the aarch64 rows. Nothing in this change is Linux-only: detection is `std::arch`, and the kernels stay behind `cfg` and the cargo features.

```text
cargo test -p galion-tensor
cargo test -p galion-tensor --no-default-features
cargo test -p galion-tensor --features simd-avx2
cargo test -p galion-tensor --features simd-avx512
cargo test -p galion-tensor --no-default-features --features simd-avx512
cargo test -p galion-tensor --features simd-avx2,simd-avx512
cargo test -p galion-tensor --features simd-neon
cargo test -p galion-tensor --features simd-avx2,simd-avx512,simd-neon
```

Do not set `RUSTFLAGS` to `-C target-cpu=native`.

## What this does not do

The boot probe, the degrade profile, and the GPU device switches in F3 are unchanged. This crate still has no GPU kernel. A default build still stays on the scalar oracle until a binary is built with the SIMD features. Consent is unchanged: `remember=deny`, `learn=deny`, `learn_in_band=false`. Samantha stays the persona. No research card was added.
