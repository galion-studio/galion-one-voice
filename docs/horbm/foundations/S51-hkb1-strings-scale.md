# S51 — HKB1 string table and per-key scale

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S51, retrieval arena gaps |
| Written | 2026-11-25. No benchmark, no weight download, no voice audio, no paid API, no merge. |
| Base | `main` `67ade7dab35894f0ff5f0665c3647e70533dedd5` |
| Gap | `docs/horbm/foundations/F-integration-deep-dive.md`, "Gaps left open": the string table is not stored in `HKB1`, a caller binds it, and there is no per-key scale. |
| This change | `horbm/crates/horbm-brain`, this note, and the three lockfiles that record the new path edges. `LOCKS.toml` and `SOUL.md` were not edited. `SOUL.md` is not in this tree. |
| Not edited | `horbm/crates/horbm` (the turn crate), `crates/horbm-keys`, `voice-stack/**`, `crates/galion-sideband`, `crates/galion-conductor`. |

## What it decides

Version 2 of the `HKB1` file stores one UTF-8 `item_id` per row and one int8 scale per row. Version 1 files, the ones `horbm_keys::KeyPack::encode` already writes, still load. A version 1 file has no string table, so every string id misses, and each row's scale is `1`.

The dot is `galion_tensor::simd::dot_i8_for`. This crate does not add a second SIMD kernel and does not pass `-C target-cpu=native`. Default features stay on the scalar oracle. `simd-avx2`, `simd-avx512`, and `simd-neon` forward to `galion-tensor` and `horbm-keys`.

A consent-denied row and a missing id return `None`. A denied row is not dotted. A short query on a denied row is still `None`. A short query on an allowed row is `HkbError::QueryLen`.

The pick of a stored clause is unchanged. Consent in `LOCKS.toml` is unchanged: `remember = "deny"`, `learn = "deny"`, `learn_in_band = false`. Samantha stays the persona. No research card was added. Tests build the file in a `Vec`.

## File

Little-endian. Magic `HKB1`. Header 64 bytes, the same width as version 1.

| Offset | Version 1 | Version 2 |
|---|---|---|
| 0..4 | `HKB1` | `HKB1` |
| 4..6 | `1` (`horbm_keys::VERSION`) | `2` (`STORED_VERSION`) |
| 6..8 | flags `0` | flags `0` |
| 8..12 | `n` `u32` | `n` `u32` |
| 12..16 | `k` `u32` | `k` `u32` |
| 16..20 | `keys_off` | `keys_off` (still 64) |
| 20..24 | `side_off` | `side_off` |
| 24..28 | zero | `scale_off` `u32` |
| 28..32 | zero | `strings_off` `u32` |
| 32..36 | zero | `strings_len` `u32` |
| 36..64 | zero | zero |

`keys_off` and `side_off` must equal `horbm_keys::layout`. The key bytes, clause ids, consent bitset, and source ids sit in that region, the same bytes version 1 would have written for the same rows. Consent is unchanged: bit 1 allows, bit 0 denies, spare bits in the last consent byte must be 0.

After that region, version 2 appends, with no extra padding:

- `n` scale bytes, one `i8` per row, two's complement. `scale_off` is the version-1 file length.
- the string table. `strings_off = scale_off + n`. `strings_len` is its byte length. The file ends on the last byte of the table.

The string table is `n` records and has no count of its own (`n` is in the header):

- `u16` little-endian byte length, from 1 through 65535
- that many UTF-8 bytes

An empty id, a duplicate id, a length that does not fit in `u16`, a non-UTF-8 record, or bytes left over after `n` records are rejected. `k` is still the version-1 range: 1 through `DOT_I8_MAX_K`, which includes the 32-wide hot path and the 256-wide width.

`horbm_keys::KeyArena` reads version 1 only. It rejects version 2 with `UnsupportedVersion(2)`. That is the version boundary. Old files keep loading in both readers. New files load in `StoredKeys`.

`StoredKeys::open` maps through `galion_tensor::MappedBytes` and prefaults before the header is parsed, then keeps an owned copy of the rows. `from_bytes` parses a buffer and does not touch the disk. Unix `mmap` and the Windows heap read stay inside `MappedBytes`. This crate has no `cfg(unix)` of its own.

## Score

The query lanes are `horbm_keys::encode_query_i8`. That mixer has no scale of its own, so the query scale is 1.

The dequantised score of an allowed row is

```text
(dot_i32 as f32) * (scale_i8 as f32)
```

`dot_i32` is the kernel's sum. A stored lane stands for `quantised_lane * scale`, so the dot with an int8 query is `dot * scale`. The `f32` product is that value, and it is the same expression on the scalar path and on every SIMD level. When the integer product is an exact `f32`, the score is that integer.

