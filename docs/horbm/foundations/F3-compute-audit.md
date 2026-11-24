# F3 — compute audit, and the mapped int8 key arena

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM foundations, agent F3 of 4 |
| Written | 2026-11-24. No benchmark, no weight download, no voice audio, no paid API. |
| Voice tree | `galion-studio/galion-one-voice` `main` `4759d419cc647f4c83385c0db694c2c504142dde` |
| Open Mind tree | `galion-studio/obsidian-open-mind` `main` `0b0a898695b0525b179b5dd791bbfa8c281b7ffc` (committed 2026-11-21). Read through the GitHub API. Not cloned. Not vendored. |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §6, §7, §11 on draft PR #207, head `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`, branch `cursor/horbm-r4-cpu-realtime-2180`. The draft is closed and unmerged. R1, R2, and R3 were read from their branches the same day. |
| This change | `crates/horbm-keys` plus a public map helper on the existing `.gq` reader. `LOCKS.toml` is untouched. No `SOUL.md` exists in either tree that was read. |

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A constant or a behaviour already in a named file. |
| **[M]** | A measurement already written in this repo. This session did not repeat it. |
| **[C]** | A published source named by R1–R4. This session did not re-fetch the paper. |
| **[T]** | A target in R4. Not measured here. |

## What the four research notes ask of this crate

R4 §6.2 says the key scan is an int8 dot plus a compare, and that the dot already lives in `galion-tensor`. R4 §6.3 says the key file is mapped at boot with `Prefault::Populate` or `Touch`, 64-byte aligned. R4 §6.4 proposes a 32-byte int8 key inside a 64-byte record **[T]**. R4 §6.5 says structure-of-arrays, and a denied consent bit is skipped before the dot. R4 §11 phase 3 is the gate this crate implements: bit-exact against scalar on a fixed fixture, and no timing claim. Phase 3 depends on phase 2. Phase 2, the in-process FTS retrieval thread, is not on `main`. The arena stays a library with no retrieval thread of its own.

R1 (branch `cursor/horbm-r1-retrieval-core-690c`) wants an exact int8 scan as well, at 256 dimensions, with one scale per vector. That width is a proposal in R1, not a lock. R2 says the default vector scan is the brute-force int8 dot `galion-tensor` already has. R3 says an int8 vector is stored only if a static embedder is later accepted, and that product quantisation waits until a vector column exists. This crate stores `k` in the header, so 32 and 256 are the same file. It stores no per-key scale. The compare is the i32 dot.

## 1. `galion-tensor` — SIMD detect

Path: `crates/galion-tensor/src/features.rs`.

`SimdLevel` is `Scalar < Neon < Avx2 < Avx512`. A kernel runs only when three switches are all true: the cargo feature is compiled (`simd-avx2`, `simd-avx512`, `simd-neon`; none of them is in `default`), `cpu_supports` is true, and `is_implemented` is true for the target arch. `selected()` caches that choice. `GALION_SIMD=scalar|off|0` forces scalar. `avx2` and `neon` cap the level. Anything else, including an unset variable, caps at AVX-512, which still has to pass the three switches.

`cpu_supports(Avx2)` requires `avx2` and `fma`. `cpu_supports(Avx512)` requires `avx512f` only. `cpu_supports(Neon)` requires `neon` on aarch64. `detected()` reports the highest CPU flag and ignores cargo features. `report()` prints both.

Tests: `selected_respects_gates`. A default build asserts `selected() == Scalar`.

Status: working. The default workspace test in `.github/workflows/gates.yml` (`cargo test --workspace`) builds this crate without the SIMD features, so that job stays on the scalar oracle. **[D]**

Gap: `cpu_supports(Avx512)` does not look at AVX-512BW. The int8 kernel does. See §2.

## 2. `galion-tensor` — kernels

Path: `crates/galion-tensor/src/simd.rs`.

| Kernel | Scalar | SIMD | Match to scalar |
|---|---|---|---|
| `dot` f32 | sequential `acc += a[i]*b[i]` | AVX2+FMA intrinsics, AVX-512 `asm!`, NEON `vfmaq_f32` | tolerance `dot_tolerance` **[D]** |
| `dot_i8` → i32 | `wrapping_add` of `i32` products | AVX2 `madd`, AVX-512BW `asm!`, NEON `vmull_s8` | bit-exact |
| `axpy` f32 | `y += alpha * x`, not fused | same per-lane order | bit-exact |

