# Hard real gates

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The spelling is **Galion**. The product is Galion Voice.

`scripts/eval/v2-hard-loop.sh` runs on this CPU. It does not call a cloud
voice, a remote model, or another machine. The mouth for **HARD-PASS** is
the in-tree Piper path: `weights/runtime/piper/piper` and
`en_US-ljspeech-medium.onnx` plus its json. Flite is not that voice.
Clone protocol PCM is not that voice. The harness does not download the pack.

The overall line is **HARD-PASS** or **HARD-FAIL**. A component that cannot
be measured here is **HARD-FAIL**, and its name is on the report line.
`GALION_CLONE_STUB_PCM` and `GALION_CLONE_STUB_FRAMES` cannot yield
**HARD-PASS**. `product_claim` is true only for real Piper PCM.
`engine` is `piper` only then.

The report line carries the verdict, `ttfa_ms`, allocs, late frames,
persona HTTP, `/dot`, and `missing`. An unmeasured clock is
`ttfa_ms=absent`. It is not a number.

Sign-off of `ADMIN-UI-CHECK.md` waits on live CPU Piper and LLM traces.
That page stays unsigned. The date stays blank.

## Gates

| Gate | Met when | How the harness proves it |
|---|---|---|
| Piper PCM | The Piper binary and the LJSpeech medium pack are on this box, and one JSON line returns s16le PCM at the `sample_rate` in the voice json. | Spawn `piper --json-input --output_raw` on this CPU. No network. A missing pack is `missing: Piper`. |
| TTFA | The same run times the first PCM byte. | `ttfa_ms` is that clock. No byte means `ttfa_ms=absent` and **HARD-FAIL**. |
| Sampling | 24 kHz heartbeat, 1920 samples, 80 ms EmotionClock, allocs=0, late=pause. | `cargo test -p galion-heartbeat --test heartbeat` and `--test zero_alloc`. |
| Vosk | The local Vosk model is in the demo log for this run. | `weights/asr/vosk-model-small-en-us-0.15`. No download. A missing model is `missing: vosk`. |
| LLM | Hermes on `127.0.0.1` or `localhost`, or a local student `.gq` named by `GALION_STUDENT_GQ`, and the words are not the persona table. | `POST /v2/say`. The table line "Your mother died this morning." is not a generation. Canned empathy fails the gate. |
| Thinking | A nerve or thought file is already on disk. | `GALION_NERVE_LOG` or `GALION_THOUGHT_LOG`. The harness does not write one and does not sign the checklist. |
| Persona | `GALION_CLONE_LOCAL_REFS=1`, both clip hashes match, galion-emotion is on a live `--feed real` dot, and playback HTML contains the Jarvis hub texture. | Sidecars stay outside git. A missing texture or clip is named and is **HARD-FAIL**. |
| Chain | Vosk, the floor, Piper, the 80 ms clock, playback, and `ttfa_ms` all happened in this run. | Each link is recorded. One gap fails the chain. |

`GALION_CLONE_WEIGHTS_SOURCE=hf` remains a fetch source. This harness does
not call it.

`LOCKS.toml` is not edited. Wavs are not committed.

— Maciej Grajczyk
