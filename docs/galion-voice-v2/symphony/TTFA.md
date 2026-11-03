# TTFA

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Time to first audio is the wait until sound exists. Three clocks stay separate. A header, a JSON byte, or a TTFB is not audio.

| Clock | Start | Stop | What a number means |
|---|---|---|---|
| Flash, this CPU | the clone child accepted one JSON line | the first PCM sample is in the parent buffer | product claim only when `engine` is `model` |
| Virtual user | end of inject | a reply WAV is in hand, plus leading silence | `ttfa_kind` = `loopback-full-utterance` |
| Web / PSTN targets | user speech offset | first non-silent agent audio | desk targets in [80MS.md](80MS.md), not yet this machine |

There is no Chatterbox model named Flash. Upstream `generate()` returns when the utterance is finished. The owner's flash question is the first clock: CPU, this machine, no cloud. The plan page is [FLASH-EVAL.md](../FLASH-EVAL.md).

## Run

Default. CI uses this. The line has `measured: false` and no `ttfa_ms`.

```bash
scripts/eval/flash-ttfa.sh
```

Opt-in pipe clock. `product_claim` stays false. `engine` is `protocol-stub`.

```bash
GALION_FLASH_EVAL=1 GALION_CLONE_STUB_PCM=1 scripts/eval/flash-ttfa.sh
```

Loopback utterance clock, filled only when Piper or flite wrote a WAV:

```bash
scripts/eval/virtual-user.sh
```

The report is `galion-gates/results/presence-eval/latest.json`. With no speech backend, `ttfa_ms` is null and `ttfa_status` is `unmeasured`. Doc: [PRESENCE-EVAL.md](../../eval/PRESENCE-EVAL.md).

A product number is copied into [SELF-EVAL.md](SELF-EVAL.md) when `engine` is `model` and the command, the machine, and the persona are in the evidence. A stub clock can be filed as evidence of the pipe. It is labeled stub.

## Gate

**PASS** when the default harness prints `measured: false` with no `ttfa_ms`, and any number that is filed carries its engine label.

**FAIL** when a stub or an unmeasured run is written up as a product TTFA.

**BLOCKED** when a model engine has not spoken, so the product clock has no sample. The harness itself can still **PASS** on the default line.

— Maciej Grajczyk
# TTFA — live-sector streaming latency

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.


This note is the latency gate for live streaming sectors. A live sector shares the heartbeat. The bar for that sector is one frame: **80 ms**.

`LOCKS.toml` is read by the scripts. This note leaves it unchanged.

## The frame bar

The heartbeat is 80 ms (`[clock] frame_ms = 80`, `frame_hz = 12.5`, `configurable = false`). `galion-heartbeat` publishes the same constant, `FRAME_MS = 80`. Both presence specs set `frame_ms` to 80. Compute inside the frame stays `frame_budget_p95_ms` (60 in the lock file). A late frame is a pause.

`live-sectors.json` names the live sectors. The frame script refuses a live sector whose `frame_bar_ms` is anything other than 80.

| Live sector | What the frame is | Presence must, from the locks |
|---|---|---|
| heartbeat | The clock itself. | — |
| barge-in | Yield, counted in frames. | `barge_in_ms` ≤ 200. P1's should is one frame (≤ 80). |
| onset | Frames from the end of the you-band to the first voiced me-band. | `onset_p50_ms` ≤ 300. |
| e-to-a | Feeling band. An E-in shift to the first E-out change. | `e_to_a_frames` ≤ 2 and `e_to_a_ms` ≤ 160. 160 = 80 × 2. |
| first playable frame | Once a sample exists, the sector may hold it for one frame before it is eligible to play. | Wall-clock `ttfa_ms` ≤ 80, and only on a measured file. |

`outside_bar` in the same file names clocks that have their own harness: PSTN mouth-to-ear, the phone loopback full utterance, the Flash clone clock, and the Piper warmth bench.

## Presence gates

Onset, barge-in, and e-to-a stay on `galion-presence` (`specs/presence.json`, `specs/presence-wav.json`).

| Gate | Cap | Where the cap is written |
|---|---|---|
| onset | p50 ≤ 300 ms | `[gates] onset_p50_max_ms`, and the run-level must in both spec files |
| barge-in | ≤ 200 ms | `[gates] barge_in_max_ms`. P1 and P8 must. |
| e-to-a | ≤ 2 frames and ≤ 160 ms | `affect_to_voice_max_frames`, `affect_to_voice_max_ms`. P3 must. |

The gate fails when a spec target is looser than the lock. A tighter target still passes. The harness already rejects engagement, retention, session length, time in session, dwell, and speech elicited. The scripts reject those names too.

## Stub and measured

**Stub.** `cargo run --offline -q -p galion-presence -- <spec> --json`, the same mock as `galion-gates/ps-presence.sh`. The figures are frame counts times 80. They are reproducible, labelled `kind: stub`, `measured: false`. The default mock is `yield_frames = 1`, `endpoint_frames = 3`, `affect_lag_frames = 1`. On the shipped specs the mock reports barge-in 80 ms and onset p50 240 ms. E-to-a is 1 frame (80 ms) on the frame-scripted file (P3). The wav specs carry onset and barge-in and do not script an affect shift. Digests: presence `9f657f360c565dbf`, wav `db7541ec9f6f42c7`. A stub receipt has no `ttfa_ms` field. A product time in `LEDGER.md` comes from a measured file that passed this gate.

**Measured.** Set `GALION_TTFA_MEASURED` to a JSON file:

```json
{
  "schema": "galion-ttfa-measured/v1",
  "kind": "measured",
  "frame_ms": 80,
  "host": "<this machine>",
  "engine": "<live-sector engine>",
  "ttfa_ms": 0,
  "onset_p50_ms": 0,
  "barge_in_ms": 0,
  "e_to_a_ms": 0,
  "e_to_a_frames": 0
}

The zeros are the shape. A real file carries the clocks from a run on `host`. The gate accepts the file when `frame_ms` is 80, `kind` is `measured`, `host` is set, `engine` names a live engine, `ttfa_ms` is at most 80, and the three presence clocks sit inside the lock caps. Engines named `mock`, `stub`, `protocol-stub`, or `unwired` are refused. The stub replay still runs beside the file, so a measured file cannot hide a red mock.

No measured file is shipped in this tree. Until one is passed in, the receipt says the wall-clock is absent.


docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh
docs/galion-voice-v2/symphony/scripts/ttfa-frame.sh
docs/galion-voice-v2/symphony/scripts/ttfa-presence.sh
docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --self-check
docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --write-eval

Exit `0` is pass, `1` is a red gate, `2` is usage or IO. Stdout is one JSON object, schema `galion-ttfa-gate/v1`. `--self-check` builds temporary measured files: an over-budget clock, a wrong frame, a stub engine, and a forbidden metric name each fail, and a file sitting on the 80 ms bar passes. `--write-eval` refreshes the receipt in `SELF-EVAL.md` and `receipts/ttfa-stub.json`. It refuses to run while `GALION_TTFA_MEASURED` is set, so a host clock stays out of the committed stub record.

## Receipt

A passing stub run is recorded in [`SELF-EVAL.md`](SELF-EVAL.md). Section G is complete for the stub path. The measured path stays open until a file is supplied.

2026-11-03 · Europe/Zagreb
