# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""PersonaPlex exporters.

NVIDIA has not published a fine-tune dump. The default adapter writes the
stereo layout published by Kyutai's moshi-finetune, which PersonaPlex is
built on (see :mod:`emotion_v2_dataset.formats`).

These clips are single-speaker emotion lines, not duplex conversations.
The adapter puts the line on the left channel and silence on the right, and
it estimates word times by splitting the known transcript evenly. It does not
run Whisper. Re-annotate with ``annotate.py`` before a real training job if
you need model-derived times.

The community adapter is optional. It is not what ``annotate.py`` writes.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from emotion_v2_dataset.audio import (
    AudioBuffer,
    read_wav,
    resample_linear,
    write_stereo_wav,
)
from emotion_v2_dataset.formats import (
    COMMUNITY_AGENT_SPEAKER,
    MOSHI_SPEAKER,
    PERSONAPLEX_SAMPLE_RATE,
    PERSONAPLEX_SOURCES,
)
from emotion_v2_dataset.manifest import ManifestRow, write_jsonl

ADAPTERS = ("moshi", "community")


@dataclass(frozen=True)
class ExportResult:
    adapter: str
    train_jsonl: str
    eval_jsonl: str
    stereo_dir: str
    prompts_jsonl: str
    train_count: int
    eval_count: int
    assumptions: tuple[str, ...]


def export_personaplex(
    rows: list[ManifestRow],
    run_dir: Path,
    *,
    adapter: str = "moshi",
    eval_fraction: float = 0.0,
    absolute_paths: bool = False,
) -> ExportResult:
    if adapter not in ADAPTERS:
        raise ValueError("personaplex adapter must be 'moshi' or 'community'")
    if not 0.0 <= eval_fraction < 1.0:
        raise ValueError("eval_fraction must be in [0, 1)")
    out_dir = run_dir / "personaplex"
    stereo = out_dir / "stereo"
    stereo.mkdir(parents=True, exist_ok=True)
    speaker = MOSHI_SPEAKER if adapter == "moshi" else COMMUNITY_AGENT_SPEAKER
    train: list[dict[str, object]] = []
    evaluation: list[dict[str, object]] = []
    prompts: list[dict[str, object]] = []
    for row in rows:
        clip_id = Path(row.audio_path).stem
        wav_path = stereo / f"{clip_id}.wav"
        json_path = stereo / f"{clip_id}.json"
        source = run_dir / row.audio_path
        left, duration = _left_channel(source)
        right = AudioBuffer(np.zeros(len(left.samples), dtype=np.float64), left.sample_rate)
        write_stereo_wav(wav_path, left, right)
        sidecar = _sidecar(row, duration, speaker, adapter)
        json_path.write_text(json.dumps(sidecar, ensure_ascii=False), encoding="utf-8")
        manifest_path: str | Path
        if absolute_paths:
            manifest_path = wav_path.resolve()
        else:
            manifest_path = wav_path.relative_to(out_dir).as_posix()
        entry = {"path": str(manifest_path), "duration": duration}
        if _hold_out(clip_id, eval_fraction):
            evaluation.append(entry)
        else:
            train.append(entry)
        prompts.append(
            {
                "id": clip_id,
                "emotion": row.emotion,
                "intensity": row.intensity,
                "text": row.text,
                "voice_id": row.voice_id,
                "text_prompt": (
                    f"Speak the following line in a {row.emotion} manner "
                    f"at intensity {row.intensity:.2f}: {row.text}"
                ),
                "note": (
                    "Side metadata. The moshi trainer reads train.jsonl and "
                    "the sibling alignments JSON, not this file."
                ),
            }
        )
    train_path = out_dir / "train.jsonl"
    eval_path = out_dir / "eval.jsonl"
    prompts_path = out_dir / "prompts.jsonl"
    write_jsonl(train_path, train)
    write_jsonl(eval_path, evaluation)
    write_jsonl(prompts_path, prompts)
    return ExportResult(
        adapter=adapter,
        train_jsonl=str(train_path.relative_to(run_dir).as_posix()),
        eval_jsonl=str(eval_path.relative_to(run_dir).as_posix()),
        stereo_dir=str(stereo.relative_to(run_dir).as_posix()),
        prompts_jsonl=str(prompts_path.relative_to(run_dir).as_posix()),
        train_count=len(train),
        eval_count=len(evaluation),
        assumptions=_assumptions(adapter),
    )


def _left_channel(source: Path) -> tuple[AudioBuffer, float]:
    samples, rate, channels = read_wav(source)
    if channels == 2:
        mono = samples[:, 0]
    else:
        mono = samples
    audio = resample_linear(AudioBuffer(mono, rate), PERSONAPLEX_SAMPLE_RATE)
    duration = float(len(audio.samples)) / float(audio.sample_rate)
    return audio, duration


def _sidecar(row: ManifestRow, duration: float, speaker: str, adapter: str) -> dict[str, object]:
    alignments = uniform_alignments(row.text, duration, speaker)
    if adapter == "moshi":
        # annotate.py writes this object and nothing else.
        return {"alignments": alignments}
    return {
        "text_prompt": (
            f"Speak in a {row.emotion} manner at intensity {row.intensity:.2f}."
        ),
        "turns": [
            {
                "index": 0,
                "speaker": speaker,
                "start": 0.0,
                "end": duration,
                "text": row.text,
            }
        ],
        "alignments": alignments,
        "context_injections": [],
    }


def uniform_alignments(text: str, duration: float, speaker: str) -> list[list[object]]:
    """Even word spans across the clip. Estimates, not a forced aligner."""
    words = text.split()
    if not words:
        words = [text]
    span = duration / len(words) if duration > 0 else 0.0
    rows: list[list[object]] = []
    for index, word in enumerate(words):
        start = index * span
        end = duration if index == len(words) - 1 else (index + 1) * span
        rows.append([word, [start, end], speaker])
    return rows


def _hold_out(clip_id: str, fraction: float) -> bool:
    if fraction <= 0:
        return False
    # Stable, dependency-free split. Not cryptographic.
    bucket = sum(clip_id.encode("utf-8")) % 10_000
    return bucket < int(fraction * 10_000)


def _assumptions(adapter: str) -> tuple[str, ...]:
    return (
        "NVIDIA PersonaPlex has no published fine-tune file format.",
        "Default files follow kyutai-labs/moshi-finetune: JSONL {path, duration}, "
        "24 kHz stereo, left = rendered line, right = silence, sibling JSON "
        "alignments labelled SPEAKER_MAIN.",
        "Word times are a uniform split of the known transcript, not Whisper.",
        "Clips are single-speaker emotion lines, not full-duplex conversations.",
        "prompts.jsonl is side metadata and is not the trainer manifest.",
        f"adapter={adapter}",
        "sources=" + ",".join(item["id"] for item in PERSONAPLEX_SOURCES),
    )
