---
license: mit
pipeline_tag: audio-classification
base_model:
- emotion2vec/emotion2vec_plus_base
tags:
- onnx
---

# emotion2vec_plus_base
export onnx use https://github.com/ame700/emotion2vec/blob/main/scripts/onnx/README.md

## Usage
```
import json
import numpy as np
import onnxruntime as ort
import torchaudio

def softmax(x: np.ndarray) -> np.ndarray:
    e = np.exp(x - x.max())
    return e / e.sum()

onnx_path = "./emotion2vec_plus_base.onnx"
onnx_head = "./emotion2vec_head.json"
wav_path = "test.wav" # 16k 

def main():
    head = json.load(open(onnx_head))
    W = np.array(head["weight"], dtype=np.float32)
    B = np.array(head["bias"], dtype=np.float32) 
    labels = head["labels"]

    sess = ort.InferenceSession(onnx_path, providers=["CPUExecutionProvider"])
    audio, sr = torchaudio.load(wav_path)
    audio = audio[0].numpy()
    feats = sess.run(None, {sess.get_inputs()[0].name: audio.reshape(1, -1)})[0]
    pooled = feats[0].mean(axis=0)
    probs = softmax(W @ pooled + B)
    order = np.argsort(-probs)
    emotion_result = labels[order[0]]
    print(f"wav: {wav_path}, emotion_result: {emotion_result}")

if __name__ == "__main__":
    main()
```