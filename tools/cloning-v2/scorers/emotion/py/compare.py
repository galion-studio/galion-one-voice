"""Compare galion-emotion (Rust) JSON vs crosscheck.py (Python) JSON. stdlib only."""
import json, sys, statistics as st
rust = json.load(open(sys.argv[1])); py = json.load(open(sys.argv[2]))
orig = json.load(open("/workspace/swarm/voice-v2-sprint/own-clone-analysis/scores/emotion_superb.json"))
rf = ([rust["reference"]] if rust.get("reference") else []) + rust["files"]
rmap = {f["file"]: f for f in rf}
def key(f):  # map to own-clone-analysis original labels
    parts = f.split("/")
    who = "samantha" if "samantha" in f else "jarvis"
    if parts[-1] == "reference.wav": return orig[who]["reference"]
    return orig[who]["clones"].get(parts[-1][:-4])
rows = []; agree = 0; agree_orig = 0; pyorig = 0; n = 0
dprob = []; davd = {"arousal": [], "valence": [], "dominance": []}; dpx = {}
for p in py["files"]:
    r = rmap.get(p["file"])
    if r is None: continue
    n += 1
    o = key(p["file"])
    if r.get("ser"):
        agree += r["ser"]["top"] == p["ser_top"]
        if o: agree_orig += r["ser"]["top"] == o["top"]; pyorig += p["ser_top"] == o["top"]
        dprob.append(max(abs(r["ser"]["probs"][k] - p["ser"][k]) for k in p["ser"]))
    if r.get("avd") and p.get("avd"):
        for k in davd: davd[k].append(abs(r["avd"][k] - p["avd"][k]))
    for k, v in p.get("praat", {}).items():
        rv = r["proxies"].get(k)
        if rv is not None: dpx.setdefault(k, []).append((rv, v))
    rows.append((p["file"].split("voice-v2-sprint/")[-1], r["ser"]["top"] if r.get("ser") else "-", p["ser_top"], o["top"] if o else "-",
                 round(dprob[-1], 4) if r.get("ser") else None,
                 {k: round(r["avd"][k] - p["avd"][k], 4) for k in davd} if r.get("avd") and p.get("avd") else None))
print(f"files: {n}")
if dprob:
    print(f"SER top-label agreement Rust vs Python(this run): {agree}/{n}; max|dprob| mean {st.mean(dprob):.4f} max {max(dprob):.4f}")
    print(f"SER top-label agreement Rust vs original REPORT labels: {agree_orig}/{n}; Python(this run) vs original: {pyorig}/{n}")
for k, v in davd.items():
    if v: print(f"AVD |d{k}| mean {st.mean(v):.4f} max {max(v):.4f}")
print("proxy (Rust vs Praat/numpy): mean Rust, mean Py, mean |diff|, Pearson r")
for k, pairs in dpx.items():
    a = [x for x, _ in pairs]; b = [y for _, y in pairs]
    try: r_ = st.correlation(a, b)
    except Exception: r_ = float("nan")
    print(f"  {k:22s} {st.mean(a):8.3f} {st.mean(b):8.3f} {st.mean(abs(x-y) for x,y in pairs):8.3f}  r={r_:.3f}")
print("per-file: file | rust | py | orig | max|dprob| | dAVD")
for r in rows: print(" ", *r)
