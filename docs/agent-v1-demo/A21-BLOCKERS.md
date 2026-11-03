# A21 · Real-speech blockers · 2026-11-02 16:10 CET (Europe/Zagreb) · updated by FP-03

Context: Q2 says the EN reply must say words. That is **met by A21**: a neural TTS running on CPU, built per owner decision t123u. Flite is now a fallback only.

## CLOSED (FP-03)
- [x] **A21 neural TTS weights on disk.** The pack is `weights/tts/en_US-ljspeech-medium.onnx` plus its `.onnx.json`.
  - It is the Piper VITS model, 22.05 kHz, 63,531,379 B.
  - It comes from HF `rhasspy/piper-voices` at repo sha `b4e358ffcb232591bf73a540e70f7de528e4174a`. That repo is MIT, and the LJSpeech dataset is public domain.
  - The files are not vendored (`weights/.gitignore` = `*`). Fetch them with `scripts/a21/fetch-a21.sh`.
- [x] **sha256 pinned.** The pins are in `weights.rs` `PINNED`, in the `<file>.sha256` sidecars and in `fetch-a21.sh`:
  - `.onnx` `6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22`
  - `.onnx.json` `141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860`
  - runtime tarball `piper_linux_x86_64.tar.gz` (2023.11.14-2) `a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992`
- [x] **Decoder wired (CPU, no GPU).** The `A21WeightTts` slot runs the Piper runtime in a separate process from `weights/runtime/piper/piper`, or from `$GALION_A21_RUNTIME`.
  - The Piper runtime is MIT and uses onnxruntime 1.14.1 (MIT) on CPU.
  - Output is deterministic: `noise_scale=0`, `noise_w=0`. `GALION_A21_NOISE=natural` opts into sampled prosody.
  - Output is resampled from 22.05 to 24 kHz.
  - There is no `ort` or candle crate in the tree, so `check-no-forbidden` stays green.
- [x] **Honest slot status.** `WeightStatus` = `present-verified` only when the file is there, the sha256 matches and the runtime is present.
  - CLI, JSON and soft-check output show `SPEECH=a21|flite|ABSENT`.
  - With `GALION_SPEECH=a21` and no pack the run exits 3. It never pretends.
- [x] **Swap without breaking the one-dot flow.** `GALION_SPEECH=auto` (the default) tries A21 first. Flite is used only if the pack or runtime is missing.
  - The reply WAV feeds `--reply`, so the speaking state lasts 17 frames = 1360 ms of words.
  - The 80 ms Heartbeat and the Rika path are unchanged (C1–C3, C12 PASS).
- [x] **Sound reaches the speakers.** `galion-live-demo --realtime --reply` plays the reply WAV with paplay, pw-play, aplay or ffplay when speaking starts, and kills it on barge-in (`crates/galion-live-demo/src/play.rs`).
  - Opt out with `--no-play` or `GALION_PLAY=0`.
  - There is no audio element on the HTML page.
- [x] Dead `V1_SPEECH_CMD` / `scripts/v1/speech.sh` branch removed from `run-v1-demo.sh`.

## TRUE remaining blockers
1. **LOCKS ratification of the Piper pin — written 2026-11-03.** `[formats.exceptions.a21_piper]` records t123u: out-of-process Piper, `en_US-ljspeech-medium`, no in-tree ort, sunset `2027-02-01`. Global `foreign_runtime_allowed=false` and `weight_formats` are unchanged. Do not widen the pin to ASR or Chatterbox. espeak-ng (item 2) is still open.
2. **espeak-ng is GPL-3 (counsel).** Piper phonemizes with espeak-ng 1.52 (libespeak-ng + espeak-ng-data, bundled in the Piper tarball). The runtime is separate, out of process and unlinked from Galion code, but counsel must confirm GPL-3 is acceptable to ship.
   - The MIT-only alternative is a Galion-native phonemizer, which is not built.
3. **Voice choice and consent.** LJSpeech is a public-domain single female reader. Confirm it is acceptable as the demo voice under the LOCKS consent rules, or pick another MIT/PD Piper voice. Swapping voices means changing the file and its pin only.
4. **Galion-native decoder** (`.gq` / `.safetensors` on galion-tensor) is not built. `GALION_NATIVE_DECODER_WIRED=false`. Piper is the CPU decoder until then.
5. **Not A21 TTS, but still stubs:** Mimi `IS_STUB` (the demo runs with `--mimi off`), StubAsr (no real speech recognition), MockLlm (echo reply), FixedEmotion.
6. **Platform:** the pinned runtime is linux x86_64 only. aarch64 would need another pinned tarball.
7. **Deploy:** out of scope. VPS 51.38.137.193 needs SSH access, which is not available. Nothing was deployed or pushed.

## Run
```bash
scripts/a21/fetch-a21.sh            # one-time, sha256-verified; --check = verify only (no network)
scripts/run-en-cascade-demo.sh      # SPEECH=a21 -> target/galion-demo/en-cascade.wav (words)
scripts/run-v1-demo.sh              # one dot on :7878; reply words play to speakers when speaking starts
GALION_SPEECH=flite scripts/run-en-cascade-demo.sh   # force fallback
```

— Maciej Grajczyk
