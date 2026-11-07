#!/usr/bin/env python3
"""Collect PR #87 laptop fidelity reports into one JSON file.

Reads the four eval JSON files that eval-laptop.sh writes (amy, alan, long,
long-multi). Missing files stay missing. Numbers are copied, never invented.
Amy and Alan are different speakers and are not averaged.
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

CLIPS = ("amy", "alan", "long", "long-multi")
FIELDS = (
    "schema",
    "wer",
    "wer_percent",
    "wer_pass",
    "ecapa",
    "ecapa_pass",
    "embed_cosine",
    "embed_note",
    "mos_proxy",
    "emotion_match",
    "emotion_match_pass",
    "emotion_match_source",
    "timbre_applied",
    "timbre_strength",
    "rollout_claim",
)


def clip_record(out_dir: Path, name: str) -> dict:
    path = out_dir / f"{name}-eval.json"
    empty = {field: None for field in FIELDS}
    if not path.is_file():
        return {"clip": name, "status": "missing", "report_name": path.name, **empty}
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return {
            "clip": name,
            "status": "unreadable",
            "report_name": path.name,
            "error": str(exc),
            **empty,
        }
    if not isinstance(doc, dict):
        return {
            "clip": name,
            "status": "unreadable",
            "report_name": path.name,
            "error": "eval JSON was not an object",
            **empty,
        }
    record = {"clip": name, "status": "measured", "report_name": path.name}
    for field in FIELDS:
        record[field] = doc.get(field)
    return record


def build(out_dir: Path, *, cache: str | None, gpu_note: str | None) -> dict:
    clips = [clip_record(out_dir, name) for name in CLIPS]
    return {
        "schema": "galion-laptop-fidelity/1",
        "pr": 87,
        "script": "tools/cloning-v2/fidelity/scripts/eval-laptop.sh",
        "clips_expected": list(CLIPS),
        "clips": clips,
        "measured_count": sum(1 for c in clips if c["status"] == "measured"),
        "missing_count": sum(1 for c in clips if c["status"] != "measured"),
        "do_not_average": ["amy", "alan"],
        "embed_cosine_is_ecapa": False,
        "rollout_gate": {
            "reporting_only": True,
            "wer_max": 0.018,
            "ecapa_min_exclusive": 0.7057,
            "emotion_match_min": 0.83,
            "claimed": False,
        },
        "cache_dir": cache,
        "gpu_note": gpu_note,
        "audio_committed": False,
    }


def print_summary(doc: dict, result: Path) -> None:
    print(f"pr87 fidelity summary: {result}")
    print("amy and alan are separate speakers and are not averaged")
    print("embed_cosine is not ECAPA; empty wer/ecapa/emotion_match means unmeasured")
    for clip in doc["clips"]:
        if clip["status"] != "measured":
            print(f"  {clip['clip']}: {clip['status']}")
            continue
        print(
            f"  {clip['clip']}: embed_cosine={_fmt(clip.get('embed_cosine'))} "
            f"mos_proxy={_fmt(clip.get('mos_proxy'))} "
            f"wer={_fmt(clip.get('wer'))} ecapa={_fmt(clip.get('ecapa'))} "
            f"emotion_match={_fmt(clip.get('emotion_match'))}"
        )
    print(
        "rollout gate is reporting only "
        "(WER <= 1.8%, ECAPA > 0.7057, emotion_match >= 0.83) and is not claimed"
    )


def _fmt(value: object) -> str:
    if value is None:
        return "unmeasured"
    return str(value)


def write_result(out_dir: Path, result: Path, cache: str) -> dict:
    gpu_path = out_dir / "gpu.txt"
    gpu_note = gpu_path.read_text(encoding="utf-8").strip() if gpu_path.is_file() else None
    doc = build(out_dir, cache=cache, gpu_note=gpu_note)
    result.parent.mkdir(parents=True, exist_ok=True)
    result.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    return doc


def self_check() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp)
        (out / "amy-eval.json").write_text(
            json.dumps(
                {
                    "schema": "schema-check",
                    "embed_cosine": 0.5,
                    "mos_proxy": 1.0,
                    "embed_note": "assembler fixture, not a measurement",
                }
            ),
            encoding="utf-8",
        )
        doc = build(out, cache=None, gpu_note=None)
        by = {c["clip"]: c for c in doc["clips"]}
        assert by["amy"]["status"] == "measured"
        assert by["amy"]["embed_cosine"] == 0.5
        assert by["amy"]["wer"] is None
        assert by["alan"]["status"] == "missing"
        assert by["alan"]["embed_cosine"] is None
        assert by["long"]["status"] == "missing"
        assert by["long-multi"]["status"] == "missing"
        assert "average" not in doc
        assert doc["do_not_average"] == ["amy", "alan"]
        assert doc["rollout_gate"]["claimed"] is False
        assert "0.999" not in json.dumps(doc)
    print("collect_fidelity schema_check=pass measured=false")
    return 0


def main(argv: list[str]) -> int:
    if argv == ["--self-check"]:
        return self_check()
    if len(argv) != 3:
        print(
            "usage: collect_fidelity.py OUT_DIR RESULT_JSON CACHE_DIR",
            file=sys.stderr,
        )
        print("       collect_fidelity.py --self-check", file=sys.stderr)
        return 2
    out_dir, result, cache = Path(argv[0]), Path(argv[1]), argv[2]
    doc = write_result(out_dir, result, cache)
    print_summary(doc, result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
