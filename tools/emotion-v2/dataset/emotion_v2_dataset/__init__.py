# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Generic Emotion V2 training-data pipeline.

Python, same side of the repo as the Chatterbox worker and the pipeline
tests. It does not link into the Rust symphony crates.
"""

from emotion_v2_dataset.pipeline import RunConfig, RunResult, run_pipeline

__all__ = ["RunConfig", "RunResult", "run_pipeline"]
__version__ = "0.1.0"
