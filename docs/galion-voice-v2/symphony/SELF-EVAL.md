# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

A section is complete when its gate passes and the receipt below matches the script.
`LOCKS.toml` is read by the gate and left unchanged.

## G · Latency / TTFA

Gate: `docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh`.
Detail: [`TTFA.md`](TTFA.md).

The stub block is the mock presence replay (onset, barge-in, e-to-a) on an 80 ms frame. `measured.present` is false until a measured file is passed on a later run. `--write-eval` refuses while `GALION_TTFA_MEASURED` is set, so a host clock cannot overwrite this record.

<!-- TTFA-RECEIPT:BEGIN -->
```json
{
  "frame_bar": "pass",
  "frame_ms": 80,
  "kind": "stub",
  "locks_edited": false,
  "measured": {
    "present": false
  },
  "owner": "Maciej Grajczyk",
  "program": "Galion Voice",
  "schema": "galion-ttfa-gate/v1",
  "stub": {
    "backend": "mock",
    "barge_in": "pass",
    "barge_in_ms": 80,
    "barge_in_one_frame_should": "pass",
    "e_to_a": "pass",
    "e_to_a_frames": 1,
    "e_to_a_ms": 80,
    "frame_ms": 80,
    "kind": "stub",
    "measured": false,
    "onset": "pass",
    "onset_p50_ms": 240,
    "runs": [
      {
        "backend": "mock",
        "barge_in": "pass",
        "barge_in_ms": 80,
        "barge_in_one_frame_should": "pass",
        "digest_fnv1a64": "9f657f360c565dbf",
        "e_to_a": "pass",
        "e_to_a_frames": 1,
        "e_to_a_ms": 80,
        "frame_ms": 80,
        "kind": "stub",
        "measured": false,
        "onset": "pass",
        "onset_p50_ms": 240,
        "spec": "presence",
        "verdict": "green"
      },
      {
        "backend": "mock",
        "barge_in": "pass",
        "barge_in_ms": 80,
        "barge_in_one_frame_should": "pass",
        "digest_fnv1a64": "db7541ec9f6f42c7",
        "e_to_a": "absent",
        "e_to_a_frames": null,
        "e_to_a_ms": null,
        "frame_ms": 80,
        "kind": "stub",
        "measured": false,
        "onset": "pass",
        "onset_p50_ms": 240,
        "spec": "presence-wav",
        "verdict": "green"
      }
    ]
  },
  "verdict": "pass"
}
```
<!-- TTFA-RECEIPT:END -->

— Maciej Grajczyk
2026-11-03 · Europe/Zagreb
