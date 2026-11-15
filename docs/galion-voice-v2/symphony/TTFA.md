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

## This session

2026-11-15, Europe/Zagreb (+01:00). Swarm 10 lane B, worker ttfa. Notes: [ttfa.md](../../swarm-10/2026-11-15/latency-harness/ttfa.md). The lane plan is draft PR #151 (`docs/swarm-10/2026-11-15/latency-harness/PLAN.md` on `cursor/latency-harness-plan-68d6`). This change does not edit that file.

The original-ref Nano product clock stays **BLOCKED** on A0 and A1. That measurement belongs to worker B3. Nothing in this session is `chatterbox-nano`, a clone of the original Samantha or Jarvis references, Flite, or an 80 ms first-playable-frame pass. `product_claim: true` below is Piper `en_US-ljspeech-medium` only. These clocks are not copied into `SELF-EVAL.md`. No `galion-ttfa-measured/v1` file is added. No WAV is committed.

### Cited Piper receipt

Host `grok-bot-vm-511777615`. Commit `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`. Source: the local untracked baseline `NEXT-ITERATION.md` §4.3–§4.5 (written 2026-11-15 21:56 +01:00 on that host). That file is not in git. This worker did not re-run the commands on that host.

`scripts/eval/piper-stream-ttfa.sh`, 21:53 +01:00, exit 0, wall 8.579 s. The drain WAV stayed at `/tmp/voice-next-baseline/piper-ttfa.wav` (161324 bytes) and is not in the repo.

```text
engine=piper voice=en_US-ljspeech-medium pack=present sha256=pinned
runtime=weights/runtime/piper/piper cpu_only=true measured=true stub=false product_claim=true
SpeechStack cold_first_pcm_ms=398 warm_first_pcm_ms=20 worker_reused=true piper_spawns=1
raw_spawn cold_spawn_first_pcm_byte_ms=363.8 warm_line_first_pcm_byte_ms=23.8
drain in_rate=22050 out_rate=24000 frame_samples=1920 tick_ms=80 frames=42 late_ticks=0 late_policy=pause audio_ms=3343.7
```

`scripts/eval/virtual-user.sh`, 21:55 +01:00. Verdict green, 15/15. Report `galion-gates/results/presence-eval/20261115T205505Z.json` (gitignored). `asr_mode=interim` (not recognition). `llm=canned`. `speech_backend=a21-weights` (Piper `en_US-ljspeech-medium`). `ttfa_ms=78`, `ttfa_status=measured`, `ttfa_kind=loopback-full-utterance`. That is the wall from the end of inject until the reply WAV, plus leading silence. It is not a streaming first frame and not a clone clock. PSTN was not run. This worker did not re-run the loopback. On host `cursor` the loopback is **not measured**.

Default `scripts/eval/flash-ttfa.sh` on that host: exit 0 in 0.465 s.

```text
{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}
```

### This worker, host `cursor`

Same commit `8980e1a`. 2026-11-15 22:06 +01:00. rustc 1.85.0 (the image default was 1.83.0; the 1.85 toolchain was installed before the harness result below). 4 CPUs. The A21 pack was absent, then `scripts/a21/fetch-a21.sh` (exit 0) wrote it under gitignored `weights/`. Nothing from that fetch is in git.

`scripts/eval/piper-stream-ttfa.sh --wav /tmp/swarm10-b1-piper-ttfa.wav` at 22:06:10 +0100. Exit 0. Wall 5.290 s. Stdout, one JSON object, engine `piper`, `stub` false, `product_claim` true for this Piper voice only:

```text
{"engine": "piper", "voice": "en_US-ljspeech-medium", "pack": "present", "sha256": "pinned", "runtime": "weights/runtime/piper/piper", "cpu_only": true, "measured": true, "stub": false, "product_claim": true, "speechstack": {"pack": "present", "measured": true, "voice": "en_US-ljspeech-medium", "cold_first_pcm_ms": 339, "warm_first_pcm_ms": 21, "piper_pid": 4282, "piper_spawns": 1, "worker_reused": true, "pcm_cache": "none", "phrase": "Hi! This clause is the warm path check.", "clock": "speak entry until first clause PCM"}, "cold_first_pcm_ms": 339, "warm_first_pcm_ms": 21, "raw_spawn": {"cold_spawn_first_pcm_byte_ms": 350.4, "warm_line_first_pcm_byte_ms": 18.5, "cold_pcm_bytes": 147968, "warm_pcm_bytes": 119296, "clock": "spawn (cold) / stdin write (warm) until first stdout byte"}, "cold_spawn_first_pcm_byte_ms": 350.4, "warm_source": "speechstack", "drain": {"in_rate": 22050, "out_rate": 24000, "frame_samples": 1920, "tick_ms": 80, "frames": 42, "late_ticks": 0, "late_policy": "pause", "audio_ms": 3355.2, "ticks": "pppppppppppppppppppppppppppppppppppppppppp"}, "phrase_cold": "Streaming check. Galion Voice on Piper CPU.", "phrase_warm": "Hi! This clause is the warm path check.", "wav": "/tmp/swarm10-b1-piper-ttfa.wav"}
```

