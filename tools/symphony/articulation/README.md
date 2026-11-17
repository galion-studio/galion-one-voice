# galion-articulation

Perpetual Open MIT License. (c) 2026 Galion Labs.

Symphony articulation layer: throat / mouth / nose per 80 ms tick, and the TractPrint reference-vs-clone score. Design: [docs/galion-voice-v2/symphony/ARTICULATION-LAYER.md](../../../docs/galion-voice-v2/symphony/ARTICULATION-LAYER.md).

Reused, unchanged: praatfan_rust (MIT, Rust, in process); refcheck noise-floor and RT60 (Rust, path dependency); SPARC, DisVoice, Phonet (MIT, Python, out of process, `workers/`); Rhubarb Lip Sync (MIT, C++ CLI). Optional and out of process, only when already installed: openSMILE eGeMAPSv02, galion-emotion (MSP-dim), DeepFilterNet, Demucs, pyannote, Silero, galion-ecapa, YAMNet, a local laughter checkout. Own code: the 80 ms tick glue, TractPrint v2, the 24-category emotion trajectory, and the decode / select / loop glue.

Prosody and burst share one 24-category inventory (`configs/emotions-v2.toml`). The twelve Nature Human Behaviour names are the reliable prosody tier, not a shorter list. Neutral is the baseline. Breath is physiology. `seductive` renders as prosody `desire`. A laugh is burst `amusement`.

Worker environment (laptop WSL, CPU torch):

```bash
uv venv -p 3.11 venv && . venv/bin/activate
uv pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
uv pip install speech-articulatory-coding==0.1.0 disvoice==0.1.10 phonet==0.3.7
# Reconcile old upstream requirements: TF/Keras 2.15, scipy top-level fft/ifft, NumPy 1.x.
uv pip uninstall tensorflow
uv pip install --reinstall tensorflow-cpu==2.15.1 keras==2.15.0 numpy==1.26.4 pandas==2.2.3 scipy==1.11.4 "setuptools<81" python_speech_features praat-parselmouth==0.4.7
```

```bash
target/release/galion-artic decode --wav ref.wav --emotion seductive --env cafe --out profile.json
target/release/galion-artic select --wav scene.wav --enrol enrol.wav --out segments.json
target/release/galion-artic loop --ref ref.wav --emotion desire --env cafe --out report.json
```

`loop` writes a profile, renders with the in-process stand-in (gain and time-scale from the trajectory), reads the result with the same decoder, and corrects exaggeration and speed. `--render-cmd` runs an external cloner; the profile path is `$GALION_PROFILE`. The stand-in is not Chatterbox.

Never commit audio or per-voice output JSON. No model download is part of the build.
