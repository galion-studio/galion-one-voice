#!/usr/bin/env python3
"""Extract OLD (universal) column from existing results. No re-scoring, no writes outside results-persona/."""
from __future__ import annotations
import json
from pathlib import Path
from datetime import datetime, timezone, timedelta

ROOT = Path("/workspace/swarm/voice-v2-sprint/cloning-v2")
PROBE = ROOT / "scorer-probe"
OUT = PROBE / "results-persona" / "old"
PRESENCE = 0.2
GATE = 0.83
ZAGREB = timezone(timedelta(hours=1), name="Europe/Zagreb")

# Map probe group -> primary detector field used by universal scorer
PRIMARY = {
    "crying": ("efficientat_max", "crying"),
    "seductive": ("clap_probs", "seductive"),
    "love": ("clap_probs", "love"),
    "laughter": ("efficientat_max", "laughter"),
    "joyful": ("e2v", "happy"),
    "aggressive": ("e2v", "angry"),
    "disgust": ("e2v", "disgusted"),
    "fear": ("e2v", "fearful"),
    "sad": ("e2v", "sad"),
    "surprise": ("e2v", "surprised"),
    "neutral": ("e2v", "neutral"),
}

def now():
    return datetime.now(ZAGREB).isoformat()

def load_match(voice: str) -> dict:
    return json.loads((ROOT / f"scorers/universal/results/{voice}-universal/RESULTS.json").read_text())

def pack_leaderboard(lb):
    return {c["category"]: {
        "score": c["score"],
        "strength": c["strength"],
        "source": c["source"],
        "best_line": c.get("best_line"),
        "best_match": c.get("best_match"),
        "mean_match": c.get("mean_match"),
        "moment": c.get("moment"),
    } for c in lb}

def pack_set(name, block, renders=None):
    em = block["emotion_match"]
    return {
        "set": name,
        "n": block.get("n"),
        "renders": renders or block.get("renders"),
        "emotion_match": em,
        "category_blend": block["category_blend"],
        "emb_sim": block["emb_sim"],
        "gate_0_83": bool(em is not None and em >= GATE),
        "per_emotion": pack_leaderboard(block["leaderboard"]),
        "source_file": None,  # filled by caller
    }

