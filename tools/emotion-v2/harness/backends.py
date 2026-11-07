# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Pluggable categorical emotion backends.

The default recogniser is ``superb/wav2vec2-base-superb-er``, and only when a
complete weight directory is already on disk. This module never downloads.

The stub is a deterministic spectral stand-in. CPU CI runs the stub. It does
not claim to recognise emotion; it returns a stable posterior so the harness,
the thresholds, and the report can be tested without weights.
"""

from __future__ import annotations

import math
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

from .audio import linear_resample
from .config import emotion_v2_root, repo_root
from .emotions import EMOTIONS, STUB_FREQ_HZ
from .metrics import l2_normalize, softmax

SUPERB_ID = "superb/wav2vec2-base-superb-er"
# IEMOCAP label order published with the SUPERB emotion-recognition head.
SUPERB_LABELS: tuple[str, ...] = ("neu", "hap", "ang", "sad")


class BackendUnavailable(RuntimeError):
    """The requested recogniser cannot run locally. Nothing was downloaded."""


@dataclass(frozen=True)
class Analysis:
    labels: tuple[str, ...]
    posterior: tuple[float, ...]
    embedding: tuple[float, ...]
    # Element ids are labels. They are not features of the acoustic posterior.
    elements: tuple[str, ...] = ()

    def distribution(self) -> list[dict[str, object]]:
        """Emotion weights with the fixed element id. Acoustics are unchanged."""
        from harness.line_table import element_ids_for

        elements = self.elements or element_ids_for(self.labels)
        if len(elements) != len(self.labels):
            elements = element_ids_for(self.labels)
        return [
            {"emotion": label, "weight": float(weight), "element": element}
            for label, weight, element in zip(self.labels, self.posterior, elements, strict=True)
        ]


class EmotionBackend(Protocol):
    name: str

    def labels(self) -> tuple[str, ...]:
        """Category names, in posterior order."""

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        """Posterior over ``labels`` plus one embedding of the same signal."""


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


class StubBackend:
    """Deterministic fake posteriors over Samantha's twelve emotions.

    A pure tone at ``STUB_FREQ_HZ[emotion]`` peaks on that emotion. The same
    samples always produce the same posterior and the same embedding. The
    embedding is the L2-normalized bin power, not the posterior.
    """

    name = "stub"

    def labels(self) -> tuple[str, ...]:
        return EMOTIONS

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        windowed = _hann(samples)
        powers = [
            _goertzel_power(windowed, sample_rate, STUB_FREQ_HZ[emotion])
            for emotion in EMOTIONS
        ]
        logits = [math.log(power + 1e-12) for power in powers]
        posterior = softmax(logits)
        # Powers, not log-powers: a pure tone is nearly one-hot after L2,
        # so embedding cosine separates emotions the posterior also separates.
        embedding = l2_normalize(powers)
        from harness.line_table import element_ids_for

        return Analysis(
            labels=EMOTIONS,
            posterior=tuple(posterior),
            embedding=tuple(embedding),
            elements=element_ids_for(EMOTIONS),
        )


def _weight_files_present(directory: Path) -> bool:
    if not directory.is_dir() or not (directory / "config.json").is_file():
        return False
    names = ("model.safetensors", "pytorch_model.bin", "model.safetensors.index.json")
    return any((directory / name).is_file() for name in names)


def _snapshot_dir(cache_root: Path) -> Path | None:
    model_dir = cache_root / "models--superb--wav2vec2-base-superb-er"
    snapshots = model_dir / "snapshots"
    if not snapshots.is_dir():
        return None
    for candidate in sorted(path for path in snapshots.iterdir() if path.is_dir()):
        if _weight_files_present(candidate):
            return candidate
    return None


def discover_superb_weights() -> Path | None:
    """Return a local weight directory, or None.

    ``EMOTION_V2_SUPERB_WEIGHTS`` is exclusive: when it is set, no other cache
    is consulted and a missing path stays missing. This function does not
    create directories and does not contact a hub.
    """
    override = os.environ.get("EMOTION_V2_SUPERB_WEIGHTS")
    if override is not None:
        path = Path(override).expanduser()
        return path if _weight_files_present(path) else None
    local = repo_root() / "weights" / "emotion-v2" / "wav2vec2-base-superb-er"
    if _weight_files_present(local):
        return local
    bundled = emotion_v2_root() / "weights" / "wav2vec2-base-superb-er"
    if _weight_files_present(bundled):
        return bundled
    caches: list[Path] = []
    for key in ("HF_HUB_CACHE", "HUGGINGFACE_HUB_CACHE"):
        raw = os.environ.get(key)
        if raw:
            caches.append(Path(raw).expanduser())
    hf_home = os.environ.get("HF_HOME")
    if hf_home:
        caches.append(Path(hf_home).expanduser() / "hub")
    caches.append(Path.home() / ".cache" / "huggingface" / "hub")
    seen: set[Path] = set()
    for cache in caches:
        resolved = cache.resolve() if cache.exists() else cache
        if resolved in seen:
            continue
        seen.add(resolved)
        found = _snapshot_dir(cache)
        if found is not None:
            return found
    return None


class SuperbBackend:
    """``superb/wav2vec2-base-superb-er`` from a local directory.

    ``from_pretrained`` is called with that directory and ``local_files_only``.
    The hub id is never passed to the loader.
    """

    name = SUPERB_ID

    def __init__(self, weights_dir: Path) -> None:
        self.weights_dir = weights_dir
        self._model: object | None = None
        self._extractor: object | None = None
        self._labels: tuple[str, ...] | None = None

    def labels(self) -> tuple[str, ...]:
        if self._labels is None:
            self._load()
        assert self._labels is not None
        return self._labels

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        model, extractor, labels = self._load()
        target_rate = int(getattr(extractor, "sampling_rate", 16000))
        audio = linear_resample(samples, sample_rate, target_rate)
        features = extractor(audio, sampling_rate=target_rate, return_tensors="pt")
        import torch

        with torch.no_grad():
            output = model(**features, output_hidden_states=True)
        logits = output.logits[0].detach().float().cpu().tolist()
        if len(logits) != len(labels):
            raise BackendUnavailable(
                f"superb head returned {len(logits)} logits for {len(labels)} labels"
            )
        hidden_states = getattr(output, "hidden_states", None)
        if not hidden_states:
            raise BackendUnavailable("superb model did not return hidden states; refusing to guess an embedding")
        hidden = hidden_states[-1][0].detach().float().cpu()
        embedding = l2_normalize(hidden.mean(dim=0).tolist())
        from harness.line_table import element_ids_for

        posterior = tuple(softmax(logits))
        return Analysis(
            labels=labels,
            posterior=posterior,
            embedding=tuple(embedding),
            elements=element_ids_for(labels),
        )

    def _load(self) -> tuple[object, object, tuple[str, ...]]:
        if self._model is not None and self._extractor is not None and self._labels is not None:
            return self._model, self._extractor, self._labels
        if not _weight_files_present(self.weights_dir):
            raise BackendUnavailable(
                f"superb weights are not complete at {self.weights_dir}; refusing to download"
            )
        try:
            from transformers import AutoFeatureExtractor, AutoModelForAudioClassification
        except ImportError as exc:
            raise BackendUnavailable(
                "torch and transformers are not installed; refusing to download them or the weights"
            ) from exc
        directory = str(self.weights_dir)
        extractor = AutoFeatureExtractor.from_pretrained(directory, local_files_only=True)
        model = AutoModelForAudioClassification.from_pretrained(directory, local_files_only=True)
        model.eval()
        labels = _labels_from_config(getattr(model, "config", None))
        self._extractor = extractor
        self._model = model
        self._labels = labels
        return model, extractor, labels


def _labels_from_config(config: object) -> tuple[str, ...]:
    mapping = getattr(config, "id2label", None)
    if isinstance(mapping, dict) and mapping:
        pairs: list[tuple[int, str]] = []
        for key, value in mapping.items():
            try:
                pairs.append((int(key), str(value)))
            except (TypeError, ValueError):
                continue
        if pairs:
            pairs.sort(key=lambda item: item[0])
            return tuple(label for _, label in pairs)
    return SUPERB_LABELS


def resolve_backend(choice: str | None = None) -> EmotionBackend:
    """Select a backend.

    ``auto`` (the default) uses SUPERB when a local weight directory is
    complete, and the stub otherwise. ``superb`` fails when the weights are
    absent. It does not fall back and it does not download.
    """
    selected = (choice or os.environ.get("EMOTION_V2_BACKEND") or "auto").strip().lower()
    if selected in {"stub", "fake"}:
        return StubBackend()
    if selected in {"auto", "default"}:
        found = discover_superb_weights()
        if found is None:
            return StubBackend()
        return SuperbBackend(found)
    if selected in {"superb", SUPERB_ID}:
        found = discover_superb_weights()
        if found is None:
            raise BackendUnavailable(
                "superb/wav2vec2-base-superb-er weights are not present locally; "
                "refusing to download. CPU CI uses --backend stub."
            )
        return SuperbBackend(found)
    raise BackendUnavailable(
        f"unknown backend {choice!r}; expected auto, stub, or {SUPERB_ID}"
    )
