#!/usr/bin/env python3
"""Persona-aware emotion_match CLI (opt-in; does not replace universal_match).

Computes the 0.83 gate over the persona's own target emotions only.
Out-of-set emotions (e.g. seductive on Jarvis) are n/a, not 0.

This CLI is the hook Agent 3 can call for a full rescore. It can reuse
cached universal JSON (windows + line scores) via --reuse-windows and
--reuse-lines so models do not have to run again.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "py"))

from detectors import load_detectors, e2v_probs  # noqa: E402
from persona_lib import (  # noqa: E402
    emotion_match, load_persona, resolve_persona_path, strength, NA,
)


def _as_rec(raw: dict) -> dict:
    rec = dict(raw)
    if "e2v" not in rec and raw.get("probs"):
        rec["e2v"] = {"probs": raw["probs"]}
    return rec


def load_json_list(path: Path, key_hint=None):
    doc = json.loads(path.read_text())
    if isinstance(doc, list):
        return doc
    for k in (key_hint, "files", "lines", "windows"):
        if k and isinstance(doc.get(k), list):
            return doc[k]
    raise ValueError(f"cannot find a list of records in {path}")


def main():
    ap = argparse.ArgumentParser(description="Persona-aware emotion_match (opt-in)")
    ap.add_argument("--persona", required=True)
    ap.add_argument("--moments-json", help="list of {category, window rec} or universal RESULTS.json")
    ap.add_argument("--reuse-windows", help="base_windows_universal.json or windows list")
    ap.add_argument("--reuse-lines", action="append", default=[], help="name=path to line-score JSON")
    ap.add_argument("--out", required=True)
    ap.add_argument("--plugin-dir", default=None)
    ap.add_argument(
        "--nebula-reference",
        default=None,
        help="JSON payload or list of payloads (weights + vad) for the offline nebula field. Does not change emotion_match.",
    )
    args = ap.parse_args()
    nebula_reference = None
    if args.nebula_reference:
        nebula_reference = json.loads(Path(args.nebula_reference).read_text())

    persona = load_persona(resolve_persona_path(args.persona))
    detectors = load_detectors(plugin_dir=Path(args.plugin_dir) if args.plugin_dir else None)

    moments = []
    if args.moments_json:
        doc = json.loads(Path(args.moments_json).read_text())
        if isinstance(doc, dict) and "moments" in doc:
            # universal RESULTS.json: moments reference windows by span; caller should also pass windows
            moments = doc["moments"]
        elif isinstance(doc, list):
            moments = doc
        else:
            raise ValueError("--moments-json must be a list or a RESULTS.json with moments")

    windows = []
    if args.reuse_windows:
        windows = [_as_rec(w) for w in load_json_list(Path(args.reuse_windows))]

    if moments and windows:
        # attach matching window recs by span or id if missing
        by_span = {}
        for w in windows:
            if "start_s" in w and "end_s" in w:
                by_span[f"{float(w['start_s']):.3f}-{float(w['end_s']):.3f}"] = w
            if w.get("id"):
                by_span[w["id"]] = w
        filled = []
        for m in moments:
            if m.get("window") or m.get("rec"):
                filled.append(m)
                continue
            span = m.get("span") or m.get("moment")
            w = by_span.get(span) or by_span.get(m.get("category"))
            mm = dict(m)
            if w is not None:
                mm["window"] = w
            filled.append(mm)
        moments = filled
    elif not moments and windows:
        # derive in-set moments from windows using decision strengths
        from persona_lib import ALIASES
        seen = []
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
                seen.append({"category": spec.id, "strength": best_s, "window": best})
        moments = seen

    sets = {}
    for spec in args.reuse_lines:
        name, path = spec.split("=", 1)
        lines = [_as_rec(x) for x in load_json_list(Path(path))]
        for i, ln in enumerate(lines):
            ln.setdefault("id", Path(str(ln.get("path") or ln.get("file") or f"t{i:02d}")).stem)
        sets[name] = emotion_match(
            line_recs=lines,
            moments=moments,
            persona=persona,
            detectors=detectors,
            nebula_reference=nebula_reference,
            nebula_reference_recs=windows,
        )

    # ceiling: score windows against themselves if we have them
    ceiling = None
    if windows:
        ceil_lines = []
        for i, w in enumerate(windows):
            rec = dict(w)
            rec.setdefault("id", w.get("id") or f"w{i:02d}")
            ceil_lines.append(rec)
        ceiling = emotion_match(
            line_recs=ceil_lines,
            moments=moments,
            persona=persona,
            detectors=detectors,
            nebula_reference=nebula_reference,
            nebula_reference_recs=windows,
        )

    out = {
        "schema": "galion-persona-match/1",
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
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out, indent=2))
    print(args.out)


if __name__ == "__main__":
    main()
