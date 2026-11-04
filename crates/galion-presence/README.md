# galion-presence (A13)

Presence harness for Galion Voice. Harness before model.

```
cargo run --offline -p galion-presence -- specs/presence.json [--out report.json] [--json]
```

Exit codes: `0` green, `1` red (a `must` failed or the 5-minute budget ran out), `2` invalid spec or IO error.

- Schema: `specs/presence.json` (`galion-presence/v1`). Each spec has `must` (blocking) and `should` (advisory) assertions.
- Metrics: `barge_in_ms`, `onset_ms`, run-level `onset_p50_ms` and `onset_p95_ms`, `e_to_a_frames`/`e_to_a_ms`, `attunement`, `affect_out_changes`, `me_onsets_while_user_speaking`, `late_frames`, and `lean_in_ms` (Rika placeholder, reported as `pending` until A10).
- There is no engagement metric. The validator rejects `engagement`, `retention`, `session_length`, `time_in_session`, `dwell` and `speech_elicited`.
- Backend: `MockBackend`, which is deterministic, does not allocate in `step()`, and needs no weights. The student plugs in later as another `ReplayBackend`.
- The report is byte-identical for the same input. `digest_fnv1a64` is a non-cryptographic determinism fingerprint, and wall-clock timing is left out of it.

## WAV/PCM input (`you_wav`)

A spec may drive the you-band from audio instead of frame-scripted `you` segments (the two are mutually exclusive):

```json
"you_wav": "fixtures/p7-turn-mono-8k16.wav"
"you_wav": {"path": "fixtures/p8-barge-stereo-8k16.wav", "speech_dbfs": -35, "breath_dbfs": -55}
```

- Paths resolve against the spec file's directory (the CLI uses the spec file's directory; `spec::parse_presence_at(src, base)` in code).
- Format: RIFF/WAVE integer PCM (format tag 1, or 0xFFFE EXTENSIBLE with the PCM sub-format), mono or stereo (stereo is downmixed as (L+R)/2), 8-bit unsigned or 16/24/32-bit signed, 8–192 kHz. The rate must give whole 80 ms frames. Float, compressed, >2 channels and RF64 are rejected with an error.
- Each 80 ms frame is classified by RMS dBFS: `speech` >= `speech_dbfs` (default -35), `breath` >= `breath_dbfs` (default -55), otherwise `silence`. A trailing partial frame still counts as a frame.
- `frames` is optional with `you_wav`. If you omit it, the audio length is used. If you give it, short audio is padded with silence and long audio is cut.
- The states are run-length encoded into ordinary `you` segments, so the replay, the metrics and `MockBackend` don't change.
- Fixtures: `specs/presence-wav.json` (P7 mono turn onset, P8 stereo barge-in) plus `specs/fixtures/*.wav` (synthetic 220 Hz sine + seeded LCG noise). To regenerate: `cargo test --offline -p galion-presence --test wav -- --ignored regenerate_fixtures`. WAV golden digest on the mock: `db7541ec9f6f42c7`.

## Gate

`galion-gates/ps-presence.sh` runs as soft gate **PS** in `run-gates.sh`, after G6. It runs both spec files and writes `results/<ts>.presence.json` and `results/<ts>.presence-wav.json`. Exit codes: 0 PASS, 1 FAIL, 3 SKIP. With `GALION_GATES_HARD=1`, a failure counts toward the exit code.

Perpetual Open MIT. © 2026 Maciej Grajczyk.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
