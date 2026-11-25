# S54 — Retrieval scan over the diary arena

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S54, retrieval over the diary arena |
| Written | 2026-11-25. One release micro-benchmark, no weight download, no voice audio, no paid API, no merge. |
| Base | `main` `5d14d7b6c589884691f1b9e001f00f7664a219ea` (PR #315) |
| Gap | The retrieval thread does not call the `Int8Scorer` trait or scan the diary arena. S51 left that sentence in place. F-integration says the same thing about the diary's 256-wide hashing arena. |
| This change | `horbm/crates/horbm-brain` and this note. `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, and the root workspace manifest were not edited. `SOUL.md` is not in this tree. |
| Not edited | `horbm/crates/horbm` (the turn crate), `crates/horbm-keys`, `voice-stack/**`, `crates/galion-sideband`, `crates/galion-conductor`. |

## What it decides

`DiaryScan` scores one query against the HKB1 arena (`StoredKeys`, versions 1 and 2) and a person-diary arena (`DiaryArena`). Both implement `horbm_brain::Int8Scorer`. The dot is `galion_tensor::simd::dot_i8_for`. This crate does not add a second SIMD kernel and does not pass `-C target-cpu=native`. Default features stay on the scalar oracle. `simd-avx2`, `simd-avx512`, and `simd-neon` are the features S51 already forwards.

The scan writes into a buffer the caller owns. After `DiaryScan::open`, a query does not allocate. Setup allocates the arenas, the string index, and the scratch: one query buffer per arena and two top-k hit buffers of length `k`.

`remember` false matches `LOCKS.toml` `[consent] remember = "deny"`. That flag leaves the diary out. The diary leg is not prepared and not dotted. HKB1 rows keep their own allow bits. A consent-denied row in either arena is not dotted. `learn = "deny"` and `learn_in_band = false` are unchanged. Samantha stays the persona. No research card was added.

The diary matrix is the shape `horbm-memory` already stores at git revision `974e5944`: 256 int8 lanes, one `f32` scale, one `f32` norm. This crate does not link `horbm-memory`, does not open SQLite, and does not copy that crate's AVX2 kernel. The caller passes the rows in. A denied row is `allow: false`. Omitting the row is the same result.

## Score

`Int8Scorer::score(item_id, query) -> Option<i32>` is the single-id contract the turn crate already named. `None` is a missing id or a consent-denied row. The integer is the raw dot from the selected kernel. On `StoredKeys` that call allocates the query vector. On `DiaryArena` the query sits in a stack array of `DIARY_DIM` (256). The scan does not call `score`. It calls `prepare_query`, `dot_allowed`, and `lift` on the same trait, so the query bytes stay in the scratch from `open`.

`prepare_query` for keys is `encode_query_into`, the same mixer as `horbm_keys::encode_query_i8`, written into the scratch. The query scale is 1. A version-1 file still has no string table, so `score` is `None` for every id. The scan still ranks those rows by index. `DiaryScan::item_id` is `None` for a version-1 hit.

`prepare_query` for the diary is the hashing embedder: lowercase alphanumeric tokens, a token longer than 64 bytes dropped, FNV-1a, two signed lanes, L2 norm, then symmetric int8. Stopwords stay in the vector. This matches the diary's `HashingEmbedder` at `974e5944`. A query with no tokens has norm 0. `ranks_query` is false, and the diary leg does not dot.

The key lift is `dequant_score` from S51, the same expression on every kernel:

```text
(dot as f32) * (scale as f32)
```

The diary lift matches the diary's dense scan. A non-positive dot, a zero query norm, or a zero row norm drops the row:

```text
(dot as f32) * query_scale * row_scale / (query_norm * row_norm)
```

`dot_allowed` does not read the consent bit. The scan reads `allowed` first. A denied row is not dotted, including when the query is empty or the wrong width for a single-id call. The S51 example still holds through `lift`: scale `-3`, lanes `[4, 1, -2]`, query `[2, -5, 7]`, dot `-11`, lift `33`, on the scalar path and on every SIMD request, including an unaligned query.

## Top-k merge

The two native scores are different units (a dequantised key dot, and a diary cosine). The merge does not compare them to each other. Each arena keeps its own best `k` rows: higher native, then the lower row index. Rank `r` is 1-based in that list. The fused score is the diary's reciprocal-rank term, with the same constant the diary uses (`SCAN_RRF_K` = 60):

```text
1 / (60 + r)
```

The same rank from both arenas ties. The diary hit is written first, then the key hit. `ScanHit::score` is that fused value. `ScanHit::native` is the arena score. The call writes `n` hits at the front of the caller's buffer and leaves the tail alone. `n` is at most `min(out.len(), k)`. `SCAN_CAP` is 4, the same cap as `horbm::MEMORY_HIT_CAP`.

A one-slot output with both arenas live is the diary's best allowed row. An empty diary, a blank query, or `remember` false leaves a key-only list, ranked by the key native, with fused scores `1/61`, `1/62`, and so on.

## Turn crate, not changed

`horbm/crates/horbm/src/retrieve.rs` `asked` still calls `store.search_lexical`. `horbm/crates/horbm/src/keys.rs` still implements `horbm::Int8Scorer` for `horbm_keys::StringKeyArena` and returns the raw `i32` dot. This PR does not edit either file.

The one call site is `asked`, which `publish_ask` runs for a granted ask. Replace the lexical search with the scan the worker already holds:

```rust
let n = scan.scan_selected_into(text, &mut hit_buf);
```

`hit_buf` is `[horbm_brain::ScanHit; MEMORY_HIT_CAP]`, allocated when the worker starts. `scan` is a `DiaryScan` built when `apply_consent` opens the store and dropped when the grant falls (`*store = None`). That build is setup, not a second scoring call:

```rust
let keys = horbm_brain::StoredKeys::open(path, horbm_brain::KeyPrefault::Populate)?;
let diary = horbm_brain::DiaryArena::from_rows(rows);
let scan = horbm_brain::DiaryScan::open(keys, diary, true, horbm_brain::SCAN_CAP);
```

`rows` are the person-diary `emb`, `emb_scale`, and `emb_norm` columns, with `allow` false on a consent-denied row. Building `MemoryHit` values from `hit_buf[..n]` still allocates the published cell, which `MemoryHit::from_hit` already does. The scan call does not. `ScanPool::Diary` is the memory pool. `ScanPool::Keys` is the knowledge pool. `item_id` comes from `scan.item_id(hit)`. Do not call this from `Heartbeat::step`.

`horbm::Int8Scorer::score` stays `Option<i32>`. The fused `f32` does not need that trait to change. A version-1 arena opened through `StringKeyArena` keeps the old `i32` dot.

## Micro-benchmark

Not a gate. Ignored test `timed_scan_of_one_thousand_rows`. Release profile, 10 warmup queries, then 100 samples. The figure is the minimum of those samples. The test also prints the high median (`samples[50]` of the sorted 100).

One query, `remember` granted, top-k 4. 1,000 key rows of width 32 and 1,000 diary rows of width 256. Every 17th key row and every 19th diary row is consent-denied. The query text is `where is the kettle`.

Host: Linux, `Intel(R) Xeon(R) Processor`, flags include `fma`, `avx2`, `avx512f`, `avx512bw`. rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

| Build | Kernel the test printed | Minimum | High median |
|---|---|---|---|
| default features | `scalar` | 58425 ns (58.425 µs) | 58740 ns (58.740 µs) |
| `--features simd-avx2,simd-avx512` | `avx512` | 23150 ns (23.150 µs) | 24796 ns (24.796 µs) |

The 5 ms figure in R4 is a target. This measurement is not that gate. Nothing in this crate is Linux-only. `Prefault::Populate` stays inside `MappedBytes`, which already shipped that split.

## Tests

No card JSON, no `said_by`, no `claim_type`, no audio file. Key rows come from `StoredPack` or `horbm_keys::KeyPack` in the test. Diary rows are literals or `embed_diary_into`. The counting allocator lives in `tests/scan_alloc.rs`, outside the library, so the library stays `forbid(unsafe_code)`.

Covered:

- `encode_query_into` matches `encode_query_i8` on empty text, `q-lane`, `where is the kettle`, `id-β`, and `İ`, at widths 1, 7, 32, and 256
- the diary hash matches an independent copy of the `974e5944` embedder, including a 64-byte token, a 65-byte token that must be dropped, and `İ`
- identical diary text scores above 0.98, and a shared-token line beats an unrelated line
- the S51 scale example is lift `33` on every kernel, including an unaligned query
- a consent-denied key row and a consent-denied diary row are absent; scalar, selected, AVX2, AVX-512, and NEON requests match by `f32::to_bits` on pool, row, fused score, and native score
- `remember` false drops the diary and does not prepare it; allowed keys remain, with fused scores `1/61`, `1/62`, …
- a one-slot merge keeps the diary's best row
- a blank query skips the diary and still scores keys; a version-1 file returns the allowed row and `item_id` is `None`
- an empty id, a duplicate id, and a short diary vector are rejected; an empty diary loads
- a counting stand-in records dots: a denied row is not dotted, `remember` false does not prepare the diary, a zero-norm query does not dot the diary, and an empty output buffer returns 0
- a diary row with norm 0 is not a hit; the same text with a real norm is
- after `open`, 100 queries in the counting-allocator test allocate nothing, including `id-β`, an empty query, `set_remember`, and the scalar, selected, AVX2, AVX-512, and NEON requests

`cargo test -p horbm-brain --manifest-path horbm/Cargo.toml` passed 24 tests, 1 ignored, on the default build and again with `--features simd-avx2,simd-avx512`. The ignored test is the micro-benchmark. `cargo clippy -p horbm-brain --all-targets --manifest-path horbm/Cargo.toml -- -D warnings` is clean for both builds.

`cargo test --manifest-path horbm/Cargo.toml --workspace --locked` passed 259 tests, 2 ignored, 0 failed. One ignored test was already ignored. The other is this micro-benchmark. `cargo test --workspace --locked` at the repo root passed 1070 tests, 5 ignored, 0 failed. Those 5 were already ignored. `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets --locked -- -D warnings` and `cargo clippy --workspace --all-targets --locked -- -D warnings` were both clean. rustc 1.85.0 on Linux.
