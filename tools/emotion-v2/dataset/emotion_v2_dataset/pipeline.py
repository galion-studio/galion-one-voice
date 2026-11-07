# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Emotion V2 training-data run.

Order, for each line: render, quality filter, environment augmentation, manifest.
The dry render is scored. Augmentation does not rerun the scorers, so the
manifest scores describe the clean take and the environment fields describe
the copy that was written.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any, Callable

from emotion_v2_dataset.audio import stable_seed, write_wav, sha256_file
from emotion_v2_dataset.augment import augment
from emotion_v2_dataset.backends import RenderEngine, RenderJob, make_backend
from emotion_v2_dataset.consent import (
    BLOCKED_VOICE_SUBSTRINGS,
    ConsentManifest,
    load_consent,
    require_training_consent,
)
from emotion_v2_dataset.environments import (
    IR_LICENCE,
    NOISE_LICENCE,
    resolve_environments,
    snr_for,
)
from emotion_v2_dataset.errors import ConsentError, RealVoiceRefused
from emotion_v2_dataset.exporters import export_personaplex
from emotion_v2_dataset.manifest import ManifestRow, write_jsonl
from emotion_v2_dataset.paths import assert_safe_output, check_id, find_repo_root, slug
from emotion_v2_dataset.plugins import load_scorer_plugins, resolve_references, taxonomy_source
from emotion_v2_dataset.profile import VoiceProfile, load_profile, require_real_references
from emotion_v2_dataset.quality import (
    QualityThresholds,
    RenderedClip,
    decide,
    score_clip,
)
from emotion_v2_dataset.scenarios import Scenario, load_scenarios, select_scenarios
from emotion_v2_dataset.taxonomy import require_emotion


@dataclass
class RunConfig:
    profile_path: Path
    scenarios_path: Path
    output_root: Path
    run_id: str
    backend: str = "stub"
    consent_path: Path | None = None
    emotions: tuple[str, ...] | None = None
    environments: tuple[str, ...] | None = None
    scenario_ids: tuple[str, ...] | None = None
    max_scenarios_per_emotion: int | None = None
    snr_levels: tuple[float | None, ...] | None = None
    thresholds: QualityThresholds | None = None
    scorers: list[Any] | None = None
    eval_fraction: float = 0.0
    personaplex_adapter: str = "moshi"
    absolute_personaplex_paths: bool = False
    today: date | None = None
    engine: RenderEngine | None = None
    repo_root: Path | None = None
    importer: Callable[[str], Any] | None = None


@dataclass
class RunResult:
    run_dir: Path
    manifest_path: Path
    rows: list[ManifestRow]
    rejected: list[dict[str, Any]]
    record: dict[str, Any]


