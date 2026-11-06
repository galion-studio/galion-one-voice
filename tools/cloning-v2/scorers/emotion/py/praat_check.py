"""Praat (parselmouth) proxy reference, two-pass Hirst floor/ceiling (q25*0.75, q75*1.5) to suppress octave errors."""
import sys, os, json
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "site"))
import numpy as np, soundfile as sf, parselmouth
from parselmouth.praat import call
out = sys.argv[1]; res = []
for f in sys.argv[2:]:
    x, sr = sf.read(f, dtype="float64")
    snd = parselmouth.Sound(x, sampling_frequency=sr)
    p1 = snd.to_pitch_ac(time_step=0.01, pitch_floor=60, pitch_ceiling=500).selected_array["frequency"]
    p1 = p1[p1 > 0]; lo, hi = np.percentile(p1, 25) * 0.75, np.percentile(p1, 75) * 1.5
    f0 = snd.to_pitch_ac(time_step=0.01, pitch_floor=lo, pitch_ceiling=hi).selected_array["frequency"]; f0 = f0[f0 > 0]
    med = np.median(f0); st = 12 * np.log2(f0 / med)
    pp = call(snd, "To PointProcess (periodic, cc)", lo, hi)
    res.append({"file": os.path.abspath(f), "floor": lo, "ceiling": hi, "praat": {
        "f0_mean_hz": float(f0.mean()), "f0_median_hz": float(med), "f0_sd_st": float(st.std(ddof=1)),
        "f0_range_st_p5_p95": float(np.percentile(st, 95) - np.percentile(st, 5)),
        "jitter_local_pct": call(pp, "Get jitter (local)", 0, 0, 0.0001, 0.02, 1.3) * 100,
        "shimmer_local_pct": call([snd, pp], "Get shimmer (local)", 0, 0, 0.0001, 0.02, 1.3, 1.6) * 100}})
json.dump({"files": res}, open(out, "w"), indent=1)
