# W20-03 — Sector 04 Rika · local next-steps status

*2026-11-02 ~12:40 CET (Europe/Zagreb, UTC+1) · LOCAL ONLY — no push/gh/VPS/weights/training.*

## Inputs read
- `/workspace/w3-briefs/04-RIKA.md`, `/workspace/galion-voice-v4/research/D20-RIKA-MOMENT.md`
- `crates/galion-heartbeat/` (lib.rs, presence.rs, tests/heartbeat.rs, presence_p6.rs, zero_alloc.rs)
- `galion-emotion/src/rika.rs` (note: emotion crate is at repo root `galion-emotion/`, not `crates/`)

## Gate check (run locally, `cargo test --offline`)
| Crate | Result |
|---|---|
| galion-emotion | 72 unit + 2 doc/integration PASS, 0 fail |
| galion-heartbeat | 11 + 2 + 1 + 2 PASS, 0 fail (Rika off-by-default, SENSE+ADAPT gating, no-session-no-Rika, late frame, LOCKS `rika_land_max_ms = 160`, `rika_yield_max_frames = 1`) |
R1–R9 fixtures all referenced in `rika.rs` tests. **Code is green → no code changes made.**

## Gaps noted (small, not fixed — need owner/other lanes)
1. No dedicated Rika fixture file in `galion-emotion/fixtures/` (only `affect-ref-synthetic.txt`); R1–R9 are inline in tests. Optional future: export canned R1–R9 frame streams for A14 demo/presence replay.
2. `galion-emotion` has no `tests/` integration dir; heartbeat tests cover integration.
3. D20 header path says `/workspace/galion-voice-20/…`; canonical copy now lives at `galion-voice-v4/research/`. Doc-only drift.

## Next steps (unchanged, blocked externally)
- Owner: trigger reading (K18 / Q-RIKA Q3, Q9), soft-ack default, false-trigger budget, ANTICIPATE bit, name origin.
- A14: Rika demo opener + re-pin.
- A21: SenseVoice-Small licence before real affect head (keep RMS/ZCR stub).
- VPS R0–R3: `rika_cpu_us_per_frame_p95` (all latency numbers remain `measured-stub`).

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
