# galion-articulation

Perpetual Open MIT License. (c) 2026 Galion Labs.

Symphony articulation layer: throat / mouth / nose per 80 ms tick, and the TractPrint reference-vs-clone score. Design: [docs/galion-voice-v2/symphony/ARTICULATION-LAYER.md](../../../docs/galion-voice-v2/symphony/ARTICULATION-LAYER.md).

Reused, unchanged: praatfan_rust (MIT, Rust, in process); SPARC, DisVoice, Phonet (MIT, Python, out of process, `workers/`); Rhubarb Lip Sync (MIT, C++ CLI). Own code: the 80 ms tick glue and TractPrint (`src/score.rs`).

Worker environment (laptop WSL, CPU torch):

```bash
uv venv -p 3.11 venv && . venv/bin/activate
uv pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
uv pip install speech-articulatory-coding==0.1.0 disvoice==0.1.10 phonet==0.3.7
# Reconcile old upstream requirements: TF/Keras 2.15, scipy top-level fft/ifft, NumPy 1.x.
uv pip uninstall tensorflow
uv pip install --reinstall tensorflow-cpu==2.15.1 keras==2.15.0 numpy==1.26.4 pandas==2.2.3 scipy==1.11.4 "setuptools<81" python_speech_features praat-parselmouth==0.4.7
```

Never commit audio or per-voice output JSON.
