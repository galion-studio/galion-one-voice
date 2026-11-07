# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Command line for the Emotion V2 harness.

CPU runs pass ``--backend stub``. SUPERB is selected only when weights are
already on disk. This program does not download weights and does not deploy.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .backends import BackendUnavailable, resolve_backend
from .config import (
    ConfigError,
    default_scenarios_path,
    default_thresholds_path,
    load_thresholds,
)
from .emotions import EMOTIONS, canonical_emotion
from .report import write_reports
from .score import score_all


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="harness",
        description="Score Samantha's twelve emotions against per-emotion references.",
    )
    sub = parser.add_subparsers(dest="command", required=True)
    score = sub.add_parser("score", help="write per-emotion JSON and a markdown table")
    score.add_argument("--render-dir", type=Path, required=True)
    score.add_argument("--reference-dir", type=Path, required=True)
    score.add_argument("--scenarios", type=Path, default=None)
    score.add_argument("--config", type=Path, default=None)
    score.add_argument("--out-dir", type=Path, required=True)
    score.add_argument(
        "--backend",
        default=None,
        help="auto (default), stub, or superb/wav2vec2-base-superb-er",
    )
    score.add_argument("--speaker-ref", type=Path, default=None)
    score.add_argument(
        "--require-guards",
        action="store_true",
        help="fail when galion-wer or galion-ecapa is absent",
    )
    score.add_argument(
        "--emotion",
        action="append",
        default=None,
        help="score only this emotion; repeat for several. Default: all twelve",
    )
    args = parser.parse_args(argv)
    if args.command != "score":
        parser.error(f"unknown command {args.command}")
    try:
        thresholds = load_thresholds(args.config)
        backend = resolve_backend(args.backend)
        emotions = EMOTIONS if not args.emotion else tuple(canonical_emotion(name) for name in args.emotion)
        summary = score_all(
            render_root=args.render_dir,
            reference_root=args.reference_dir,
            scenario_root=args.scenarios or default_scenarios_path(),
            thresholds=thresholds,
            backend=backend,
            emotions=emotions,
            speaker_ref=args.speaker_ref,
            require_guards=args.require_guards,
        )
    except (ConfigError, BackendUnavailable, KeyError) as exc:
        print(f"emotion-v2: {exc}", file=sys.stderr)
        return 2
    write_reports(args.out_dir, summary)
    print(render_line(summary), file=sys.stdout)
    return 0 if summary.passed else 1


def render_line(summary) -> str:
    passed = sum(1 for row in summary.scores if row.passed)
    return (
        f"{summary.backend}: {passed}/{len(summary.scores)} emotions passed "
        f"(tuning {summary.tuning_hz} Hz) -> "
        + ("PASS" if summary.passed else "FAIL")
    )


if __name__ == "__main__":
    raise SystemExit(main())
