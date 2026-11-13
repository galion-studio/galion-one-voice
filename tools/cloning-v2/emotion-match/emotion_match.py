#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# emotion_match = 0.70*category_blend+0.30*emb_sim
#
# category_blend is the histogram intersection of the categorical posterior
# (softmax of the classifier log-probabilities) between the reference and a clip.
# emb_sim is the cosine of the L2-normalised utterance embeddings from the same
# encoder. Both are means over the clips. The weights above are applied in
# emotion_match(); this process does not invent a constant.
#
# Per-sector mode (--manifest) scores each render against that emotion's own
# reference, pinned by path + sha256. The 0.83 gate is unchanged and is applied
# per sector: one weak sector cannot hide inside the aggregate. The single-ref
# --ref path remains for the historical series only; it is not the V2 gate.
#
# Weights: superb/wav2vec2-base-superb-er (Apache-2.0), already in the local
# Hugging Face cache, or --backend stub (no weights). CPU only. The cache is
# outside this git checkout. This tool does not download weights.

from __future__ import annotations

import os
import sys

_DIR = os.path.dirname(os.path.abspath(__file__))
if _DIR not in sys.path:
    sys.path.insert(0, _DIR)

from scorelib import (  # noqa: E402
    CATEGORY_W,
    EMB_W,
    FORMULA,
    GATE,
    ClipScore,
    aggregate,
    emotion_match,
    meets_gate,
    pair_score,
    score_sector,
)
from sector_refs import (  # noqa: E402
    SectorRefError,
    collect_jobs,
    load_manifest,
    required_sectors,
    resolve_ref,
)

DEFAULT_MODEL = (
    "/home/galion/workspace/galion/cache/hf/hub/"
    "models--superb--wav2vec2-base-superb-er/snapshots/"
    "441a7599c3b22107314dcbd9166621c5c83f2cc5"
)


def usage(code: int) -> None:
    sys.stdout.write(
        "emotion_match — 0.70*category_blend+0.30*emb_sim\n"
        "usage: emotion_match --manifest REFS.json [--renders-dir DIR] [--root DIR]\n"
        "                     [--backend stub|superb] [--out out.json] [clip.wav...]\n"
        "       emotion_match --ref REF.wav clip.wav... [--threads N] [--out out.json]\n"
        "       emotion_match --self-test\n"
        "\n"
        "Per-sector mode (--manifest) scores each render against its own emotion\n"
        f"sector's reference. The gate stays {GATE}. Weights stay {FORMULA}.\n"
        "The manifest maps each emotion to a reference path and sha256. A missing\n"
        "or mismatched sector reference is a hard failure; the tool will not fall\n"
        "back to a single mixed clip.\n"
    )
    raise SystemExit(code)


def self_test() -> None:
    got = emotion_match(0.9, 0.8)
    expect = 0.70 * 0.9 + 0.30 * 0.8
    if (
        abs(got - expect) > 1e-12
        or FORMULA != "0.70*category_blend+0.30*emb_sim"
        or CATEGORY_W != 0.70
        or EMB_W != 0.30
        or GATE != 0.83
        or not meets_gate(0.83)
        or meets_gate(0.83 - 1e-4)
    ):
        sys.stderr.write(f"self-test failed got={got} expect={expect}\n")
        raise SystemExit(1)
    sys.stdout.write(f"ok {got}\n")
    raise SystemExit(0)


def parse(argv: list[str]):
    if "--help" in argv or "-h" in argv:
        usage(0)
    if "--self-test" in argv:
        self_test()
    ref = None
    out = None
    threads = 2
    files: list[str] = []
    manifest = None
    renders_dir = None
    root = None
    backend = None
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--ref":
            i += 1
            ref = argv[i] if i < len(argv) else None
        elif a == "--out":
            i += 1
            out = argv[i] if i < len(argv) else None
        elif a == "--threads":
            i += 1
            threads = int(argv[i]) if i < len(argv) else 2
        elif a == "--manifest":
            i += 1
            manifest = argv[i] if i < len(argv) else None
        elif a == "--renders-dir":
            i += 1
            renders_dir = argv[i] if i < len(argv) else None
        elif a == "--root":
            i += 1
            root = argv[i] if i < len(argv) else None
        elif a == "--backend":
            i += 1
            backend = argv[i] if i < len(argv) else None
        elif a == "--gate":
            i += 1
            raw = argv[i] if i < len(argv) else None
            try:
                value = float(raw)
            except (TypeError, ValueError):
                sys.stderr.write("emotion_match: --gate needs a number\n")
                usage(2)
            if value != GATE:
                sys.stderr.write(
                    f"emotion_match: gate is locked at {GATE}; refusing --gate {value}\n"
                )
                raise SystemExit(2)
        elif a.startswith("--"):
            sys.stderr.write(f"unknown flag {a}\n")
            usage(2)
        else:
            files.append(a)
        i += 1
    if manifest:
        if backend is None:
            backend = "stub" if os.environ.get("EMOTION_V2_BACKEND") == "stub" else "superb"
        return {
            "mode": "per-sector",
            "manifest": manifest,
            "renders_dir": renders_dir,
            "root": root,
            "files": files,
            "out": out,
            "threads": max(1, min(threads, 2)),
            "backend": backend,
        }
    if not ref or not files:
        usage(2)
    return {
        "mode": "single-ref",
        "ref": ref,
        "files": files,
        "out": out,
        "threads": max(1, min(threads, 2)),
        "backend": backend or "superb",
    }


