# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Fixed gas-line ids for the thirteen Nebula emotions.

The id is a label the visualiser binds to a particle. It does not change
pitch, energy, spectral tilt, or voice conditioning. Wavelengths are the
nominal air-line values already written into the id.
"""

from __future__ import annotations

# Emotion V2 order, then the Samantha-only overlay. The strings are the
# contract in docs/emotion-v2/NEBULA-LIVE-FEED.md.
GAS_LINES: dict[str, str] = {
    "neutral": "HeII-541.2",
    "calm": "Hbeta-486.1",
    "happy": "[NII]-575.5",
    "amused": "HeI-587.6",
    "excited": "NeI-594.5",
    "tender": "HeI-667.8",
    "sad": "HeII-468.6",
    "disappointed": "[ClIII]-551.7",
    "angry": "[SIII]-631.2",
    "fearful": "Hdelta-410.2",
    "surprised": "[OIII]-495.9",
    "disgusted": "[ClIII]-553.8",
    "seductive": "Halpha-656.3",
}

# The twelve shared Emotion V2 categories, in inventory order.
EMOTIONS: tuple[str, ...] = tuple(key for key in GAS_LINES if key != "seductive")
SEDUCTIVE = "seductive"

VOICES: tuple[str, ...] = ("samantha", "jarvis")


def wavelength_nm(gas_line: str) -> float:
    """Return the nominal wavelength stored in a gas-line id."""
    species, separator, wave = gas_line.rpartition("-")
    if separator != "-" or not species:
        raise ValueError(f"gas line {gas_line!r} has no wavelength")
    return float(wave)


def emotions_for(voice_id: str) -> tuple[str, ...]:
    """Emotion ids a voice may carry. Seductive is Samantha only."""
    if voice_id == "samantha":
        return EMOTIONS + (SEDUCTIVE,)
    if voice_id == "jarvis":
        return EMOTIONS
    known = ", ".join(VOICES)
    raise KeyError(f"unknown voice {voice_id!r}; expected one of {known}")
