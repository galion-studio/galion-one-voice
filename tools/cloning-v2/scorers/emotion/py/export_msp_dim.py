# One-time export of audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim (CC-BY-NC-SA-4.0) per model card.
import torch, torch.nn as nn
from transformers.models.wav2vec2.modeling_wav2vec2 import Wav2Vec2Model, Wav2Vec2PreTrainedModel
torch.set_num_threads(2)
class RegressionHead(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.dense = nn.Linear(config.hidden_size, config.hidden_size)
        self.dropout = nn.Dropout(config.final_dropout)
        self.out_proj = nn.Linear(config.hidden_size, config.num_labels)
    def forward(self, x):
        return self.out_proj(self.dropout(torch.tanh(self.dense(self.dropout(x)))))
class EmotionModel(Wav2Vec2PreTrainedModel):
    def __init__(self, config):
        super().__init__(config)
        self.config = config
        self.wav2vec2 = Wav2Vec2Model(config)
        self.classifier = RegressionHead(config)
        self.post_init()
    def forward(self, input_values):
        h = self.wav2vec2(input_values)[0]
        h = torch.mean(h, dim=1)
        return h, self.classifier(h)
if __name__ == "__main__":
    m = EmotionModel.from_pretrained("audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim").eval()
    class W(nn.Module):
        def __init__(s, m): super().__init__(); s.m = m
        def forward(s, x): return s.m(x)[1]
    x = torch.randn(1, 32000)
    torch.onnx.export(W(m), (x,), "models/msp_dim.onnx", input_names=["input_values"], output_names=["adv"],
        dynamic_axes={"input_values": {1: "samples"}}, opset_version=17, dynamo=False)
    print("ok", m.config.id2label)