def _reexec_for_superb() -> None:
    try:
        import torch  # noqa: F401
        return
    except ImportError:
        pass
    cand = os.environ.get("GALION_PYTHON", "/home/galion/workspace/galion/venv/bin/python")
    if os.path.isfile(cand) and os.path.realpath(cand) != os.path.realpath(sys.executable):
        os.execv(cand, [cand, *sys.argv])


def load16(path: str):
    import wave

    import numpy as np
    import torch
    import torchaudio

    with wave.open(path) as handle:
        sr = handle.getframerate()
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        raw = handle.readframes(handle.getnframes())
    if width == 2:
        samples = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
    elif width == 4:
        samples = np.frombuffer(raw, dtype="<i4").astype(np.float32) / 2147483648.0
    elif width == 1:
        samples = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    else:
        raise SystemExit(f"{path}: unsupported sample width {width}")
    if channels > 1:
        samples = samples.reshape(-1, channels).mean(axis=1)
    wav = torch.from_numpy(np.ascontiguousarray(samples))
    if sr != 16000:
        wav = torchaudio.functional.resample(wav, sr, 16000)
    return wav.contiguous()


def _torch_as_prob(row):
    import torch

    row = row.detach().float().cpu().reshape(-1)
    total = float(row.sum())
    if torch.any(row < -1e-5) or abs(total - 1.0) > 1e-2:
        row = torch.softmax(row, dim=0)
    else:
        row = row / row.sum().clamp_min(1e-12)
    return row


def _torch_as_unit(emb):
    e = emb.detach().float().cpu().reshape(-1)
    return e / e.norm().clamp_min(1e-12)


def load_classifier(threads: int):
    import torch

    torch.set_num_threads(threads)
    try:
        torch.set_num_interop_threads(1)
    except RuntimeError:
        pass
    os.environ.setdefault("CUDA_VISIBLE_DEVICES", "")
    snap = os.environ.get("GALION_EMOTION_MODEL", DEFAULT_MODEL)
    if not os.path.isdir(snap):
        raise SystemExit(f"emotion model snapshot missing: {snap}")
    from transformers import Wav2Vec2FeatureExtractor, Wav2Vec2ForSequenceClassification

    extractor = Wav2Vec2FeatureExtractor.from_pretrained(snap, local_files_only=True)
    model = Wav2Vec2ForSequenceClassification.from_pretrained(snap, local_files_only=True)
    model.eval()
    captured = {}

    def _hook(_module, inputs, _output):
        captured["emb"] = inputs[0].detach()

    model.classifier.register_forward_hook(_hook)
    return extractor, model, captured


def embed_and_prob(bundle, wav):
    import torch

    extractor, model, captured = bundle
    features = extractor(wav.numpy(), sampling_rate=16000, return_tensors="pt")
    with torch.no_grad():
        logits = model(**features).logits
    if "emb" not in captured:
        raise SystemExit("emotion classifier did not expose an embedding")
    return _torch_as_prob(logits), _torch_as_unit(captured["emb"])


def _superb_analyse(bundle, path) -> tuple[list[float], list[float]]:
    prob, emb = embed_and_prob(bundle, load16(str(path)))
    return prob.tolist(), emb.tolist()


def _resolve_backend(name: str, threads: int):
    key = (name or "superb").strip().lower()
    if key in {"stub", "cpu-stub", "sine"}:
        from stub import StubBackend

        backend = StubBackend()
        return backend.name, backend.analyse, "stub (sine fixtures; not a recogniser)", None
    if key in {"superb", "superb/wav2vec2-base-superb-er", "auto"}:
        _reexec_for_superb()
        try:
            bundle = load_classifier(threads)
        except SystemExit:
            raise
        except Exception as exc:  # pragma: no cover - environment-specific
            raise SystemExit(f"emotion_match: superb backend unavailable: {exc}") from exc
        return (
            "superb/wav2vec2-base-superb-er",
            lambda path, _bundle=bundle: _superb_analyse(_bundle, path),
            "Apache-2.0",
            bundle,
        )
    raise SystemExit(
        f"emotion_match: unknown backend {name!r}; expected stub or superb"
    )


