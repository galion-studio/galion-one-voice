#!/usr/bin/env python3
"""Cross-check galion-expression.

1. Recompute the published gate numbers and the verdict cases in Python,
   then diff them against `galion-expression diagnose`.
2. Track a synthetic 200 Hz sine with an independent autocorrelation and
   with the Rust YIN tracker.
3. Fetch a CC0 clip at runtime (Wikimedia Commons File:Ului12.wav, not
   committed) and require a finite profile.

Build first:
  cargo build --manifest-path pipeline/expression/Cargo.toml
"""
from __future__ import annotations

import json
import math
import struct
import subprocess
import sys
import urllib.request
import wave
from pathlib import Path

HERE = Path(__file__).resolve()
EXPR = HERE.parents[1]
CLONING = HERE.parents[3]
DIAG = CLONING / "scorers" / "persona" / "py"
sys.path.insert(0, str(DIAG))

from expression_diag import (  # noqa: E402
    attribute_love,
    channel_allowed,
    diagnose,
    emotion_match,
    load_persona_policy,
)

CC0_URL = "https://upload.wikimedia.org/wikipedia/commons/1/16/Ului12.wav"
CC0_NOTE = "Wikimedia Commons File:Ului12.wav (CC0), fetched at runtime"


def binary() -> Path:
    for name in ("debug", "release"):
        path = EXPR / "target" / name / "galion-expression"
        if path.is_file():
            return path
    raise SystemExit(
        "galion-expression is not built. Run:\n"
        "  cargo build --manifest-path tools/cloning-v2/pipeline/expression/Cargo.toml"
    )


