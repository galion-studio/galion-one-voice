# galion-demo — Galion Voice canned demo (Phase 3 skeleton)

Perpetual Open MIT License. No downloads. No model required.

Produces a reproducible demo artifact on any cheap CPU box (pure Python 3 stdlib):

```bash
./run_demo.sh                 # gen + run + checksums
python3 galion_demo.py gen    # only write synthetic input
python3 galion_demo.py run --in my16bit.wav --out reply.wav --capture cap.txt   # your own 16-bit PCM wav
```

## Artifacts (`out/`)
| file | what |
|---|---|
| `demo_input.wav` | synthetic placeholder "user": 3 tone-burst utterances (f0 180/240/130 Hz → neutral / excited / angry proxy, vibrato), a breath-in-silence at 3.3–3.6 s (the Rika moment), 4 Hz click track (timing reference), 16 kHz mono, 10 s |
| `demo_output_stereo.wav` | dual-band result: **L = user band, R = agent band** (full-duplex shape: quiet backchannel while user talks, finite reply after each turn; pitch/energy follow Galion's **counterbalanced target**, never the caller — decision F), 16 kHz stereo |
| `console_capture.txt` | scripted console capture: per-200 ms USER/AGENT level bars + emotion label + RTF |
| `SHA256SUMS` | checksums for the run |

## Pipeline
`band A in → 20 ms frames → RMS + zero-crossing pitch proxy → caller VAD (stub) → consent gate → counterbalance policy (StubExpressor::respond port) → prosody plan (StubExpressor::plan port) → band B out`

Consent (default **deny**): `--consent none|sense,adapt`, `--purpose personal|accessibility|workplace|education`.
Without SENSE+ADAPT, or in workplace/education (hard off, not overridable), nothing is read: Galion stays neutral-warm and the console says "no reading".

Counterbalance (decision F, A09 §4): angry → calmer, lower dominance, 250 ms pre-pause; anxious → calmer; sad/tired → gentle low arousal; **joy/excited is the only mirrored state** (shared slightly below the caller). `python3 galion_demo.py selftest` checks a 378-point VAD grid and that the old mirror behaviour is rejected.

Rika opener (`--rika-opener`, opt-in, OFF by default): Python port of `galion-emotion/src/rika.rs` on 80 ms heartbeat frames. User speaks, falls silent, takes a breath (tone shift inside silence, ≥ 2-frame hysteresis) → ARMED (lean-in, held breath), and after 12 armed frames a single **wordless soft hum** ("mm", 80 ms, ~14 dB below reply) → user speaks → CAUGHT/yield. Never words. Needs SENSE+ADAPT and an allowed purpose; otherwise Rika is OFF and the user band is never read by it.

KD exporter (`--kd-out file.jsonl`): every demo spec the canned path fails is written as one distillation-target line (decision D); see `docs/A14-kd-targets-and-opener.md`.

Backend selection: if `GALION_MODEL` points to an existing model file, the live hook in `try_live_model()` is where the galion-tensor student (safetensors) gets wired. Until then — or if unset/missing — it **falls back to the canned responder**, and the capture header states which backend ran and why. The demo never fails for lack of a model.

## Honest limits
- The agent band is synthesized tones, not speech. It demonstrates timing, duplex overlap, and emotion-following — not language.
- The emotion proxy is a heuristic, not a trained classifier (no dominance proxy: D fixed 0.5, confidence 0.3).
- The Rika port is the RMS/ZCR stub path only; the Rust R1–R9 fixtures (G6) stay authoritative.
- RTF printed is for the canned path only; it is **not** a model benchmark.

## Recording (optional)
- **vhs**: `vhs demo.tape` → `out/demo.gif`
- **asciinema**: `asciinema rec out/demo.cast -c ./run_demo.sh`
- Audio→video for sharing: `ffmpeg -i out/demo_output_stereo.wav -filter_complex "[0:a]showwaves=s=1000x300:mode=line:split_channels=1[v]" -map "[v]" -map 0:a out/demo.mp4`

## Next
Replace `canned_reply()` with a call into `crates/galion-tensor` (Rust) once the student checkpoint exists; keep the canned path as the CI fallback/golden test.

## Golden (A14)
`./golden.sh` → selftest + deterministic canned runs (`--canned`, RTF labelled "canned", no wall clock) in 4 variants (default no-consent, consented opener, workplace, education; workplace/education output must be byte-identical to default) + KD exports, hashed into `out/canned/demo.artifact` and checked against `../galion-gates/golden/demo.sha256`. Wired into `galion-gates/run-gates.sh` as soft gate **DG**. Unit tests: `python3 -m unittest discover -s tests`. See `docs/A14-kd-targets-and-opener.md`.