`DOT_I8_MAX_K` is `(1 << 31) / (128 * 128) - 1` **[D]**. At that length the worst-case product still fits in an `i32`. Public entry points are `dot_for`, `dot_i8_for`, `axpy_for`. An unsupported level returns the scalar function, so the caller does not need `unsafe`.

AVX-512 int8 is compiled only with `feature = "simd-avx512"` on `x86_64`, and `dot_i8_level` (added with this change, same conditions the old `dot_i8_for` match used) returns `Avx512` only when `avx512bw` is also present. Otherwise an AVX-512 request uses AVX2 if that feature is compiled and the CPU has it, else scalar. `features::report()` can still print `selected=avx512` while the int8 kernel that runs is AVX2.

Tests in the same file: `dot_scalar_matches_naive`, `dot_all_levels_within_tolerance_of_scalar_oracle`, `dot_handles_unaligned_subslices`, `dot_i8_all_levels_bit_exact`, `dot_i8_max_k_is_safe`, `axpy_all_levels_bit_exact`, plus `i8_from_bytes_keeps_the_bit_pattern` and `dot_i8_level_matches_the_kernel_that_runs`. With the SIMD features off, the non-scalar arms compile out and the tests still compare against scalar.

Status: working. No new SIMD crate. No `target-cpu=native` in this crate.

`i8_from_bytes` is the one new helper: a `&[u8]` view as `&[i8]`. The key file is bytes. The kernel takes `i8`.

## 3. `galion-tensor` — quantisation

Path: `crates/galion-tensor/src/gemv_i8.rs`.

`quantize_row_i8` is symmetric absmax: scale = max(|x|) / 127, round to nearest, clamp to ±127. An all-zero row returns scale `1.0` and zeros. `QuantRowsI8` holds a row-major `[m, k]` `i8` matrix and one `f32` scale per row. `gemv_i8_into` dots each row with `dot_i8_for(selected())`, then applies `acc as f32 * (w_scale * x_scale)` plus an optional bias. That epilogue is the same expression at every level, so the f32 result is bit-exact against `gemv_i8_scalar_ref`. `Exec::Auto` goes parallel, behind the `parallel` feature (rayon, on by default), at `GEMV_I8_PAR_THRESHOLD` = 1,048,576 elements and rows above `GEMV_I8_ROW_BLOCK` = 64 **[D]**.

`crates/galion-tensor/src/gemv.rs` is the f32 GEMV. It points at `gemv_i8` for the int8 path.

`crates/galion-tensor/src/storage.rs` can hold `I8 { data: Arc<Vec<i8>>, scale: f32 }` and dequantises on `get_f32`. The module comment still says the mmap-backed block is future. The `.gq` reader maps on its own and does not store tensors in `Storage`.

Tests: `quantize_row_roundtrip_and_edge_cases`, `gemv_i8_all_levels_and_exec_bit_exact_vs_scalar_oracle`, `gemv_i8_shape_errors`, `gemv_i8_accuracy_vs_f32_within_quantization_bound`, and `tests/zero_alloc_gemv.rs` (`gemv_paths_are_alloc_free`). `benches/gemv.rs` exists and prints numbers when someone runs it. This session did not run the bench.

Status: working for W8A8 GEMV. The HORBM key compare does not go through it. A per-row scale would change the ranking, and version 1 of the key file has no scale column. The integer kernel is the one R4 §6.2 names.

## 4. `galion-tensor` — mmap and prefault

Path: `crates/galion-tensor/src/gq.rs`.

`.gq` is reader-only. Magic `GALIONGQ`, version 1, header 64 bytes, alignment a power of two from 64 to 65536 **[D]**. Dtypes are f32, bf16, and symmetric per-tensor int8 (`GqDType::I8`, real = q * scale, scale must be finite and > 0). The production writer is still deferred. Synthetic writers live in this file's tests and in `crates/galion-symphony/src/gq_write.rs` and `crates/galion-ml-voice`.

`Prefault`:

| Variant | Behaviour |
|---|---|
| `None` | lazy faults |
| `WillNeed` | `madvise(MADV_WILLNEED)` on Unix, result ignored |
| `Populate` | Linux `MAP_POPULATE` at map time. Everywhere else, the same page walk as `Touch` |
| `Touch` | one volatile read per 4096-byte page **[D]** |

