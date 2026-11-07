# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Post-render augmentation: synthetic impulse, synthetic noise, then channel."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from emotion_v2_dataset.audio import (
    RENDER_RATE,
    AudioBuffer,
    apply_ir,
    mix_snr,
    simulate_headset,
    simulate_phone,
    synthetic_impulse,
    synthetic_noise,
)
from emotion_v2_dataset.environments import Environment


@dataclass(frozen=True)
class Augmented:
    audio: AudioBuffer
    environment: str
    snr_db: float | None
    ir_source: str
    ir_licence: str
    noise_source: str
    noise_licence: str
    telephony_hz: int | None
    process_note: str


def augment(
    dry: AudioBuffer,
    environment: Environment,
    snr_db: float | None,
    *,
    seed: int,
) -> Augmented:
    if dry.sample_rate != RENDER_RATE:
        raise ValueError(
            f"augmentation expects {RENDER_RATE} Hz dry audio, got {dry.sample_rate}"
        )
    impulse = synthetic_impulse(
        dry.sample_rate,
        environment.rt60_s,
        environment.early_taps,
        seed=seed,
    )
    wet = apply_ir(dry.samples, impulse)
    # Noise is built at the reverberant length, then scaled to the requested SNR.
    noise = synthetic_noise(environment.noise_kind, len(wet), dry.sample_rate, seed=seed + 17)
    mixed = mix_snr(wet, noise, snr_db)
    telephony_hz: int | None = None
    rate = dry.sample_rate
    if environment.telephony_hz == 8000:
        phone = simulate_phone(mixed, dry.sample_rate)
        samples = phone.samples
        rate = phone.sample_rate
        telephony_hz = 8000
    elif environment.headset:
        samples = simulate_headset(mixed, dry.sample_rate)
    else:
        samples = mixed
    peak = float(np.max(np.abs(samples))) if len(samples) else 0.0
    if peak > 1.0:
        samples = samples / peak
    return Augmented(
        audio=AudioBuffer(samples, rate),
        environment=environment.name,
        snr_db=snr_db,
        ir_source=environment.ir_source,
        ir_licence=environment.ir_licence,
        noise_source=environment.noise_source,
        noise_licence=environment.noise_licence,
        telephony_hz=telephony_hz,
        process_note=environment.process_note,
    )
