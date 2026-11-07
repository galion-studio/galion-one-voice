# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Citations for the PersonaPlex export adapters.

NVIDIA's PersonaPlex README, paper, and project page describe the model and
its training mix. They do not publish a fine-tune file schema. PersonaPlex is
a Moshi fine-tune, and the Moshi fine-tune README does publish one. The
default adapter follows that schema. A second adapter follows an unofficial
community trainer and is not the default.
"""

from __future__ import annotations

PERSONAPLEX_SOURCES: tuple[dict[str, str], ...] = (
    {
        "id": "nvidia-readme",
        "url": "https://github.com/NVIDIA/personaplex/blob/main/README.md",
        "note": (
            "Inference, voice prompts, and text role prompts. No training-file schema."
        ),
    },
    {
        "id": "nvidia-project",
        "url": "https://research.nvidia.com/labs/adlr/personaplex/",
        "note": (
            "Training mix: Fisher English conversations plus synthetic dialogues "
            "rendered with Chatterbox TTS. No JSONL or wav layout."
        ),
    },
    {
        "id": "nvidia-paper",
        "url": "https://arxiv.org/abs/2602.06053",
        "note": "PersonaPlex paper. Architecture and training mix, not a dump format.",
    },
    {
        "id": "nvidia-hf",
        "url": "https://huggingface.co/nvidia/personaplex-7b-v1",
        "note": "Weights card. Names the Fisher corpus and does not define a fine-tune dump.",
    },
    {
        "id": "nvidia-issue-5",
        "url": "https://github.com/NVIDIA/personaplex/issues/5",
        "note": (
            "NVIDIA did not provide sample fine-tune code. A comment points at "
            "kyutai-labs/moshi-finetune."
        ),
    },
    {
        "id": "moshi-finetune-readme",
        "url": "https://github.com/kyutai-labs/moshi-finetune/blob/main/README.md",
        "note": (
            "Published format: JSONL rows {path, duration} and stereo wavs, "
            "left = model, right = user."
        ),
    },
    {
        "id": "moshi-annotate",
        "url": "https://github.com/kyutai-labs/moshi-finetune/blob/main/annotate.py",
        "note": (
            "Sibling JSON is {alignments: [[text, [start, end], SPEAKER_MAIN], ...]} "
            "for the left channel."
        ),
    },
    {
        "id": "community-personaplex-finetune",
        "url": "https://github.com/emotion-machine-org/personaplex-finetune/blob/main/AGENTS.md",
        "note": (
            "Unofficial trainer. Same path/duration JSONL, but speaker labels "
            "SPEAKER_BROKER and SPEAKER_CLIENT, plus a text_prompt field. "
            "Optional adapter only."
        ),
    },
)

# Sample rate Moshi / PersonaPlex audio is prepared at. The loader can resample,
# and this exporter writes 24 kHz so a trainer can read the files directly.
PERSONAPLEX_SAMPLE_RATE = 24_000

MOSHI_SPEAKER = "SPEAKER_MAIN"
COMMUNITY_AGENT_SPEAKER = "SPEAKER_BROKER"
COMMUNITY_USER_SPEAKER = "SPEAKER_CLIENT"
