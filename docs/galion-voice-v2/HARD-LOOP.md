# Hard loop

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The spelling is **Galion**. The product is Galion Voice.

This is the hard real-time pass through the V2 loop. It is the check that
sits past the stub self-eval. A stub result is not this result.

## Stub and hard

Two scripts still exist, and both are stubs:

| Script | What a green run is |
|---|---|
| `scripts/eval/v2-persona-selftest.sh` | **STUB-PASS.** Persona files and, if `:7878` is already up, the mood table. No weights, no clip hash, no Nano PCM. |
| `scripts/eval/flash-ttfa.sh` | **STUB-PASS** of the pipe when it prints anything. Default JSON is `measured: false` and has no `ttfa_ms`. `GALION_CLONE_STUB_PCM=1` may print a clock with `product_claim: false`. That clock is not a Flash result. |

The hard entry is `scripts/eval/v2-hard-loop.sh`.

Its overall line is one of three words:

| Overall | Meaning |
|---|---|
| `STUB-PASS` | The harness finished. Something the hard loop needs was absent, or the only PCM was the protocol stub. **Not a hard pass.** |
| `HARD-PASS` | Every gate below was met, including real Nano weights, both reference hashes, and a streaming first PCM from engine `model`. |
| `HARD-FAIL` | A gate ran and failed. Exit 1. |

`HARD-PASS` is refused unless those real inputs were on the machine.
`GALION_HARD_REQUIRE=1` turns a `STUB-PASS` into exit 2, for a host that
was supposed to have the weights and the clips. Exit 0 on `STUB-PASS` means
the harness told the truth. It does not mean the voice loop passed.

A gate line says `met`, `absent`, `failed`, or `stub`. `met` is that gate
only. One met gate does not make the loop `HARD-PASS`.

## What it penetrates

The loop, in order. Generate stays off `Heartbeat::step`. The tick is not
asked to synthesise.

1. **Tree.** No clone weight (`.safetensors`, `.pt`, `.onnx`, anything new under `weights/` besides `weights/.gitignore`) and no reference clip are tracked. The two presence-eval fixtures already in `specs/fixtures/` are the phone harness, not Samantha or Jarvis, and they are the only wav allowlist. Any other audio file fails the gate. `LOCKS.toml` is hashed before and after. This harness does not open it to decide anything, and it does not write it. `reference_clips` stays whatever the owner left. A change of that file during the run is `HARD-FAIL`.
2. **Unallowlisted reference.** The worker is sent `reference` set to the string `clip.wav` with the protocol stub armed. Exit 3, no PCM, the rights-hold line. That is the hold. It is not a generate.
3. **Reference hashes.** Only when `GALION_CLONE_LOCAL_REFS=1` and `GALION_CLIP_STORE` is an absolute directory **outside** this repo. For `samantha` and `jarvis` the file is `<store>/<id>/reference.wav` and the allowlist is the sidecar `<store>/<id>/reference.wav.sha256` (64 hex, `sha256sum` form). A mismatch is `HARD-FAIL`. The wav is not deleted. No hash is written into git, into `scripts/clone/PINS.sha256`, or into `LOCKS.toml`. `PINS.sha256` is the weight pin file, not a clip pin.
4. **Weight pins.** Nano's four safetensors (`shared/s3gen`, `shared/s3gen_meanflow`, `shared/ve`, `nano/t3_nano_v1`) against `scripts/clone/PINS.sha256`, under `weights/clone` or `GALION_CLONE_WEIGHTS`. Missing pack is `absent`. A partial pack or a bad hash is `HARD-FAIL`. Nothing is downloaded. `GALION_CLONE_FETCH` is stripped from the worker environment. The fetch script is not invoked.
5. **Nano generate.** When the pins match, one JSON line goes to `scripts/clone/chatterbox_worker.py`. PCM has to come back with stderr `galion-clone-engine: model`. `GALION_CLONE_STUB_PCM=1` is forced to `stub` even if weights exist. If the refs hashed and the weights matched, the request carries `audio_prompt_path` to the Samantha wav. Today's worker still exits 3 on that field. That is `HARD-FAIL`: conditioned generate is not available, so the loop must not be called a pass. The harness does not loosen the worker to hide that.
6. **TTFA streaming.** The clock starts when the request is written and stops at the first stdout byte. `whole_ms` stops at `galion-clone-utt:`. Streaming evidence is a `galion-clone-first` stderr line before that marker, or the first PCM byte at least 20 ms before the marker. Engine `model` without that evidence is `HARD-FAIL`, including a short whole-utterance under the kill line. A whole-utterance path at **≥ 1500 ms** is the blocking regression (the 1.8–2.4 s path). The hearable budget on the report is 160 ms. A protocol-stub clock is labelled `protocol-stub` and `product_claim: false`, and the JSON omits `ttfa_ms` when nothing was measured.
7. **Persona HTTP.** If `BASE` (default `http://127.0.0.1:7878`) answers `/health`, the harness posts the grief, joy, vulnerability, and quiet lines, pins Jarvis, and checks `/v2` redirects to `/`, `/v2/state` stays `rights: hold` and `film_voice: false`, and no canned empathy line comes back. Down, with no launch, is `absent`, not a pass.
8. **Live one-dot.** `GET /dot` has to move `t_ms` or `engine.frames`. Speaking has to show up as `mode` and `state` `speaking`, and a non-speaking sample has to show up in the same poll, so speak wins over the other mode rather than a frozen frame. `GALION_HARD_HTTP=1` builds `dot_console_demo` with `dot-viz-web` and runs it `--synthetic --realtime` on `127.0.0.1:7879` (override `GALION_HARD_ADDR`). An attached idle server that never speaks is `absent` for this gate, not `HARD-PASS`.
9. **late=pause.** `cargo test -p galion-heartbeat --test heartbeat late_` runs `late_frame_is_a_pause_no_extra_trigger_no_burst` and `short_pcm_frame_is_treated_as_late_pause`. A late frame is paused, me onset stays closed, and there is no catch-up burst. `GALION_HARD_SKIP_CARGO=1` leaves this gate `absent`.

`/dot` does not currently carry `paused`. The pause gate is the heartbeat test, not a guess at the picture. The picture is the Admin UI check.

## Commands

```bash
scripts/eval/v2-hard-loop.sh --self-check
scripts/eval/v2-hard-loop.sh
GALION_HARD_REQUIRE=1 scripts/eval/v2-hard-loop.sh
GALION_HARD_HTTP=1 scripts/eval/v2-hard-loop.sh
```

A report is printed and, when the directory can be created, copied to
`target/hard-loop/latest.txt`. That directory is gitignored. It is not a
product claim by itself. Read the `overall:` line.

On a clean tree with no weights and no clip store the honest overall line
is `STUB-PASS`, then `HARD-PASS of the loop: refused`, then
`product_claim: false`.

## What this file will not do

- It will not flip `LOCKS.toml`.
- It will not commit a wav or a weight.
- It will not delete a Samantha or Jarvis wav on a hash mismatch.
- It will not call `scripts/clone/fetch-clone-weights.sh`.
- It will not treat Piper, flite, or the protocol stub as Nano.
- It will not sign the Admin UI check. That page is `ADMIN-UI-CHECK.md`. Maciej Grajczyk has to sign it.

— Maciej Grajczyk
