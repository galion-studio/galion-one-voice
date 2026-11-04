# W20-06 — Sector 07 Presence — STATUS

2026-11-02 ~12:40 CET (Europe/Zagreb, UTC+1). LOCAL ONLY: nothing pushed, no gh, no VPS, no weights.

## Done
- **P6 rename** (a brief "Open" item): `P6-rika-lean-in-placeholder` → `P6-rika-lean-in`. The description now says lean_in_ms is null on the mock until A10 Rika lands. The assertions and numbers are unchanged.
- **Golden digest updated**, because the spec id is part of the report: `9bbf9c77853f9737` → `9f657f360c565dbf`. The new value is in `crates/galion-presence/tests/replay.rs`, `galion-gates/ps-presence.sh` and `w3-briefs/07-PRESENCE.md` (the README listed no P1–P6 digest). The WAV digest `db7541ec9f6f42c7` is unchanged.

## Verified
- `cargo test --offline -p galion-presence`: all green (4+13+4+1 tests, 1 ignored = fixture regen).
- `cargo test --offline -p galion-heartbeat`: green (11 tests).
- `galion-heartbeat-presence`: 6/6 PASS, verdict GREEN, P6 lean_in=160 ms.
- Determinism: two CPU runs give the same digest. The report files differ only in `timing.elapsed_ms`, which is wall-clock time and by design not part of the digest.

## Still open
- Single CLI for mock vs heartbeat (circular dep): not addressed.
- Real-audio ReplayBackend: blocked on Mimi kernels and weights.
- PS should become a hard gate once a real backend exists.
- Option for later: add a `--no-timing` flag so whole report files are byte-identical.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
