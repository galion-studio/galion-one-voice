# Hard real gates

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The spelling is **Galion**. The product is Galion Voice.

`scripts/eval/v2-hard-loop.sh` prints **HARD-PASS** only when every gate
below is `met`. A missing piece is `absent` or `stub`, and the overall
line is **STUB-PASS**. A gate that ran and broke is **HARD-FAIL**.
`GALION_CLONE_STUB_PCM=1` and `GALION_CLONE_STUB_FRAMES=1` are stub PCM.
They never yield **HARD-PASS**. `product_claim` is true only for PCM
whose engine is `chatterbox-nano` or `nano`.

The report line always names the verdict, `ttfa_ms`, allocs, late frames,
persona HTTP, and `/dot`. An unmeasured clock is `ttfa_ms=absent` or
`ttfa_ms=stub`. It is not a number.

This page does not sign `ADMIN-UI-CHECK.md`. The date on that page stays
blank.

## Gates

| Gate | Met when | How the harness proves it |
|---|---|---|
| Real voice | Pinned Nano weights, and the worker returns s16le PCM with engine `chatterbox-nano` or `nano`. | One JSON request to `scripts/clone/chatterbox_worker.py`. Stub flags and engine `protocol-stub` or `model` are not this gate. |
| Real sampling | 24 kHz, 1920 samples, 80 ms EmotionClock frames, `Heartbeat::step` allocs=0, late=pause. | `cargo test -p galion-heartbeat --test heartbeat` (`frame_constants_match_locks`, the two late-frame tests) and `--test zero_alloc` (`step_allocates_nothing_with_rika_on`). The 80 ms frame is the emotion clock. |
| Real LLM | Samantha or Jarvis words from Hermes on loopback, or from a student `.gq` file the operator named. No canned empathy line. | `POST /v2/say`. The persona table ("Your mother died this morning.") is `stub`. `GALION_HERMES_URL` must be `http://127.0.0.1` or `http://localhost`. `GALION_STUDENT_GQ` must be a `.gq` file. Anything else is not contacted. |
| Real thinking | A nerve or thought trace is already on disk. | Read `GALION_NERVE_LOG` or `GALION_THOUGHT_LOG` if the file exists. The harness does not create it. A fake interim fails the gate. Maciej reads it. The checklist is not ticked. |
| Real persona | `GALION_CLONE_LOCAL_REFS=1`, both clip hashes match, galion-emotion is on a live `--feed real` dot, and the playback page contains the Jarvis hub texture. | Sidecars outside the git tree. `/dot` from a harness-launched real feed. `GET /` must contain both `hub` and `texture`. A missing texture stays `absent`. |
| Full chain | Vosk model actually in the demo log, the instruction floor answered, Nano PCM streamed, `/dot` `t_ms` moved on the 80 ms grid, and playback showed speaking while Nano PCM existed, with a real `ttfa_ms`. | Each link is a boolean. One missing link keeps the chain `absent`. A whole-utterance Nano path at or above 1500 ms is `HARD-FAIL` on the TTFA gate. |

`GALION_CLONE_WEIGHTS_SOURCE=hf` remains a source in
`scripts/clone/fetch-clone-weights.sh`, beside `release`. The harness
checks that the branch is still there. It does not download.

`LOCKS.toml` is not read to allow a clip and is not edited.
Reference wavs are not committed and are not deleted on a hash mismatch.

— Maciej Grajczyk
