# W21-05 · Sector 15 Gates + docs · STATUS: DONE
*2026-11-02 14:27 (Europe/Zagreb, UTC+1). Local only, no push, no VPS.*

## G2 scrub (forbidden legacy-format string → ".gq only" / "legacy container formats")
- docs/agent-wave20/W20-10-STATUS.md, W20-11-STATUS.md, W20-14-STATUS.md
- docs/cascade/EN-CASCADE-LANE.md, docs/multilingual/PROTOTYPE.md
- crates/galion-cascade/src/lib.rs (2 doc comments + `MockLlm::name()` string; no test asserts on it), crates/galion-cascade/Cargo.toml (description)
- Deny-list fixtures untouched: G2 already excludes `galion-gates/` and `scripts/check-no-forbidden.sh`, so the intentional DENY strings in gate scripts remain. Result: **G2 PASS, 0 hits, 0 files**.

## Runbook
`docs/LIVE-DEMO.md` — `scripts/run-live-demo.sh` / `--showcase`, flags + env (LIVE_REALTIME, LIVE_EN_CASCADE, LIVE_DOT, LIVE_OUT), companion scripts (run-en-cascade-demo.sh, run-dot-console-demo.sh), artifacts, honest limits, gates, troubleshooting.

## Soft gate LV
`galion-gates/lv-live-demo.sh`, wired in `run-gates.sh` after DG (+ JSON + summary). Runs `run-live-demo.sh --showcase` into a temp dir with DISPLAY/WAYLAND_DISPLAY cleared and `GALION_HEADLESS=1`, timeout `GALION_LV_TIMEOUT` (180 s), checks stereo WAV + capture for default/opener/workplace. SKIP if script absent; counts only with `GALION_GATES_HARD=1`; `GALION_GATES_NO_LV=1` skips. Never needs a display.

## Plans refreshed (Next steps)
/workspace/galion-voice-20/v4/plans/06, 07, 08, 11, 15.

## Final local gate run (galion-gates/results/20261102T142657+0100.json)
G0 PASS · G1 PASS (325 passed, 0 failed) · G2 PASS · G3 PASS · soft G4 PASS · G6 PASS · PS PASS · LK PASS · DG PASS · LV PASS · R0–R3 PENDING (no VPS). Exit 0.

Note: two earlier runs went red only because other agents were writing files at the time: G1 (W21-01/02 sources were half-written) and LV (run-live-demo.sh changed while it was running). Both pass now.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
