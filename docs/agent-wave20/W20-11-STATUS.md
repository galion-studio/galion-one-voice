# W20-11 — Sector 15 Gates/CI status
*2026-11-02 ~12:36 CET (Europe/Zagreb, UTC+1). LOCAL ONLY — no push/gh/SSH; GALION_VPS_HOST unset.*

## Local gate dry run (12:35 CET)
`galion-gates/run-gates.sh` → `results/20261102T123534+0100.json`, fails=0.
- G0 PASS (bandwidth CV 3.86% <5%, no warn this run), G1 PASS (247 tests, up from 220), G2 PASS, G3 PASS (65 crates clean)
- G4 PASS — snapshot == golden `618a6b26…c21dd`, deterministic, present() p99 = 9498 ns (<1 ms)
- G6 PASS, PS PASS, LK PASS, DG PASS (demo golden `63133d73…c8a7` is now wired into runner — brief's "demo golden not wired" item is closed)
- R0–R3 PENDING (GALION_VPS_HOST not set — intentional)

## G4 console snapshot prep
- Golden is stable across runs; no re-pin needed. Re-pin only when galion-console/galion-render UI changes intentionally (via `g4-render.sh`, review the diff, commit the golden later after the owner says to merge).
- Proposed additions (not implemented): (1) a second snapshot at a narrow 80x24 terminal size; (2) a p99 budget that warns at 100 µs (currently ~9.5 µs) to catch regressions well before the 1 ms hard limit.

## G5 dry design notes (.gq pack) — not wired, still blocked on A7 writer / `gq-pack`
- Input: `.gq` artifact + manifest (sha256, tensor table, quant scheme, version).
- Checks: (a) magic/version header; (b) the manifest sha256 matches the file; (c) tensor table offsets are aligned and in bounds, with no overlap; (d) round-trip dequant vs scalar ref within tolerance on sampled blocks (reuse G1 ref ops); (e) zero legacy-container-format usage, .gq only (consistent with G2); (f) loads with mmap and no per-token alloc.
- Status if missing: `G5 PENDING "gq-pack not present"` (soft, same pattern as R-gates), so it never turns the run red before the writer exists.
- Uses only published lab resources / synthetic tensors. No weights and no training.

## Remaining open
Zero-alloc gate wrapper; clippy not installed; CI push (K4, owner); R0–R3 (VPS); G5 (A7).

No files changed except this status doc.
