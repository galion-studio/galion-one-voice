#!/usr/bin/env python3
"""Build VALIDATE12.md from scores/{tag}_{emo,ecapa,pros,wer,mos,hygiene,artic}.json(+l)."""
from __future__ import annotations
import json, math, statistics
from pathlib import Path
from collections import Counter

ROOT = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix")
SCORES = ROOT / "scores"
RUNS = ROOT / "runs"
TAGS = [
    ("sam_base12", "baseline own-samantha (cbx-emo, t=0)"),
    ("sd_r2_up12", "r2-t0.8 (dev settings)"),
    ("sd_r2_t012", "r2-t0"),
]
# Also include production baseline scores if present (sam_old)
EXTRA = [("sam_old", "production baseline (own-clone-analysis, prior binary)")]

def load_json(p: Path):
    if not p.exists():
        return None
    return json.loads(p.read_text())

def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None

def persona_dists(tag: str):
    """Prefer batch.jsonl cand dist; else recompute from pros + target."""
    batch = RUNS / tag / "batch.jsonl"
    dists = []
    if batch.exists():
        for line in batch.read_text().splitlines():
            if not line.strip():
                continue
            o = json.loads(line)
            if "cand" in o and "dist" in o:
                dists.append(o["dist"])
    if dists:
        return mean(dists), dists
    # fallback: compute from pros.jsonl + target
    target = json.loads((ROOT / "targets/samantha.json").read_text())
    pros_path = SCORES / f"{tag}_pros.jsonl"
    if not pros_path.exists():
        return None, []
    dists = []
    for line in pros_path.read_text().splitlines():
        if not line.strip():
            continue
        p = json.loads(line)["prosody"]
        dists.append(prosody_distance(p, target))
    return mean(dists), dists

def prosody_distance(p, t):
    # Mirror cbx-emo/src/prosody.rs:234-254
    def st(a, b):
        return 12.0 * math.log2(max(a, 1e-9) / max(b, 1e-9))
    f0_err = abs(st(p["f0_median_hz"], t["f0_median_hz"]))
    rate_err = abs(math.log(max(p["rate_wps"], 1e-9) / max(t["rate_wps"], 1e-9))) / 0.10
    er_err = 0.5 * abs(p["energy_range_db"] - t["energy_range_db"]) / 3.0
    tilt_err = 0.5 * abs(p["tilt_db"] - t["tilt_db"]) / 3.0
    f0sd_err = 0.25 * abs(p["f0_sd_st"] - t["f0_sd_st"])
    d = f0_err + rate_err + er_err + tilt_err + f0sd_err
    # runaway rate guard (approx from source)
    if p["rate_wps"] > t["rate_wps"] * 1.8:
        d += 5.0
    return d

