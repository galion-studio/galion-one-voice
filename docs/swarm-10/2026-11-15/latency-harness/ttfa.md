# Swarm 10 — latency-harness — ttfa

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-15 22:07 +01:00.

The spelling is **Galion**. The product is Galion Voice.

| | |
|---|---|
| Role | B1 ttfa. Product streaming TTFA harness. Piper receipts only. |
| Lane plan | Draft PR [#151](https://github.com/galion-studio/galion-one-voice/pull/151), branch `cursor/latency-harness-plan-68d6`, commit `36b1e8b`. This PR does not edit `PLAN.md`. |
| Branch | `cursor/latency-harness-ttfa-d251`, from `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0`. |
| Worker host | `cursor`. Linux x86_64. `nproc` = 4. Not `grok-bot-vm-511777615`. |
| Rust | Image default rustc 1.83.0. Installed `1.85.0` (`4d91de4e4 2025-02-17`) with `rustup toolchain install 1.85.0 --profile minimal` and `RUSTUP_TOOLCHAIN=1.85.0` before the harness result that counts. |

## What was checked

Lane plan B1. [TTFA.md](../../../galion-voice-v2/symphony/TTFA.md), [PIPER-STREAM-TTFA.md](../../../galion-voice-v2/PIPER-STREAM-TTFA.md), [FLASH-EVAL.md](../../../galion-voice-v2/FLASH-EVAL.md), `scripts/eval/flash-ttfa.sh`, `scripts/eval/piper-stream-ttfa.sh`, `scripts/eval/piper_stream_ttfa.py`. The cited numbers are from the local untracked baseline `NEXT-ITERATION.md` §4.3–§4.5 (host `grok-bot-vm-511777615`, 2026-11-15 21:56 +01:00, commit `8980e1a`). That file is not committed.

`STREAM-TTFA.md`, `SELF-EVAL.md`, `80MS.md`, and `ONE-PERCENT.md` were not edited. Nano first-PCM stays with B3. The heartbeat verdict stays with B2. p99 stays with B4.

## Cited Piper receipt (not measured on this host)

Not re-run on `grok-bot-vm-511777615`. Filed as Piper `en_US-ljspeech-medium` only. Not a clone of the original Samantha or Jarvis references. Not Flite. Not `chatterbox-nano`. Not an 80 ms first-playable-frame claim.

`scripts/eval/piper-stream-ttfa.sh`, 21:53 +01:00, exit 0, wall 8.579 s. Drain WAV `/tmp/voice-next-baseline/piper-ttfa.wav` (161324 bytes), not in git.

```text
engine=piper voice=en_US-ljspeech-medium pack=present sha256=pinned
runtime=weights/runtime/piper/piper cpu_only=true measured=true stub=false product_claim=true
SpeechStack cold_first_pcm_ms=398 warm_first_pcm_ms=20 worker_reused=true piper_spawns=1
raw_spawn cold_spawn_first_pcm_byte_ms=363.8 warm_line_first_pcm_byte_ms=23.8
drain in_rate=22050 out_rate=24000 frame_samples=1920 tick_ms=80 frames=42 late_ticks=0 late_policy=pause audio_ms=3343.7
```

`product_claim=true` in that block is the Piper voice only.

`scripts/eval/virtual-user.sh`, 21:55 +01:00. Verdict green, 15/15. Report `galion-gates/results/presence-eval/20261115T205505Z.json` (gitignored). `asr_mode=interim`. `llm=canned`. `speech_backend=a21-weights`. `ttfa_ms=78`, `ttfa_status=measured`, `ttfa_kind=loopback-full-utterance`. Canned plus Piper. End of inject until a reply WAV, plus leading silence. Not a streaming first frame. Not a clone clock. PSTN not run.

Default `scripts/eval/flash-ttfa.sh` on that host: exit 0 in 0.465 s. `{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}`. No `ttfa_ms`.

The raw `piper-stream-ttfa` JSON object from that host was not in this worker's tree. The block above is the baseline's text log, not a reconstructed script stdout.

## What changed

- `docs/galion-voice-v2/symphony/TTFA.md` — the stale "this session" block now files the cited Piper receipt and this host's Piper remeasure. The 2026-11-06 laptop table is kept under its own date. The live-sector note states that no `galion-ttfa-measured/v1` file is shipped and that these clocks do not close the 80 ms first-playable frame.
- This notes file.

`LOCKS.toml`, `SOUL.md`, `config.yaml`, `REFS.sha256`, voice TOMLs, `SELF-EVAL.md`, and `PLAN.md` were not edited. No WAV, weight, token, or server address was committed.

## Tests and commands on host `cursor`

`scripts/check-locks.sh`: `OK: all Eureka locks hold (LOCKS.toml)`. Exit 0.

`scripts/eval/flash-ttfa.sh` on the image rustc 1.83.0, before the 1.85 install. Exit 2. The compiler refused the example (`galion-ml-voice` requires rustc 1.85). Printed:

```text
{"measured":false,"product_claim":false,"engine":null,"note":"flash example did not run; no model engine ran; no TTFA number"}
```

That line is the missing-compiler path. It is not the default harness result.

`scripts/eval/piper-stream-ttfa.sh` before any fetch. Exit 2. Pack absent. **Not measured.**

```text
{"engine": "piper", "voice": "en_US-ljspeech-medium", "measured": false, "stub": false, "product_claim": false, "pack": "absent", "reason": "A21 pack missing (run scripts/a21/fetch-a21.sh)"}
```

`scripts/eval/clone-stream-ttfa.sh` once, still before the fetch. Combined stdout and stderr, exit 2. The stub clock was rejected (`gate` `FAIL`, `stub_clock` `FAIL`, `product_claim` false, no `ttfa_ms`). The pack-absent line was:

```text
{"engine":null,"gate":"BLOCKED","hard_pass":false,"measured":false,"model_measured":false,"note":"Piper pack absent. No model engine ran. product_claim is false.","piper_measured":false,"product_claim":false,"stub":false}
```

Not re-run after the fetch. Not a Nano result. A 15 ms stub was not treated as a pass.

`scripts/a21/fetch-a21.sh` at 2026-11-15 21:06:02Z–21:06:06Z (22:06 +01:00). Exit 0. Public URLs only (Hugging Face piper-voices, GitHub rhasspy/piper 2023.11.14-2). sha256 pins in the script matched. Files stayed under `weights/`, which `weights/.gitignore` ignores (`git check-ignore` matched both the onnx and `weights/runtime/piper/piper`). Smoke line: `smoke: 52340 B raw PCM in 369 ms (CPU)`. Then `A21=PRESENT (Piper VITS en_US-ljspeech-medium, CPU)`. The smoke raw file was removed by the script. `git status --short` was empty after the fetch.

`scripts/eval/flash-ttfa.sh` with rustc 1.85.0. Exit 0. No `ttfa_ms`.

```text
{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}
```

`scripts/eval/piper-stream-ttfa.sh --wav /tmp/swarm10-b1-piper-ttfa.wav` with rustc 1.85.0. Started 2026-11-15 22:06:10 +0100. Commit still `8980e1a` (docs were not committed yet). Exit 0. Wall 5.290 s. WAV 161324 bytes at that `/tmp` path. Not committed. Stdout:

```text
{"engine": "piper", "voice": "en_US-ljspeech-medium", "pack": "present", "sha256": "pinned", "runtime": "weights/runtime/piper/piper", "cpu_only": true, "measured": true, "stub": false, "product_claim": true, "speechstack": {"pack": "present", "measured": true, "voice": "en_US-ljspeech-medium", "cold_first_pcm_ms": 339, "warm_first_pcm_ms": 21, "piper_pid": 4282, "piper_spawns": 1, "worker_reused": true, "pcm_cache": "none", "phrase": "Hi! This clause is the warm path check.", "clock": "speak entry until first clause PCM"}, "cold_first_pcm_ms": 339, "warm_first_pcm_ms": 21, "raw_spawn": {"cold_spawn_first_pcm_byte_ms": 350.4, "warm_line_first_pcm_byte_ms": 18.5, "cold_pcm_bytes": 147968, "warm_pcm_bytes": 119296, "clock": "spawn (cold) / stdin write (warm) until first stdout byte"}, "cold_spawn_first_pcm_byte_ms": 350.4, "warm_source": "speechstack", "drain": {"in_rate": 22050, "out_rate": 24000, "frame_samples": 1920, "tick_ms": 80, "frames": 42, "late_ticks": 0, "late_policy": "pause", "audio_ms": 3355.2, "ticks": "pppppppppppppppppppppppppppppppppppppppppp"}, "phrase_cold": "Streaming check. Galion Voice on Piper CPU.", "phrase_warm": "Hi! This clause is the warm path check.", "wav": "/tmp/swarm10-b1-piper-ttfa.wav"}
```

This host, Piper only: SpeechStack cold **339 ms**, warm **21 ms**. Raw spawn first PCM byte **350.4 ms**, warm line **18.5 ms**. Drain **42** frames, **0** late ticks, late=pause, audio **3355.2 ms**. `product_claim` true means this Piper checkpoint, not a clone.

`scripts/eval/virtual-user.sh` on this host: **not run**. Loopback on this host: **not measured**. The 78 ms figure stays the cited canned+Piper loopback from the other host.

`cargo test --workspace`: **not run**. CI `gates.yml` runs it. `main` at `8980e1a` had a green `gates` run: Actions `43660665688` (2026-11-15T00:56:24Z, conclusion success).

`GALION_FLASH_EVAL` and `GALION_CLONE_STUB_PCM` were not set. No protocol-stub clock was filed as a product TTFA.

## CI

Pending on this PR's head when the notes were written. The worker marks the PR ready only after every non-skipped check on that head is green. This worker does not merge.

## Blockers

| Item | State |
|---|---|
| Original-ref Nano TTFA and the 80 ms first-playable frame | **BLOCKED** on A0 (owner) and A1. Not measured here. B3's row. |
| This host before the A21 fetch | Piper stream **not measured** (pack absent, exit 2). |
| Loopback on this host | **not measured**. |
| A0 t273 vs t623u | BLOCKED-OWNER. This PR does not flip `LOCKS.toml` or `REFS.sha256`. |

## Decisions for Maciej

1. A0 is still open. Until you re-open the t273 originals or state that main stays on t623u, no Nano original-ref TTFA will be filed from this lane as a pass. Piper 398/20 and 339/21 are not that pass.
2. Live `/one` stays Piper plus the Samantha persona until a clone mouth is actually live. `product_claim: true` on these receipts does not change that.
3. The 80 ms first-playable frame is not claimed from cold 398 ms, warm 20 ms, loopback 78 ms, or this host's 339/21. The drain tick is 80 ms with 0 late ticks on both Piper runs. That is the Piper PCM drain, not the Nano frame bar.
4. The cited receipt and this host's remeasure are both Piper. Neither replaces the other.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
