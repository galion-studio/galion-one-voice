# Experiment smoke test (not shipped).
import sys; sys.path.insert(0,'.')
import nano_fast as N, numpy as np, soundfile as sf
M,l=N.load(4); print('load',l)
V=N.Voice(M,'amy'); print('cond',V.cond_ms,'kv',V.kv_ms, V.cond_emb.shape)
a,st=N.generate(M,V,"Sure, give me a second.",seed=1); print(st)
a,st=N.generate(M,V,"Sure, give me a second.",seed=1,kv_cache=False); print(st)
ps=[]
for c,st in N.generate_stream(M,V,"I found the old letters in a shoebox under the stairs.",seed=1): ps.append(c)
print([len(p) for p in ps], {k:v for k,v in st.items() if k!='chunks'}, st['chunks'][:2])
sf.write('/tmp/lat240-smoke-stream.wav', np.concatenate(ps), M.sr)
