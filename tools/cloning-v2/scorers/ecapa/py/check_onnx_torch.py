import torch, numpy as np
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN
torch.set_num_threads(2); torch.manual_seed(0)
W='/workspace/swarm/voice-v2-sprint'
mdl=ECAPA_TDNN(input_size=80,channels=[1024,1024,1024,1024,3072],kernel_sizes=[5,3,3,3,1],dilations=[1,2,3,4,1],attention_channels=128,lin_neurons=192)
mdl.load_state_dict(torch.load(f'{W}/ecapa-local/embedding_model.ckpt',map_location='cpu')); mdl.eval()
for T in (150,777,2400):
    x=torch.randn(1,T,80)*5; np.save(f'out/x{T}.npy',x.numpy())
    with torch.no_grad(): np.save(f'out/y{T}.npy',mdl(x).reshape(1,192).numpy())
