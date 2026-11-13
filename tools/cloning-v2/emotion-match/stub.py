# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""CPU stub analyser and sine fixtures for emotion_match.

No torch, no transformers, no weights, no network. A pure tone at
STUB_FREQ_HZ[emotion] peaks on that sector. These signals are test
fixtures, not voices and not a model of affect.
"""

from __future__ import annotations

import math
import struct
import wave

from scorelib import as_prob, as_unit, softmax
from sector_refs import KNOWN_SECTORS, Path, SHARED_SECTORS

# Shared-sector frequencies match the Emotion V2 harness so a 432 Hz sine
# is still the Amused fixture (the 432 Hz tuning standard). Seductive is
# the Samantha-only overlay and sits above Disgusted.
STUB_FREQ_HZ: dict[str, float] = {
    "neutral": 210.0,
    "calm": 270.0,
    "happy": 330.0,
    "amused": 432.0,
    "excited": 510.0,
    "tender": 630.0,
    "sad": 750.0,
    "disappointed": 870.0,
    "angry": 1050.0,
    "fearful": 1230.0,
    "surprised": 1470.0,
    "disgusted": 1710.0,
    "seductive": 1890.0,
}

STUB_LABELS: tuple[str, ...] = KNOWN_SECTORS


class StubAudioError(ValueError):
    """A fixture WAV is missing or not 16-bit PCM."""


def synthesize_tone(
    freq_hz: float,
    sample_rate: int = 16000,
    seconds: float = 0.25,
    amplitude: float = 0.2,
) -> list[float]:
    if sample_rate <= 0 or seconds <= 0:
        raise StubAudioError("sample rate and duration must be positive")
    count = int(sample_rate * seconds)
    step = 2.0 * math.pi * freq_hz / sample_rate
    return [amplitude * math.sin(step * index) for index in range(count)]


def write_wav(path: Path, samples: list[float], sample_rate: int = 16000) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    frames = bytearray()
    for sample in samples:
        clipped = max(-1.0, min(1.0, float(sample)))
        frames += struct.pack("<h", int(round(clipped * 32767.0)))
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(frames)
    return path


def write_sector_tone(path: Path, emotion: str, **kwargs) -> Path:
    from sector_refs import canonical_sector

    key = canonical_sector(emotion)
    return write_wav(path, synthesize_tone(STUB_FREQ_HZ[key], **kwargs), kwargs.get("sample_rate", 16000))


def read_wav(path: Path) -> tuple[list[float], int]:
    if not path.is_file():
        raise StubAudioError(f"missing wav: {path}")
    try:
        handle = wave.open(str(path), "rb")
    except wave.Error as exc:
        raise StubAudioError(f"unreadable wav {path}: {exc}") from exc
    with handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        raw = handle.readframes(handle.getnframes())
    if width != 2:
        raise StubAudioError(
            f"{path} is {width * 8}-bit; the stub fixture reader accepts 16-bit PCM only"
        )
    if channels < 1 or rate <= 0:
        raise StubAudioError(f"{path} has unusable format (channels={channels} rate={rate})")
    count = len(raw) // 2
    if count == 0:
        raise StubAudioError(f"{path} is empty")
    unpacked = struct.unpack("<" + "h" * count, raw)
    if channels == 1:
        samples = [value / 32768.0 for value in unpacked]
    else:
        samples = []
        for index in range(0, count, channels):
            chunk = unpacked[index : index + channels]
            samples.append(sum(chunk) / (channels * 32768.0))
    return samples, rate


def _hann(samples: list[float]) -> list[float]:
    count = len(samples)
    if count <= 1:
        return list(samples)
    scale = 2.0 * math.pi / (count - 1)
    return [
        sample * (0.5 - 0.5 * math.cos(scale * index))
        for index, sample in enumerate(samples)
    ]


def _goertzel_power(samples: list[float], sample_rate: int, freq_hz: float) -> float:
    count = len(samples)
    if count == 0 or sample_rate <= 0:
        return 0.0
    bin_index = int(round(count * freq_hz / sample_rate))
    angle = (2.0 * math.pi * bin_index) / count
    coefficient = 2.0 * math.cos(angle)
    first = 0.0
    second = 0.0
    for sample in samples:
        current = sample + coefficient * first - second
        second = first
        first = current
    return first * first + second * second - coefficient * first * second


def analyse_samples(samples: list[float], sample_rate: int) -> tuple[list[float], list[float]]:
    """Return (posterior, unit embedding) over STUB_LABELS."""
    windowed = _hann(samples)
    powers = [_goertzel_power(windowed, sample_rate, STUB_FREQ_HZ[name]) for name in STUB_LABELS]
    posterior = as_prob(softmax([math.log(power + 1e-12) for power in powers]))
    # Powers, not log-powers: a pure tone is nearly one-hot after L2, so
    # embedding cosine separates sectors the posterior also separates.
    embedding = as_unit(powers)
    return posterior, embedding


def analyse_path(path: Path) -> tuple[list[float], list[float]]:
    samples, rate = read_wav(path)
    return analyse_samples(samples, rate)


class StubBackend:
    """Deterministic fake posteriors. CPU CI uses this. It is not a recogniser."""

    name = "stub"
    labels = STUB_LABELS

    def analyse(self, path: Path) -> tuple[list[float], list[float]]:
        return analyse_path(path)


# Re-export for tests that want the twelve shared names.
__all__ = [
    "STUB_FREQ_HZ",
    "STUB_LABELS",
    "SHARED_SECTORS",
    "StubAudioError",
    "StubBackend",
    "analyse_path",
    "analyse_samples",
    "read_wav",
    "synthesize_tone",
    "write_sector_tone",
    "write_wav",
]