def summarize_tag(tag: str):
    emo = load_json(SCORES / f"{tag}_emo.json")
    ecapa = load_json(SCORES / f"{tag}_ecapa.json")
    wer = load_json(SCORES / f"{tag}_wer.json")
    mos = load_json(SCORES / f"{tag}_mos.json")
    hyg = load_json(SCORES / f"{tag}_hygiene.json")
    artic = load_json(SCORES / f"{tag}_artic.json")
    if emo is None or ecapa is None:
        return None
    files = {Path(f["file"]).stem: f for f in emo["files"]}
    labels = Counter(f["ser"]["top"] for f in emo["files"])
    pd_mean, pd_list = persona_dists(tag)
    # map ecapa scores by stem
    ecapa_scores = {}
    for k, v in ecapa.get("scores", {}).items():
        ecapa_scores[Path(k).stem] = v
    wer_by = {}
    if wer:
        for f in wer.get("files", []):
            wer_by[Path(f["path"]).stem] = f.get("wer")
    mos_by = {}
    if mos:
        for f in mos.get("files", mos if isinstance(mos, list) else []):
            if isinstance(f, dict):
                path = f.get("path") or f.get("file")
                if path:
                    # subjective mos field name variants
                    mos_by[Path(path).stem] = f.get("mos_subjective") or f.get("subjective") or f.get("mos")
    # summary mos
    mos_mean = None
    if mos:
        if "summary" in mos and isinstance(mos["summary"], dict):
            mos_mean = mos["summary"].get("mean_subjective") or mos["summary"].get("mean_mos")
        if mos_mean is None and mos_by:
            mos_mean = mean(mos_by.values())
        if mos_mean is None:
            # try flat
            vals = []
            for f in mos.get("files", []):
                for key in ("mos_subjective", "subjective", "mos", "squim_subjective"):
                    if key in f:
                        vals.append(f[key]); break
                if "subjective" in f and isinstance(f["subjective"], dict) and "mos" in f["subjective"]:
                    vals.append(f["subjective"]["mos"])
            mos_mean = mean(vals)
    wer_mean = None
    if wer:
        wer_mean = wer.get("summary", {}).get("mean_wer") if isinstance(wer.get("summary"), dict) else None
        if wer_mean is None and wer_by:
            wer_mean = mean(wer_by.values())
        if wer_mean is None:
            wer_mean = mean([f.get("wer") for f in wer.get("files", [])])

    per_line = []
    for tid in [f"t{i:02d}" for i in range(1, 13)]:
        f = files.get(tid)
        if not f:
            continue
        ser = f["ser"]
        per_line.append({
            "id": tid,
            "label": ser["top"],
            "ang": ser["probs"]["ang"],
            "hap": ser["probs"].get("hap"),
            "neu": ser["probs"]["neu"],
            "sad": ser["probs"].get("sad"),
            "emotion_distance": f["vs_ref"]["emotion_distance"],
            "ecapa": ecapa_scores.get(tid),
            "wer": wer_by.get(tid),
            "mos": mos_by.get(tid),
        })

    return {
        "tag": tag,
        "n": len(emo["files"]),
        "emotion_distance": emo["summary"]["mean_emotion_distance"],
        "avd_distance": emo["summary"]["mean_avd_dist"],
        "ecapa": ecapa.get("mean"),
        "wer": wer_mean,
        "mos": mos_mean,
        "persona_distance": pd_mean,
        "labels": dict(labels),
        "per_line": per_line,
        "f0_hz": mean([f["proxies"]["f0_median_hz"] for f in emo["files"]]),
        "hyg": hyg,
        "artic": artic,
    }

def fmt(x, nd=4):
    if x is None:
        return "—"
    if isinstance(x, float):
        return f"{x:.{nd}f}"
    return str(x)

