import torch, torchaudio, numpy as np, onnxruntime as ort
torch.set_num_threads(2); torch.backends.mha.set_fastpath_enabled(False)
o=ort.SessionOptions(); o.intra_op_num_threads=2; o.inter_op_num_threads=1
so=ort.InferenceSession('models/squim_objective.onnx',o); ss=ort.InferenceSession('models/squim_subjective.onnx',o)
obj=torchaudio.pipelines.SQUIM_OBJECTIVE.get_model().eval(); sub=torchaudio.pipelines.SQUIM_SUBJECTIVE.get_model().eval()
w,_=torchaudio.load('/workspace/swarm/voice-v2-sprint/own-clone-analysis/wav/jarvis/t09.wav'); w=torchaudio.functional.resample(w,24000,16000)
r,_=torchaudio.load('/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav'); r=torchaudio.functional.resample(r,24000,16000)
with torch.no_grad():
  for T in (16000,33333,48000,80017,w.shape[1]):
    x=w[:,:T]; a=torch.stack(obj(x),1).numpy(); b=so.run(None,{'waveform':x.numpy()})[0]
    rr=r.repeat(1,T//r.shape[1]+1)[:,:T] if r.shape[1]<T else r[:,:T]
    c=sub(x,r).item(); d=ss.run(None,{'waveform':x.numpy(),'reference':rr.numpy()})[0]
    print(T,a,b,np.abs(a-b).max(),c,d)
