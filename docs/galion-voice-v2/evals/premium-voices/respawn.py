import sys,time,statistics; sys.path.insert(0,'.')
from bench import Piper, cfg
for m,sid in [('en_US-libritts_r-medium',126),('en_GB-vctk-medium',95),('en_US-ljspeech-medium',None)]:
    xs=[]
    for _ in range(5):
        t0=time.perf_counter(); p=Piper(m); p.say("Okay.",sid); xs.append((time.perf_counter()-t0)*1000); p.close()
    print(f"{m} spawn+warm('Okay.') ms: median {statistics.median(xs):.0f}  max {max(xs):.0f}  runs {[round(x) for x in xs]}",flush=True)
