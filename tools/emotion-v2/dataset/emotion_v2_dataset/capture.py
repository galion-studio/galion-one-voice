# G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
# Developed and distributed by Galion Labs
# Perpetual Open MIT License
# Copyright © 2026 Galion Labs. All rights reserved.
"""Capture-spec gate for every clip the pipeline writes.

High-quality-only is the default. Each written clip must be 48 kHz, 24-bit
PCM mono, have integrated loudness of -20 LUFS +/- 2 LU (ITU-R BS.1770-4,
mono, gated), a true peak at or below -1 dBTP, and zero clipped samples.
The numbers mirror ``docs/emotion-v2/capture-spec.toml``. A test checks
that they still match that file.

Measurement is done by existing open tools (see ``BRICKS``). Loudness is
measured, then the clip is levelled to the target. A clip that
cannot reach the target without breaking the true-peak or clipping limit is
rejected, not limited or compressed.
"""

from __future__ import annotations

import math
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

import numpy as np

from emotion_v2_dataset.audio import AudioBuffer
from emotion_v2_dataset.errors import QualityError

SPEC_RELATIVE_PATH = Path("docs") / "emotion-v2" / "capture-spec.toml"


@dataclass(frozen=True)
class CaptureSpec:
    sample_rate_hz: int = 48_000
    bit_depth: int = 24
    channels: int = 1
    target_lufs: float = -20.0
    tolerance_lu: float = 2.0
    block_ms: int = 400
    hop_ms: int = 100
    absolute_gate_lufs: float = -70.0
    relative_gate_lu: float = 10.0
    max_true_peak_dbtp: float = -1.0
    true_peak_oversample: int = 4
    max_clipped_samples: int = 0
    normalize_loudness: bool = True

    def as_dict(self) -> dict[str, Any]:
        return asdict(self)


HIGH_QUALITY_CAPTURE = CaptureSpec()


def load_capture_spec(path: Path) -> CaptureSpec:
    """Read the shared TOML spec. Only the fields this gate uses are read."""
    import tomllib

    data = tomllib.loads(Path(path).read_text(encoding="utf-8"))
    try:
        return CaptureSpec(
            sample_rate_hz=int(data["format"]["sample_rate_hz"]),
            bit_depth=int(data["format"]["bit_depth"]),
            channels=int(data["format"]["channels"]),
            target_lufs=float(data["loudness"]["target_lufs"]),
            tolerance_lu=float(data["loudness"]["tolerance_lu"]),
            block_ms=int(data["loudness"]["block_ms"]),
            hop_ms=int(data["loudness"]["hop_ms"]),
            absolute_gate_lufs=float(data["loudness"]["absolute_gate_lufs"]),
            relative_gate_lu=float(data["loudness"]["relative_gate_lu"]),
            max_true_peak_dbtp=float(data["true_peak"]["max_dbtp"]),
            true_peak_oversample=int(data["true_peak"]["oversample"]),
            max_clipped_samples=int(data["clipping"]["max_clipped_samples"]),
        )
    except KeyError as exc:
        raise QualityError(f"capture spec is missing {exc}") from exc


@dataclass(frozen=True)
class CaptureResult:
    accepted: bool
    reason: str
    audio: AudioBuffer
    loudness_lufs: float | None
    true_peak_dbtp: float | None
    clipped_samples: int
    gain_db: float


def clip_level(bit_depth: int) -> float:
    """Full-scale code for the bit depth, as a float in -1..1."""
    full = float(2 ** (int(bit_depth) - 1))
    return (full - 1.0) / full


def count_clipped(samples: np.ndarray, bit_depth: int = 24) -> int:
    """Samples at or beyond the largest code the bit depth can store."""
    if len(samples) == 0:
        return 0
    return int(np.count_nonzero(np.abs(np.asarray(samples, dtype=np.float64)) >= clip_level(bit_depth)))


