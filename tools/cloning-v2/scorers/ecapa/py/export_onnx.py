# One-time export: speechbrain ECAPA (same ckpt/config as bakeoff-t648/tools/score_t648.py) -> ONNX.
import torch, sys
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN
torch.set_num_threads(2)
W='/workspace/swarm/voice-v2-sprint'
mdl=ECAPA_TDNN(input_size=80,channels=[1024,1024,1024,1024,3072],kernel_sizes=[5,3,3,3,1],dilations=[1,2,3,4,1],attention_channels=128,lin_neurons=192)
mdl.load_state_dict(torch.load(f'{W}/ecapa-local/embedding_model.ckpt',map_location='cpu')); mdl.eval()
class Wrap(torch.nn.Module):
    def __init__(s,m): super().__init__(); s.m=m
    def forward(s,feats):  # feats [1,T,80] already sentence-mean-normalized
        return s.m(feats).reshape(1,192)
w=Wrap(mdl).eval()
x=torch.randn(1,300,80)
out=sys.argv[1] if len(sys.argv)>1 else 'models/ecapa.onnx'
torch.onnx.export(w,(x,),out,input_names=['feats'],output_names=['emb'],dynamic_axes={'feats':{1:'T'}},opset_version=17,dynamo=False)
for T in (150,300,777):
    x=torch.randn(1,T,80); 
    with torch.no_grad(): print(T, w(x)[0,:4].tolist())
print('exported',out)
