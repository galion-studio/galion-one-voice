#!/usr/bin/env python3
"""Persona scorer CLI.

Scores wavs with the same underlying models as scorers/universal
(emotion2vec+, EfficientAT, CLAP) then applies persona detectors:
  crying = emotion2vec+ sad (EfficientAT crying is logged only)
  trained classes and yelling/laughter/logic unchanged
  love/seductive from detectors/love_seductive* if present, else legacy CLAP

Does not modify scorers/universal/. Opt-in only.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UNI_PY = ROOT.parent / "universal" / "py"
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "py"))
sys.path.insert(0, str(UNI_PY))

from detectors import decision_scores, load_detectors  # noqa: E402
from persona_lib import apply_persona_scores, load_persona, resolve_persona_path  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description="Persona-aware emotion file scorer (opt-in)")
    ap.add_argument("wavs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--persona", default=None, help="path or own-samantha / own-jarvis")
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--ref", default=None)
    ap.add_argument("--skip-clap", action="store_true")
    ap.add_argument("--skip-e2v", action="store_true")
    ap.add_argument("--skip-efficientat", action="store_true")
    ap.add_argument("--reuse-json", default=None, help="reuse e2v/clap/efficientat from a prior score_universal JSON")
    ap.add_argument("--plugin-dir", default=None, help="override detectors/ directory for love_seductive* discovery")
    args = ap.parse_args()

    persona = None
    if args.persona:
        persona = load_persona(resolve_persona_path(args.persona))

    plugin_dir = Path(args.plugin_dir) if args.plugin_dir else None
    detectors = load_detectors(plugin_dir=plugin_dir)

    t0 = time.time()
    files = []
    if args.reuse_json:
        doc = json.loads(Path(args.reuse_json).read_text())
        by_name = {}
        by_path = {}
        for rec in doc.get("files") or []:
            fp = rec.get("file") or rec.get("path") or ""
            by_path[fp] = rec
            by_name[Path(fp).name] = rec
        for wav in args.wavs:
            rec = dict(by_path.get(str(Path(wav).resolve())) or by_path.get(wav) or by_name.get(Path(wav).name) or {})
            if not rec:
                rec = {"file": str(Path(wav).resolve()), "reuse_miss": True}
            overlay = apply_persona_scores(rec, persona=persona, detectors=detectors)
            rec = dict(rec)
            rec.update(overlay)
            files.append(rec)
    else:
        from score_universal import (  # imported only when we need models
            ClapZeroShot, Emotion2VecORT, load_wav, load_wav_16k,
            mean_centred_cosine, score_efficientat_files, wait_mem,
        )
        wait_mem(3000)
        e2v = None if args.skip_e2v else Emotion2VecORT(threads=args.threads)
        clap = None
        if not args.skip_clap:
            wait_mem(3000)
            clap = ClapZeroShot()
        ref_e2v = None
        if args.ref and e2v is not None:
            ref_e2v = e2v.score(load_wav(args.ref, 16000))
        for p in args.wavs:
            wait_mem(2500)
            audio16 = load_wav_16k(p)
            rec = {"file": str(Path(p).resolve()), "duration_s": float(len(audio16) / 16000.0)}
            if e2v is not None:
                rec["e2v"] = e2v.score(audio16)
                if ref_e2v is not None:
                    rec["e2v"]["emb_cosine_mean_centred_vs_ref"] = mean_centred_cosine(
                        rec["e2v"]["embedding"], ref_e2v["embedding"]
                    )
            if clap is not None:
                rec["clap"] = clap.score(load_wav(p, 48000), sr=48000)
            files.append(rec)
        if not args.skip_efficientat:
            eat_by = score_efficientat_files([Path(p) for p in args.wavs], threads=args.threads)
            if eat_by:
                for rec in files:
                    key = rec["file"]
                    st = eat_by.get(key) or eat_by.get(str(Path(key)))
                    if st is not None:
                        rec["efficientat"] = st
                        rec["efficientat_source"] = "efficientat"
        for rec in files:
            rec.update(apply_persona_scores(rec, persona=persona, detectors=detectors))

    plugin_names = [d.name for d in detectors]
    out = {
        "schema": "galion-persona-emotion/1",
        "backend": "persona",
        "default": False,
        "persona": None if persona is None else {
            "id": persona.id,
            "display_name": persona.display_name,
            "gate": persona.gate,
            "presence": persona.presence,
            "judged": persona.judged_ids(),
            "path": str(persona.path) if persona.path else None,
        },
        "detectors_loaded": plugin_names,
        "crying": "emotion2vec+ sad (EfficientAT crying logged only)",
        "files": files,
        "timing": {"wall_s": time.time() - t0},
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out))
    print(args.out)


if __name__ == "__main__":
    main()
