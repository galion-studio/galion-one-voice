#!/usr/bin/env python3
"""Agent-3 local wrapper: same math as scorers/persona/py/persona_match.py, but
caches detector.score(rec) by wav path so love_seductive CLAP runs once per file.

Does not edit scorers/persona/. Writes only under results-persona/new/.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path("/workspace/swarm/voice-v2-sprint/cloning-v2")
sys.path.insert(0, str(ROOT / "scorers/persona"))
sys.path.insert(0, str(ROOT / "scorers/persona/py"))

from detectors import load_detectors  # noqa: E402
from persona_lib import (  # noqa: E402
    emotion_match, load_persona, resolve_persona_path, strength, NA,
)


class CachingDetector:
    def __init__(self, inner):
        self.inner = inner
        self.name = getattr(inner, "name", type(inner).__name__)
        self.categories = getattr(inner, "categories", ())
        self._cache: dict = {}

    def score(self, rec):
        key = rec.get("path") or rec.get("file") or id(rec)
        hit = self._cache.get(key)
        if hit is not None:
            return hit
        out = self.inner.score(rec) or {}
        self._cache[key] = out
        return out


def wrap_detectors(dets):
    return [CachingDetector(d) for d in dets]


def _as_rec(raw: dict) -> dict:
    rec = dict(raw)
    if "e2v" not in rec and raw.get("probs"):
        rec["e2v"] = {"probs": raw["probs"]}
    return rec


def load_json_list(path: Path):
    doc = json.loads(path.read_text())
    if isinstance(doc, list):
        return doc
    for k in ("files", "lines", "windows"):
        if isinstance(doc.get(k), list):
            return doc[k]
    raise ValueError(f"cannot find a list of records in {path}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--persona", required=True)
    ap.add_argument("--reuse-windows", required=True)
    ap.add_argument("--reuse-lines", action="append", default=[])
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    persona = load_persona(resolve_persona_path(args.persona))
    detectors = wrap_detectors(load_detectors())
    print(f"[cache] detectors={[d.name for d in detectors]}", flush=True)

    windows = [_as_rec(w) for w in load_json_list(Path(args.reuse_windows))]
    print(f"[cache] windows n={len(windows)}", flush=True)

    # Derive in-set moments (same as persona_match.py without --moments-json)
    moments = []
    for spec in persona.emotions:
        best = None
        best_s = -1.0
        for w in windows:
            s = strength(spec.id, w, persona=persona, detectors=detectors)
            if s == NA or s is None:
                continue
            s = float(s)
            if s + 1e-12 >= persona.presence and s > best_s:
                best_s = s
                best = w
        if best is not None:
            moments.append({"category": spec.id, "strength": best_s, "window": best})
            print(f"[moment] {spec.id} strength={best_s:.6f} path={best.get('path') or best.get('id')}", flush=True)
        else:
            print(f"[moment] {spec.id} ABSENT (< presence {persona.presence})", flush=True)

    sets = {}
    for spec in args.reuse_lines:
        name, path = spec.split("=", 1)
        lines = [_as_rec(x) for x in load_json_list(Path(path))]
        for i, ln in enumerate(lines):
            ln.setdefault("id", Path(str(ln.get("path") or ln.get("file") or f"t{i:02d}")).stem)
        print(f"[set] {name} n={len(lines)}", flush=True)
        sets[name] = emotion_match(
            line_recs=lines, moments=moments, persona=persona, detectors=detectors
        )
        print(
            f"[set] {name} emotion_match={sets[name].get('emotion_match')} "
            f"pass={sets[name].get('pass')}",
            flush=True,
        )

    ceiling = None
    if windows:
        ceil_lines = []
        for i, w in enumerate(windows):
            rec = dict(w)
            rec.setdefault("id", w.get("id") or f"w{i:02d}")
            ceil_lines.append(rec)
        ceiling = emotion_match(
            line_recs=ceil_lines, moments=moments, persona=persona, detectors=detectors
        )
        print(
            f"[ceiling] emotion_match={ceiling.get('emotion_match')} pass={ceiling.get('pass')}",
            flush=True,
        )

    # cache stats
    cache_stats = {
        d.name: len(d._cache) for d in detectors if hasattr(d, "_cache")
    }

    out = {
        "schema": "galion-persona-match/1",
        "wrapper": "results-persona/persona_match_cached.py (Agent 3; path-cached detectors)",
        "persona": {
            "id": persona.id,
            "display_name": persona.display_name,
            "gate": persona.gate,
            "judged": persona.judged_ids(),
            "path": str(persona.path) if persona.path else None,
        },
        "BASE-self-ceiling": ceiling,
        "sets": sets,
        "moments_n": len(moments),
        "moments": [
            {
                "category": m["category"],
                "strength": m["strength"],
                "path": (m["window"].get("path") or m["window"].get("file")),
                "id": m["window"].get("id"),
                "start_s": m["window"].get("start_s"),
                "end_s": m["window"].get("end_s"),
            }
            for m in moments
        ],
        "detector_cache_sizes": cache_stats,
        "love_seductive_plugin": str(
            ROOT / "scorers/persona/detectors/love_seductive.py"
        ),
        "love_seductive_mtime": Path(
            ROOT / "scorers/persona/detectors/love_seductive.py"
        ).stat().st_mtime,
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out, indent=2))
    print(args.out, flush=True)


if __name__ == "__main__":
    main()
