# Clone worker: Chatterbox Turbo ONNX fp32 (`galion-clone-cbx`)

Owner t652u item 6. A separate process and systemd unit (`deploy/systemd/galion-clone-cbx.service`),
never part of the live voice path. Piper stays the live TTS. The deployed Jarvis voice must not change.

## Policy

Cloning is Rust-first. `galion-clone-cbx` is the Rust (ort) process. Piper training on own-voice clips is forbidden. Reference wavs stay outside git and are read-only. The unit does not replace the live voice. This page does not measure WER, ECAPA, MOS, or a timing. Those gates stay **BLOCKED**. No audio file is added. `LOCKS.toml` is not edited.

## Worker

- Engine: `cbx` (Rust, ort) running `ResembleAI/chatterbox-turbo-ONNX@d21799bd` in **fp32**, the t648 bake-off
  winner (`docs/galion-voice-v2/sprint-v2/bakeoff-t648/REPORT.md`). The quantized graph fails on cbx's bundled ORT
  (`GatherBlockQuantized`). The bake-off report holds its own timings. This page does not restate them. WER, ECAPA, MOS, and timings stay **BLOCKED** here. There is no streaming, so this process stays off the live voice. The deployed Jarvis voice must not change.
- Bind: `127.0.0.1:7880` only. A non-loopback `GALION_CBX_ADDR` exits 2. The unit also sets `IPAddressAllow=localhost`.
- Consent: `POST /register?persona=samantha|jarvis` runs `galion_consent::clone_gate` once, then builds the cbx
  voice profile from the gated reference. CC0 voices also need `GALION_CBX_ACCEPT_CC0=1`. `POST /speak?persona=`
  (body: text, 600 bytes max) needs a registered persona and returns `audio/wav`. There is no consent work per synthesis.
  `POST /unregister?persona=` drops the registration.
- Off during live calls: any request returns 409 while a call is live, and a running cbx job is killed when a call starts.
  A call counts as live if the flag file `state/live-call` exists, an ESTABLISHED TCP connection is on port 7878, or the
  last 120 summary lines of the live log show a non-idle dot.
- `GET /health` reports the engine, dtype, revision, cloning state (`available` or the live-call reason) and the
  registered personas.