def run_per_sector(args: dict) -> int:
    from pathlib import Path

    try:
        manifest = load_manifest(
            Path(args["manifest"]),
            root=Path(args["root"]) if args["root"] else None,
        )
        jobs = collect_jobs(
            manifest,
            renders_dir=Path(args["renders_dir"]) if args["renders_dir"] else None,
            clips=[Path(item) for item in args["files"]],
        )
        backend_name, analyse, licence, _bundle = _resolve_backend(
            args["backend"], args["threads"]
        )
        sectors = []
        for emotion in required_sectors(jobs):
            ref = resolve_ref(manifest, emotion)
            ref_prob, ref_emb = analyse(ref.path)
            clips: list[ClipScore] = []
            for job in jobs:
                if job.emotion != emotion:
                    continue
                render_prob, render_emb = analyse(job.path)
                category, sim, score = pair_score(render_prob, render_emb, ref_prob, ref_emb)
                clips.append(
                    ClipScore(
                        path=str(job.path),
                        emotion=emotion,
                        line_id=job.id,
                        category_blend=category,
                        emb_sim=sim,
                        emotion_match=score,
                    )
                )
            sectors.append(
                score_sector(emotion, clips, str(ref.path), ref.sha256)
            )
    except (SectorRefError, FileNotFoundError) as exc:
        sys.stderr.write(f"emotion_match: {exc}\n")
        return 2
    report = aggregate(sectors, persona=manifest.persona, backend=backend_name)
    body = report.as_dict()
    body["licence"] = licence
    body["threads"] = args["threads"]
    body["manifest"] = str(manifest.source)
    body["model"] = backend_name
    text = _write_json(body, args["out"])
    sys.stdout.write(text + "\n")
    return 0 if report.passed else 1


def run_single_ref(args: dict) -> int:
    """Historical single-reference path. Not the Emotion V2 gate."""
    import json

    _reexec_for_superb()
    import torch  # noqa: F401  # after reexec

    def intersection_t(p, q):
        n = min(p.numel(), q.numel())
        return float(torch.minimum(p[:n], q[:n]).sum())

    def cosine_t(a, b):
        n = min(a.numel(), b.numel())
        return float(torch.dot(a[:n], b[:n]))

    ref, files, out, threads = args["ref"], args["files"], args["out"], args["threads"]
    clf = load_classifier(threads)
    pref, eref = embed_and_prob(clf, load16(ref))
    rows = []
    blends = []
    sims = []
    matches = []
    for path in files:
        p, e = embed_and_prob(clf, load16(path))
        cb = intersection_t(pref, p)
        es = cosine_t(eref, e)
        em = emotion_match(cb, es)
        blends.append(cb)
        sims.append(es)
        matches.append(em)
        rows.append(
            {
                "path": path,
                "category_blend": cb,
                "emb_sim": es,
                "emotion_match": em,
            }
        )
    n = len(matches)
    category_blend = sum(blends) / n
    emb_sim = sum(sims) / n
    # Same weights as the per-clip line. Mean of the mix equals the mix of the means.
    score = emotion_match(category_blend, emb_sim)
    body = {
        "tool": "emotion_match",
        "formula": FORMULA,
        "model": "superb/wav2vec2-base-superb-er",
        "licence": "Apache-2.0",
        "device": "cpu",
        "threads": threads,
        "mode": "single-ref",
        "note": (
            "single-ref mode compares every render to one mixed reference; "
            "it is the historical series, not the Emotion V2 gate. "
            "Use --manifest for per-sector references."
        ),
        "ref": ref,
        "n": n,
        "category_blend": category_blend,
        "emb_sim": emb_sim,
        "emotion_match": score,
        "files": rows,
        "summary": {
            "category_blend": category_blend,
            "emb_sim": emb_sim,
            "emotion_match": score,
        },
    }
    text = json.dumps(body, indent=2)
    sys.stdout.write(text + "\n")
    if out:
        parent = os.path.dirname(out)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(out, "w", encoding="utf-8") as fh:
            fh.write(text + "\n")
    return 0


def _write_json(body: dict, out: str | None) -> str:
    import json

    text = json.dumps(body, indent=2)
    if out:
        parent = os.path.dirname(out)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(out, "w", encoding="utf-8") as fh:
            fh.write(text + "\n")
    return text


def main(argv: list[str] | None = None) -> int:
    args = parse(sys.argv[1:] if argv is None else argv)
    if args["mode"] == "per-sector":
        return run_per_sector(args)
    return run_single_ref(args)


if __name__ == "__main__":
    raise SystemExit(main())
