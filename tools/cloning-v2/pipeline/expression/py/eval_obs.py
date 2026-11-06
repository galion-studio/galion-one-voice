#!/usr/bin/env python3
"""One-clip CLAP love/seductive observation for galion-expression eval.

Uses the persona love plugin's prompt banks. Weights stay on the machine
that already has them (laion/clap-htsat-fused under scorers/universal/clap).
Set GALION_CLAP_DEVICE=cuda for one sequential pass on an 8 GB card.
Does not load Qwen. Does not call Piper.
"""
from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve()
CLONING = HERE.parents[3]
DET = CLONING / "scorers" / "persona" / "detectors"
sys.path.insert(0, str(DET))

from love_seductive import LOVE_ALIGN_MIN, score_from_audio  # noqa: E402


def persona_id(path: Path) -> str:
    if path.suffix == ".toml" and path.is_file():
        doc = tomllib.loads(path.read_text())
        return str(doc.get("id") or path.stem)
    return path.stem


def main() -> int:
    ap = argparse.ArgumentParser(description="CLAP love/seductive observation (local weights)")
    ap.add_argument("--wav", required=True)
    ap.add_argument("--persona", required=True, help="own-samantha.toml or a persona id")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    wav = Path(args.wav)
    if not wav.is_file():
        print(f"missing wav {wav}", file=sys.stderr)
        return 2
    pid = persona_id(Path(args.persona))
    detail = score_from_audio(wav, pid, {})
    love = float(detail["love_mean_cos"])
    neu = float(detail["neu_mean_cos"])
    obs = {
        "schema": "galion-love-warmth-obs/1",
        "label": "eval",
        "persona": pid,
        "decision": float(detail["love"]),
        "love": float(detail["love"]),
        "seductive": float(detail["seductive"]),
        "love_mean_cos": love,
        "neutral_mean_cos": neu,
        "sed_mean_cos": float(detail["sed_mean_cos"]),
        "margin": love - neu,
        "e2v_sad": float(detail.get("e2v_sad") or 0.0),
        "align_min": LOVE_ALIGN_MIN,
        "method": detail.get("chosen_love_method"),
    }
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(obs, indent=2) + "\n")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
