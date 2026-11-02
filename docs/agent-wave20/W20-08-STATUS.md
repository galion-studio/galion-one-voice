# W20-08 — Sector 09 Student adaptation (no training) — STATUS

*2026-11-02, ~12:40 CET (Europe/Zagreb, UTC+1). Local only: no git push, no gh, no VPS, no weights, no training.*

## Done
- **New crate `crates/galion-student-backend`** (zero external deps, depends on `galion-presence`), added to workspace members.
  - `ADAPT_MANIFEST`: Mimi (codec), Moshiko (dialogue, `.gq`), tokenizer, SenseVoice-Small (inbound SER adapter). All `sha256 = None`, licences UNKNOWN/pending A21 — nothing invented.
  - `load_slot(dir, manifest, counsel_cleared)`: load-path stub. Order: A21 gate → file present → sha pinned. Today returns `CounselPending` / `NotPlaced`. No download, no generation.
  - `StudentBackend::stub()` implements `ReplayBackend` (`name = "student-stub"`): silent me-band, `AffectOut::NEUTRAL_WARM`, zero-alloc step.
  - 4 unit tests, `cargo test --offline -p galion-student-backend` → 4/4 pass.
- **`galion-student/configs/adapt-manifest.toml`** (`galion.adapt.v1`): mirrors the manifest, int8 PTQ only, weights dir outside repo, PersonaPlex = eval data only, H6 + S0/S1 KD parked.
- `openmind-slot.toml` `[abi].student_crate` comment updated (proposed → created).

## Not done / next
- Hash verification + `.gq` mmap via `galion-tensor` once real files exist.
- Run the P1–P8 / R1–R9 replay against `StudentBackend`; record failures as targets (not training jobs). Needs a runner hook in `galion-presence` (owner of that crate should wire it).
- Mimi adapter path (real `step`) — blocked on weights on disk.

## Blockers
- A21 counsel sign-off; owner places weights + records sha256.
- R0 VPS probe for 7B-class sizing/RTF.
- C1 real `VoiceBackend` trait (oom crate not on box).

## Notes
- Workspace `Cargo.toml` was concurrently edited by another agent (added `galion-cascade`); my member was appended after it — check on merge.
