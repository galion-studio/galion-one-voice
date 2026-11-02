# W20-09 — Sector 10 Hermes side-band — STATUS

*2026-11-02, ~12:40 CET (Europe/Zagreb). LOCAL ONLY — no push, no VPS, no weights.*

## Done
- `crates/galion-sideband/src/client.rs`: non-blocking side-band client stub, zero deps, std only.
  - `HermesTransport` trait (C5 gateway + wire format stay open; plug in later).
  - `NullTransport`: offline, answers lookups with `NoKnowledge::Unreachable` (honest-filler path).
  - `SidebandClient::spawn(transport, queue)`: all Hermes I/O runs on a `hermes-sideband` side thread.
  - `submit()` = bounded `try_send` (full queue → `QueueFull`, dropped, never waits).
  - `poll(turn)` = `try_recv` only; returns the reply, or `NoKnowledge::Timeout` past `RETRIEVAL_CAP` (3 s); late replies are dropped.
- `tests/client.rs`: 3 tests (honest Unreachable; 5× submit+poll against a 200 ms transport finish < 80 ms; unknown turn → None).
- `cargo test -p galion-sideband`: 4/4 pass (3 new + 1 existing).

## Hot-path guarantee
Hermes is never on the 80 ms path: the hot loop only calls the two non-blocking methods; the transport may block freely on its own thread.

## Open / blockers
- C5 gateway, C6 memory (owner) → real transport not written.
- Wire format (serde) still unchosen; the crate stays zero-dep until it is.
- No test for the 3 s timeout path (it would add 3 s to CI); a test hook for injectable clock is a follow-up.
- Not wired into the heartbeat/engine yet.
