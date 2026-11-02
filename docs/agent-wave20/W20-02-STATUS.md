# W20-02 · Sector 03 Eureka — status refresh

*W20-02 · 2026-11-02 ~12:40 CET (Europe/Zagreb, UTC+1) · local only, nothing pushed.*
Sources read: `w3-briefs/03-EUREKA.md`, `w3-briefs/EUREKA-STATUS.md`, `galion-voice-v4/03-EUREKA-CARRYOVER.md`, `galion-voice-20/eureka/00–04 + INDEX`.

## Verdict
Eureka **insight: DONE**. **Runtime proof: PARTIAL** (stub path only). **Owner open-before §A: 0/11 answered.**

## Done on disk (verified this run)
- `cargo test --workspace --offline`: **247 passed / 0 failed** (was 220 at 00:57 CET; 59 in the 11-01 carryover).
- Harness exists: `specs/presence.json`, `specs/presence-wav.json`, `crates/galion-presence` (spec, replay, backend, tests/replay.rs).
- Rika in code: `galion-emotion/src/rika.rs`; crates `galion-heartbeat`, `galion-mimi`, `galion-sideband` present.
- Locks: `LOCKS.toml` present. (PS/G6 R1–R9/P6 `lean_in_ms=160` results are as reported in EUREKA-STATUS; not re-run individually here.)

## Doc drift (still open — not edited by W20-02)
- `galion-voice-20/eureka/01-HEARTBEAT.md` §"Where it stands" still says "59 tests… harness, presence.json, replay gate, G6 do not exist". Wrong vs disk.
- `03-OPEN-BEFORE.md` §B.2 says harness "Specified in D20; not built" — now built (stub).
- Carryover "Rika: nothing built; G6 not wired" — superseded.

## Open-before checklist
### §A — owner-paced (no answers on disk; proposed defaults only, NOT confirmed)
| # | Item | Proposed default | State |
|---|---|---|---|
| 1 | Rika trigger reading (K18) | As proposed | Owner |
| 2 | Cascade = fallback behind feature (K15) | As proposed | Owner |
| 3 | Cascade runtime on galion-tensor (K16) | As proposed | Owner |
| 4 | Late-frame breath me-band only (K17) | As proposed | Owner |
| 5 | Cross-session Rika memory (E5) | RAM only | Owner |
| 6 | ANTICIPATE bit (E6) | Reuse SENSE+ADAPT | Owner |
| 7 | Side-band exposure (E7) | Internal; lean_in_ms in presence.json | Owner |
| 8 | Soft ack default | Off | Owner |
| 9 | False-trigger budget/refractory | 0 R4, ≤1/min, 8 s | Owner |
| 10 | Rika in cascade lane (E9) | Duplex only v0 | Owner |
| 11 | Origin of name "Rika" | Unknown | Owner |

### §B — gate before coding counts
| # | Item | State |
|---|---|---|
| 1 | Clean V4 docs + Eureka | Done |
| 2 | Harness format / G6 | **Done on disk (stub)** — doc says not built |
| 3 | Scalar oracle + bench rule | Done (rule + oracle) |
| 4 | Weights licence / A21 counsel | Blocked — owner/counsel |
| 5 | LICENSE/NOTICE, copyright line | Owner/counsel |
| 6 | Home of galion-voice-core (C1) | Owner |
| 7 | Live Hermes (C5) / memory (C6) | Owner |
| 8 | Push/CI go-ahead (K4) | Owner (push forbidden this wave) |
| 9 | VPS hardening + probe (R0–R3) | Blocked — owner |
| 10 | GPU budget (H6) | Owner (no training anyway) |
| 11 | Demo mirrors emotion (A14) | Agent-fixable; check A14 owner in ASSIGNMENTS |

## Still missing for the full Eureka *moment* (agent-paced vs blocked)
- Real audio through real Mimi + adapted model — blocked on A21 weights.
- P1–P8 / R1–R9 replay on adapted backend — after the above.
- Real affect head (still RMS/ZCR stub) — agent work, no blocker.
- Reference-VPS latency — blocked on owner VPS.
- Doc-drift fixes above — agent work (A20 scope).

— W20-02