def conform(audio: AudioBuffer, spec: CaptureSpec) -> CaptureResult:
    """Resample, level, and check one clip. Returns the audio to write."""
    if spec.channels != 1:
        raise QualityError("the dataset pipeline writes mono clips only")
    clipped_in = count_clipped(audio.samples, spec.bit_depth)
    if clipped_in > spec.max_clipped_samples:
        return CaptureResult(
            False,
            f"capture: {clipped_in} clipped samples before levelling",
            audio,
            None,
            None,
            clipped_in,
            0.0,
        )
    resampled = resample(audio, spec.sample_rate_hz)
    measured = integrated_lufs(resampled.samples, resampled.sample_rate, spec)
    if measured is None:
        return CaptureResult(
            False,
            "capture: loudness below the absolute gate (silent or too short)",
            resampled,
            None,
            None,
            0,
            0.0,
        )
    gain_db = 0.0
    levelled = resampled
    if spec.normalize_loudness:
        gain_db = spec.target_lufs - measured
        levelled = AudioBuffer(resampled.samples * (10.0 ** (gain_db / 20.0)), resampled.sample_rate)
        measured = integrated_lufs(levelled.samples, levelled.sample_rate, spec)
        if measured is None:  # pragma: no cover - gain cannot drop a clip below the gate here
            return CaptureResult(False, "capture: loudness unmeasurable", levelled, None, None, 0, gain_db)
    peak = true_peak_dbtp(levelled.samples, spec.true_peak_oversample, levelled.sample_rate)
    clipped = count_clipped(levelled.samples, spec.bit_depth)
    if abs(measured - spec.target_lufs) > spec.tolerance_lu:
        reason = (
            f"capture: loudness {measured:.2f} LUFS outside "
            f"{spec.target_lufs:.1f} +/- {spec.tolerance_lu:.1f}"
        )
        return CaptureResult(False, reason, levelled, measured, peak, clipped, gain_db)
    if clipped > spec.max_clipped_samples:
        return CaptureResult(
            False, f"capture: {clipped} clipped samples", levelled, measured, peak, clipped, gain_db
        )
    if peak > spec.max_true_peak_dbtp:
        reason = f"capture: true peak {peak:.2f} dBTP above {spec.max_true_peak_dbtp:.1f}"
        return CaptureResult(False, reason, levelled, measured, peak, clipped, gain_db)
    return CaptureResult(True, "pass", levelled, measured, peak, clipped, gain_db)


# ------------------------------------------------------------------ bricks
# No DSP is written here. Loudness is pyloudnorm (MIT), resampling and the
# 4x oversampling for true peak are python-samplerate (MIT, bundles
# libsamplerate BSD-2-Clause). This file only configures them to the spec.

BRICKS = {
    "pyloudnorm": "0.2.0 MIT - ITU-R BS.1770-4 integrated loudness",
    "samplerate": "0.2.4 MIT (libsamplerate BSD-2-Clause) - sinc_best resampling",
}


def _bricks() -> tuple[Any, Any]:
    try:
        import pyloudnorm  # type: ignore[import-not-found]
        import samplerate  # type: ignore[import-not-found]
    except ImportError as exc:
        raise QualityError(
            "high-quality capture gate needs pyloudnorm and samplerate: "
            "pip install -r tools/emotion-v2/dataset/requirements-hq.txt"
        ) from exc
    return pyloudnorm, samplerate


def resample(audio: AudioBuffer, sample_rate: int) -> AudioBuffer:
    """Band-limited resample with libsamplerate ``sinc_best``."""
    sample_rate = int(sample_rate)
    if sample_rate == audio.sample_rate:
        return AudioBuffer(audio.samples.copy(), sample_rate)
    if len(audio.samples) == 0:
        return AudioBuffer(np.zeros(0), sample_rate)
    _loud, src = _bricks()
    out = src.resample(audio.samples.astype(np.float32), sample_rate / audio.sample_rate, "sinc_best")
    return AudioBuffer(np.asarray(out, dtype=np.float64), sample_rate)


def integrated_lufs(samples: np.ndarray, sample_rate: int, spec: CaptureSpec = HIGH_QUALITY_CAPTURE) -> float | None:
    """pyloudnorm BS.1770-4 meter, mono. ``None`` when nothing clears the gate."""
    loud, _src = _bricks()
    data = np.asarray(samples, dtype=np.float64)
    if len(data) < int(round(sample_rate * spec.block_ms / 1000.0)):
        return None
    meter = loud.Meter(sample_rate, filter_class="K-weighting", block_size=spec.block_ms / 1000.0)
    value = float(meter.integrated_loudness(data))
    if not math.isfinite(value) or value <= spec.absolute_gate_lufs:
        return None
    return value


def true_peak_dbtp(samples: np.ndarray, oversample: int = 4, sample_rate: int = 48_000) -> float:
    """BS.1770 true peak: max of the signal oversampled ``oversample`` times."""
    data = np.asarray(samples, dtype=np.float64)
    if len(data) == 0:
        return -math.inf
    up = resample(AudioBuffer(data, sample_rate), sample_rate * int(oversample)).samples
    peak = max(float(np.max(np.abs(up))), float(np.max(np.abs(data))))
    return -math.inf if peak <= 0.0 else 20.0 * math.log10(peak)
