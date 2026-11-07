import torch, sys
from transformers import Wav2Vec2ForSequenceClassification
torch.set_num_threads(2)
m = Wav2Vec2ForSequenceClassification.from_pretrained("superb/wav2vec2-base-superb-er").eval()
class W(torch.nn.Module):
    def __init__(s, m): super().__init__(); s.m = m
    def forward(s, x): return torch.softmax(s.m(input_values=x).logits, -1)
x = torch.randn(1, 32000)
torch.onnx.export(W(m), (x,), "models/superb_er.onnx", input_names=["input_values"], output_names=["probs"],
    dynamic_axes={"input_values": {1: "samples"}}, opset_version=17, dynamo=False)
print("ok", m.config.id2label)
