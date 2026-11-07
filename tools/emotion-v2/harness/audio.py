# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""16-bit PCM helpers and synthetic tones.

The tone generator is a test signal. It is not a voice and it is not a
renderer. Scoring reads WAV files the caller already has.
"""

from __future__ import annotations

import math
import struct
import wave
from pathlib import Path


class AudioError(ValueError):
    """A WAV file is missing, empty, or not 16-bit PCM."""


def synthesize_tone(
    freq_hz: float,
    sample_rate: int = 16000,
    seconds: float = 0.4,
    amplitude: float = 0.2,
) -> list[float]:
    """A deterministic sine. Used by unit tests and by nothing in CI scoring."""
    if sample_rate <= 0:
        raise AudioError(f"sample rate must be positive, got {sample_rate}")
    if seconds <= 0:
        raise AudioError(f"duration must be positive, got {seconds}")
    count = int(sample_rate * seconds)
    step = 2.0 * math.pi * freq_hz / sample_rate
    return [amplitude * math.sin(step * i) for i in range(count)]


def write_wav(path: Path, samples: list[float], sample_rate: int) -> None:
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


def read_wav(path: Path) -> tuple[list[float], int]:
    """Return mono float samples in [-1, 1] and the sample rate.

    Stereo is averaged to mono. Only 16-bit PCM is accepted, so a compressed
    or float file fails before any model code runs.
    """
    if not path.is_file():
        raise AudioError(f"missing wav: {path}")
    try:
        handle = wave.open(str(path), "rb")
    except wave.Error as exc:
        raise AudioError(f"unreadable wav {path}: {exc}") from exc
    with handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        frames = handle.getnframes()
        raw = handle.readframes(frames)
    if width != 2:
        raise AudioError(f"{path} is {width * 8}-bit; Emotion V2 reads 16-bit PCM")
    if channels < 1:
        raise AudioError(f"{path} has no channels")
    if rate <= 0:
        raise AudioError(f"{path} has sample rate {rate}")
    count = len(raw) // 2
    if count == 0:
        raise AudioError(f"{path} is empty")
    unpacked = struct.unpack("<" + "h" * count, raw)
    if channels == 1:
        samples = [value / 32768.0 for value in unpacked]
    else:
        samples = []
        for index in range(0, count, channels):
            chunk = unpacked[index : index + channels]
            samples.append(sum(chunk) / (channels * 32768.0))
    if not samples:
        raise AudioError(f"{path} is empty")
    return samples, rate


def list_wavs(directory: Path) -> list[Path]:
    if not directory.is_dir():
        return []
    found = [
        path
        for path in directory.iterdir()
        if path.is_file() and path.suffix.lower() in {".wav", ".wave"}
    ]
    return sorted(found, key=lambda path: path.name)


def linear_resample(samples: list[float], source_rate: int, target_rate: int) -> list[float]:
    """Linear resample used only when a local recogniser expects another rate."""
    if source_rate <= 0 or target_rate <= 0:
        raise AudioError(f"bad sample rate {source_rate} -> {target_rate}")
    if source_rate == target_rate or not samples:
        return list(samples)
    duration = len(samples) / source_rate
    target_count = max(1, int(round(duration * target_rate)))
    last = len(samples) - 1
    out: list[float] = []
    for index in range(target_count):
        position = index * source_rate / target_rate
        left = min(int(position), last)
        right = min(left + 1, last)
        fraction = position - left
        out.append(samples[left] * (1.0 - fraction) + samples[right] * fraction)
    return out
