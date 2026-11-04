# Vosk mic ASR (letter A)

Perpetual Open MIT License · © 2026 Maciej Grajczyk

The phone mic on `http://127.0.0.1:7878/` uses **Vosk** when the small English
model is on disk. The worker is a separate process (`galion-asr-vosk`). It
dlopens libvosk. Nothing in `galion-cascade` links that library. `LOCKS.toml`
is unchanged.

If the model is missing, the screen still starts. Typed text still works.
A mic turn returns an error. The lane does **not** invent a sentence from
how long you talked.

This note does not claim a measured word-error rate.

## One-time setup (CPU)

Linux x86_64. About 40 MB for the model, about 7 MB for the library.
No GPU.

```bash
scripts/asr/fetch-vosk.sh
cargo build -p galion-asr-vosk --bin galion-asr-vosk
scripts/run-v1-screen.sh
```

`fetch-vosk.sh` checks sha256 and unpacks into `weights/` (gitignored, not
vendored):

| Piece | Path |
|---|---|
| Model `vosk-model-small-en-us-0.15` | `weights/asr/vosk-model-small-en-us-0.15/` |
| libvosk 0.3.45 | `weights/asr/libvosk/libvosk.so` |

The model directory is ready when both `am/final.mdl` and `conf/model.conf`
are files. The 0.15 archive also has `graph/HCLr.fst` and `graph/Gr.fst`
(a lookahead graph, not one precompiled HCLG).

`scripts/run-v1-screen.sh` then prints `mic ASR: vosk (...)` and the orb at
`:7878` posts mic audio to `/turn`. The reply is still the canned English
line, spoken by Piper or flite. The **transcript** is Vosk's words
(`asr=vosk`).

## If you skip the download

The screen prints `mic ASR: Vosk model missing` and sets `GALION_ASR=stub`.
Typed turns (`POST /v2/say`, and any text the page already injects) still
run. Mic audio that is not a stub fixture comes back as an error tone, with
an empty transcript. It is not "hello", "hello how are you", or
"can you hear me" chosen by duration.

`scripts/run-v1-screen.sh` also refuses an exported `GALION_ASR=interim`.
That value is test-only. The presence-eval harness still constructs
`AsrMode::Interim` in code to prove the old label does not claim recognition.

## Environment

| Variable | Meaning |
|---|---|
| `GALION_ASR=vosk` | Use the worker. This is what the screen sets when the model is present. |
| `GALION_ASR` unset or `stub` | Fixtures decode. Real speech is `AsrUnratified`. |
| `GALION_ASR=off` | Always `AsrUnratified`. |
| `GALION_VOSK_MODEL` | Model directory. Default `weights/asr/vosk-model-small-en-us-0.15`. |
| `GALION_VOSK_LIB` | `libvosk.so`. Default `weights/asr/libvosk/libvosk.so`. |
| `GALION_VOSK_BIN` | Worker binary. The screen script sets this after `cargo build`. |
| `GALION_VOSK_THREADS` | `OMP_NUM_THREADS` / `OPENBLAS_NUM_THREADS` for the child when those are unset. Default `2`. |
| `GALION_VOSK_GRAMMAR=intents` | Optional. Sends the canned phrase list plus `[unk]`. If Vosk cannot build that recognizer, the turn errors. It does not guess a phrase. |

Leave `GALION_VOSK_GRAMMAR` unset for the mic. Free decoding returns the
words the model produced.

Other CPU architectures: the pinned zip is linux x86_64. Install libvosk
yourself and set `GALION_VOSK_LIB`. The model zip is not tied to one CPU.

## What the worker speaks

stdin/stdout NDJSON, protocol `galion-asr-vosk/1`. PCM is 16 kHz mono s16le,
base64, one utterance per line. The worker feeds libvosk in 20 ms frames
(320 samples), with word times on (`set_words`). It writes `partial` lines
and one `final`. A problem is `{"type":"error",...}` and a non-zero exit.
`vosk_gpu_init` is not called.

`[unk]` tokens are dropped. A final with no words left is an empty turn,
not a filled-in sentence.

## Licence

Vosk and the small-en model are Apache-2.0 (Alpha Cephei / Kaldi). They are
not copied into this repository. Attribution: https://github.com/alphacep/vosk-api
and https://alphacephei.com/vosk/models .

## Sources

- Model zip: `https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip`
  sha256 `30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498`
- Library zip: `https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip`
  sha256 `bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2`

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
