# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Command line for the Emotion V2 dataset pipeline.

From the repository root::

    python -m emotion_v2_dataset run --run-id demo

``PYTHONPATH`` must include ``tools/emotion-v2/dataset``. The default backend
is the synthetic stub.

High-quality-only is the default: every clip needs WER, ECAPA, and emotion
scores (``--scorer-module`` or the ``emotion_v2_ci`` plugin), the rollout gate
(WER <= 0.018, ECAPA > 0.7057, emotion >= 0.83 per class), and the capture
spec (48 kHz / 24-bit, -20 LUFS, no clipping). Without scorers every clip is
rejected. ``--allow-unscored-synthetic-test`` is for synthetic stub tests only. ``--backend chatterbox`` still needs a consent file
and, in-process, an engine this CLI does not provide.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import replace
from datetime import date
from pathlib import Path

from emotion_v2_dataset.errors import (
    ConsentError,
    DatasetError,
    PitchPolicyError,
    QualityError,
    RealVoiceRefused,
)
from emotion_v2_dataset.paths import DATA_DIR, default_output_root
from emotion_v2_dataset.pipeline import RunConfig, run_pipeline
from emotion_v2_dataset.quality import (
    HIGH_QUALITY_THRESHOLDS,
    SYNTHETIC_TEST_THRESHOLDS,
    QualityThresholds,
    thresholds_from_toml,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="emotion_v2_dataset")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="render, filter, augment, and write manifests")
    run.add_argument("--profile", type=Path, default=DATA_DIR / "stub_profile.json")
    run.add_argument("--scenarios", type=Path, default=DATA_DIR / "scenarios.json")
    run.add_argument("--output-root", type=Path, default=None)
    run.add_argument("--run-id", required=True)
    run.add_argument("--backend", choices=("stub", "chatterbox"), default="stub")
    run.add_argument("--consent", type=Path, default=None)
    run.add_argument(
        "--emotions",
        default=None,
        help="Emotion names separated by |. Default: all twelve.",
    )
    run.add_argument(
        "--environments",
        default=None,
        help="Environment names or ids separated by |.",
    )
    run.add_argument("--max-scenarios-per-emotion", type=int, default=None)
    run.add_argument(
        "--snr",
        default=None,
        help="SNR levels in dB, separated by |, for non-dry environments.",
    )
    run.add_argument("--eval-fraction", type=float, default=0.0)
    run.add_argument("--personaplex-adapter", choices=("moshi", "community"), default="moshi")
    run.add_argument("--absolute-paths", action="store_true")
    run.add_argument("--max-wer", type=float, default=None)
    run.add_argument("--min-ecapa", type=float, default=None)
    run.add_argument("--min-emotion", type=float, default=None,
                     help="Threshold for every emotion class. High-quality mode accepts only >= 0.83.")
    run.add_argument("--thresholds", type=Path, default=None,
                     help="thresholds.toml with [guards] and per-class [emotion.targets].")
    run.add_argument("--scorer-module", action="append", default=None,
                     help="Python module with build_scorers(config) -> list. Repeatable.")
    run.add_argument(
        "--require-scorers",
        action="store_true",
        help="Kept for old scripts. Scorers are always required unless "
        "--allow-unscored-synthetic-test is given.",
    )
    run.add_argument(
        "--allow-unscored-synthetic-test",
        action="store_true",
        help="SYNTHETIC STUB TESTS ONLY. Keep clips with missing scorers and skip "
        "the capture spec. Refused for a real profile or the chatterbox backend.",
    )
    run.add_argument(
        "--scorer-device",
        choices=("cpu", "auto", "cuda"),
        default="cpu",
        help="cpu (production default, all threads); auto = CUDA if available else all CPU threads.",
    )
    run.add_argument("--scorer-threads", type=int, default=None)
    run.add_argument("--today", default=None, help="Override today's date (YYYY-MM-DD).")
    args = parser.parse_args(argv)
    if args.command != "run":
        parser.error("unknown command")
        return 2
    if args.allow_unscored_synthetic_test and args.require_scorers:
        parser.error("--require-scorers and --allow-unscored-synthetic-test conflict")
    base = SYNTHETIC_TEST_THRESHOLDS if args.allow_unscored_synthetic_test else HIGH_QUALITY_THRESHOLDS
    if args.thresholds is not None:
        loaded = thresholds_from_toml(args.thresholds)
        base = replace(loaded, require_scorers=base.require_scorers)
    if args.max_wer is not None:
        base = replace(base, max_wer=args.max_wer)
    if args.min_ecapa is not None:
        base = replace(base, min_ecapa=args.min_ecapa)
    if args.min_emotion is not None:
        base = replace(
            base,
            min_emotion=args.min_emotion,
            min_emotion_by_class={name: args.min_emotion for name in base.min_emotion_by_class},
        )
    thresholds: QualityThresholds = base
    today = date.fromisoformat(args.today) if args.today else None
    config = RunConfig(
        profile_path=args.profile,
        scenarios_path=args.scenarios,
        output_root=args.output_root or default_output_root(),
        run_id=args.run_id,
        backend=args.backend,
        consent_path=args.consent,
        emotions=_split(args.emotions),
        environments=_split(args.environments),
        max_scenarios_per_emotion=args.max_scenarios_per_emotion,
        snr_levels=_snrs(args.snr),
        thresholds=thresholds,
        eval_fraction=args.eval_fraction,
        personaplex_adapter=args.personaplex_adapter,
        absolute_personaplex_paths=args.absolute_paths,
        today=today,
        scorer_modules=tuple(args.scorer_module) if args.scorer_module else None,
        allow_unscored_synthetic_test=args.allow_unscored_synthetic_test,
        scorer_device=args.scorer_device,
        scorer_threads=args.scorer_threads,
    )
    try:
        result = run_pipeline(config)
    except (ConsentError, PitchPolicyError, RealVoiceRefused, QualityError) as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 2
    except DatasetError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    counts = result.record["counts"]
    print(
        f"wrote {counts['manifest_rows']} rows to {result.manifest_path} "
        f"({counts['rejected']} rejected)"
    )
    return 0


def _split(value: str | None) -> tuple[str, ...] | None:
    if value is None:
        return None
    parts = tuple(part.strip() for part in value.split("|") if part.strip())
    return parts or None


def _snrs(value: str | None) -> tuple[float, ...] | None:
    if value is None:
        return None
    return tuple(float(part.strip()) for part in value.split("|") if part.strip())


if __name__ == "__main__":
    raise SystemExit(main())
