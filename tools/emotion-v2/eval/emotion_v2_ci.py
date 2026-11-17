# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# Samantha · Galion Voice V2. 432 Hz tuning standard.
"""Scorer plugin for the Emotion V2 dataset HQ gate (#182 ``--scorer-module emotion_v2_ci``).

Glue only. ``build_scorers(config)`` returns three scorers named ``wer``, ``ecapa`` and
``emotion`` (the interface in tools/emotion-v2/dataset/emotion_v2_dataset/plugins.py).
Each clip is written to a temp tree once and scored by the SAME Rust tools as the eval CI
(scripts/eval/emotion-eval.sh --matrix):
  galion-wer (vosk) · galion-ecapa (ONNX Runtime) · galion-emotion matrix (emotion2vec+, ONNX Runtime)
so the dataset gate and the clone eval can never disagree about a number.

Config keys (or env): models_root (GALION_MODELS), bin_dir (GALION_EVAL_BIN: folder with the three
release binaries), speaker_ref (GALION_EVAL_SPEAKER_REF), reference_dir (GALION_EVAL_REFERENCE_DIR,
REF/<emotion>/*.wav), eval_config (default tools/emotion-v2/eval/emotions.toml), device (cpu|auto|cuda).
A tool that fails or a value the tool could not measure raises ScorerUnavailable: the HQ run stops,
it never passes it.
"""
from __future__ import annotations

import json
import os
import subprocess
import tempfile
import wave
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]


class ScorerUnavailable(RuntimeError):
    pass


def _cfg(config: dict[str, Any], key: str, env: str, default: str | None = None) -> str:
    v = config.get(key) or os.environ.get(env) or default
    if not v:
        raise ScorerUnavailable(f"emotion_v2_ci: set config[{key!r}] or ${env}")
    return str(v)


def _write_wav(path: Path, samples: Any, rate: int) -> None:
    import numpy as np

    pcm = (np.clip(np.asarray(samples, dtype=np.float64).reshape(-1), -1.0, 1.0) * 32767.0).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(int(rate))
        w.writeframes(pcm.tobytes())


def _one_emotion_config(src: Path, emotion: str, dst: Path) -> None:
    head, *blocks = src.read_text(encoding="utf-8").split("\n[[emotion]]\n")
    keep = [b for b in blocks if f'id = "{emotion}"' in b]
    if not keep:
        raise ScorerUnavailable(f"emotion_v2_ci: {emotion!r} is not in {src}")
    dst.write_text(head + "\n[[emotion]]\n" + keep[0].split("\n#")[0].rstrip() + "\n", encoding="utf-8")


class _Runner:
    def __init__(self, config: dict[str, Any]) -> None:
        self.models = Path(_cfg(config, "models_root", "GALION_MODELS", "/mnt/g/galion/models"))
        self.bins = Path(_cfg(config, "bin_dir", "GALION_EVAL_BIN"))
        self.spk = _cfg(config, "speaker_ref", "GALION_EVAL_SPEAKER_REF")
        self.refs = _cfg(config, "reference_dir", "GALION_EVAL_REFERENCE_DIR")
        self.eval_cfg = Path(_cfg(config, "eval_config", "GALION_EVAL_CONFIG", str(ROOT / "tools/emotion-v2/eval/emotions.toml")))
        self.device = str(config.get("device") or os.environ.get("EMOTION_EVAL_DEVICE") or "cpu")
        self.threads = str(config.get("num_threads") or os.cpu_count() or 1)
        self.cache: dict[int, tuple[Any, dict[str, Any]]] = {}

    def _run(self, args: list[str], log: Path) -> None:
        with log.open("w") as fh:
            c = subprocess.run(args, stdout=fh, stderr=subprocess.STDOUT, check=False)
        if c.returncode != 0:
            raise ScorerUnavailable(f"{Path(args[0]).name} exit {c.returncode}: {log.read_text()[-400:]}")

    def row(self, clip: Any) -> dict[str, Any]:
        hit = self.cache.get(id(clip))
        if hit is not None and hit[0] is clip:
            return hit[1]
        e = clip.emotion
        with tempfile.TemporaryDirectory(prefix="emotion-v2-ci-") as t:
            t = Path(t)
            d = t / "build" / e
            d.mkdir(parents=True)
            for sub in ("wer", "ecapa", "out"):
                (t / sub).mkdir()
            wav = d / "clip.wav"
            _write_wav(wav, clip.audio.samples, clip.audio.sample_rate)
            (d / "clip.txt").write_text(clip.text, encoding="utf-8")
            m = self.models
            self._run([str(self.bins / "galion-wer"), "--text-dir", str(d), "--model", str(m / "asr/vosk-model-small-en-us-0.15"),
                       "--lib", str(m / "asr/vosk-linux-x86_64-0.3.45/libvosk.so"), "--jobs", "1",
                       "--out", str(t / "wer" / f"{e}.json"), str(wav)], t / "wer.log")
            self._run([str(self.bins / "galion-ecapa"), "--ref", self.spk, "--model", str(m / "speaker/ecapa/ecapa.onnx"),
                       "--threads", self.threads, "--no-ceiling", "--out", str(t / "ecapa" / f"{e}.json"), str(wav)], t / "ecapa.log")
            cfg = t / "one.toml"
            _one_emotion_config(self.eval_cfg, e, cfg)
            self._run([str(self.bins / "galion-emotion"), "matrix", "--config", str(cfg), "--build-dir", str(t / "build"),
                       "--reference-dir", self.refs, "--models-root", str(m), "--wer-dir", str(t / "wer"),
                       "--ecapa-dir", str(t / "ecapa"), "--device", self.device, "--threads", "0", "--out-dir", str(t / "out")],
                      t / "matrix.log")
            rows = json.loads((t / "out" / "matrix.json").read_text())["rows"]
        row = next(r for r in rows if r["emotion"] == e)
        self.cache[id(clip)] = (clip, row)
        return row


class _Scorer:
    def __init__(self, name: str, key: str, runner: _Runner) -> None:
        self.name, self._key, self._runner = name, key, runner

    def score(self, clip: Any) -> float:
        v = self._runner.row(clip).get(self._key)
        if v is None:
            raise ScorerUnavailable(f"{self.name} not measured for {clip.emotion!r} (no reference clips?)")
        return float(v)


def build_scorers(config: dict[str, Any]) -> list:
    runner = _Runner(dict(config or {}))
    return [_Scorer("wer", "wer", runner), _Scorer("ecapa", "ecapa", runner), _Scorer("emotion", "emotion_match", runner)]
