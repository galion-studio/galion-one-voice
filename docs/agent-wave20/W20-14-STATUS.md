# W20-14 — Soft blocker `.gq` / G5 · STATUS
*2026-11-02 ~12:38 CET (Europe/Zagreb, UTC+1). LOCAL ONLY — no git push / gh / VPS / downloads / real weights / he-IL.*

**State:** 🟡 synthetic writer prototype + G5 gate sketch landed as **tests/scripts only**.
**The weights path is NOT clear.** Real `gq-pack` and real G5 remain **BLOCKED** (A21 counsel sign-off on P-A, HF URIs, upstream sha256, W4 licence, `weights/LICENCES.toml` / `docs/WEIGHTS.md` absent, no weight on disk — see `galion-voice-20/v4/agents/W2-A07-STATUS.md`). Nothing here changes BACKLOG #10's BLOCKED status.

## Read first
- `galion-voice-20/v4/agents/A07-gq-format.md` (spec v1, §6 converter design, §8 "no writer in the runtime crate until a real weight exists").
- `galion-voice-20/v4/agents/W2-A07-STATUS.md` (writer blocked, unblock checklist).
- `crates/galion-tensor/src/gq.rs` (reader: mmap+prefault, per-tensor SHA-256, SHA256SUMS; 8 unit tests).
- `docs/agent-wave20/W20-11-STATUS.md` (G5 dry design — aligned with it below).

## What I did (files)
| File | Change |
|---|---|
| `crates/galion-tensor/tests/gq_synthetic_g5.rs` | **New** integration test (≈430 lines). Runtime crate `src/` untouched — respects A07 §8 lock. Uses only the public reader API. |
| `galion-gates/g5-gq-synthetic.sh` | **New** standalone gate sketch. **Not wired** into `run-gates.sh` (W20-11 / A16 own the runner). Prints `G5 SYNTHETIC-PASS — …` (never plain `PASS`), JSON has `"mode":"synthetic","real_g5":"PENDING"`. |
| `docs/agent-wave20/W20-14-STATUS.md` | This file. |

### Inside `gq_synthetic_g5.rs` (test-only prototype of A07 §6)
- **Synthetic safetensors subset** emitter + tiny parser (F32/BF16 only; rejects other dtypes — "no silent casts", out-of-range/overlapping `data_offsets`, size≠shape·dtype, header > 100 MB).
- **`SynWriter` (`pack_bytes` / `pack_to_file`)**: header placeholder → table → pad → each tensor aligned → back-patched header; per-tensor SHA-256; alignment validated (pow2, 64..65536); unique/valid names; deterministic order (lexicographic, or explicit first-use order list with unlisted names appended lexicographically; unknown order names rejected); atomic `*.gq.tmp` → `fsync` → `rename`.
- **Optional I8 quant** `i8-sym` per-tensor (`scale = amax/127`, round, clamp ±127 — same as `Tensor::to_i8_symmetric`), with `keep_float` substring list (norm/embed stay float). All-zero tensor → scale 1.0 (reader requires > 0).
- **G5 check** (`g5_check`/`g5_run`): pack → `GqFile::open(Prefault::Populate)` → file digest equals writer digest → `verify_all` → count match → dims match → **raw-byte bit-exact** for F32/BF16 (catches −0.0 / NaN payload drift that float compare misses) → I8 within `|x − q·s| ≤ s/2` (+1e-3 slack) → `SHA256SUMS` emitted + `verify_sha256sums` → `DATA_MANIFEST.toml`-style text (marked SYNTHETIC, licence "n/a").

### Tests (all synthetic, seeded, no I/O outside `$TMPDIR`)
`cargo test -p galion-tensor --offline --test gq_synthetic_g5` → **8 passed**:
1. `w20_14_safetensors_subset_roundtrip` — emit/parse identity + 5 rejection cases.
2. `w20_14_writer_output_is_valid_gq_every_alignment` — align 64/128/4096/65536 accepted by reader, offsets aligned, zero-copy `f32_view`; 0/32/96/2¹⁷ rejected.
3. `w20_14_writer_is_deterministic_and_order_aware` — input order irrelevant, byte-identical repacks, first-use order honoured, offsets monotonic.
4. `w20_14_writer_rejects_bad_inputs` — duplicate, empty, >1024-byte names; zero-tensor file valid.
5. `w20_14_i8_quant_within_tolerance_and_keep_float` — dtype selection + tolerance.
6. `w20_14_g5_sketch_end_to_end_on_disk` — full on-disk G5 at align 4096, every prefault mode, tmp renamed away, rerun byte-identical.
7. `w20_14_g5_fails_on_corruption_and_source_drift` — data flip → `verify_all` fail; source drift → bit-exact fail; missing tensor; bad SHA256SUMS.
8. `w20_14_random_synthetic_roundtrip_fuzz` — 60 random F32/BF16 files, rank 0–3, 4 alignments.

Full crate: `cargo test -p galion-tensor --offline` → 60 unit + 8 + 4 + 1 passed, 0 failed, no warnings from the new file. `galion-gates/g5-gq-synthetic.sh` → `G5 SYNTHETIC-PASS`, rc 0 (12:37 CET). New file contains zero legacy-container-format refs (G2 safe).

## G5 round-trip gate — sketch (for when weights clear)
| Step | Real G5 (future) | Synthetic today |
|---|---|---|
| Input | owner-placed `*.safetensors` + upstream sha256 from `weights/LICENCES.toml` | seeded generator → `synthetic.safetensors` |
| 0 | verify source sha256 vs LICENCES.toml (A21) | n/a (synthetic) |
| 1 | `gq-pack --in … --out x.gq --align 4096 [--order order.txt] [--quant i8-sym --keep-f32 'norm\|embed']` | `pack_to_file` |
| 2 | determinism: pack twice → identical sha256 | ✅ tested |
| 3 | `GqFile::open(Populate)`; `verify_all` | ✅ |
| 4 | every tensor: dims equal; F32/BF16 raw bit-exact; I8 ≤ s/2 | ✅ |
| 5 | `SHA256SUMS` + `DATA_MANIFEST.toml` written & verified (`sha256sum -c` compatible) | ✅ (text only, SYNTHETIC-labelled) |
| 6 | negatives: corrupted copy must fail | ✅ |
| 7 (W20-11 (f)) | mmap load, no per-token alloc during `step()` | ❌ not yet — needs A6 Mimi using `f32_view`; candidate for `tests/zero_alloc*.rs` pattern |
| Runner status | `G5 PENDING "gq-pack not present / weights not clear"` until then (per W20-11) | `G5 SYNTHETIC-PASS` standalone only |

## Proposed next steps (not done)
1. When the owner says weights are clear: promote the writer from the test into a `tools`-only bin (`gq-pack`, `gq-inspect`) — **never** into the runtime path; reuse this test's logic verbatim; add a real JSON parser behind a tool-only feature or keep the in-tree subset hardened.
2. W20-11/A16: optionally wire `g5-gq-synthetic.sh` as a soft line `G5(syn)` while keeping real G5 `PENDING`.
3. Add F16→BF16 (documented lossy) path + block-quant dtype codes only with new codes (A07 §6.2/6.3).
4. Hook A6 (W20-15) Mimi synthetic kernels to load from a synthetic `.gq` to cover the "mmap + zero alloc per step" check.

## Not done / guardrails kept
No change to `gq.rs` or any `src/`; no change to `run-gates.sh`, LEDGER, BACKLOG; no git push/gh/PR; no VPS/SSH; no downloads; no real or invented weights; no he-IL. **Do not read this as "weights path clear".**

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