Unix uses `mmap` `PROT_READ|MAP_PRIVATE` through libc, with `munmap` on drop. Every other OS, including Windows, reads the file into a `Vec<u64>` (8-byte alignment). `GqFile::open` parses after the map. A page walk runs only after a successful parse.

`MappedBytes::open` is the glue added for the key arena. It calls the same `load_backing` and `apply_prefault` as `GqFile::open`, and it walks pages before returning, including when the caller then rejects the header. `GqFile::open` still walks only after a successful parse. Tests: the existing `mmap_open_with_every_prefault` now also checks `MappedBytes` against the file bytes, and `tests/gq_synthetic_g5.rs` still opens synthetic `.gq` files (8 tests, run this session).

`GqFile::raw` is zero-copy bytes. `GqFile::tensor` for `I8` dequantises to f32. There is still no `i8` view method on `GqFile`. Callers use `i8_from_bytes` on `raw`.

Status: working. The Windows path is a full read, the same path `.gq` already uses. This session typechecked that cfg with `cargo check --target x86_64-pc-windows-gnu`. It did not boot Windows.

## 5. Threads inside `galion-tensor`

Path: `crates/galion-tensor/src/threading.rs`.

`Exec::{Scalar, Parallel, Auto}`. `Auto` uses rayon when the `parallel` feature is on and the work is at least `PAR_THRESHOLD` = 16,384 **[D]**. `init_thread_pool` builds a named `galion-tensor-{i}` pool. The key arena depends on `galion-tensor` with `default-features = false`, so the scan does not start that pool. The dot is one function pointer per row on the caller's thread. That matches R4 §6.1: one retrieval thread, no async runtime on the audio clock.

## 6. `galion-boot` hardware probe

Path, Open Mind: `crates/galion-boot/src/hardware.rs`. File comment: port of `hardware.sh`, work item B54.

`hardware(Probe)` reads:

- `MemTotal` and `MemAvailable` from `{proc_root}/meminfo`, KiB to MiB by integer division. A missing file or a missing key yields 0.
- `std::thread::available_parallelism`, or 0 if that call fails.
- `std::env::consts::ARCH` and `OS`.
- `nvidia-smi --query-gpu=name,memory.total,memory.used --format=csv,noheader,nounits` through the injected `Runner`. `parse_nvidia` takes the first CSV row. Three numeric fields are required. Failure or a short row yields no NVIDIA gpu.
- If NVIDIA is absent, `drm_card` is true when `{sys_root}/class/drm` contains a `card*` node whose name has no `-`.

Free GPU MiB is `total - used` for an NVIDIA row, and 0 when the only signal is DRM. The JSON then includes `weights`, `mouth`, `llm`, `rule`, and `line` from `openmind_survival::degrade::profile`. `worker_mib` is 512 **[D]**. `cap` is `(ram_free_mib / 512).min(100)`.

`machine()` copies that into a studio card. `provider` is `"gpu"` when `weights` is true, otherwise `"cpu"`. `publicIp` is the caller-supplied host string, or empty. The file comment says there is no public-IP lookup.

Tests: `crates/galion-boot/tests/boot.rs` `hardware_profiles`. A fake meminfo of 16,384,000 KiB total and 8,192,000 KiB available, with `nvidia-smi` failing, expects the CPU line, `weights=false`, `mouth=true`, `gpu` null, `cap` 15. An RTX 4090 row `24564, 1000` expects `weights=true` and `free_mib` 23564. A DRM `card0` with WAN down and LAN up expects `llm=lan` and `machine.provider=cpu`.

Status: working on Linux paths and a fake `Runner`. It does not read SIMD flags, NUMA nodes, or CUDA compute capability. On a Windows laptop, `/proc/meminfo` and `/sys/class/drm` are absent unless the caller points `proc_root` and `sys_root` at stand-ins. RAM then reads as 0. NVIDIA still counts if `nvidia-smi` returns a row.

There is no crate named `galion-hw` in either tree.

## 7. `openmind-survival` degrade profile

Path: `crates/openmind-survival/src/degrade.rs`. File comment: B24, first match wins.

