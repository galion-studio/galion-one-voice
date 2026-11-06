"""Python reference for galion-emotion (cross-check + benchmark only; not used at runtime).
Usage: python crosscheck.py OUT.json [--no-dim] [--resampler soxr_hq|poly] FILE...
"""
import sys, json, time, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "site"))
import numpy as np, soundfile as sf, torch, librosa
from scipy.signal import resample_poly
torch.set_num_threads(2); torch.set_num_interop_threads(1)
args = sys.argv[1:]
out = args.pop(0)
no_dim = "--no-dim" in args; args = [a for a in args if a != "--no-dim"]
rs = "soxr_hq"
if "--resampler" in args:
    i = args.index("--resampler"); rs = args[i + 1]; del args[i:i + 2]
no_praat = "--no-praat" in args; args = [a for a in args if a != "--no-praat"]
t_wall0, t_cpu0 = time.perf_counter(), time.process_time()
from transformers import Wav2Vec2ForSequenceClassification
sys.path.insert(0, os.path.dirname(__file__))
ser = Wav2Vec2ForSequenceClassification.from_pretrained("superb/wav2vec2-base-superb-er").eval()
labels = [ser.config.id2label[i] for i in range(4)]
dim = None
if not no_dim:
    from export_msp_dim import EmotionModel
    dim = EmotionModel.from_pretrained("audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim").eval()
load_s = time.perf_counter() - t_wall0
res = []
for f in args:
    x, sr = sf.read(f, dtype="float32")
    if x.ndim > 1: x = x.mean(1)
    tw, tc = time.perf_counter(), time.process_time()
    if sr != 16000:
        x16 = librosa.resample(x, orig_sr=sr, target_sr=16000, res_type="soxr_hq") if rs == "soxr_hq" else \
              resample_poly(x, 16000 // np.gcd(sr, 16000), sr // np.gcd(sr, 16000)).astype(np.float32)
    else: x16 = x
    r = {"file": f}
    with torch.inference_mode():
        p = torch.softmax(ser(torch.from_numpy(x16)[None]).logits, -1)[0].numpy()
        r["ser"] = {l: float(v) for l, v in zip(labels, p)}; r["ser_top"] = labels[int(p.argmax())]
        if dim is not None:
            xn = (x16 - x16.mean()) / np.sqrt(x16.var() + 1e-7)
            a, d, v = dim(torch.from_numpy(xn.astype(np.float32))[None])[1][0].tolist()
            r["avd"] = {"arousal": a, "valence": v, "dominance": d}
    r["model_wall_s"] = time.perf_counter() - tw; r["model_cpu_s"] = time.process_time() - tc
    if not no_praat:
        import parselmouth
        from parselmouth.praat import call
        snd = parselmouth.Sound(x.astype(np.float64), sampling_frequency=sr)
        pitch = snd.to_pitch_ac(time_step=0.01, pitch_floor=60, pitch_ceiling=500)
        f0 = pitch.selected_array["frequency"]; f0 = f0[f0 > 0]
        med = np.median(f0); st = 12 * np.log2(f0 / med)
        pp = call(snd, "To PointProcess (periodic, cc)", 60, 500)
        jit = call(pp, "Get jitter (local)", 0, 0, 0.0001, 0.02, 1.3) * 100
        shim = call([snd, pp], "Get shimmer (local)", 0, 0, 0.0001, 0.02, 1.3, 1.6) * 100
        # energy DR with the same definition as Rust (25ms/10ms RMS dB, active = within 35 dB of max & > -55 dBFS, p95-p5)
        hop, win = 160, 400
        db = []
        for i in range(len(x16) // hop + 1):
            c = i * hop; a0 = max(0, c - win // 2); b0 = min(len(x16), c + win // 2)
            db.append(10 * np.log10(np.mean(x16[a0:b0] ** 2) + 1e-12) if b0 > a0 else -120)
        db = np.array(db); thr = max(db.max() - 35, -55); act = db[db > thr]
        r["praat"] = {"f0_mean_hz": float(f0.mean()), "f0_median_hz": float(med), "f0_sd_st": float(st.std(ddof=1)),
                      "f0_range_st_p5_p95": float(np.percentile(st, 95) - np.percentile(st, 5)),
                      "jitter_local_pct": float(jit), "shimmer_local_pct": float(shim),
                      "energy_dr_db": float(np.percentile(act, 95) - np.percentile(act, 5))}
    res.append(r)
json.dump({"resampler": rs, "model_load_s": load_s, "wall_s": time.perf_counter() - t_wall0,
           "cpu_s": time.process_time() - t_cpu0, "files": res}, open(out, "w"), indent=1)
print("done", out)
