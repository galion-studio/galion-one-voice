# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Synthetic signals, wav I/O, and open band-limit / codec simulations.

No recorded room, no downloaded noise, no voice clone. The stub voice is a
harmonic stack plus a little noise, tuned from ``pitch_centre_hz`` on the
432 Hz standard.
"""

from __future__ import annotations

import hashlib
import wave
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from emotion_v2_dataset.errors import PipelineError
from emotion_v2_dataset.profile import PITCH_STANDARD_HZ, forbid_pitch_standard
from emotion_v2_dataset.taxonomy import EMOTION_CENTS, EMOTION_TILT, require_emotion

RENDER_RATE = 24_000
PHONE_RATE = 8_000


@dataclass
class AudioBuffer:
    samples: np.ndarray
    sample_rate: int

    def __post_init__(self) -> None:
        self.samples = np.asarray(self.samples, dtype=np.float64).reshape(-1)
        self.sample_rate = int(self.sample_rate)
        if self.sample_rate < 1:
            raise PipelineError("sample rate must be positive")

    @property
    def duration_s(self) -> float:
        if len(self.samples) == 0:
            return 0.0
        return float(len(self.samples)) / float(self.sample_rate)


def synthesize_stub(
    *,
    text: str,
    emotion: str,
    intensity: float,
    pitch_centre_hz: float,
    pitch_standard_hz: float = PITCH_STANDARD_HZ,
    sample_rate: int = RENDER_RATE,
) -> AudioBuffer:
    """Sine-stack plus noise. Does not read a reference and does not retune to 440."""
    forbid_pitch_standard(pitch_standard_hz)
    emotion = require_emotion(emotion)
    if not text.strip():
        raise PipelineError("refusing to render empty text")
    if not 0.0 <= intensity <= 1.0:
        raise PipelineError("intensity must be between 0 and 1")
    words = max(1, len(text.split()))
    duration = min(2.4, max(0.45, 0.11 * words))
    count = int(sample_rate * duration)
    time = np.arange(count, dtype=np.float64) / sample_rate
    cents = EMOTION_CENTS[emotion] * float(intensity)
    # Cents relative to the given centre. The 440/432 concert ratio is never applied.
    fundamental = float(pitch_centre_hz) * (2.0 ** (cents / 1200.0))
    vibrato = 1.0 + (0.004 + 0.008 * float(intensity)) * np.sin(2.0 * np.pi * 4.5 * time)
    phase = 2.0 * np.pi * fundamental * np.cumsum(vibrato) / sample_rate
    tilt = EMOTION_TILT[emotion]
    signal = np.sin(phase)
    signal += tilt * 0.40 * np.sin(2.0 * phase)
    signal += tilt * 0.18 * np.sin(3.0 * phase)
    envelope = np.minimum(1.0, np.minimum(time / 0.02, (duration - time) / 0.04))
    envelope = np.clip(envelope, 0.0, 1.0)
    generator = np.random.default_rng(_seed(text, emotion, intensity, pitch_centre_hz))
    noise = 0.015 * (0.4 + tilt) * generator.standard_normal(count)
    signal = (signal + noise) * envelope
    peak = float(np.max(np.abs(signal))) + 1e-9
    signal = 0.45 * signal / peak
    return AudioBuffer(signal, sample_rate)


def write_wav(path: Path, audio: AudioBuffer) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    pcm = _pcm16(audio.samples)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(audio.sample_rate)
        handle.writeframes(pcm.tobytes())


def write_stereo_wav(path: Path, left: AudioBuffer, right: AudioBuffer) -> None:
    if left.sample_rate != right.sample_rate:
        raise PipelineError("stereo channels must share a sample rate")
    count = max(len(left.samples), len(right.samples))
    left_pcm = _pcm16(_pad(left.samples, count))
    right_pcm = _pcm16(_pad(right.samples, count))
    interleaved = np.empty(count * 2, dtype=np.int16)
    interleaved[0::2] = left_pcm
    interleaved[1::2] = right_pcm
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(2)
        handle.setsampwidth(2)
        handle.setframerate(left.sample_rate)
        handle.writeframes(interleaved.tobytes())


def read_wav(path: Path) -> tuple[np.ndarray, int, int]:
    """Return ``(samples, sample_rate, channels)``. Samples are float in -1..1.

    Mono returns shape ``(n,)``. Stereo returns shape ``(n, 2)``.
    """
    with wave.open(str(path), "rb") as handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        frames = handle.getnframes()
        raw = handle.readframes(frames)
    if width != 2:
        raise PipelineError(f"only 16-bit wav is read, got width {width}")
    pcm = np.frombuffer(raw, dtype=np.int16).astype(np.float64) / 32767.0
    if channels == 1:
        return pcm, rate, 1
    if channels == 2:
        return pcm.reshape(-1, 2), rate, 2
    raise PipelineError(f"unsupported channel count {channels}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resample_linear(audio: AudioBuffer, sample_rate: int) -> AudioBuffer:
    if sample_rate == audio.sample_rate:
        return AudioBuffer(audio.samples.copy(), sample_rate)
    if len(audio.samples) == 0:
        return AudioBuffer(np.zeros(0), sample_rate)
    duration = len(audio.samples) / audio.sample_rate
    count = max(1, int(round(duration * sample_rate)))
    source_t = np.linspace(0.0, 1.0, len(audio.samples), endpoint=False)
    target_t = np.linspace(0.0, 1.0, count, endpoint=False)
    return AudioBuffer(np.interp(target_t, source_t, audio.samples), sample_rate)


def brickwall(samples: np.ndarray, sample_rate: int, low_hz: float, high_hz: float) -> np.ndarray:
    if len(samples) < 8:
        return samples.copy()
    spectrum = np.fft.rfft(samples)
    freqs = np.fft.rfftfreq(len(samples), d=1.0 / sample_rate)
    spectrum[(freqs < low_hz) | (freqs > high_hz)] = 0
    return np.fft.irfft(spectrum, n=len(samples)).real.astype(np.float64)


def simulate_phone(samples: np.ndarray, sample_rate: int) -> AudioBuffer:
    """Narrowband telephone: 300–3400 Hz, then integer decimation to 8 kHz.

    The band limit is an in-process FFT brickwall (not a downloaded codec).
    Decimation happens after the brickwall so a 6 kHz tone does not alias in.
    """
    if sample_rate % PHONE_RATE != 0:
        raise PipelineError("phone simulation expects a sample rate divisible by 8000")
    limited = brickwall(samples, sample_rate, low_hz=300.0, high_hz=3400.0)
    factor = sample_rate // PHONE_RATE
    trimmed = limited[: len(limited) // factor * factor]
    if len(trimmed) == 0:
        return AudioBuffer(np.zeros(1), PHONE_RATE)
    down = trimmed.reshape(-1, factor).mean(axis=1)
    return AudioBuffer(down, PHONE_RATE)


def simulate_headset(samples: np.ndarray, sample_rate: int) -> np.ndarray:
    """Headset-like band limit plus ITU-T G.711 mu-law.

    G.711 mu-law is an open ITU algorithm. This is not a Bluetooth SBC
    bitstream and not a vendor codec. No codec library is downloaded.
    """
    limited = brickwall(samples, sample_rate, low_hz=80.0, high_hz=7_500.0)
    return mulaw_roundtrip(limited)


def mulaw_roundtrip(samples: np.ndarray, mu: float = 255.0) -> np.ndarray:
    clipped = np.clip(samples, -1.0, 1.0)
    encoded = np.sign(clipped) * np.log1p(mu * np.abs(clipped)) / np.log1p(mu)
    quantized = np.round((encoded + 1.0) * 0.5 * 255.0) / 255.0 * 2.0 - 1.0
    quantized = np.clip(quantized, -1.0, 1.0)
    return np.sign(quantized) * (np.expm1(np.abs(quantized) * np.log1p(mu)) / mu)


def mix_snr(speech: np.ndarray, noise: np.ndarray, snr_db: float | None) -> np.ndarray:
    """Add ``noise`` so its RMS is ``snr_db`` below the speech RMS.

    ``None`` leaves the speech unchanged (dry).
    """
    if snr_db is None:
        return speech.copy()
    speech = np.asarray(speech, dtype=np.float64)
    if len(speech) == 0:
        return speech.copy()
    noise = _fit_noise(np.asarray(noise, dtype=np.float64), len(speech))
    speech_rms = float(np.sqrt(np.mean(speech**2) + 1e-18))
    noise_rms = float(np.sqrt(np.mean(noise**2) + 1e-18))
    target = speech_rms / (10.0 ** (float(snr_db) / 20.0))
    return speech + noise * (target / noise_rms)


def apply_ir(speech: np.ndarray, impulse: np.ndarray) -> np.ndarray:
    speech = np.asarray(speech, dtype=np.float64)
    impulse = np.asarray(impulse, dtype=np.float64).reshape(-1)
    if len(impulse) == 0:
        return speech.copy()
    if len(impulse) == 1 and abs(float(impulse[0]) - 1.0) < 1e-12:
        return speech.copy()
    wet = np.convolve(speech, impulse, mode="full")
    peak = float(np.max(np.abs(wet))) + 1e-12
    target = max(float(np.max(np.abs(speech))), 1e-3)
    wet *= min(1.0, target / peak)
    return wet


def synthetic_impulse(
    sample_rate: int,
    rt60_s: float,
    early_taps: int,
    seed: int,
    max_samples: int = 4800,
) -> np.ndarray:
    """Direct path, a few early taps, exponential decay. Generated here.

    Nothing is downloaded. The decay constant uses the usual RT60 definition
    (energy down 60 dB, amplitude factor ``exp(-6.907 t / RT60)``).
    """
    if rt60_s <= 0:
        return np.array([1.0], dtype=np.float64)
    count = int(sample_rate * min(rt60_s * 1.1, 0.35))
    count = max(32, min(count, max_samples))
    generator = np.random.default_rng(seed)
    time = np.arange(count, dtype=np.float64) / sample_rate
    decay = np.exp(-6.907 * time / rt60_s)
    impulse = generator.standard_normal(count) * decay * 0.15
    impulse[0] = 1.0
    earliest = max(1, int(0.004 * sample_rate))
    latest = max(earliest + 1, int(0.045 * sample_rate))
    for _ in range(early_taps):
        tap = int(generator.integers(earliest, min(latest, count)))
        impulse[tap] += float(generator.uniform(0.15, 0.55))
    peak = float(np.max(np.abs(impulse))) + 1e-12
    return (impulse / peak).astype(np.float64)


def synthetic_noise(kind: str, count: int, sample_rate: int, seed: int) -> np.ndarray:
    """In-process noise. Not a recording of a place or a person."""
    if count <= 0:
        return np.zeros(0, dtype=np.float64)
    generator = np.random.default_rng(seed)
    white = generator.standard_normal(count)
    if kind == "none":
        return np.zeros(count, dtype=np.float64)
    if kind == "room":
        return _moving_average(white, max(1, sample_rate // 400))
    if kind == "rumble":
        return _moving_average(white, max(1, sample_rate // 80))
    if kind == "babble":
        time = np.arange(count, dtype=np.float64) / sample_rate
        stack = np.zeros(count, dtype=np.float64)
        for index, freq in enumerate((137.0, 183.0, 241.0, 307.0, 389.0)):
            phase = float(generator.uniform(0.0, 2.0 * np.pi))
            gate = 0.45 + 0.55 * np.sin(2.0 * np.pi * (0.6 + 0.17 * index) * time)
            stack += np.sin(2.0 * np.pi * freq * time + phase) * gate
        stack += 0.35 * white
        return stack
    if kind == "street":
        gust = _moving_average(white, max(1, sample_rate // 30))
        return gust + 0.25 * white
    if kind == "line":
        return white
    if kind == "hiss":
        return white
    raise PipelineError(f"unknown noise kind {kind}")


def dominant_hz(samples: np.ndarray, sample_rate: int, fmin: float = 70.0, fmax: float = 500.0) -> float:
    """Autocorrelation fundamental, for tests of the stub sine."""
    signal = np.asarray(samples, dtype=np.float64)
    signal = signal - float(np.mean(signal))
    if len(signal) < int(sample_rate / fmin) + 2:
        raise PipelineError("signal is too short to estimate pitch")
    correlation = np.correlate(signal, signal, mode="full")
    correlation = correlation[len(correlation) // 2 :]
    low = max(1, int(sample_rate / fmax))
    high = min(len(correlation) - 1, int(sample_rate / fmin))
    if high <= low:
        raise PipelineError("pitch search range is empty")
    lag = low + int(np.argmax(correlation[low:high]))
    return float(sample_rate) / float(lag)


def measure_snr_db(clean: np.ndarray, mixed: np.ndarray) -> float:
    residual = np.asarray(mixed, dtype=np.float64) - np.asarray(clean, dtype=np.float64)
    noise = float(np.sqrt(np.mean(residual**2) + 1e-18))
    speech = float(np.sqrt(np.mean(np.asarray(clean, dtype=np.float64) ** 2) + 1e-18))
    return 20.0 * float(np.log10(speech / noise))


def _pcm16(samples: np.ndarray) -> np.ndarray:
    clipped = np.clip(np.asarray(samples, dtype=np.float64), -1.0, 1.0)
    return np.round(clipped * 32767.0).astype(np.int16)


def _pad(samples: np.ndarray, count: int) -> np.ndarray:
    if len(samples) >= count:
        return samples[:count]
    out = np.zeros(count, dtype=np.float64)
    out[: len(samples)] = samples
    return out


def _fit_noise(noise: np.ndarray, count: int) -> np.ndarray:
    if len(noise) == count:
        return noise
    if len(noise) == 0:
        return np.zeros(count, dtype=np.float64)
    reps = int(np.ceil(count / len(noise)))
    return np.tile(noise, reps)[:count]


def _moving_average(samples: np.ndarray, window: int) -> np.ndarray:
    if window <= 1:
        return samples.copy()
    cumulative = np.cumsum(np.insert(samples, 0, 0.0))
    averaged = (cumulative[window:] - cumulative[:-window]) / window
    pad = window - 1
    return np.pad(averaged, (pad, 0), mode="edge")


def stable_seed(*parts: object) -> int:
    return _seed(*parts)


def _seed(*parts: object) -> int:
    payload = "|".join(str(part) for part in parts).encode("utf-8")
    return int.from_bytes(hashlib.sha256(payload).digest()[:8], "little")
