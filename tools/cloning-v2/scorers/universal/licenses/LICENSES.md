# Universal emotion scorer — model licences (verified 2026-11-05 Europe/Zagreb)

## emotion2vec+ base
- **Weights origin:** https://huggingface.co/emotion2vec/emotion2vec_plus_base
- **ONNX export used:** https://huggingface.co/ziyu12345/emotion2vec_plus_base_onnx (MIT for the export packaging; underlying weights FunASR Model License)
- **Weight licence:** FunASR Model Open Source License Agreement **v1.1**  
  https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE  
  Commercial use OK with attribution and retaining the model name (`emotion2vec+ base` / FunASR).
- **Classes:** angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown

## MS-CLAP 2023
- **Code:** https://github.com/microsoft/CLAP — **MIT**
- **Weights (HF):** https://huggingface.co/microsoft/msclap — card declares **`ms-pl`** (Microsoft Public License; commercial use permitted)
- **PyPI package msclap:** MIT
- Used for zero-shot text-audio prompts (love, seductive, logic/reason, yelling, …) — no trained classes for those labels.

Rejected alternatives: any NONCOMMERCIAL / CC-BY-NC weights.

## EfficientAT mn10_as (AudioSet 527) — PRIMARY for yelling / crying / laughter
- **ONNX weights:** https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx
- **Licence:** MIT
- **SHA-256:** `bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504` (20,200,590 bytes)
- **Input:** raw mono waveform @ 32 kHz, shape `[W, N]`; **output:** AudioSet probabilities `[W, 527]` (opset 18)
- **Attribution:** EfficientAT mn10_as © 2022 Florian Schmid (https://github.com/fschmid56/EfficientAT); ONNX export by Chris-Kode
- **Class map (verified against AudioSet `class_labels_indices.csv`):**
  - yelling ← max(Shout=8, Yell=11, Screaming=14, Children shouting=13); Battle cry(12) excluded
  - crying ← Crying, sobbing=22 only; Baby cry, infant cry(23) excluded for adult Samantha/Jarvis clone eval
  - laughter ← max(Laughter=16, Baby laughter=17, Giggle=18, Belly laugh=20, Chuckle, chortle=21)
- **Runtime:** Rust `ort` crate (`galion-efficientat`), windowed max/mean pooling; leaderboard source label `efficientat`
- CLAP remains for love / seductive / logic / reason. emotion2vec+ remains for its trained classes.
