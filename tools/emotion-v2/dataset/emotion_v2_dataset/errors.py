# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Errors for the Emotion V2 dataset pipeline."""

from __future__ import annotations


class DatasetError(Exception):
    """Base error. The message is safe to print; it does not include audio."""


class ConsentError(DatasetError):
    """A real-voice or Chatterbox render lacked a valid training consent record."""


class PitchPolicyError(DatasetError):
    """The voice was not on the 432 Hz standard, or a retune to 440 Hz was requested."""


class RealVoiceRefused(DatasetError):
    """The pipeline refused to synthesize or clone a voice."""


class ProfileError(DatasetError):
    """The voice profile is missing a required field or names an unknown emotion."""


class ScenarioError(DatasetError):
    """The scenario bank does not cover the requested emotions."""


class QualityError(DatasetError):
    """Scorer configuration is inconsistent."""


class PipelineError(DatasetError):
    """The run cannot be started or written."""