def main():
    rows = []
    for tag, _ in TAGS + EXTRA:
        s = summarize_tag(tag)
        if s:
            rows.append(s)
    # Prefer primary three
    primary = [summarize_tag(t) for t,_, in [(a,b) for a,b in TAGS]]
    primary = [p for p in primary if p]

    lines = []
    lines.append("# VALIDATE12 — own-samantha-v2-emo-r2 on full 12 lines")
    lines.append("")
    lines.append(f"Completed (Europe/Zagreb). Real measured scores only. Renders via `cbx-emo` fp32, `--intra-threads 2`, `nice -n 10`, `--seed 0`.")
    lines.append("")
    lines.append("## Summary (n=12)")
    lines.append("")
    lines.append("| Voice / setting | ECAPA ↑ | WER ↓ | MOS ↑ | Emotion dist ↓ | Persona dist ↓ | SER labels |")
    lines.append("|---|---:|---:|---:|---:|---:|---|")
    label_order = ["sam_base12", "sd_r2_up12", "sd_r2_t012", "sam_old"]
    names = {
        "sam_base12": "baseline own-samantha (cbx-emo t=0)",
        "sd_r2_up12": "r2-t0.8 (dev: temp 0.8, top-k 1000, top-p 0.95, rep-unique, punc-norm)",
        "sd_r2_t012": "r2-t0 (temperature 0 defaults)",
        "sam_old": "production baseline (prior speak binary, for reference)",
    }
    by = {r["tag"]: r for r in rows}
    for tag in label_order:
        r = by.get(tag)
        if not r:
            continue
        lab = r["labels"]
        lab_s = ", ".join(f"{k} {lab[k]}" for k in ("ang", "hap", "neu", "sad") if lab.get(k))
        lines.append(
            f"| {names.get(tag, tag)} | {fmt(r['ecapa'])} | {fmt(r['wer'], 3) if r['wer'] is not None else '—'} | {fmt(r['mos'], 3)} | {fmt(r['emotion_distance'])} | {fmt(r['persona_distance'])} | {lab_s} |"
        )
    lines.append("")

    # Per-line label table for the three primary
    lines.append("## Per-line SER labels and angry/neutral probs")
    lines.append("")
    for tag in ("sam_base12", "sd_r2_up12", "sd_r2_t012"):
        r = by.get(tag)
        if not r:
            continue
        lines.append(f"### {names[tag]}")
        lines.append("")
        lines.append("| Line | Label | P(ang) | P(neu) | P(hap) | Emotion dist | ECAPA |")
        lines.append("|---|---|---:|---:|---:|---:|---:|")
        for pl in r["per_line"]:
            lines.append(
                f"| {pl['id']} | {pl['label']} | {pl['ang']:.4f} | {pl['neu']:.4f} | {pl['hap']:.4f} | {pl['emotion_distance']:.4f} | {fmt(pl['ecapa'])} |"
            )
        lines.append("")

    # Label changes baseline -> r2 variants
    lines.append("## Label changes vs matched-binary baseline")
    lines.append("")
    base = by.get("sam_base12")
    if base:
        base_lab = {pl["id"]: pl["label"] for pl in base["per_line"]}
        for tag in ("sd_r2_up12", "sd_r2_t012"):
            r = by.get(tag)
            if not r:
                continue
            changes = []
            for pl in r["per_line"]:
                b = base_lab.get(pl["id"])
                if b and b != pl["label"]:
                    changes.append(f"{pl['id']}: {b}→{pl['label']}")
            lines.append(f"- **{tag}**: " + (", ".join(changes) if changes else "no label changes"))
        lines.append("")

    # Verdict
    lines.append("## Verdict")
    lines.append("")
    base = by.get("sam_base12") or by.get("sam_old")
    r28 = by.get("sd_r2_up12")
    r20 = by.get("sd_r2_t012")
    def holds(cand, baseline, name):
        if not cand or not baseline:
            return f"- **{name}**: incomplete scores"
        ecapa_ok = cand["ecapa"] >= baseline["ecapa"] - 0.01
        emo_ok = cand["emotion_distance"] < baseline["emotion_distance"]
        verdict = "HOLD" if (ecapa_ok and emo_ok) else "DOES NOT HOLD"
        lines.append(
            f"- **{name} vs {baseline['tag']}**: ECAPA {cand['ecapa']:.4f} vs {baseline['ecapa']:.4f} (floor {baseline['ecapa']-0.01:.4f}) → {'OK' if ecapa_ok else 'FAIL'}; "
            f"emotion dist {cand['emotion_distance']:.4f} vs {baseline['emotion_distance']:.4f} → {'better' if emo_ok else 'not better'}. **{verdict}**"
        )
        return verdict
    v1 = holds(r28, base, "r2-t0.8") if base else None
    v2 = holds(r20, base, "r2-t0") if base else None
    lines.append("")
    if base and r28:
        overall = "YES" if (r28["ecapa"] >= base["ecapa"] - 0.01 and r28["emotion_distance"] < base["emotion_distance"]) else "NO"
        lines.append(f"**Does r2 hold on 12 lines (primary: r2-t0.8 vs baseline)? {overall}.**")
        if r20:
            overall0 = "YES" if (r20["ecapa"] >= base["ecapa"] - 0.01 and r20["emotion_distance"] < base["emotion_distance"]) else "NO"
            lines.append(f"**Does r2-t0 hold on 12 lines? {overall0}.**")
    lines.append("")
    lines.append("## Notes")
    lines.append("")
    lines.append("- Identity guard: ECAPA not below baseline − 0.01.")
    lines.append("- Emotion distance: galion-emotion fp32 vs `clip-store-own/samantha/reference.wav`.")
    lines.append("- Persona distance: cbx-emo prosody::distance to `targets/samantha.json` (from persona reference proxies).")
    lines.append("- Originals / own-samantha / .bak untouched; no commits/pushes.")
    lines.append("")

    out = ROOT / "VALIDATE12.md"
    out.write_text("\n".join(lines) + "\n")
    print(f"wrote {out} ({len(lines)} lines)")
    # also dump machine-readable
    (ROOT / "analysis" / "validate12.json").write_text(json.dumps({r["tag"]: r for r in rows}, indent=2, default=str))
    print("tags:", list(by))

if __name__ == "__main__":
    main()