Thresholds: `MIN_GPU_FREE_MIB = 8192`, `MIN_RAM_MIB = 8192` **[D]**.

| Rule, in order | Condition | `llm` | `weights` | Line |
|---|---|---|---|---|
| `gpu-room` | GPU present, free ≥ 8192 MiB, RAM ≥ 8192 MiB | `local-gpu` | true | "Obsidian has room for local weights." |
| `wan-down-lan-up` | WAN down, LAN up | `lan` | false | "Internet is down. Hermes uses the LAN model." |
| `offline` | WAN down, LAN down | `cpu-light` | false | "No network. CPU-light: probes and desk only, work is queued." |
| `gpu-low` | GPU present (the earlier GPU rule missed) | `api` | false | "A card is here, but not enough free memory. Hermes stays on the laptop model." |
| `cpu-only` | everything else | `api` | false | "CPU only. Hermes stays on the laptop model. Weights stay off." |

`mouth` is `ram_total_mib >= 8192`, independent of the rule. A 2048 MiB CPU with WAN up is `cpu-only` / `api` / mouth false. A GPU with 9000 MiB free and only 4000 MiB RAM misses `gpu-room` (RAM) and hits `gpu-low` / `api`. A DRM card with free 0 and WAN up also hits `gpu-low` / `api`. `gpu-room` wins even when WAN and LAN are both down.

Tests: `table` in the same file, seven rows, including the CPU-only → `Api` row and the offline → `CpuLight` row.

Status: working, and it is the profile `galion-boot` writes into `hardware.json`. It chooses an LLM location. It does not choose a kernel, a core mask, or a device for an int8 dot.

## 8. How today's probe supports "use all of the hardware"

The owner ask is that every instance detect the machine and use all of it, including an NVIDIA GPU when one is present, and stay CPU-only when none is present. The code that exists does the detect and then makes a different choice.

What the probe can see, on Linux:

- CPU count and arch, from `available_parallelism` and `consts::ARCH`. It does not see AVX2, AVX-512, or NEON. That detect is `galion-tensor` `features.rs`, and only inside a binary built with the matching cargo feature.
- RAM total and available, from `/proc/meminfo`.
- An NVIDIA card's name, total MiB, used MiB, and free MiB, when `nvidia-smi` prints a row.
- A non-NVIDIA DRM card, as present with free MiB 0.
- LAN and WAN, as booleans the caller passes in. The probe does not dial anything itself.

What that information changes:

- Free GPU ≥ 8192 MiB and RAM ≥ 8192 MiB sets `llm=local-gpu` and `weights=true`. The line says the box has room for local weights. It does not name a library, a device id, or a kernel.
- A card that misses that bar sets `llm=api`. A CPU with the network up also sets `llm=api`. R4 §7 and §11 phase 8 say the voice must not follow the `api` line, and that a GPU flag must not move the mouth off the CPU brain. Phase 8 is not this change.
- `mouth` is a RAM threshold for "can run the voice worker", not a device.

Other detectors in `galion-one-voice`, each with its own policy:

| Path | What it detects | What it uses |
|---|---|---|
| `runtimes/galion-piper-rt/src/device.rs` | `nvidia-smi -L`, `/dev/nvidia0`, `/proc/driver/nvidia/version`, `CUDA_VISIBLE_DEVICES` empty or `-1` hides the device | Empty, `cpu`, and `auto` stay on CPU. `cuda` or `gpu` is tried only when the probe sees a device and a CUDA onnxruntime library is on disk. Otherwise it logs and stays on CPU. `DEFAULT_CUDA_ARENA_BYTES` is 2 GiB, a cap, and the file says it is not a measurement of what Piper allocates **[D]**. This process is the LOCKS Piper exception. It is not linked into the workspace. |
| `tools/symphony/sound-out/src/device.rs` and `tools/symphony/voice-v2/src/device.rs` | `/dev/nvidia0`, `/dev/nvidiactl`, `CUDA_VISIBLE_DEVICES`, `/dev/kfd` | With `GALION_DEVICE` unset, a visible NVIDIA device sets the request to CUDA. The same function's note says the Chatterbox worker process stays on CPU. ROCm sets the request and then keeps the clone on CPU, because `cbx` has no ROCm provider. |
| `voice-stack/samantha-voice-clone/src/device.rs` | a Python probe of `torch.cuda.is_available()` | `auto` and `cpu` stay on CPU even when the probe says cuda. `cuda` is used only when the probe agrees. |
| `voice-stack/local-server/src/mouth.rs` | the clone probe, and only when the device request is `cuda` | Piper is the mouth. The s711 checkpoint is selected only for `GALION_SAMANTHA_DEVICE=cuda` plus local files plus a visible CUDA device. `auto` stays on Piper. |
| `crates/galion-ml-voice` | tests require the default features not to name `cuda` | The worker clears `CUDA_VISIBLE_DEVICES`. |

