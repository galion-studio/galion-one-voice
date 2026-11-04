# tools/legacy

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Retired code kept for reference and for parity tests. Nothing here ships and
nothing in the live turn path calls it.

## consent.py.bak

Was `scripts/clone/consent.py` (added in PR #24). The persona floor in
`galion-render` ran `python3 consent.py check` for Samantha and Jarvis each
time it built a floor, and the result rode on every turn's `clone_live`.

Owner decision (Mate), 2026-11-04 02:39 CET: "Do the consent check
only when a voice is cloned, not on every turn." Owner rule: Rust first, no
Python in shipped code.

The check now lives in the Rust crate `crates/galion-consent` (same order,
same reasons, same `consent.txt` / `reference.wav.sha256` / `LOCKS.toml`
formats). It runs when the clone slot is built (`CloneTts::try_new`), when a
clip is generated at rest (`examples/gqvoice.rs`), and when a clip is
registered (`scripts/clone/ingest-clip.sh` → `galion-consent check`). The
persona floor only reads the in-memory verdict.

The Rust gate adds two stricter refusals: `celebrity` (LOCKS "No celebrity
clone"; Armstrong) and `expired` (an ISO `expiry` date in the past).

The file is renamed `.bak` so nothing imports or executes it by accident. The
parity test `crates/galion-consent/tests/parity.rs` copies it into a scratch
tree as `scripts/clone/consent.py` and runs it there with `python3`. Python is
used in that test only.
