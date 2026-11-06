---
license: mit
library_name: onnx
pipeline_tag: audio-classification
tags:
  - audioset
  - audio-tagging
  - efficientat
  - onnx
---

# EfficientAT mn10_as, ONNX

A modified version of the `mn10_as` checkpoint of [EfficientAT](https://github.com/fschmid56/EfficientAT)
(`mn10_as_mAP_471.pt` from release `v0.0.1`, code at commit `a425fdce92572e602a1d5634799bd9f1f2efa806`),
© 2022 Florian Schmid, licensed under MIT. The licence is in `LICENSE`.

## Changes from the original

- **Exported to ONNX** (opset 18) with the MobileNetV3 and a sigmoid, so the graph returns the
  527 AudioSet class probabilities rather than logits.
- **Feature extraction built in.** The graph takes 32 kHz mono samples and holds the log-mel front
  end of EfficientAT's `AugmentMelSTFT` in eval mode: pre-emphasis, an 800-sample Hann window in a
  1024-point frame, hop 320, 128 Kaldi mel banks from 0 to 15 kHz, log, (x + 4.5) / 5.

Weights are unchanged (float32). `export_efficientat.py` (with `common.py`) made this file and
checks it against EfficientAT's own pipeline: largest difference 2.5e-6.

## File

`efficientat-mn10-as.onnx`, a single file of 20,200,590 bytes,
SHA-256 `bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504`.

| input | type | shape | meaning |
| --- | --- | --- | --- |
| `samples` | f32 | `[W, N]` | `W` windows of mono 32 kHz audio, 0.5 s to 60 s long |

| output | type | shape | meaning |
| --- | --- | --- | --- |
| `probabilities` | f32 | `[W, 527]` | AudioSet class probabilities per window |

The model was trained on 10 s clips; very short windows alone give poor answers.