SpeechStack cold 339 ms / warm 21 ms. Raw spawn first PCM byte 350.4 ms / warm line 18.5 ms. Drain 42 frames, 0 late ticks, late=pause, audio 3355.2 ms. The `/tmp` WAV was 161324 bytes and was not committed. These figures sit beside the cited 398 / 20 / 0 late ticks. They do not replace them. They are still Piper.

Default `scripts/eval/flash-ttfa.sh` on rustc 1.85.0: exit 0.

```text
{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}
```

No `ttfa_ms`. A protocol-stub clock was not armed.

### Earlier session (galion-laptop, 2026-11-06)

Kept. Not re-run. No model engine ran. Product TTFA on that laptop was **BLOCKED**. `scripts/eval/flash-ttfa.sh` exited 2 (`cc` missing) and printed:

```text
{"measured":false,"product_claim":false,"engine":null,"note":"flash example did not run; no model engine ran; no TTFA number"}
```

| Command | Result | What the command printed |
|---|---|---|
| `scripts/eval/flash-ttfa.sh` | BLOCKED | The line above. Exit 2. |
| `scripts/eval/piper-stream-ttfa.sh` | BLOCKED | `{"engine": "piper", "voice": "en_US-ljspeech-medium", "measured": false, "stub": false, "product_claim": false, "pack": "absent", "reason": "A21 pack missing (run scripts/a21/fetch-a21.sh)"}`. Exit 2. |
| `scripts/eval/clone-stream-ttfa.sh` | BLOCKED | `{"engine":null,"gate":"BLOCKED","hard_pass":false,"measured":false,"model_measured":false,"note":"Piper pack absent. No model engine ran. product_claim is false.","piper_measured":false,"product_claim":false,"stub":false}`. Exit 2. |
| `python3 scripts/eval/piper_lat240_bench.py` | BLOCKED | `{"engine":"piper","measured":false,"product_claim":false,"reason":"Piper runtime or voice pack is not on this machine; no TTFA number"}`. Exit 2. |
| `python3 scripts/eval/piper_ort_threads.py` | BLOCKED | `{"engine":"piper","measured":false,"product_claim":false,"reason":"Piper runtime or voice pack is not on this machine; no TTFA number"}`. Exit 2. |
| `scripts/eval/v2-hard-loop.sh --self-check` | PASS | Exit 0. Classifiers held. The script printed `HARD-PASS of the loop: refused`. No product TTFA. |

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

No `galion-ttfa-measured/v1` file is shipped in this tree. Until one is passed in, the receipt says the wall-clock is absent. The 2026-11-15 Piper clocks in the session above are not that file. They do not set `ttfa_ms` to 80, and they do not close the first-playable-frame bar. That bar waits on a measured Nano file.


docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh
docs/galion-voice-v2/symphony/scripts/ttfa-frame.sh
docs/galion-voice-v2/symphony/scripts/ttfa-presence.sh
docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --self-check
docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --write-eval

Exit `0` is pass, `1` is a red gate, `2` is usage or IO. Stdout is one JSON object, schema `galion-ttfa-gate/v1`. `--self-check` builds temporary measured files: an over-budget clock, a wrong frame, a stub engine, and a forbidden metric name each fail, and a file sitting on the 80 ms bar passes. `--write-eval` refreshes the receipt in `SELF-EVAL.md` and `receipts/ttfa-stub.json`. It refuses to run while `GALION_TTFA_MEASURED` is set, so a host clock stays out of the committed stub record.

## Receipt

A passing stub run is recorded in [`SELF-EVAL.md`](SELF-EVAL.md). Section G is complete for the stub path. The measured path stays open until a file is supplied.

2026-11-03 · Europe/Zagreb

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