Checked example, scale `-3`, quantised row `[4, 1, -2]`, query `[2, -5, 7]`:

- real lanes are `-12, -3, 6`
- real dot is `33`
- int8 dot is `-11`
- dequantised score is `-11 * -3 = 33`

Version 1 uses scale `1`, so an allowed row's score is the integer dot as `f32`. String lookup on a version 1 file is `None`, because the ids were never in the file. `score_row_at` still scores that row by index.

`score_scalar_text` forces `SimdLevel::Scalar`. `score_selected_text` uses `features::selected()`, then `dot_i8_for`, which applies the AVX-512BW rule inside `galion-tensor`. `active_kernel` is `dot_i8_level(selected())`.

## Turn crate, not changed

`horbm/crates/horbm/src/keys.rs` still implements `Int8Scorer` for `horbm_keys::StringKeyArena` and returns `score_selected_text` as `Option<i32>`. That is the raw dot. The caller still passes the string table into `StringKeyArena::open`. This PR does not edit that file.

To score a version-2 file from the turn, the call would be:

```rust
let keys = horbm_brain::StoredKeys::open(path, horbm_brain::KeyPrefault::Populate)?;
let score: Option<f32> = keys.score_selected_text(item_id, query);
```

`Int8Scorer::score` returns `Option<i32>`. The dequantised `f32` does not fit that trait unless `horbm/crates/horbm/src/retrieve.rs` and `keys.rs` change. Those files were left alone. A version-1 arena opened through `StringKeyArena` keeps the old `i32` dot. `StoredKeys` on the same bytes reports scale `1` and misses every string id.

`galion-cascade`'s manifest comment still says `horbm-brain` has zero dependencies. The crate now path-depends on `horbm-keys` and `galion-tensor` with default features off. No crates.io crate was added. That comment was not edited; the cascade crate is outside this change. `Cargo.lock`, `horbm/Cargo.lock`, and `voice-stack/Cargo.lock` each gained those two names under `horbm-brain` and no other lines.

The retrieval thread still does not call the scorer. Asks still use the lexical diary path.

## Tests

No card JSON, no `said_by`, no `claim_type`, no audio file. Rows come from `horbm_keys::shaped_pack` or from literals in the test, then `StoredPack::encode` into a `Vec`. One test writes that buffer under the process temp directory so `StoredKeys::open` and `KeyArena::open` both run, then deletes the directory. It is not a fixture in the repo.

Covered:

- version 1 loads, scales are `1`, string scores are `None`, a denied row is `None` and is not rejected for a short query, an allowed row matches the scale-1 reference
- version 2 round-trips ids, including `id-β`, and its key region matches the version-1 encoding of the same rows
- empty version 1 and empty version 2 both load and are not the same bytes
- the integer example above is `33` on every kernel, including an unaligned query
- widths 1, 7, 15, 32, 33, 64, 65, and 256: scalar, selected, AVX2, AVX-512, and NEON requests match an independent `i32` sum plus the same `f32` epilogue, compared with `f32::to_bits`
- a missing id and a denied row are `None` on those paths
- bad magic, a foreign version, non-zero flags, a dirty consent bit, a non-zero reserved byte, a trailing byte, a truncated table, a duplicate id, an empty id, a non-UTF-8 id, `k = 0`, `k` above `DOT_I8_MAX_K`, and an id longer than 65535 bytes are rejected
- `KeyArena` rejects the version-2 temp file with `UnsupportedVersion(2)`

On this host the CPU advertises `avx2`, `fma`, `avx512f`, and `avx512bw`. The default build's `active_kernel` is scalar. With `--features simd-avx2,simd-avx512` the kernel test requires AVX2 or AVX-512, and `dot_i8_level` selects AVX-512 when BW is present. Both builds passed, 13 tests each (8 new, 5 existing pick tests).

`cargo clippy --manifest-path horbm/Cargo.toml -p horbm-brain --all-targets -- -D warnings` is clean for the default build and for `simd-avx2,simd-avx512`.

Rustc 1.85.0 on Linux. `cargo test --manifest-path horbm/Cargo.toml --workspace` passed 245 tests, 1 already ignored, 0 failed. `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` was clean. `cargo test --workspace` at the repo root passed 1066 tests, 5 already ignored, 0 failed. `cargo clippy --workspace --all-targets -- -D warnings` at the repo root was clean.

Nothing in this crate is Linux-only. `Prefault::Populate` is `MAP_POPULATE` on Linux and a page walk elsewhere, inside `MappedBytes`, which already shipped that split.