def primary_score(line, group):
    kind, key = PRIMARY[group]
    if kind == "efficientat_max":
        return float(line["efficientat_max"].get(key, 0.0))
    if kind == "clap_probs":
        return float(line["clap_probs"].get(key, 0.0))
    if kind == "e2v":
        return float(line["e2v"].get(key, 0.0))
    return None

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    summary = json.loads((PROBE / "results/SUMMARY.json").read_text())
    probe_raw = json.loads((PROBE / "results/probe_universal_all.json").read_text())

    # index probe raw by persona/id
    raw_by = {}
    for f in probe_raw["files"]:
        path = f["file"]
        persona = "samantha" if "/samantha/" in path else "jarvis" if "/jarvis/" in path else None
        if not persona:
            continue
        stem = Path(path).stem
        raw_by[(persona, stem)] = f

    out = {
        "schema": "galion-persona-rescore-old/1",
        "scorer": "universal (unchanged)",
        "generated_at": now(),
        "timezone": "Europe/Zagreb (UTC+1)",
        "presence_threshold": PRESENCE,
        "gate": GATE,
        "reused_inputs": {
            "probe_summary": str(PROBE / "results/SUMMARY.json"),
            "probe_raw": str(PROBE / "results/probe_universal_all.json"),
            "samantha_match": str(ROOT / "scorers/universal/results/samantha-universal/RESULTS.json"),
            "jarvis_match": str(ROOT / "scorers/universal/results/jarvis-universal/RESULTS.json"),
            "note": "Same wav inputs as current clone renders and probe; no re-render, no re-score.",
        },
        "voices": {},
    }

    for voice in ("samantha", "jarvis"):
        match = load_match(voice)
        src = str(ROOT / f"scorers/universal/results/{voice}-universal/RESULTS.json")
        ceil = pack_set("ceiling", match["BASE-self-ceiling"],
                        renders=str(ROOT.parent / f"clip-store-own/{voice}/original.wav"))
        ceil["source_file"] = src
        ceil["moments_present"] = [
            {"category": m["category"], "strength": m["strength"], "source": m["source"], "span": m["span"]}
            for m in match["moments"]
        ]
        sets = {"ceiling": ceil}
        for set_name, block in match["sets"].items():
            # normalize coherent alias
            alias = "coherent" if set_name == "global-fix-coherent" else set_name
            packed = pack_set(alias, block)
            packed["source_file"] = src
            packed["universal_set_key"] = set_name
            sets[alias] = packed

        # probe lines
        probe_lines = []
        probe_by_emotion = {}
        for line in summary["lines"]:
            if line["persona"] != voice:
                continue
            group = line["group"]
            score = primary_score(line, group)
            detected = bool(score is not None and score >= PRESENCE)
            # also keep SUMMARY's detected_0.2 primary if present
            det_map = line.get("detected_0.2") or {}
            entry = {
                "id": line["id"],
                "group": group,
                "path": line["path"],
                "duration_s": line["duration_s"],
                "primary_score": score,
                "primary_detector": f"{PRIMARY[group][0]}.{PRIMARY[group][1]}",
                "detected_ge_0.2": detected,
                "detected_0.2_map": det_map,
                "e2v_top": line["e2v_top"],
                "e2v_top_conf": line["e2v_top_conf"],
                "e2v_sad": line["e2v"]["sad"],
                "e2v_happy": line["e2v"]["happy"],
                "efficientat_crying_max": line["efficientat_max"]["crying"],
                "clap_seductive": line["clap_probs"]["seductive"],
                "clap_love": line["clap_probs"]["love"],
            }
            # attach raw if present
            raw = raw_by.get((voice, line["id"]))
            if raw:
                entry["raw_file"] = raw["file"]
            probe_lines.append(entry)
            probe_by_emotion.setdefault(group, {"n": 0, "scores": [], "detected_n": 0})
            probe_by_emotion[group]["n"] += 1
            probe_by_emotion[group]["scores"].append(score)
            probe_by_emotion[group]["detected_n"] += int(detected)

        probe_means = {}
        for g, info in probe_by_emotion.items():
            scores = info["scores"]
            mean = sum(scores) / len(scores) if scores else None
            probe_means[g] = {
                "n": info["n"],
                "mean_primary_score": mean,
                "detect_rate_0.2": info["detected_n"] / info["n"] if info["n"] else None,
                "primary_detector": f"{PRIMARY[g][0]}.{PRIMARY[g][1]}",
            }

        out["voices"][voice] = {
            "sets": sets,
            "probe": {
                "set": "original-voice-probe",
                "n_lines": len(probe_lines),
                "lines": probe_lines,
                "per_emotion": probe_means,
                "source_file": str(PROBE / "results/SUMMARY.json"),
                "note": "Probe scores are per-line primary-detector values, not emotion_match vs moments.",
            },
        }

    (OUT / "OLD.json").write_text(json.dumps(out, indent=2))
    # also a compact gate table
    rows = []
    for voice, v in out["voices"].items():
        for sname, s in v["sets"].items():
            rows.append({
                "voice": voice, "set": sname,
                "emotion_match": s["emotion_match"],
                "gate_0_83": s["gate_0_83"],
                "category_blend": s["category_blend"],
                "emb_sim": s["emb_sim"],
            })
    (OUT / "OLD-gates.json").write_text(json.dumps(rows, indent=2))
    print(OUT / "OLD.json")
    for r in rows:
        print(f"  {r['voice']:9s} {r['set']:10s} em={r['emotion_match']:.6f} gate={'PASS' if r['gate_0_83'] else 'FAIL'}")

if __name__ == "__main__":
    main()
