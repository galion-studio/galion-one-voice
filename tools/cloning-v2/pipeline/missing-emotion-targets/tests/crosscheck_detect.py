#!/usr/bin/env python3
"""Python cross-check of the Rust detector.

Reimplements universal_match.strength / find_moments presence logic against a
score_universal JSON (or TARGETS.json + original_windows.json) and diffs the
present/missing sets. Does not modify the scorer.
"""
from __future__ import annotations
import json, sys
from pathlib import Path

PRESENCE = 0.20
E2V_TRAINED = {
    "joyful": "happy", "happy": "happy", "aggressive": "angry",
    "disgust": "disgusted", "fear": "fearful", "sad": "sad",
    "neutral": "neutral", "surprise": "surprised",
}
EAT_STATES = ["yelling", "crying", "laughter"]
BLEND_IDS = [
    "joyful", "aggressive", "disgust", "fear", "sad", "neutral", "surprise",
    "love", "seductive", "yelling", "crying", "laughter", "logic",
]


def strength(cat, e2v_probs, clap_probs, eat_states=None):
    if cat in EAT_STATES:
        eat_states = eat_states or {}
        st = eat_states.get(cat) or {}
        if isinstance(st, dict):
            return float(st.get("max", 0.0) or 0.0)
        return float(st or 0.0)
    if cat in E2V_TRAINED:
        return float(e2v_probs.get(E2V_TRAINED[cat], 0.0))
    if cat in clap_probs:
        return float(clap_probs.get(cat, 0.0))
    return 0.0


def eat_of(rec):
    return rec.get("efficientat") or {}


def detect_files(files):
    out = {}
    for cat in BLEND_IDS:
        best = None
        mx = 0.0
        for rec in files:
            s = strength(cat, (rec.get("e2v") or {}).get("probs") or {},
                         (rec.get("clap") or {}).get("probs") or {}, eat_of(rec))
            mx = max(mx, s)
            if s + 1e-12 >= PRESENCE and (best is None or s > best[0]):
                best = (s, rec)
        out[cat] = {"present": best is not None, "strength": best[0] if best else mx}
    return out


def main():
    if len(sys.argv) < 2:
        print("usage: crosscheck_detect.py original_windows.json [TARGETS.json]", file=sys.stderr)
        return 2
    doc = json.loads(Path(sys.argv[1]).read_text())
    files = doc["files"]
    got = detect_files(files)
    print("python present:", [c for c, v in got.items() if v["present"]])
    print("python missing:", [c for c, v in got.items() if not v["present"]])
    for c, v in got.items():
        print(f"  {c:12} present={v['present']} strength={v['strength']:.10f}")
    if len(sys.argv) > 2:
        tgt = json.loads(Path(sys.argv[2]).read_text())
        mism = []
        for c, v in got.items():
            e = tgt["emotions"][c]
            origin = e["origin"]
            want = "original-present" if v["present"] else "synthesized"
            if origin != want:
                mism.append(f"{c}: rust origin={origin} python want={want}")
            if v["present"]:
                rs = float(e["target"])
                if abs(rs - v["strength"]) > 1e-9:
                    mism.append(f"{c}: rust target={rs} python={v['strength']}")
        if mism:
            print("MISMATCH:")
            for m in mism:
                print(" ", m)
            return 1
        print("cross-check OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
