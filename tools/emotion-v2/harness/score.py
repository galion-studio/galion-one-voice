# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Score one emotion, and score Samantha's twelve.

For each emotion the categorical posterior of the renders is compared with
the posterior of that emotion's reference (cosine of the two mean
posteriors). Embeddings are compared the same way. WER and ECAPA are the
existing guards, called only when their scorers are present.
"""

from __future__ import annotations

import tempfile
from dataclasses import dataclass
from pathlib import Path

from .audio import AudioError, list_wavs, read_wav
from .backends import Analysis, BackendUnavailable, EmotionBackend
from .config import ConfigError, Scenario, Thresholds, emotion_subdir
from .emotions import EMOTIONS, canonical_emotion, display_name
from .guards import GuardResult, guard_accepts, locate_scorer, score_ecapa, score_wer
from .metrics import cosine, mean_vector, meets_min


@dataclass
class EmotionScore:
    emotion: str
    display: str
    backend: str
    tuning_hz: int
    persona: str
    target: float
    posterior_vs_reference: float | None
    embedding_similarity: float | None
    embedding_min: float | None
    emotion_pass: bool
    embedding_pass: bool | None
    wer: GuardResult
    ecapa: GuardResult
    passed: bool
    labels: tuple[str, ...]
    render_posterior: tuple[float, ...] | None
    reference_posterior: tuple[float, ...] | None
    scenarios: tuple[Scenario, ...]
    renders: tuple[str, ...]
    references: tuple[str, ...]
    speaker_ref: str | None
    error: str | None

    def as_dict(self) -> dict[str, object]:
        return {
            "schema": "galion-emotion-v2-score/1",
            "persona": self.persona,
            "tuning_hz": self.tuning_hz,
            "emotion": self.emotion,
            "display": self.display,
            "backend": self.backend,
            "labels": list(self.labels),
            "aggregation": "mean_posterior_cosine",
            "posterior_vs_reference": _round(self.posterior_vs_reference),
            "reference_posterior": _round_seq(self.reference_posterior),
            "render_posterior": _round_seq(self.render_posterior),
            "embedding_similarity": _round(self.embedding_similarity),
            "embedding_min": self.embedding_min,
            "embedding_pass": self.embedding_pass,
            "target": self.target,
            "emotion_pass": self.emotion_pass,
            "wer": self.wer.as_dict(),
            "ecapa": self.ecapa.as_dict(),
            "scenarios": [{"id": item.stem, "text": item.text} for item in self.scenarios],
            "renders": list(self.renders),
            "references": list(self.references),
            "speaker_ref": self.speaker_ref,
            "pass": self.passed,
            "error": self.error,
        }


@dataclass
class Summary:
    persona: str
    tuning_hz: int
    backend: str
    require_guards: bool
    scores: list[EmotionScore]

    @property
    def passed(self) -> bool:
        return bool(self.scores) and all(row.passed for row in self.scores)

    def as_dict(self) -> dict[str, object]:
        return {
            "schema": "galion-emotion-v2-summary/1",
            "persona": self.persona,
            "tuning_hz": self.tuning_hz,
            "backend": self.backend,
            "require_guards": self.require_guards,
            "pass": self.passed,
            "emotions": [
                {
                    "emotion": row.emotion,
                    "display": row.display,
                    "posterior_vs_reference": _round(row.posterior_vs_reference),
                    "target": row.target,
                    "embedding_similarity": _round(row.embedding_similarity),
                    "wer": row.wer.value if row.wer.status == "ok" else row.wer.status,
                    "ecapa": row.ecapa.value if row.ecapa.status == "ok" else row.ecapa.status,
                    "pass": row.passed,
                    "error": row.error,
                }
                for row in self.scores
            ],
        }


def score_emotion(
    emotion: str,
    render_dir: Path,
    reference_dir: Path,
    scenario_dir: Path,
    thresholds: Thresholds,
    backend: EmotionBackend,
    speaker_ref: Path | None = None,
    require_guards: bool = False,
    wer_bin: Path | None = None,
    ecapa_bin: Path | None = None,
) -> EmotionScore:
    """Score one emotion.

    ``render_dir`` and ``reference_dir`` are that emotion's directories.
    ``scenario_dir`` holds the fixed ``*.txt`` lines for the WER guard.
    Reference clips are emotion exemplars, so stems are not paired: the
    harness averages each side, then takes a cosine.
    """
    key = canonical_emotion(emotion)
    return _score_emotion(
        key,
        render_dir,
        reference_dir,
        scenario_dir,
        thresholds,
        backend,
        speaker_ref,
        require_guards,
        wer_bin,
        ecapa_bin,
    )


def score_all(
    render_root: Path,
    reference_root: Path,
    scenario_root: Path,
    thresholds: Thresholds,
    backend: EmotionBackend,
    emotions: tuple[str, ...] = EMOTIONS,
    speaker_ref: Path | None = None,
    require_guards: bool = False,
    wer_bin: Path | None = None,
    ecapa_bin: Path | None = None,
) -> Summary:
    rows = [
        _score_emotion(
            canonical_emotion(emotion),
            emotion_subdir(render_root, emotion),
            emotion_subdir(reference_root, emotion),
            emotion_subdir(scenario_root, emotion),
            thresholds,
            backend,
            speaker_ref,
            require_guards,
            wer_bin,
            ecapa_bin,
        )
        for emotion in emotions
    ]
    return Summary(
        persona=thresholds.persona,
        tuning_hz=thresholds.tuning_hz,
        backend=backend.name,
        require_guards=require_guards,
        scores=rows,
    )


def _score_emotion(
    emotion: str,
    render_dir: Path,
    reference_dir: Path,
    scenario_dir: Path,
    thresholds: Thresholds,
    backend: EmotionBackend,
    speaker_ref: Path | None,
    require_guards: bool,
    wer_bin: Path | None,
    ecapa_bin: Path | None,
) -> EmotionScore:
    target = thresholds.target_for(emotion)
    scenarios, scenario_error = _read_scenarios(scenario_dir)
    renders = list_wavs(render_dir)
    references = list_wavs(reference_dir)
    chosen_ref = speaker_ref if speaker_ref is not None else (references[0] if references else None)
    common = dict(
        emotion=emotion,
        display=display_name(emotion),
        backend=backend.name,
        tuning_hz=thresholds.tuning_hz,
        persona=thresholds.persona,
        target=target,
        embedding_min=thresholds.embedding_min,
        scenarios=scenarios,
        renders=tuple(path.name for path in renders),
        references=tuple(path.name for path in references),
        speaker_ref=str(chosen_ref) if chosen_ref is not None else None,
    )
    if scenario_error:
        return _failed(common, scenario_error, thresholds, require_guards)
    if not renders:
        return _failed(common, f"no render wavs in {render_dir}", thresholds, require_guards)
    if not references:
        return _failed(common, f"no reference wavs in {reference_dir}", thresholds, require_guards)
    try:
        labels, render_posterior, render_embedding = _mean_analysis(backend, renders)
        ref_labels, reference_posterior, reference_embedding = _mean_analysis(backend, references)
    except (AudioError, BackendUnavailable, ValueError, RuntimeError) as exc:
        return _failed(common, str(exc), thresholds, require_guards)
    if labels != ref_labels:
        return _failed(
            common,
            "render and reference posteriors use different labels",
            thresholds,
            require_guards,
        )
    posterior_score = cosine(render_posterior, reference_posterior)
    embedding_score = cosine(render_embedding, reference_embedding)
    emotion_pass = meets_min(posterior_score, target)
    if thresholds.embedding_min is None:
        embedding_pass = None
    else:
        embedding_pass = meets_min(embedding_score, thresholds.embedding_min)
    with tempfile.TemporaryDirectory(prefix="emotion-v2-guards-") as tmp:
        work = Path(tmp)
        wer = score_wer(
            locate_scorer("wer", wer_bin),
            renders,
            scenario_dir,
            thresholds.wer_max,
            work,
        )
        ecapa = score_ecapa(
            locate_scorer("ecapa", ecapa_bin),
            renders,
            chosen_ref,
            thresholds.ecapa_min,
            work,
        )
    passed = emotion_pass and guard_accepts(wer, require_guards) and guard_accepts(ecapa, require_guards)
    if embedding_pass is False:
        passed = False
    return EmotionScore(
        posterior_vs_reference=posterior_score,
        embedding_similarity=embedding_score,
        emotion_pass=emotion_pass,
        embedding_pass=embedding_pass,
        wer=wer,
        ecapa=ecapa,
        passed=passed,
        labels=labels,
        render_posterior=tuple(render_posterior),
        reference_posterior=tuple(reference_posterior),
        error=None,
        **common,
    )


def _read_scenarios(scenario_dir: Path) -> tuple[tuple[Scenario, ...], str | None]:
    if not scenario_dir.is_dir():
        return (), f"missing scenario directory: {scenario_dir}"
    files = sorted(
        (path for path in scenario_dir.iterdir() if path.is_file() and path.suffix == ".txt"),
        key=lambda path: path.name,
    )
    if not files:
        return (), f"no scenario texts in {scenario_dir}"
    loaded: list[Scenario] = []
    try:
        for path in files:
            text = path.read_text(encoding="utf-8").strip()
            if not text:
                raise ConfigError(f"empty scenario text: {path}")
            loaded.append(Scenario(stem=path.stem, text=text, path=path))
    except (OSError, ConfigError) as exc:
        return (), str(exc)
    return tuple(loaded), None


def _mean_analysis(
    backend: EmotionBackend, wavs: list[Path]
) -> tuple[tuple[str, ...], list[float], list[float]]:
    analyses: list[Analysis] = []
    labels: tuple[str, ...] | None = None
    for wav in wavs:
        samples, rate = read_wav(wav)
        analysis = backend.analyse(samples, rate)
        if labels is None:
            labels = analysis.labels
        elif analysis.labels != labels:
            raise ValueError(f"{wav.name} changed the posterior labels")
        if len(analysis.posterior) != len(labels):
            raise ValueError(f"{wav.name} posterior length does not match its labels")
        if abs(sum(analysis.posterior) - 1.0) > 1e-4:
            raise ValueError(f"{wav.name} posterior does not sum to 1")
        if any(value < -1e-6 for value in analysis.posterior):
            raise ValueError(f"{wav.name} posterior has a negative probability")
        analyses.append(analysis)
    assert labels is not None
    posterior = mean_vector([list(item.posterior) for item in analyses])
    embedding = mean_vector([list(item.embedding) for item in analyses])
    return labels, posterior, embedding


def _failed(common: dict, error: str, thresholds: Thresholds, require_guards: bool) -> EmotionScore:
    if require_guards:
        wer = GuardResult("wer", "error", None, thresholds.wer_max, "<=", False, error)
        ecapa = GuardResult("ecapa", "error", None, thresholds.ecapa_min, ">", False, error)
    else:
        wer = GuardResult("wer", "absent", None, thresholds.wer_max, "<=", None, "not scored")
        ecapa = GuardResult("ecapa", "absent", None, thresholds.ecapa_min, ">", None, "not scored")
    embedding_pass = False if thresholds.embedding_min is not None else None
    return EmotionScore(
        posterior_vs_reference=None,
        embedding_similarity=None,
        emotion_pass=False,
        embedding_pass=embedding_pass,
        wer=wer,
        ecapa=ecapa,
        passed=False,
        labels=(),
        render_posterior=None,
        reference_posterior=None,
        error=error,
        **common,
    )


def _round(value: float | None) -> float | None:
    if value is None:
        return None
    return round(float(value), 6)


def _round_seq(values: tuple[float, ...] | None) -> list[float] | None:
    if values is None:
        return None
    return [round(float(value), 6) for value in values]
