# Wave-21 · Progress · 2026-11-02 (Europe/Zagreb)

Goal: one-command local live demo by EOD — `scripts/run-live-demo.sh` / `scripts/run-live-demo.sh --showcase`.
Runbook: `docs/LIVE-DEMO.md`. Local only: no push, no VPS, no weights.

| ID | Sector | Deliverable | State |
|---|---|---|---|
| W21-01 | 07 Presence + 04 Rika + demo | `crates/galion-live-demo` + `scripts/run-live-demo.sh` (WAV → 80 ms heartbeat → stereo + capture; `--showcase`) | DONE |
| W21-02 | 01 Stack / A6 Mimi | `MimiPassthrough` synthetic Mimi lane in student-backend / live demo (`--mimi`) | DONE |
| W21-03 | 11 Cascade + ML EN | `scripts/run-en-cascade-demo.sh` EN cascade lane (mock traits; `LIVE_EN_CASCADE=1`) | DONE |
| W21-04 | 06 Render + 08 Console | `scripts/run-dot-console-demo.sh` + `W21-04-ATTACH.md` (one-dot + live console; `--dot` / `LIVE_DOT=1`) | DONE |
| W21-05 | 15 Gates + docs | G2 scrub, `docs/LIVE-DEMO.md`, soft gate LV, plan Next-steps refresh | DONE — see W21-05-STATUS.md |

## Gates (local, offline)
See W21-05-STATUS.md for the final run. R0–R3 stay PENDING (no VPS by rule).

— Maciej Grajczyk
