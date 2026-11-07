"""Validate galion-artic syllable-nuclei counts against text-derived syllable counts (heuristic English syllabifier)."""
import json, re, sys, subprocess
def syl(w):
    w = w.lower().strip("'")
    w = re.sub(r"[^a-z]", "", w)
    if not w: return 0
    if w in ("dna",): return 3
    if w in ("i'm","im"): return 1
    w2 = re.sub(r"(?:[^laeiouy]es|ed|[^laeiouy]e)$", "", w) if len(w) > 3 else w
    w2 = re.sub(r"^y", "", w2)
    n = len(re.findall(r"[aeiouy]{1,2}", w2))
    return max(1, n)
def count(t): return sum(syl(w) for w in re.findall(r"[A-Za-z']+", t))
binp, outp = sys.argv[1], sys.argv[2]
pairs = json.loads(sys.argv[3])  # [[wav, textfile], ...]
r = json.loads(subprocess.check_output(["nice","-n","10",binp,"profile","-j","2","--compact"]+[p[0] for p in pairs]))
rows = []
for (w, tf), p in zip(pairs, r["files"]):
    n_text = count(open(tf).read()); n_det = p["rate"]["n_syllables"]
    rows.append({"wav": w, "text_syl": n_text, "detected": n_det, "ratio": round(n_det / n_text, 3)})
import statistics as st
summ = {"n": len(rows), "mean_ratio": round(st.mean(x["ratio"] for x in rows),3), "mean_abs_err_pct": round(100*st.mean(abs(1-x["ratio"]) for x in rows),1),
        "corr_text_vs_detected": round(__import__("numpy").corrcoef([x["text_syl"] for x in rows],[x["detected"] for x in rows])[0,1],3)}
json.dump({"summary": summ, "rows": rows}, open(outp, "w"), indent=1); print(json.dumps(summ))
