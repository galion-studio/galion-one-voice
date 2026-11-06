"""Cross-check galion-artic frame tracks (F0, F1-F3) against praat-parselmouth at identical frame times.
usage: crosscheck.py BIN file.wav [file.wav...]  -> JSON summary on stdout"""
import sys, json, subprocess, numpy as np, parselmouth
from parselmouth.praat import call
binp, files = sys.argv[1], sys.argv[2:]
out = {}
for f in files:
    r = json.loads(subprocess.check_output(["nice","-n","10",binp,"profile","--frames","--compact"]+__import__("os").environ.get("ARTIC_ARGS","").split()+[f]))
    p = r["files"][0]; fr = p["frames"]; t = np.array(fr["t"])
    ceil = p["analysis"]["formant_ceiling_hz"]
    snd = parselmouth.Sound(f)
    if snd.n_channels > 1: snd = snd.convert_to_mono()
    pit = snd.to_pitch_ac(time_step=0.01, pitch_floor=65, pitch_ceiling=600)
    pf = np.array([pit.get_value_at_time(x) or np.nan for x in t], dtype=float)
    pf = np.nan_to_num(pf, nan=0.0)
    fo = snd.to_formant_burg(time_step=0.01, max_number_of_formants=5, maximum_formant=ceil, window_length=0.025, pre_emphasis_from=50)
    pr = {k: np.array([fo.get_value_at_time(k, x) for x in t], dtype=float) for k in (1,2,3)}
    res = {"formant_ceiling": ceil, "n_frames": len(t)}
    for name in ("f0", "f0_yin"):
        rf = np.array([v or 0.0 for v in fr[name]], dtype=float)
        both = (rf > 0) & (pf > 0)
        cents = 1200*np.log2(rf[both]/pf[both]) if both.any() else np.array([])
        res[name] = {"praat_voiced_frames": int((pf>0).sum()), "rust_voiced_frames": int((rf>0).sum()), "both_voiced": int(both.sum()),
            "voicing_agreement": round(float(((rf>0)==(pf>0)).mean()),3),
            "median_abs_cents": round(float(np.median(np.abs(cents))),2) if len(cents) else None,
            "gross_error_rate_20pct": round(float((np.abs(cents) > 1200*np.log2(1.2)).mean()),4) if len(cents) else None,
            "median_hz_rust_vs_praat": [round(float(np.median(rf[rf>0])),2) if (rf>0).any() else None, round(float(np.median(pf[pf>0])),2)]}
    # formants on frames voiced by Praat & loud
    idb = np.array(fr["int_db"], dtype=float); thr = p["analysis"]["silence_threshold_db"]
    mask = (pf > 0) & (idb > thr)
    for k in (1,2,3):
        rv = np.array([v if v is not None else np.nan for v in fr[f"f{k}"]], dtype=float)
        m = mask & np.isfinite(rv) & np.isfinite(pr[k])
        d = rv[m]-pr[k][m]; rel = np.abs(d)/pr[k][m]
        res[f"F{k}"] = {"n": int(m.sum()), "median_abs_hz": round(float(np.median(np.abs(d))),1), "mean_abs_hz": round(float(np.mean(np.abs(d))),1),
                        "median_abs_pct": round(float(100*np.median(rel)),2), "frac_within_5pct": round(float((rel<0.05).mean()),3), "frac_within_10pct": round(float((rel<0.10).mean()),3),
                        "median_rust_vs_praat": [round(float(np.median(rv[m])),1), round(float(np.median(pr[k][m])),1)]}
    out[f] = res
print(json.dumps(out, indent=1))
