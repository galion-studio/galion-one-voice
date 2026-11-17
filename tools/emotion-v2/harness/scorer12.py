# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""Thin glue to the Rust ``galion-emotion12`` scorer (emotion2vec+ via ort).

Gate decision (card t1359s6830): per-emotion emotion2vec+ scoring, threshold
0.83 unchanged. The emotion list (14: the 12 V2 emotions + laughter +
seductive) and the class each one reads from live in
``tools/emotion-v2/emotion12/gate-emotions.toml``. This module only runs the
binary and reads its JSON. It never downloads and never changes the gate.

Binary: ``GALION_EMOTION12_BIN`` or ``tools/emotion-v2/emotion12/target/
release/galion-emotion12[.exe]``. Models: ``EMOTION_V2_MODELS_DIR`` (default
``G:\\galion\\models\\emotion-v2`` in the binary). ONNX Runtime:
``ORT_DYLIB_PATH``. CUDA is used only when present; CPU otherwise.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

from .backends import Analysis, BackendUnavailable
from .config import emotion_v2_root, repo_root
from .metrics import l2_normalize

BACKEND_ID = "emotion2vec+"
ALIASES = {"emotion2vec", "emotion2vec+", "emotion2vec-plus", "galion-emotion12", "galion12"}


def gate_emotions_path() -> Path:
    override = os.environ.get("EMOTION_V2_GATE_EMOTIONS")
    return Path(override) if override else emotion_v2_root() / "emotion12" / "gate-emotions.toml"


def locate_binary() -> Path | None:
    override = os.environ.get("GALION_EMOTION12_BIN")
    if override is not None:
        path = Path(override)
        return path if path.is_file() else None
    base = emotion_v2_root() / "emotion12" / "target" / "release"
    for name in ("galion-emotion12.exe", "galion-emotion12"):
        if (base / name).is_file():
            return base / name
    return None


def ready() -> bool:
    """True when the binary, the runtime and a models dir are all present."""
    if locate_binary() is None or not os.environ.get("ORT_DYLIB_PATH"):
        return False
    models = os.environ.get("EMOTION_V2_MODELS_DIR", r"G:\galion\models\emotion-v2")
    return (Path(models) / "emotion2vec_plus_base_onnx" / "emotion2vec_plus_base.onnx").is_file()


def run_batch(paths: list[Path], model: str = "base", device: str | None = None,
              threads: int | None = None) -> dict[str, dict]:
    binary = locate_binary()
    if binary is None:
        raise BackendUnavailable(
            "galion-emotion12 is not built (cargo build --release in tools/emotion-v2/emotion12); "
            "refusing to download anything"
        )
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "scores.json"
        cmd = [str(binary), "score", "--emotions", str(gate_emotions_path()), "--model", model,
               "--out", str(out), *[str(p) for p in paths]]
        device = device or os.environ.get("EMOTION_V2_DEVICE") or "auto"
        cmd[2:2] = ["--device", device]
        if threads:
            cmd[2:2] = ["--threads", str(int(threads))]
        if os.environ.get("EMOTION_V2_MODELS_DIR"):
            cmd[2:2] = ["--models-dir", os.environ["EMOTION_V2_MODELS_DIR"]]
        proc = subprocess.run(cmd, capture_output=True, text=True, cwd=repo_root())
        if proc.returncode != 0:
            raise BackendUnavailable(f"galion-emotion12 failed: {proc.stderr.strip()[-400:]}")
        body = json.loads(out.read_text(encoding="utf-8"))
    labels = tuple(body["labels"])
    return {
        str(row["file"]): {"labels": labels, "posterior": row["posterior"],
                           "embedding": row["embedding"], "device": body.get("device")}
        for row in body["files"]
    }


class Emotion2VecPlusBackend:
    """Per-emotion emotion2vec+ posterior over the gate set (14 emotions)."""

    name = BACKEND_ID

    def __init__(self, model: str = "base") -> None:
        self.model = model
        self._labels: tuple[str, ...] | None = None

    def labels(self) -> tuple[str, ...]:
        if self._labels is None:
            raise BackendUnavailable("labels are known after the first analysis")
        return self._labels

    def analyse_path(self, path: Path) -> Analysis:
        row = run_batch([path], self.model)[str(path)]
        self._labels = row["labels"]
        return Analysis(labels=row["labels"], posterior=tuple(row["posterior"]),
                        embedding=tuple(l2_normalize(row["embedding"])))

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        from .audio import write_wav

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "clip.wav"
            write_wav(path, samples, sample_rate)
            return self.analyse_path(path)