So a present NVIDIA GPU is visible to the boot probe, and it changes the Hermes weight/API line when 8192 MiB are free. It does not attach a GPU kernel to the voice brain. The Piper runtime can use CUDA only as an explicit opt-in, out of process. Two symphony device modules will *request* CUDA when a device node exists, and they still describe the worker as CPU. The key scan has no GPU implementation to call. `LOCKS.toml` `[formats] forbidden_runtimes` lists `candle`, `ort`, `tract`, `burn`, `llama`, `ggml`. The `ort` exception is the Piper process only.

`galion-probe/run-probe.sh` defaults `RUSTFLAGS` to `-C target-cpu=native` when the variable is unset. `tools/cloning-v2/scorers/ecapa/.cargo/config.toml` sets the same flag. Open Mind `UNIVERSAL-CPU.md` (2026-10-18) forbids `target-cpu=native` and requires SIMD behind `cfg` with a scalar fallback. `horbm-keys` does not set that flag. Its SIMD is the cargo features above, with the scalar function still compiled.

`cgroup-detect` (Open Mind) reports whether a per-run cgroup is possible. Its manifest says it never creates a cgroup.

## 9. mmap, arena, and int8 elsewhere

### In `galion-one-voice`

| Path | Behaviour | Tests | Status |
|---|---|---|---|
| `crates/galion-symphony/src/load.rs`, `gq_write.rs`, `main.rs` | Opens a directory of `.gq` through `GqFile` and `Prefault`. The CLI accepts `willneed`, `populate`, `touch`, `none`. | `crates/galion-symphony/tests/pack.rs` | Working reader/writer for symphony packs. Not a key index. |
| `crates/galion-ml-voice/src/artifact.rs` | `GqFile::open(path, Prefault::Populate)` for a clip. | crate tests | Working. Clip bytes, not retrieval keys. |
| `crates/galion-cascade/src/student.rs` | `GqFile::open(path, Prefault::None)` | crate tests | Working lazy open. |
| `crates/galion-mimi/src/weights.rs`, `rvq.rs` | Comments: f32 reference, optional int8 rows, the converter must pre-divide. | nearby unit tests for the math they implement | The int8 layout is specified. It is not the HORBM arena. |
| `crates/galion-student-backend/src/lib.rs` | Comment: a published artifact may be adapted as int8 PTQ. | crate tests | No mapped key file. |
| `tools/cloning-v2/scorers/emotion/src/main.rs` | `--int8` swaps ONNX filenames to `*.int8.onnx`. | the scorer binary | A filename switch for a third-party model file. Not linked into `horbm-keys`. |
| `tools/cloning-v2/scorers/quality/.../galion-mos.rs` | Builds an ONNX session with `with_arena_allocator(false)`. | scorer tests | Turns an ORT arena off. ORT stays outside the workspace. |
| `runtimes/galion-piper-rt` `arena_bytes` | CUDA execution-provider memory cap. | `arena_default_is_the_cap_not_a_measurement` | Working cap. Separate process. |
| `crates/galion-tensor/src/kv.rs` `KvRing` | Preallocated ring, `push` / `clear`. | `ring_wraps_chronologically`, `zero_alloc.rs` | Working KV cache. Not a key arena. |

No other `mmap` of an int8 matrix was in this tree before `MappedBytes`.

### In `obsidian-open-mind`

