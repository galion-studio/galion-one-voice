import onnxruntime as ort, numpy as np
o=ort.SessionOptions(); o.intra_op_num_threads=2; o.inter_op_num_threads=1
s=ort.InferenceSession('models/ecapa.onnx',o)
for T in (150,777,2400):
    x=np.load(f'out/x{T}.npy'); y=np.load(f'out/y{T}.npy'); z=s.run(None,{'feats':x})[0]
    c=(y*z).sum()/np.linalg.norm(y)/np.linalg.norm(z); print(T,'maxabs',abs(y-z).max(),'cos',c)
