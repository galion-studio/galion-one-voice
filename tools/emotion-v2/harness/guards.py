# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Call the existing WER and ECAPA scorers when they are on disk.

The invocation matches ``galion-voicecheck``: ``galion-wer --text-dir`` and
``galion-ecapa --ref``. A missing binary is reported as absent. This module
does not vendor those scorers and does not download models for them.
"""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

from .config import repo_root
from .metrics import ecapa_passes, wer_passes

WER_RELATIVE = Path("tools/cloning-v2/scorers/quality/bin/galion-wer")
ECAPA_RELATIVE = Path("tools/cloning-v2/scorers/ecapa/target/release/galion-ecapa")
DEFAULT_TIMEOUT_S = 120


@dataclass(frozen=True)
class GuardResult:
    name: str
    status: str
    value: float | None
    threshold: float
    comparison: str
    passed: bool | None
    detail: str

    def as_dict(self) -> dict[str, object]:
        return {
            "name": self.name,
            "status": self.status,
            "value": self.value,
            "threshold": self.threshold,
            "comparison": self.comparison,
            "pass": self.passed,
            "detail": self.detail,
        }


def guard_accepts(result: GuardResult, required: bool) -> bool:
    if result.status == "ok":
        return bool(result.passed)
    if result.status == "absent":
        return not required
    return False


def locate_scorer(kind: str, override: Path | None = None) -> Path | None:
    """Return the executable, or None when this machine does not have it.

    An explicit override or environment path is returned even when it is not
    executable, so the caller can report a broken pointer instead of skipping
    the guard.
    """
    if kind not in {"wer", "ecapa"}:
        raise ValueError(f"unknown guard {kind!r}")
    if override is not None:
        return override
    env_name = "GALION_WER" if kind == "wer" else "GALION_ECAPA"
    env_value = os.environ.get(env_name)
    if env_value:
        return Path(env_value)
    relative = WER_RELATIVE if kind == "wer" else ECAPA_RELATIVE
    candidate = repo_root() / relative
    if candidate.is_file() and os.access(candidate, os.X_OK):
        return candidate
    return None


def score_wer(
    scorer: Path | None,
    wavs: list[Path],
    text_dir: Path,
    wer_max: float,
    work_dir: Path,
    timeout_s: int = DEFAULT_TIMEOUT_S,
) -> GuardResult:
    if scorer is None:
        return _absent("wer", wer_max, "<=", "galion-wer is not on this machine")
    out_path = work_dir / "wer.json"
    command = [
        str(scorer),
        "--text-dir",
        str(text_dir),
        "--jobs",
        "2",
        "--out",
        str(out_path),
        *[str(wav) for wav in wavs],
    ]
    return _invoke(
        name="wer",
        command=command,
        out_path=out_path,
        threshold=wer_max,
        comparison="<=",
        parse=_parse_wer,
        accept=lambda value: wer_passes(value, wer_max),
        timeout_s=timeout_s,
        empty=not wavs,
    )


def score_ecapa(
    scorer: Path | None,
    wavs: list[Path],
    reference: Path | None,
    ecapa_min: float,
    work_dir: Path,
    timeout_s: int = DEFAULT_TIMEOUT_S,
) -> GuardResult:
    if scorer is None:
        return _absent("ecapa", ecapa_min, ">", "galion-ecapa is not on this machine")
    if reference is None or not reference.is_file():
        return _error("ecapa", ecapa_min, ">", "ECAPA needs a speaker reference wav")
    out_path = work_dir / "ecapa.json"
    command = [
        str(scorer),
        "--ref",
        str(reference),
        "--threads",
        "2",
        "--out",
        str(out_path),
        *[str(wav) for wav in wavs],
    ]
    return _invoke(
        name="ecapa",
        command=command,
        out_path=out_path,
        threshold=ecapa_min,
        comparison=">",
        parse=_parse_ecapa,
        accept=lambda value: ecapa_passes(value, ecapa_min),
        timeout_s=timeout_s,
        empty=not wavs,
    )


def _absent(name: str, threshold: float, comparison: str, detail: str) -> GuardResult:
    return GuardResult(name, "absent", None, threshold, comparison, None, detail)


def _error(name: str, threshold: float, comparison: str, detail: str) -> GuardResult:
    return GuardResult(name, "error", None, threshold, comparison, False, detail)


def _invoke(
    name: str,
    command: list[str],
    out_path: Path,
    threshold: float,
    comparison: str,
    parse,
    accept,
    timeout_s: int,
    empty: bool,
) -> GuardResult:
    if empty:
        return _error(name, threshold, comparison, "no render wavs to score")
    binary = Path(command[0])
    if not binary.is_file() or not os.access(binary, os.X_OK):
        return _error(name, threshold, comparison, f"scorer is not executable: {binary}")
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return _error(name, threshold, comparison, f"scorer timed out after {timeout_s}s")
    except OSError as exc:
        return _error(name, threshold, comparison, f"could not start scorer: {exc}")
    if completed.returncode != 0:
        tail = (completed.stderr or completed.stdout or "").strip()
        return _error(
            name,
            threshold,
            comparison,
            f"scorer exited {completed.returncode}: {tail[:400]}",
        )
    if not out_path.is_file():
        return _error(name, threshold, comparison, f"scorer did not write {out_path.name}")
    try:
        payload = json.loads(out_path.read_text(encoding="utf-8"))
        value = float(parse(payload))
    except (OSError, json.JSONDecodeError, KeyError, TypeError, ValueError) as exc:
        return _error(name, threshold, comparison, f"unreadable scorer JSON: {exc}")
    return GuardResult(
        name=name,
        status="ok",
        value=value,
        threshold=threshold,
        comparison=comparison,
        passed=bool(accept(value)),
        detail="scored",
    )


def _parse_wer(payload: dict) -> float:
    summary = payload["summary"]
    if "mean" not in summary:
        raise KeyError("summary.mean")
    return float(summary["mean"])


def _parse_ecapa(payload: dict) -> float:
    if "mean" not in payload:
        raise KeyError("mean")
    return float(payload["mean"])