| Path | Behaviour | Tests | Status |
|---|---|---|---|
| `crates/openmind-memory/src/store.rs` | SQLite FTS5 BM25. Manifest: "offline, std HTTP". Header comment: vec0 tables are never created or read; `/health` reports `embeddingService:false`. `upsert_l0` is "metadata + FTS, no vectors". Schema creates `embedding_meta` and the gateway-compat test prepares `SELECT key, value FROM embedding_meta`. No `PRAGMA mmap_size`. | In-file tests. The temp database is named `vectors.db`. | Working keyword store. The name `vectors.db` is the file the test opens. It is not a vector index. |
| `crates/galleon-arena` | Manifest: "Read-only paper arena: list seed gladiators and score a match. Paper integers only. No network." | crate tests | A game score, not a memory map. |
| `crates/openmind-llama` | Manifest: parse llama.cpp `/health` and `/v1/models` fixture bodies. "No sockets and no GPU." | crate tests | A parser. Not a runtime. Not linked here. `llama` stays on the forbidden list for this repo. |
| `crates/galion-voice-cpu` | Optional `candle` features (`whisper`, `kitten`) for an on-device CPU pipeline. Default features are empty. | feature-gated tests | Candle is `dev_oracle_only` / `forbidden_runtimes` in `LOCKS.toml` for the voice repo. This crate does not depend on it. |
| `crates/persona-plex-relay` | Scaffold. R4 read it as opening no socket. | its own tests | Not the CPU brain. |

`UNIVERSAL-CPU.md` is the portable-Rust rule the key crate follows: no `target-cpu=native` in its build, SIMD only behind `cfg` and a cargo feature, scalar still linked, no tokio `full`. The arena has no tokio dependency at all.

## 10. The `horbm` crate that is already on main

Path: `horbm/crates/horbm`. Its own workspace, excluded from the root workspace (`Cargo.toml` `exclude`).

`lib.rs` is `forbid(unsafe_code)`. One turn is empathy, then `CardLookup` / `KnowledgeCards` over a directory of galion-knowledge cards, then an optional research-gateway ask under `RESEARCH_CAP`, then `UNKNOWN_REPLY`. `Brain::open(memory_db, knowledge_dir, cases)` records the paths. The module docs say it does not open `horbm-memory` and does not bind a socket. The card index is an in-memory string-token table (`cards.rs`). There is no int8 column and no background retrieval thread.

CI: `.github/workflows/voice-stack-horbm-router.yml` runs `cargo test --manifest-path horbm/Cargo.toml -p horbm`. The new crate is a root-workspace member, so `cargo test --workspace` in `gates.yml` runs it, and a second step runs it again with `--features simd-avx2,simd-avx512`.

F2's retrieval trait was not on `main` at `4759d41`. This crate does not wait on it and does not invent the trait.

## 11. Phase 3 — what landed

Crate: `crates/horbm-keys`.

On disk, little-endian, magic `HKB1`, version 1, header 64 bytes **[D]**:

| Region | Where |
|---|---|
| `n`, `k`, `keys_off`, `side_off` | header |
| int8 keys, row-major `n * k` | offset 64, then padded to a multiple of 64 |
| clause id, `n` little-endian `u64` | side table |
| consent bitset, 1 = allow, 0 = deny, spare bits in the last byte must be 0 | side table |
| source id, `n` little-endian `u64` | side table |

`KEY_DIM` is 32 **[D]**, the R4 hot-path width **[T]**. `k` may be any value from 1 through `DOT_I8_MAX_K`, which covers R1's 256. `KeyArena::open(path, Prefault)` maps through `MappedBytes` and prefaults before it accepts the header. If the key pointer is not 64-byte aligned, the keys are copied once into a 64-byte-aligned buffer at open. On Unix the file offset is 64 and `mmap` is page-aligned, so the copy does not run. The test asserts that.

`dots_into` / `dots_scalar_into` / `dots_selected_into` write one i32 per row. Consent is not consulted. `dots_allowed_into` skips a denied row before the dot and writes only the allowed scores. `active_kernel()` is `dot_i8_level(selected())`.

The fixed fixture is `fixed_fixture()`, seed `0xF3F3_0001_484B_4231` **[D]**, 8 keys of width 32, row 0 denied. Its file sha256 is `a16465e5fc826fb14e00fc0e0590a5020d8081fabfac866b595bd656848e93ee` **[D]**, locked in `tests/bit_exact.rs`. The gate opens that file with `Prefault::Populate`, dots an unaligned query with the scalar kernel, with `selected()`, and with every `SimdLevel`, and compares all of them to an independent `i64` sum. The same test repeats `None`, `WillNeed`, `Touch`, and `Populate`. A second test does the same for widths 1, 7, 15, 33, 65, and 256. No elapsed time is recorded.