def run_pipeline(config: RunConfig) -> RunResult:
    run_id = check_id("run_id", config.run_id)
    profile = load_profile(config.profile_path)
    _reject_blocked_voice(profile.voice_id)
    emotions, taxonomy_name = taxonomy_source(config.importer)
    if config.emotions is not None:
        emotions = tuple(require_emotion(name) for name in config.emotions)
    references, capture_name = resolve_references(
        profile.voice_id,
        profile.references,
        config.importer,
    )
    if profile.kind == "real":
        require_real_references(references)
    consent = (
        load_consent(config.consent_path, today=config.today)
        if config.consent_path is not None
        else None
    )
    needs_consent = profile.kind == "real" or config.backend == "chatterbox"
    if needs_consent:
        consent = require_training_consent(consent, profile.voice_id, today=config.today)
    elif consent is not None:
        consent = require_training_consent(consent, profile.voice_id, today=config.today)
    consent_ok = consent is not None
    bank = load_scenarios(config.scenarios_path)
    lines = select_scenarios(
        bank,
        emotions,
        max_per_emotion=config.max_scenarios_per_emotion,
        scenario_ids=config.scenario_ids,
    )
    environments = resolve_environments(config.environments)
    thresholds = config.thresholds or QualityThresholds()
    if config.scorers is not None:
        scorers: list[Any] = list(config.scorers)
        scorer_source = "configured"
    else:
        scorers, scorer_source = load_scorer_plugins(config.importer)
    repo = config.repo_root if config.repo_root is not None else find_repo_root()
    run_dir = Path(config.output_root) / profile.voice_id / run_id
    assert_safe_output(run_dir, repo)
    if config.backend == "chatterbox" and config.engine is None:
        raise RealVoiceRefused(
            "Chatterbox backend is consent-gated and has no local engine. "
            "This package does not download weights and does not synthesize "
            "or clone a voice."
        )
    backend = make_backend(
        config.backend,
        consent=consent,
        engine=config.engine,
        today=config.today,
    )
    run_dir.mkdir(parents=True, exist_ok=True)

    rows: list[ManifestRow] = []
    rejected: list[dict[str, Any]] = []
    unused: set[str] = set()
    for line in lines:
        job = _job(profile, line, references, consent_ok)
        dry = backend.render(job)
        clip = RenderedClip(
            audio=dry,
            text=line.text,
            emotion=line.emotion,
            voice_id=profile.voice_id,
            intensity=line.intensity,
        )
        card, ignored = score_clip(clip, scorers)
        unused.update(ignored)
        decision = decide(card, thresholds)
        if not decision.accepted:
            rejected.append(
                {
                    "text": line.text,
                    "emotion": line.emotion,
                    "intensity": line.intensity,
                    "scenario_id": line.scenario_id,
                    "voice_id": profile.voice_id,
                    "reason": decision.reason,
                    "scores": decision.scores.as_dict(),
                }
            )
            continue
        for environment in environments:
            for snr in snr_for(environment, config.snr_levels):
                seed = stable_seed(
                    profile.voice_id,
                    line.scenario_id,
                    line.emotion,
                    environment.name,
                    snr,
                )
                rendered = augment(dry, environment, snr, seed=seed)
                relative = _audio_relative(line, environment.name, snr)
                target = run_dir / relative
                write_wav(target, rendered.audio)
                rows.append(
                    ManifestRow(
                        audio_path=relative.as_posix(),
                        text=line.text,
                        emotion=line.emotion,
                        intensity=line.intensity,
                        environment=environment.name,
                        snr_db=snr,
                        voice_id=profile.voice_id,
                        scores=decision.scores.as_dict(),
                        sha256=sha256_file(target),
                        sample_rate_hz=rendered.audio.sample_rate,
                        pitch_standard_hz=profile.pitch_standard_hz,
                        pitch_centre_hz=profile.pitch_centre_hz,
                        render_backend=backend.name,
                        synthetic_signal=backend.synthetic_signal,
                        scenario_id=line.scenario_id,
                        ir_source=rendered.ir_source,
                        ir_licence=rendered.ir_licence,
                        noise_source=rendered.noise_source,
                        noise_licence=rendered.noise_licence,
                        telephony_hz=rendered.telephony_hz,
                        duration_s=rendered.audio.duration_s,
                    )
                )

    manifest_path = run_dir / "manifest.jsonl"
    write_jsonl(manifest_path, [row.as_dict() for row in rows])
    write_jsonl(run_dir / "rejected.jsonl", rejected)
    exported = export_personaplex(
        rows,
        run_dir,
        adapter=config.personaplex_adapter,
        eval_fraction=config.eval_fraction,
        absolute_paths=config.absolute_personaplex_paths,
    )
    record = {
        "schema": "emotion-v2.run.v1",
        "voice_id": profile.voice_id,
        "run_id": run_id,
        "kind": profile.kind,
        "backend": backend.name,
        "synthetic_signal": backend.synthetic_signal,
        "pitch_standard_hz": profile.pitch_standard_hz,
        "pitch_centre_hz": profile.pitch_centre_hz,
        "retune_hz": None,
        "output_dir": f"{profile.voice_id}/{run_id}",
        "consent_checked": consent_ok,
        "consent_signed_date": None if consent is None else consent.signed_date.isoformat(),
        "plugins": {
            "taxonomy": taxonomy_name,
            "capture": capture_name,
            "ci": scorer_source,
        },
        "unused_scorers": sorted(unused),
        "thresholds": {
            "max_wer": thresholds.max_wer,
            "min_ecapa": thresholds.min_ecapa,
            "min_emotion": thresholds.min_emotion,
            "require_scorers": thresholds.require_scorers,
        },
        "counts": {
            "rendered": len(lines) - len(rejected),
            "rejected": len(rejected),
            "manifest_rows": len(rows),
        },
        "licences": {
            "impulse_response": IR_LICENCE,
            "noise": NOISE_LICENCE,
            "note": (
                "Impulse responses and noise are synthesized in-process. "
                "Nothing proprietary was downloaded."
            ),
        },
        "reference_dirs": references,
        "alignment": "uniform-transcript-estimate",
        "personaplex": {
            "adapter": exported.adapter,
            "train_jsonl": exported.train_jsonl,
            "eval_jsonl": exported.eval_jsonl,
            "prompts_jsonl": exported.prompts_jsonl,
            "train_count": exported.train_count,
            "eval_count": exported.eval_count,
            "assumptions": list(exported.assumptions),
        },
    }
    (run_dir / "run.json").write_text(
        json.dumps(record, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return RunResult(
        run_dir=run_dir,
        manifest_path=manifest_path,
        rows=rows,
        rejected=rejected,
        record=record,
    )


def _job(
    profile: VoiceProfile,
    line: Scenario,
    references: dict[str, str],
    consent_ok: bool,
) -> RenderJob:
    return RenderJob(
        voice_id=profile.voice_id,
        kind=profile.kind,
        text=line.text,
        emotion=line.emotion,
        intensity=line.intensity,
        pitch_standard_hz=profile.pitch_standard_hz,
        pitch_centre_hz=profile.pitch_centre_hz,
        scenario_id=line.scenario_id,
        consent_ok=consent_ok,
        references=dict(references),
    )


def _audio_relative(line: Scenario, environment: str, snr: float | None) -> Path:
    if snr is None:
        tag = "none"
    elif float(snr).is_integer():
        tag = str(int(snr))
    else:
        tag = str(snr).replace("-", "m").replace(".", "p")
    name = f"{slug(line.scenario_id)}__{slug(environment)}__snr-{tag}.wav"
    return Path("audio") / slug(line.emotion) / name


def _reject_blocked_voice(voice_id: str) -> None:
    folded = voice_id.casefold()
    for blocked in BLOCKED_VOICE_SUBSTRINGS:
        if blocked in folded:
            raise ConsentError("refusing a blocked voice id")
