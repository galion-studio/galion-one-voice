# One-time export of torchaudio SQUIM models to ONNX (opset 17). Alignment (tile/cut of NMR) is done in Rust.
import torch, torchaudio, sys
torch.set_num_threads(2)
D='models'
class Sub(torch.nn.Module):
    def __init__(s,m): super().__init__(); s.m=m
    def forward(s,w,r):  # w,r: (1,T) same T
        a=s.m.projector(s.m.ssl_model.extract_features(w)[0][-1])
        b=s.m.projector(s.m.ssl_model.extract_features(r)[0][-1])
        return 5-s.m.predictor(torch.cat((b,a),dim=2))
class MHA(torch.nn.Module):
    """Explicit self-attention with the same weights as nn.MultiheadAttention (batch_first), shape-agnostic reshapes,
    so the traced ONNX graph keeps a dynamic sequence length (the stock traced MHA bakes T into Reshape)."""
    def __init__(s,m):
        super().__init__(); s.h=m.num_heads; s.w=m.in_proj_weight; s.b=m.in_proj_bias; s.o=m.out_proj
    def forward(s,x,*a,**k):
        B,T,E=x.shape[0],x.shape[1],x.shape[2]; d=E//s.h
        q,kk,v=torch.nn.functional.linear(x,s.w,s.b).chunk(3,dim=-1)
        # one head at a time: identical math, peak memory = one (T x T) score matrix instead of n_heads of them
        ys=[]
        for h in range(s.h):
            qh,kh,vh=q[...,h*d:(h+1)*d],kk[...,h*d:(h+1)*d],v[...,h*d:(h+1)*d]
            ys.append(torch.softmax(qh@kh.transpose(-1,-2)/(d**0.5),dim=-1)@vh)
        return (s.o(torch.cat(ys,dim=-1)),None)
class Obj(torch.nn.Module):
    def __init__(s,m):
        super().__init__(); s.m=m
        for br in m.branches:
            lay=br[0]; mha=MHA(lay.self_attn)
            lay.add_module('mha_explicit',mha)
            # dropout=0.0 in SQUIM branches, no masks are used -> _sa_block(x) == self_attn(x,x,x)[0]
            lay._sa_block=(lambda mha: (lambda x,attn_mask,key_padding_mask,is_causal=False: mha(x)[0]))(mha)
    def forward(s,w):
        st,pe,sd=s.m(w); return torch.stack([st,pe,sd],dim=1)
which=sys.argv[1:] or ['sub','obj']
torch.backends.mha.set_fastpath_enabled(False)
with torch.no_grad():
  if 'sub' in which:
    m=Sub(torchaudio.pipelines.SQUIM_SUBJECTIVE.get_model().eval())
    x=torch.randn(1,48000)*0.1
    torch.onnx.export(m,(x,x.clone()),f'{D}/squim_subjective.onnx',input_names=['waveform','reference'],output_names=['mos'],
        dynamic_axes={'waveform':{1:'T'},'reference':{1:'T'}},opset_version=17,do_constant_folding=True)
  if 'obj' in which:
    m=Obj(torchaudio.pipelines.SQUIM_OBJECTIVE.get_model().eval())
    x=torch.randn(1,48000)*0.1
    torch.onnx.export(m,(x,),f'{D}/squim_objective.onnx',input_names=['waveform'],output_names=['stoi_pesq_sisdr'],
        dynamic_axes={'waveform':{1:'T'}},opset_version=17,do_constant_folding=True)
print('ok')