With `--features simd-avx2,simd-avx512` on an AVX2 CPU, `kernel_is_the_galion_tensor_detect` requires the active kernel to be `Avx2` or `Avx512`. A default build requires scalar. Both were run on this host (the host CPU advertises AVX-512F and AVX-512BW; that is a flag list, not a timing).

The crate does not read `hardware.json`, does not call `nvidia-smi`, and does not move the dot onto a GPU. There is no int8 GPU kernel in this tree to reuse.

## 12. Gaps still open

These are the five that block the owner sentence "detect the machine and use all of it" from being true of the voice brain. The rest of the audit is the evidence.

1. **The boot profile sends a CPU box, and a GPU that misses 8192 MiB free, to `llm=api`.** `degrade.rs` rule `cpu-only` and rule `gpu-low`. The probe sees the card and the free MiB. The voice path has no reader of that JSON that selects HORBM. R4 phase 8 is the change that would: CPU brain either way, GPU flag reported and unused by the mouth. Not built here.
2. **The detectors do not agree, and none of them dispatch the key scan onto a GPU.** Tensor SIMD is opt-in and CPU-only. Piper CUDA is opt-in, out of process, and is a memory cap on ORT. `sound-out` / `voice-v2` request CUDA when `/dev/nvidia0` exists and still describe the worker as CPU. Samantha stays on Piper unless `cuda` is requested. `LOCKS.toml` forbids linking `ort` into the workspace. An NVIDIA kernel for this dot would be a new runtime, not glue.
3. **A default build stays on the scalar oracle even when the CPU has AVX-512.** `gates.yml` now also runs `cargo test -p horbm-keys --features simd-avx2,simd-avx512`, which compiles the kernels and checks them. A binary that forgets the features never enters them. `selected=avx512` can still mean the int8 kernel is AVX2, because BW is a second flag (`dot_i8_level`).
4. **The map is Unix `mmap`. Windows is the `.gq` heap read.** Typechecked for `x86_64-pc-windows-gnu`, not run on GALION-STUDIO. `Storage` is still an owned `Vec`. `GqFile` still has no `i8` view of its own. `galion-probe/run-probe.sh` and the ECAPA scorer config still pass `-C target-cpu=native`. This crate does not.
5. **Phase 2 is not on main, and the key file has no per-vector scale.** `horbm` on main is the card turn, `forbid(unsafe_code)`, no int8 column. `openmind-memory` is an in-process SQLite FTS5 store behind a std HTTP binary. vec0 is never created, and `PRAGMA mmap_size` is never set. R1's per-vector scale and R4's 32-byte key are different layouts. Version 1 stores `k` and the i32 dot, and leaves the scale for a later version. The retrieval thread should call `KeyArena` when it lands. This crate does not call it.

Smaller gaps, same evidence: no NUMA in `hardware.rs` (R4 §6.5 already says not to pin in v1). `Prefault::WillNeed` is a hint. The arena allocates the caller's output buffer only; the dot itself does not allocate. Top-4 pick, FTS fusion, and the learn-pass pack swap are later phases (R4 §11 phases 4 and 7).

## 13. Build on the laptop

GALION-STUDIO is the Windows machine in `voice-stack/windows-install/README.md`. That folder keeps Cargo off `C:` by using `G:\galion\samantha`. Rust there is 1.85 or newer, host `x86_64-pc-windows-msvc`. From a checkout of this repo, in PowerShell:

```powershell
cd <checkout>
$env:CARGO_TARGET_DIR = "G:\galion\samantha\target-horbm-keys"
$env:CARGO_HOME = "G:\galion\samantha\cargo"
cargo test -p horbm-keys
cargo test -p horbm-keys --features simd-avx2,simd-avx512
cargo test -p galion-tensor --lib
```

Do not set `RUSTFLAGS` to `-C target-cpu=native`. If `G:` is not the drive that folder uses, drop the two `$env:` lines and the same three `cargo` lines still build. The first command is the scalar oracle. The second compiles the AVX kernels and, on a CPU that has AVX2, requires the active int8 kernel to be AVX2 or AVX-512. The Windows open path is the heap read inside `MappedBytes`. `Prefault::Populate` becomes the page walk, because `MAP_POPULATE` is Linux-only.