def run_diagnose(case: Path) -> dict:
    proc = subprocess.run(
        [str(binary()), "diagnose", "--case", str(case)],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(proc.stdout)


def check_gate() -> None:
    doc = json.loads((EXPR / "fixtures" / "samantha_love_gate.json").read_text())
    report = run_diagnose(EXPR / "fixtures" / "samantha_love_gate.json")
    assert report["jarvis"]["gate_pass"] is True
    assert report["jarvis"]["love_in_judged_set"] is False
    by_label = {}
    for clone in doc["clones"]:
        got = diagnose(doc["reference"], clone)
        by_label[clone["label"]] = got["verdict"]
        assert got["verdict"] == "insufficient_evidence", got
        assert abs(got["match_score"] - 0.06094556199731194) < 1e-15
        assert abs(got["reference_decision"] - 0.9390544380026881) < 1e-15
        assert abs(got["clone_decision"]) < 1e-15
    for item in report["clones"]:
        assert item["verdict"] == by_label[item["label"]]
        attr = item["attribution"]
        assert attr["pass"] is False
        assert attr["pass_if_love_matched"] is True
        published = next(s for s in doc["gate_sets"] if s["id"] == item["label"])
        py = attribute_love(published["rows"], published["emb_sim"])
        assert abs(py["emotion_match"] - published["emotion_match"]) < 1e-12
        assert abs(py["emotion_match"] - attr["emotion_match"]) < 1e-9
    print("gate", {s["id"]: s["emotion_match"] for s in doc["gate_sets"]})
    print("love-matched would pass", {c["label"]: c["attribution"]["emotion_match_if_love_matched"] for c in report["clones"]})


def check_verdicts() -> None:
    doc = json.loads((EXPR / "fixtures" / "verdict_cases.json").read_text())
    report = run_diagnose(EXPR / "fixtures" / "verdict_cases.json")
    rust = {item["label"]: item["verdict"] for item in report["clones"]}
    expect = {
        "margin-held-zero": "detector_miss",
        "real-loss": "real_loss",
        "prosody-held": "detector_miss",
        "sad-gate": "detector_miss",
        "mixed-f0": "mixed",
    }
    for clone in doc["clones"]:
        py = diagnose(doc["reference"], clone)["verdict"]
        assert py == expect[clone["label"]], (clone["label"], py)
        assert rust[clone["label"]] == py, (clone["label"], rust[clone["label"]], py)
    print("verdicts", rust)


def check_personas() -> None:
    root = CLONING / "pipeline" / "personas"
    sam = load_persona_policy(root / "own-samantha.toml")
    jar = load_persona_policy(root / "own-jarvis.toml")
    for emo in ("love", "seductive", "neutral", "logic", "joyful"):
        assert channel_allowed(sam["id"], sam["judged"], emo), emo
    for emo in ("crying", "yelling"):
        assert not channel_allowed(sam["id"], sam["judged"], emo)
        assert not channel_allowed(jar["id"], jar["judged"], emo)
    assert not channel_allowed(jar["id"], jar["judged"], "seductive")
    assert not channel_allowed(jar["id"], jar["judged"], "love")
    assert channel_allowed(jar["id"], jar["judged"], "neutral")
    assert channel_allowed(jar["id"], jar["judged"], "logic")
    assert abs(sam["gate"] - 0.83) < 1e-15
    print("personas", sam["id"], jar["id"])


def write_sine(path: Path, freq: float = 200.0, seconds: float = 1.0, amp: float = 0.1, sr: int = 16000) -> None:
    n = int(seconds * sr)
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        frames = bytearray()
        for i in range(n):
            sample = amp * math.sin(2 * math.pi * freq * i / sr)
            frames += struct.pack("<h", int(max(-1, min(1, sample)) * 32767))
        w.writeframes(frames)


def autocorr_f0(path: Path) -> float:
    with wave.open(str(path)) as w:
        sr = w.getframerate()
        raw = w.readframes(w.getnframes())
    samples = [s / 32768.0 for s in struct.unpack("<" + "h" * (len(raw) // 2), raw)]
    n = int(0.05 * sr)
    start = max(0, len(samples) // 2 - n // 2)
    frame = samples[start : start + n]
    mean = sum(frame) / len(frame)
    frame = [s - mean for s in frame]
    lo, hi = int(sr / 400), int(sr / 80)
    best_lag, best = lo, -1.0
    for lag in range(lo, hi):
        acc = sum(frame[i] * frame[i + lag] for i in range(len(frame) - lag))
        if acc > best:
            best = acc
            best_lag = lag
    return sr / best_lag


def check_sine() -> None:
    tmp = EXPR / "target" / "crosscheck"
    wav = tmp / "sine200.wav"
    write_sine(wav)
    proc = subprocess.run(
        [
            str(binary()),
            "analyze",
            "--voice",
            "cc0-sine",
            "--channel",
            f"neutral={wav}",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    profile = json.loads(proc.stdout)
    med = profile["channels"][0]["prosody"]["f0_median_hz"]
    py = autocorr_f0(wav)
    assert abs(med - 200.0) < 2.0, med
    assert abs(py - 200.0) < 2.0, py
    assert abs(med - py) < 2.0, (med, py)
    assert profile["reference_selection"] is False
    assert profile["channels"][0]["prosody"]["sung_phrases"] == []
    print(f"sine rust_f0={med:.3f} python_f0={py:.3f}")


def check_cc0() -> None:
    tmp = EXPR / "target" / "crosscheck"
    tmp.mkdir(parents=True, exist_ok=True)
    wav = tmp / "ului12.wav"
    if not wav.is_file() or wav.stat().st_size < 1000:
        req = urllib.request.Request(
            CC0_URL,
            headers={"User-Agent": "galion-expression-crosscheck/0.1 (CC0 test clip)"},
        )
        with urllib.request.urlopen(req, timeout=30) as resp:  # noqa: S310 — pinned CC0 commons URL
            wav.write_bytes(resp.read())
    proc = subprocess.run(
        [
            str(binary()),
            "analyze",
            "--voice",
            "cc0-ului12",
            "--channel",
            f"untagged={wav}",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    profile = json.loads(proc.stdout)
    pros = profile["channels"][0]["prosody"]
    assert pros["duration_s"] > 1.0, pros["duration_s"]
    assert math.isfinite(pros["rms_db"])
    assert math.isfinite(pros["spectral_tilt_db_oct"])
    assert profile["schema"] == "galion-expression-profile/1"
    print(
        f"cc0 {CC0_NOTE} duration={pros['duration_s']:.3f}s "
        f"f0_median={pros['f0_median_hz']:.2f} tilt={pros['spectral_tilt_db_oct']:.3f}"
    )


def main() -> int:
    check_personas()
    check_gate()
    check_verdicts()
    check_sine()
    check_cc0()
    print("cross-check ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
