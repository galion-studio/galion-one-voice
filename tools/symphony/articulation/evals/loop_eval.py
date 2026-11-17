#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""Articulation loop eval.

The loop is one decoder. galion-articulation writes a profile from a wav, and
the same binary reads the next wav. This script measures that loop on public
speech: a second reading of the same file, a mild gain and a mild noise
reading (the decoder noise floor), and how far fields move between real
takes. It also scores the acoustic speaker part and the emotion part.

The emotion set is neutral plus the 12 Cowen et al. 2019 prosody emotions.
Laughter, sigh and breath are vocal-burst detectors, not emotions. CI runs
the synthetic smoke test only. Dataset commands are not the CI path.

It does not render a clone, it does not download weights, and it does not
set a gate. Audio and per-clip profiles stay outside the repo.

TractPrint v2 is used when a profile carries tractprint_v2.speaker and
tractprint_v2.emotion. This branch's decoder does not write those keys, so
the run falls back to the acoustic quantiles named in config.toml.
"""

from __future__ import annotations

import argparse
import array
import csv
import json
import math
import os
import random
import shutil
import subprocess
import sys
import tomllib
import urllib.request
import wave
import zipfile
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
EVAL_DIR = Path(__file__).resolve().parent
MANIFEST = REPO / "tools" / "symphony" / "articulation" / "Cargo.toml"
Q25, Q50, Q75 = 2, 4, 6

CREMA_EMOTION = {
    "NEU": "neutral",
    "HAP": "happy",
    "SAD": "sad",
    "ANG": "angry",
    "FEA": "fearful",
    "DIS": "disgusted",
}
RAVDESS_EMOTION = {
    "01": "neutral",
    "02": "calm",
    "03": "happy",
    "04": "sad",
    "05": "angry",
    "06": "fearful",
    "07": "disgusted",
    "08": "surprised",
}


def load_config() -> dict:
    with (EVAL_DIR / "config.toml").open("rb") as handle:
        return tomllib.load(handle)


def canonical_label(native: str, cfg: dict) -> tuple[str, str]:
    """Map a dataset name to (emotion, burst). A burst is never an emotion."""
    bursts = set(cfg["bursts"])
    burst_aliases = cfg.get("burst_aliases") or {}
    if native in bursts:
        return "", native
    if native in burst_aliases:
        return "", str(burst_aliases[native])
    emotions = set(cfg["emotions"])
    if native in emotions:
        return native, ""
    mapped = (cfg.get("aliases") or {}).get(native)
    if mapped in emotions:
        return str(mapped), ""
    return "", ""


def percentile(values: list[float], p: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    x = p * (len(ordered) - 1)
    i = int(math.floor(x))
    frac = x - i
    if i + 1 >= len(ordered):
        return ordered[-1]
    return ordered[i] * (1.0 - frac) + ordered[i + 1] * frac


def dist_stats(values: list[float]) -> dict:
    if not values:
        return {"n": 0, "p50": None, "p90": None, "p95": None, "max": None}
    return {
        "n": len(values),
        "p50": percentile(values, 0.50),
        "p90": percentile(values, 0.90),
        "p95": percentile(values, 0.95),
        "max": max(values),
    }


def auc_score(positive: list[float], negative: list[float]) -> float | None:
    """P(positive > negative) + 0.5 P(equal). Higher scores are the positive class."""
    if not positive or not negative:
        return None
    n_pos = len(positive)
    n_neg = len(negative)
    order = sorted((score, 1) for score in positive)
    order.extend((score, 0) for score in negative)
    order.sort(key=lambda item: item[0])
    rank_sum = 0.0
    i = 0
    n = len(order)
    while i < n:
        j = i
        while j + 1 < n and order[j + 1][0] == order[i][0]:
            j += 1
        average = 0.5 * ((i + 1) + (j + 1))
        for k in range(i, j + 1):
            if order[k][1] == 1:
                rank_sum += average
        i = j + 1
    return (rank_sum - n_pos * (n_pos + 1) / 2.0) / (n_pos * n_neg)


def eer_score(positive: list[float], negative: list[float]) -> float | None:
    """Equal-error rate for a similarity score. Higher means more genuine."""
    if not positive or not negative:
        return None
    pos = sorted(positive)
    neg = sorted(negative)
    n_pos = len(pos)
    n_neg = len(neg)
    best_gap = 1.0
    best = 1.0

    def consider(far: float, frr: float) -> None:
        nonlocal best_gap, best
        gap = abs(far - frr)
        if gap < best_gap:
            best_gap = gap
            best = 0.5 * (far + frr)

    # Sweep the sorted impostor scores. FAR and FRR are step functions.
    # Checking each distinct score, and the point just after it, is enough.
    thresholds = sorted(set(pos) | set(neg))
    for threshold in thresholds:
        frr = _below(pos, threshold) / n_pos
        far = (n_neg - _below(neg, threshold)) / n_neg
        consider(far, frr)
        frr_after = _at_or_below(pos, threshold) / n_pos
        far_after = (n_neg - _at_or_below(neg, threshold)) / n_neg
        consider(far_after, frr_after)
    return best


def _below(ordered: list[float], threshold: float) -> int:
    lo, hi = 0, len(ordered)
    while lo < hi:
        mid = (lo + hi) // 2
        if ordered[mid] < threshold:
            lo = mid + 1
        else:
            hi = mid
    return lo


def _at_or_below(ordered: list[float], threshold: float) -> int:
    lo, hi = 0, len(ordered)
    while lo < hi:
        mid = (lo + hi) // 2
        if ordered[mid] <= threshold:
            lo = mid + 1
        else:
            hi = mid
    return lo


def mean_vec(vectors: list[list[float]]) -> list[float]:
    dim = len(vectors[0])
    n = len(vectors)
    return [sum(vector[j] for vector in vectors) / n for j in range(dim)]


def sub(a: list[float], b: list[float]) -> list[float]:
    return [x - y for x, y in zip(a, b)]


def standardize(vectors: list[list[float]]) -> list[list[float]]:
    dim = len(vectors[0])
    n = len(vectors)
    center = mean_vec(vectors)
    variance = [0.0] * dim
    for vector in vectors:
        for j in range(dim):
            delta = vector[j] - center[j]
            variance[j] += delta * delta
    scale = []
    for j in range(dim):
        std = math.sqrt(variance[j] / n)
        scale.append(std if std > 1e-8 else 1.0)
    return [[(vector[j] - center[j]) / scale[j] for j in range(dim)] for vector in vectors]


def cosine(a: list[float], b: list[float]) -> float:
    dot = norm_a = norm_b = 0.0
    for x, y in zip(a, b):
        dot += x * y
        norm_a += x * x
        norm_b += y * y
    if norm_a <= 0.0 or norm_b <= 0.0:
        return 0.0
    return dot / math.sqrt(norm_a * norm_b)


def dist2(a: list[float], b: list[float]) -> float:
    return sum((x - y) * (x - y) for x, y in zip(a, b))


def macro_f1(confusion: list[list[int]]) -> float | None:
    n = len(confusion)
    if n == 0:
        return None
    scores = []
    for i in range(n):
        tp = confusion[i][i]
        fp = sum(confusion[r][i] for r in range(n)) - tp
        fn = sum(confusion[i][c] for c in range(n)) - tp
        if tp == 0 and fp == 0 and fn == 0:
            continue
        precision = tp / (tp + fp) if tp + fp else 0.0
        recall = tp / (tp + fn) if tp + fn else 0.0
        scores.append(0.0 if precision + recall == 0.0 else 2.0 * precision * recall / (precision + recall))
    if not scores:
        return None
    return sum(scores) / len(scores)


def per_class_f1(confusion: list[list[int]], labels: list[str]) -> dict[str, float]:
    out = {}
    n = len(labels)
    for i, label in enumerate(labels):
        tp = confusion[i][i]
        fp = sum(confusion[r][i] for r in range(n)) - tp
        fn = sum(confusion[i][c] for c in range(n)) - tp
        precision = tp / (tp + fp) if tp + fp else 0.0
        recall = tp / (tp + fn) if tp + fn else 0.0
        out[label] = 0.0 if precision + recall == 0.0 else 2.0 * precision * recall / (precision + recall)
    return out


def self_check_math() -> None:
    perfect = auc_score([0.9, 0.8, 0.7], [0.2, 0.1, 0.0])
    if perfect != 1.0:
        raise SystemExit(f"auc of a separated pair is {perfect}")
    tied = auc_score([0.5, 0.5], [0.5, 0.5])
    if tied is None or abs(tied - 0.5) > 1e-12:
        raise SystemExit(f"auc of a tie is {tied}")
    zero = eer_score([0.9, 0.8], [0.1, 0.2])
    if zero is None or zero > 1e-12:
        raise SystemExit(f"eer of a separated pair is {zero}")
    confusion = [[4, 0], [0, 4]]
    score = macro_f1(confusion)
    if score != 1.0:
        raise SystemExit(f"macro-F1 of a diagonal is {score}")


def write_wav(path: Path, samples: array.array, sample_rate: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(samples.tobytes())


def read_wav(path: Path) -> tuple[int, array.array]:
    with wave.open(str(path), "rb") as handle:
        if handle.getnchannels() != 1 or handle.getsampwidth() != 2:
            raise RuntimeError(f"{path} is not mono 16-bit PCM")
        rate = handle.getframerate()
        raw = handle.readframes(handle.getnframes())
    samples = array.array("h")
    samples.frombytes(raw)
    if sys.byteorder != "little":
        samples.byteswap()
    return rate, samples


def synth_vowel(path: Path, f0: float, f1: float, f2: float, secs: float = 1.6, sample_rate: int = 16000) -> None:
    """Pulse train through two resonators. Same idea as the crate's synthetic test."""
    n = int(secs * sample_rate)
    source = [0.0] * n
    period = max(int(sample_rate / f0), 1)
    for i in range(0, n, period):
        source[i] = 1.0
    y = source
    for freq, bandwidth in ((f1, 80.0), (f2, 120.0)):
        radius = math.exp(-math.pi * bandwidth / sample_rate)
        coeff = 2.0 * radius * math.cos(2.0 * math.pi * freq / sample_rate)
        y1 = y2 = 0.0
        radius2 = radius * radius
        nxt = [0.0] * n
        for i, value in enumerate(y):
            out = value + coeff * y1 - radius2 * y2
            y2 = y1
            y1 = out
            nxt[i] = out
        y = nxt
    peak = max(abs(value) for value in y) or 1.0
    pcm = array.array("h", (int(max(-32767, min(32767, round(0.5 * value / peak * 32767)))) for value in y))
    write_wav(path, pcm, sample_rate)


def scale_wav(src: Path, dest: Path, gain: float, snr_db: float | None, rng: random.Random) -> None:
    rate, samples = read_wav(src)
    if snr_db is None:
        noise_rms = 0.0
    else:
        energy = sum(float(sample) * float(sample) for sample in samples) / len(samples)
        signal_rms = math.sqrt(energy) or 1.0
        noise_rms = signal_rms / (10.0 ** (snr_db / 20.0))
    out = array.array("h")
    for sample in samples:
        value = float(sample) * gain
        if noise_rms:
            value += rng.gauss(0.0, noise_rms)
        out.append(int(max(-32768, min(32767, round(value)))))
    write_wav(dest, out, rate)


def outside_repo(path: Path) -> None:
    resolved = path.resolve()
    if resolved == REPO or REPO in resolved.parents:
        raise SystemExit(f"refusing to write audio inside the repo: {resolved}")


def decode_job(job: tuple[str, str, str]) -> tuple[str, str]:
    binary, wav, dest = job
    path = Path(dest)
    if path.is_file() and path.stat().st_size > 0:
        return dest, ""
    tmp = path.with_suffix(".tmp.json")
    proc = subprocess.run(
        [binary, "analyze", "--wav", wav, "--out", str(tmp)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if proc.returncode != 0:
        tmp.unlink(missing_ok=True)
        tail = (proc.stderr or proc.stdout or "decode failed").strip().splitlines()
        return dest, tail[-1] if tail else "decode failed"
    tmp.replace(path)
    return dest, ""


def ensure_binary(explicit: str | None) -> Path:
    if explicit:
        path = Path(explicit)
        if not path.is_file():
            raise SystemExit(f"decoder not found: {path}")
        return path
    release = MANIFEST.parent / "target" / "release" / "galion-articulation"
    debug = MANIFEST.parent / "target" / "debug" / "galion-articulation"
    if release.is_file():
        return release
    if debug.is_file():
        return debug
    subprocess.run(["cargo", "build", "--manifest-path", str(MANIFEST)], check=True)
    if not debug.is_file():
        raise SystemExit("cargo build did not produce galion-articulation")
    return debug


def load_profile(path: Path) -> tuple[dict[str, list[float]], dict[str, str], dict | None] | None:
    try:
        doc = json.loads(path.read_text())
        summary = doc["articulation"]["summary"]
    except (OSError, json.JSONDecodeError, KeyError, TypeError):
        return None
    quantiles: dict[str, list[float]] = {}
    sections: dict[str, str] = {}
    for name, item in summary.items():
        q = item.get("q") if isinstance(item, dict) else None
        if not isinstance(q, list) or len(q) != 9:
            continue
        if not all(isinstance(value, (int, float)) and math.isfinite(value) for value in q):
            continue
        quantiles[name] = [float(value) for value in q]
        sections[name] = str(item.get("section", ""))
    art = doc.get("articulation") if isinstance(doc.get("articulation"), dict) else {}
    tp = doc.get("tractprint_v2") if isinstance(doc.get("tractprint_v2"), dict) else art.get("tractprint_v2")
    tp = tp if isinstance(tp, dict) else None
    return quantiles, sections, tp


def decode_many(binary: Path, jobs: list[tuple[str, str, str]], workers: int) -> list[str]:
    failures: list[str] = []
    if not jobs:
        return failures
    with ProcessPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(decode_job, job) for job in jobs]
        done = 0
        for future in as_completed(futures):
            dest, err = future.result()
            done += 1
            if err:
                failures.append(f"{Path(dest).stem}: {err}")
            if done % 400 == 0 or done == len(jobs):
                print(f"decode {done}/{len(jobs)}", file=sys.stderr, flush=True)
    return failures


class Clip:
    def __init__(self, clip_id: str, speaker: str, sex: str, emotion: str, wav: Path, native: str | None = None, burst: str = "") -> None:
        self.clip_id = clip_id
        self.speaker = speaker
        self.sex = sex
        # Canonical prosody emotion, or "" when the dataset name is not in the 12 + neutral.
        self.emotion = emotion
        self.native = emotion if native is None else native
        self.burst = burst
        self.wav = wav
        self.summary: dict[str, list[float]] | None = None
        self.tp: dict | None = None


def _download(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    urllib.request.urlretrieve(url, tmp)
    tmp.replace(dest)


def ensure_ravdess(root: Path, cfg: dict) -> None:
    wavs = list(root.glob("ravdess/**/*.wav"))
    if len(wavs) >= 1400:
        return
    outside_repo(root)
    spec = cfg["datasets"]["ravdess"]
    zpath = root / "ravdess" / "Audio_Speech_Actors_01-24.zip"
    if not zpath.is_file() or zpath.stat().st_size < spec["zip_bytes"] - 1000:
        print("fetch RAVDESS speech zip", file=sys.stderr, flush=True)
        _download(spec["zip_url"], zpath)
    print("unzip RAVDESS", file=sys.stderr, flush=True)
    with zipfile.ZipFile(zpath) as archive:
        archive.extractall(root / "ravdess")


def ensure_crema(root: Path, cfg: dict) -> None:
    outside_repo(root)
    audio = root / "crema-d" / "AudioWAV"
    audio.mkdir(parents=True, exist_ok=True)
    spec = cfg["datasets"]["crema-d"]
    summary = root / "crema-d" / "summaryTable.csv"
    demo = root / "crema-d" / "VideoDemographics.csv"
    if not summary.is_file():
        alt = root / "summaryTable.csv"
        if alt.is_file():
            shutil.copy(alt, summary)
        else:
            _download(spec["summary_url"], summary)
    if not demo.is_file():
        alt = root / "VideoDemographics.csv"
        if alt.is_file():
            shutil.copy(alt, demo)
        else:
            _download(spec["demographics_url"], demo)
    names = []
    with summary.open(newline="") as handle:
        for row in csv.DictReader(handle):
            name = (row.get("FileName") or "").strip().strip('"')
            if name and name != "FileName":
                names.append(name)
    missing = [name for name in names if not (audio / f"{name}.wav").is_file()]
    if not missing:
        return
    print(f"fetch CREMA-D wavs: {len(missing)}", file=sys.stderr, flush=True)

    def one(name: str) -> str | None:
        dest = audio / f"{name}.wav"
        try:
            _download(spec["audio_url"].format(name=name), dest)
        except Exception as exc:  # noqa: BLE001 — one missing public file is recorded, not fatal
            dest.unlink(missing_ok=True)
            return f"{name}: {exc}"
        return None

    # Thread pool would fight the process pool later; a modest thread fetch is only the downloader.
    from concurrent.futures import ThreadPoolExecutor

    errors = []
    with ThreadPoolExecutor(max_workers=16) as pool:
        for result in pool.map(one, missing):
            if result:
                errors.append(result)
    if errors:
        print(f"CREMA-D missing {len(errors)} (first: {errors[0]})", file=sys.stderr, flush=True)


def _tagged(clip_id: str, speaker: str, sex: str, native: str, wav: Path, cfg: dict) -> Clip:
    emotion, burst = canonical_label(native, cfg)
    return Clip(clip_id, speaker, sex, emotion, wav, native=native, burst=burst)


def index_crema(root: Path, cfg: dict) -> list[Clip]:
    demo: dict[str, str] = {}
    with (root / "crema-d" / "VideoDemographics.csv").open(newline="") as handle:
        for row in csv.DictReader(handle):
            sex = (row.get("Sex") or "").strip().lower()
            demo[str(row["ActorID"]).strip()] = "male" if sex.startswith("m") else "female"
    clips = []
    for path in sorted((root / "crema-d" / "AudioWAV").glob("*.wav")):
        parts = path.stem.split("_")
        if len(parts) < 4 or parts[0] not in demo or parts[2] not in CREMA_EMOTION:
            continue
        clips.append(_tagged(path.stem, parts[0], demo[parts[0]], CREMA_EMOTION[parts[2]], path, cfg))
    return clips


def index_ravdess(root: Path, cfg: dict) -> list[Clip]:
    clips = []
    for path in sorted(root.glob("ravdess/**/*.wav")):
        parts = path.stem.split("-")
        if len(parts) != 7 or parts[0] != "03" or parts[2] not in RAVDESS_EMOTION:
            continue
        actor = parts[6]
        sex = "male" if int(actor) % 2 == 1 else "female"
        clips.append(_tagged(path.stem, actor, sex, RAVDESS_EMOTION[parts[2]], path, cfg))
    return clips


def attach_profiles(clips: list[Clip], cache: Path, binary: Path, workers: int) -> list[str]:
    cache.mkdir(parents=True, exist_ok=True)
    jobs = []
    for clip in clips:
        dest = cache / f"{clip.clip_id}.json"
        if not dest.is_file():
            jobs.append((str(binary), str(clip.wav), str(dest)))
    failures = decode_many(binary, jobs, workers)
    failed_ids = {item.split(":", 1)[0] for item in failures}
    sections: dict[str, str] = {}
    for clip in clips:
        if clip.clip_id in failed_ids:
            continue
        loaded = load_profile(cache / f"{clip.clip_id}.json")
        if loaded is None:
            failures.append(f"{clip.clip_id}: profile unreadable")
            continue
        clip.summary, sec, clip.tp = loaded
        sections.update(sec)
    return failures


def q50_vector(clip: Clip, fields: list[str]) -> list[float] | None:
    if clip.summary is None:
        return None
    out = []
    for field in fields:
        q = clip.summary.get(field)
        if q is None:
            return None
        out.append(q[Q50])
    return out


def emotion_vector(clip: Clip, fields: list[str]) -> list[float] | None:
    if clip.summary is None:
        return None
    out = []
    for field in fields:
        q = clip.summary.get(field)
        if q is None:
            return None
        out.append(q[Q50])
        out.append(q[Q75] - q[Q25])
    return out


def tract_vectors(clips: list[Clip], cfg: dict) -> tuple[str, list[list[float] | None], list[list[float] | None]]:
    usable = [clip for clip in clips if clip.summary is not None]
    have = 0
    for clip in usable:
        speaker = clip.tp.get("speaker") if clip.tp else None
        emotion = clip.tp.get("emotion") if clip.tp else None
        if isinstance(speaker, list) and speaker and isinstance(emotion, list) and emotion:
            have += 1
    if have and have != len(usable):
        raise SystemExit("some profiles have tractprint_v2 and some do not")
    if have:
        speakers = []
        emotions = []
        for clip in clips:
            if clip.tp is None:
                speakers.append(None)
                emotions.append(None)
            else:
                speakers.append([float(x) for x in clip.tp["speaker"]])
                emotions.append([float(x) for x in clip.tp["emotion"]])
        return "tractprint_v2", speakers, emotions
    speakers = [q50_vector(clip, cfg["speaker_fields"]) for clip in clips]
    emotions = [emotion_vector(clip, cfg["emotion_fields"]) for clip in clips]
    return "acoustic summary quantiles (tractprint_v2 absent)", speakers, emotions


def verification(clips: list[Clip], vectors: list[list[float] | None], same_emotion: bool) -> dict:
    present = [(i, vector) for i, vector in enumerate(vectors) if vector is not None]
    if len(present) < 4:
        return {"measured": False, "reason": "fewer than 4 profiles"}
    std = standardize([vector for _, vector in present])
    by_index = {i: vec for (i, _), vec in zip(present, std)}
    by_speaker: dict[str, list[int]] = defaultdict(list)
    for i, _ in present:
        by_speaker[clips[i].speaker].append(i)
    speakers = list(by_speaker)
    rng = random.Random(1729)
    genuine: list[float] = []
    impostor: list[float] = []
    seen: set[tuple[int, int]] = set()
    for speaker, idxs in by_speaker.items():
        for a in idxs:
            pool = []
            for b in idxs:
                if b == a:
                    continue
                if same_emotion and clips[b].native != clips[a].native:
                    continue
                pool.append(b)
            rng.shuffle(pool)
            got = 0
            for b in pool:
                key = (a, b) if a < b else (b, a)
                if key in seen:
                    continue
                seen.add(key)
                genuine.append(cosine(by_index[a], by_index[b]))
                got += 1
                if got >= 20:
                    break
            draws = 0
            while draws < 20:
                draws += 1
                other = speakers[rng.randrange(len(speakers))]
                if other == speaker:
                    continue
                candidates = by_speaker[other]
                if same_emotion:
                    candidates = [b for b in candidates if clips[b].native == clips[a].native]
                if not candidates:
                    continue
                b = candidates[rng.randrange(len(candidates))]
                impostor.append(cosine(by_index[a], by_index[b]))
    return {
        "measured": bool(genuine and impostor),
        "same_emotion_only": same_emotion,
        "auc": auc_score(genuine, impostor),
        "eer": eer_score(genuine, impostor),
        "n_genuine": len(genuine),
        "n_impostor": len(impostor),
    }


def sex_separation(clips: list[Clip], vectors: list[list[float] | None]) -> dict:
    grouped: dict[str, list[list[float]]] = defaultdict(list)
    sex_of: dict[str, str] = {}
    for clip, vector in zip(clips, vectors):
        if vector is None:
            continue
        grouped[clip.speaker].append(vector)
        sex_of[clip.speaker] = clip.sex
    if len(grouped) < 4:
        return {"measured": False, "reason": "fewer than 4 speakers"}
    means = {speaker: mean_vec(items) for speaker, items in grouped.items()}
    order = sorted(means)
    std = standardize([means[speaker] for speaker in order])
    points = {speaker: vec for speaker, vec in zip(order, std)}
    positive: list[float] = []
    negative: list[float] = []
    for held in order:
        females = [points[s] for s in order if s != held and sex_of[s] == "female"]
        males = [points[s] for s in order if s != held and sex_of[s] == "male"]
        if not females or not males:
            continue
        direction = sub(mean_vec(females), mean_vec(males))
        norm = math.sqrt(sum(x * x for x in direction)) or 1.0
        score = sum(a * b for a, b in zip(points[held], direction)) / norm
        (positive if sex_of[held] == "female" else negative).append(score)
    return {
        "measured": bool(positive and negative),
        "level": "speaker",
        "protocol": "leave-one-speaker-out projection onto female-mean minus male-mean",
        "auc": auc_score(positive, negative),
        "eer": eer_score(positive, negative),
        "n_female": sum(1 for speaker in order if sex_of[speaker] == "female"),
        "n_male": sum(1 for speaker in order if sex_of[speaker] == "male"),
    }


def center_emotion(clips: list[Clip], vectors: list[list[float] | None]) -> dict[int, list[float]]:
    by_speaker: dict[str, list[int]] = defaultdict(list)
    for i, vector in enumerate(vectors):
        if vector is not None:
            by_speaker[clips[i].speaker].append(i)
    centered: dict[int, list[float]] = {}
    for idxs in by_speaker.values():
        neutral = [i for i in idxs if clips[i].emotion == "neutral"]
        for i in idxs:
            base_ids = neutral
            if clips[i].emotion == "neutral":
                base_ids = [j for j in neutral if j != i]
            if not base_ids:
                if clips[i].emotion == "neutral":
                    centered[i] = [0.0] * len(vectors[i])  # type: ignore[arg-type]
                continue
            base = mean_vec([vectors[j] for j in base_ids])  # type: ignore[misc]
            centered[i] = sub(vectors[i], base)  # type: ignore[arg-type]
    return centered


def classify(clips: list[Clip], centered: dict[int, list[float]], sex: str | None, labels: list[str]) -> dict:
    chosen = []
    for i, vector in centered.items():
        if sex is not None and clips[i].sex != sex:
            continue
        if clips[i].emotion not in labels:
            continue
        chosen.append(i)
    speakers = sorted({clips[i].speaker for i in chosen})
    if len(speakers) < 2 or len(chosen) < 4:
        return {"measured": False, "reason": "not enough speakers"}
    raw = [centered[i] for i in chosen]
    std = standardize(raw)
    point = {i: vec for i, vec in zip(chosen, std)}
    index = {label: n for n, label in enumerate(labels)}
    confusion = [[0 for _ in labels] for _ in labels]
    by_speaker: dict[str, list[int]] = defaultdict(list)
    for i in chosen:
        by_speaker[clips[i].speaker].append(i)
    for held in speakers:
        train: dict[str, list[list[float]]] = defaultdict(list)
        for speaker, idxs in by_speaker.items():
            if speaker == held:
                continue
            for i in idxs:
                train[clips[i].emotion].append(point[i])
        centroids = {label: mean_vec(items) for label, items in train.items() if items}
        if len(centroids) < 2:
            continue
        for i in by_speaker[held]:
            gold = clips[i].emotion
            best_label = min(centroids, key=lambda label: dist2(point[i], centroids[label]))
            confusion[index[gold]][index[best_label]] += 1
    n = sum(sum(row) for row in confusion)
    correct = sum(confusion[i][i] for i in range(len(labels)))
    counts = {label: sum(confusion[i]) for i, label in enumerate(labels)}
    majority = max(counts.values()) / n if n else None
    return {
        "measured": n > 0,
        "n": n,
        "n_speakers": len(speakers),
        "accuracy": (correct / n) if n else None,
        "majority_accuracy": majority,
        "macro_f1": macro_f1(confusion),
        "per_class_f1": per_class_f1(confusion, labels),
        "counts": counts,
        "labels": labels,
        "confusion": confusion,
    }


def noise_picks(clips: list[Clip]) -> list[Clip]:
    by_speaker: dict[str, list[Clip]] = defaultdict(list)
    for clip in clips:
        if clip.summary is not None:
            by_speaker[clip.speaker].append(clip)
    chosen = []
    for speaker in sorted(by_speaker):
        items = sorted(by_speaker[speaker], key=lambda clip: clip.clip_id)
        pick = []
        for emotion in ("neutral", "angry"):
            hit = next((clip for clip in items if clip.emotion == emotion), None)
            if hit is not None:
                pick.append(hit)
        for clip in items:
            if len(pick) >= 2:
                break
            if clip not in pick:
                pick.append(clip)
        chosen.extend(pick[:2])
    return chosen


def field_abs(a: dict[str, list[float]], b: dict[str, list[float]]) -> dict[str, float]:
    out = {}
    for field, qa in a.items():
        qb = b.get(field)
        if qb is None:
            continue
        out[field] = abs(qa[Q50] - qb[Q50])
    return out


def noise_floor(clips: list[Clip], binary: Path, work: Path, cfg: dict, workers: int) -> dict:
    picks = noise_picks(clips)
    work.mkdir(parents=True, exist_ok=True)
    gain = float(cfg["augment"]["gain"])
    snr = float(cfg["augment"]["snr_db"])
    rng = random.Random(1729)
    jobs = []
    meta = []
    for n, clip in enumerate(picks):
        gain_wav = work / f"{n:04d}-gain.wav"
        noise_wav = work / f"{n:04d}-noise.wav"
        scale_wav(clip.wav, gain_wav, gain, None, rng)
        scale_wav(clip.wav, noise_wav, 1.0, snr, rng)
        repeat = work / f"{n:04d}-repeat.json"
        gain_json = work / f"{n:04d}-gain.json"
        noise_json = work / f"{n:04d}-noise.json"
        jobs.append((str(binary), str(clip.wav), str(repeat)))
        jobs.append((str(binary), str(gain_wav), str(gain_json)))
        jobs.append((str(binary), str(noise_wav), str(noise_json)))
        meta.append((clip, repeat, gain_json, noise_json))
    failures = decode_many(binary, jobs, workers)
    buckets: dict[str, dict[str, list[float]]] = {"repeat": defaultdict(list), "gain": defaultdict(list), "noise": defaultdict(list)}
    used = 0
    for clip, repeat, gain_json, noise_json in meta:
        if clip.summary is None:
            continue
        loaded = {
            "repeat": load_profile(repeat),
            "gain": load_profile(gain_json),
            "noise": load_profile(noise_json),
        }
        if any(item is None for item in loaded.values()):
            continue
        used += 1
        for kind, item in loaded.items():
            deltas = field_abs(clip.summary, item[0])  # type: ignore[index]
            for field, value in deltas.items():
                buckets[kind][field].append(value)
    # The augmented wavs are derived signals, not dataset audio, and they stay in the cache.
    return {
        "n_files": used,
        "n_decode_failures": len(failures),
        "gain": gain,
        "snr_db": snr,
        "rule": "one neutral and one angry clip per speaker, when that emotion exists",
        "repeat": {field: dist_stats(values) for field, values in sorted(buckets["repeat"].items())},
        "gain": {field: dist_stats(values) for field, values in sorted(buckets["gain"].items())},
        "noise": {field: dist_stats(values) for field, values in sorted(buckets["noise"].items())},
    }


def pair_deltas(clips: list[Clip]) -> dict[str, dict[str, dict]]:
    by_speaker: dict[str, list[Clip]] = defaultdict(list)
    usable = []
    for clip in clips:
        if clip.summary is None:
            continue
        by_speaker[clip.speaker].append(clip)
        usable.append(clip)
    same_emotion: dict[str, list[float]] = defaultdict(list)
    any_emotion: dict[str, list[float]] = defaultdict(list)
    for items in by_speaker.values():
        for i, a in enumerate(items):
            for b in items[i + 1 :]:
                deltas = field_abs(a.summary, b.summary)  # type: ignore[arg-type]
                for field, value in deltas.items():
                    any_emotion[field].append(value)
                    if a.emotion == b.emotion:
                        same_emotion[field].append(value)
    rng = random.Random(1729)
    speakers = list(by_speaker)
    different: dict[str, list[float]] = defaultdict(list)
    different_same: dict[str, list[float]] = defaultdict(list)
    for clip in usable:
        draws = 0
        got = 0
        while got < 8 and draws < 40:
            draws += 1
            other = speakers[rng.randrange(len(speakers))]
            if other == clip.speaker:
                continue
            pool = by_speaker[other]
            mate = pool[rng.randrange(len(pool))]
            deltas = field_abs(clip.summary, mate.summary)  # type: ignore[arg-type]
            for field, value in deltas.items():
                different[field].append(value)
            same_pool = [item for item in pool if item.emotion == clip.emotion]
            if same_pool:
                mate = same_pool[rng.randrange(len(same_pool))]
                deltas = field_abs(clip.summary, mate.summary)  # type: ignore[arg-type]
                for field, value in deltas.items():
                    different_same[field].append(value)
            got += 1
    return {
        "same_speaker_same_emotion": {field: dist_stats(values) for field, values in sorted(same_emotion.items())},
        "same_speaker_any_emotion": {field: dist_stats(values) for field, values in sorted(any_emotion.items())},
        "different_speaker": {field: dist_stats(values) for field, values in sorted(different.items())},
        "different_speaker_same_emotion": {field: dist_stats(values) for field, values in sorted(different_same.items())},
        "sampling": "same-speaker pairs are exhaustive; different-speaker is 8 draws per clip, seed 1729",
    }


def recommend_fields(floor: dict, deltas: dict) -> dict:
    fields = sorted(set(floor["repeat"]) | set(floor["gain"]) | set(floor["noise"]))
    out = {}
    for field in fields:
        noises = []
        for kind in ("repeat", "gain", "noise"):
            stats = floor[kind].get(field) or {}
            if stats.get("p95") is not None:
                noises.append(stats["p95"])
        noise = max(noises) if noises else None
        take = (deltas["same_speaker_same_emotion"].get(field) or {}).get("p90")
        same_voice = None if noise is None and take is None else max(value for value in (noise, take) if value is not None)
        out[field] = {
            "same_reading_abs_median": noise,
            "same_speaker_same_emotion_abs_median": same_voice,
        }
    return {
        "status": "recommendation, not a gate",
        "rule": (
            "same_reading is the max of the p95 |median| deltas on a second decode, "
            "a 0.97 gain, and 40 dB SNR noise. same_speaker_same_emotion is the max of "
            "that floor and the p90 |median| delta between two real takes of one speaker "
            "in one emotion. A future clone bar tighter than same_reading will fail a "
            "second reading of the same file."
        ),
        "reference_vs_clone": {
            "measured": False,
            "reason": "this run has no clone renders; the same-speaker distributions are real takes, not clones",
        },
        "fields": out,
    }


def find_labels(cfg: dict) -> Path | None:
    for relative in cfg["labels"]["candidates"]:
        path = REPO / relative
        if path.is_file():
            return path
    return None


def load_directions(path: Path, emotions: set[str], fields: set[str]) -> dict[tuple[str, str, str], str]:
    with path.open("rb") as handle:
        doc = tomllib.load(handle)
    found: dict[tuple[str, str, str], str] = {}

    def direction(value: object) -> str | None:
        if isinstance(value, str):
            text = value.strip().lower()
            if text in {"higher", "up", "increase", "+"}:
                return "higher"
            if text in {"lower", "down", "decrease", "-"}:
                return "lower"
            if text in {"same", "unchanged", "flat", "0"}:
                return "same"
            return None
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            if value > 0:
                return "higher"
            if value < 0:
                return "lower"
            return "same"
        return None

    def walk(node: object, trail: list[str]) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                walk(value, trail + [str(key).lower()])
            return
        mapped = direction(node)
        if mapped is None:
            return
        emotion = next((part for part in trail if part in emotions), None)
        sex = next((part for part in trail if part in {"male", "female"}), None)
        field = next((part for part in trail if part in fields), None)
        if emotion and sex and field:
            found[(emotion, sex, field)] = mapped

    walk(doc, [])
    return found


def observed_movement(clips: list[Clip], floor: dict, allowed: set[str]) -> dict:
    """Signed median shift from each speaker's own neutral. Real measurements."""
    by_speaker: dict[str, list[Clip]] = defaultdict(list)
    for clip in clips:
        if clip.summary is not None:
            by_speaker[clip.speaker].append(clip)
    fields = sorted({field for clip in clips if clip.summary for field in clip.summary})
    noise = {}
    for field in fields:
        vals = []
        for kind in ("repeat", "gain", "noise"):
            p95 = (floor[kind].get(field) or {}).get("p95")
            if p95 is not None:
                vals.append(p95)
        noise[field] = max(vals) if vals else None
    out: dict[str, dict] = {}
    for sex in ("female", "male"):
        out[sex] = {}
        speakers = [sp for sp, items in by_speaker.items() if items and items[0].sex == sex]
        emotions = sorted({
            clip.emotion
            for sp in speakers
            for clip in by_speaker[sp]
            if clip.emotion in allowed and clip.emotion != "neutral"
        })
        for emotion in emotions:
            per_field: dict[str, dict] = {}
            for field in fields:
                deltas = []
                for speaker in speakers:
                    items = by_speaker[speaker]
                    neutral = [clip.summary[field][Q50] for clip in items if clip.emotion == "neutral" and field in clip.summary]  # type: ignore[index]
                    voiced = [clip.summary[field][Q50] for clip in items if clip.emotion == emotion and field in clip.summary]  # type: ignore[index]
                    if neutral and voiced:
                        deltas.append(sum(voiced) / len(voiced) - sum(neutral) / len(neutral))
                if not deltas:
                    continue
                mean_delta = sum(deltas) / len(deltas)
                limit = noise.get(field)
                if limit is None:
                    sign = "unknown"
                elif mean_delta > limit:
                    sign = "higher"
                elif mean_delta < -limit:
                    sign = "lower"
                else:
                    sign = "within_noise"
                per_field[field] = {"n_speakers": len(deltas), "mean_delta_q50": mean_delta, "versus_noise": sign}
            out[sex][emotion] = per_field
    return out


def label_sanity(clips: list[Clip], floor: dict, cfg: dict) -> dict:
    observed = observed_movement(clips, floor, set(cfg["emotions"]))
    path = find_labels(cfg)
    if path is None:
        return {
            "measured": False,
            "reason": "emotion-labels-v2.toml is not on this branch",
            "observed_versus_neutral": observed,
        }
    emotions = set(cfg["emotions"])
    fields = {field for sex in observed.values() for emotion in sex.values() for field in emotion}
    predicted = load_directions(path, emotions, fields)
    if not predicted:
        return {
            "measured": False,
            "reason": f"{path} contained no emotion/sex/field directions this loader understands",
            "path": str(path.relative_to(REPO)),
            "observed_versus_neutral": observed,
        }
    compared = 0
    agree = 0
    rows = []
    for (emotion, sex, field), expect in sorted(predicted.items()):
        got = observed.get(sex, {}).get(emotion, {}).get(field)
        if not got:
            rows.append({"emotion": emotion, "sex": sex, "field": field, "predicted": expect, "observed": None})
            continue
        observed_sign = got["versus_noise"]
        match = (expect == "same" and observed_sign == "within_noise") or (expect == observed_sign)
        compared += 1
        agree += int(match)
        rows.append({
            "emotion": emotion,
            "sex": sex,
            "field": field,
            "predicted": expect,
            "observed": observed_sign,
            "mean_delta_q50": got["mean_delta_q50"],
            "agree": match,
        })
    return {
        "measured": compared > 0,
        "path": str(path.relative_to(REPO)),
        "n_compared": compared,
        "n_agree": agree,
        "agreement": (agree / compared) if compared else None,
        "rows": rows,
        "observed_versus_neutral": observed,
    }


def coverage(clips: list[Clip], emotions: list[str]) -> dict:
    counts: dict[str, dict[str, int]] = {emotion: {"female": 0, "male": 0} for emotion in emotions}
    for clip in clips:
        if clip.summary is None or clip.emotion not in counts:
            continue
        counts[clip.emotion][clip.sex] += 1
    return counts


def evaluate_dataset(name: str, clips: list[Clip], root: Path, binary: Path, cfg: dict, workers: int) -> dict:
    cache = root / "cache" / name
    print(f"{name}: decode {len(clips)} clips", file=sys.stderr, flush=True)
    failures = attach_profiles(clips, cache, binary, workers)
    decoded = [clip for clip in clips if clip.summary is not None]
    source, speaker_vecs, emotion_vecs = tract_vectors(clips, cfg)
    present_emotions = [emotion for emotion in cfg["emotions"] if any(clip.emotion == emotion and clip.summary for clip in clips)]
    # Drop classes the classifier cannot see.
    labels = [emotion for emotion in present_emotions if emotion != "" ]
    print(f"{name}: noise floor", file=sys.stderr, flush=True)
    floor = noise_floor(decoded, binary, root / "cache" / f"{name}-floor", cfg, workers)
    print(f"{name}: pair deltas", file=sys.stderr, flush=True)
    deltas = pair_deltas(decoded)
    centered = center_emotion(clips, emotion_vecs)
    emotion = {
        "centering": "each clip minus that speaker's other neutral clips",
        "classifier": "leave-one-speaker-out nearest centroid",
        "pooled": classify(clips, centered, None, labels),
        "female": classify(clips, centered, "female", labels),
        "male": classify(clips, centered, "male", labels),
    }
    sexes = {}
    for sex in ("female", "male"):
        n = sum(1 for clip in decoded if clip.sex == sex)
        sexes[sex] = n
    return {
        "schema": cfg["schema"],
        "dataset": name,
        "licence": cfg["datasets"][name]["licence"],
        "decoder": {
            "command": "galion-articulation analyze",
            "bricks": ["praatfan_rust 0.1.16"],
            "workers_not_run": ["sparc", "disvoice", "phonet", "rhubarb"],
            "note": "Acoustic throat and mouth fields only. Nose posterior, EMA and glottal statics were not run.",
        },
        "vector_source": source,
        "speaker_fields": cfg["speaker_fields"],
        "emotion_fields": cfg["emotion_fields"],
        "n_clips_indexed": len(clips),
        "n_decoded": len(decoded),
        "n_failed": len(failures),
        "failures_sample": failures[:8],
        "n_by_sex": sexes,
        "emotions": cfg["emotions"],
        "bursts": cfg["bursts"],
        "coverage": coverage(decoded, cfg["emotions"]),
        "burst_coverage": _burst_coverage(decoded, cfg["bursts"]),
        "speaker_part": {
            "same_vs_different": verification(clips, speaker_vecs, False),
            "same_vs_different_same_emotion": verification(clips, speaker_vecs, True),
            "sex": sex_separation(clips, speaker_vecs),
        },
        "emotion_part": emotion,
        "noise_floor": floor,
        "field_deltas": deltas,
        "recommended_field_bars": recommend_fields(floor, deltas),
        "label_sanity": label_sanity(decoded, floor, cfg),
        "rollout_gate_unchanged": cfg["rollout_gate"],
    }


def _burst_coverage(clips: list[Clip], bursts: list[str]) -> dict:
    counts: dict[str, dict[str, int]] = {burst: {"female": 0, "male": 0} for burst in bursts}
    for clip in clips:
        if clip.summary is None or clip.burst not in counts:
            continue
        counts[clip.burst][clip.sex] += 1
    return counts


def self_check_taxonomy(cfg: dict) -> None:
    """The 12 prosody emotions, neutral, and the three burst detectors. No audio."""
    expected = [
        "neutral",
        "adoration",
        "amusement",
        "anger",
        "awe",
        "confusion",
        "contempt",
        "desire",
        "disappointment",
        "distress",
        "fear",
        "interest",
        "sadness",
    ]
    if list(cfg["emotions"]) != expected:
        raise SystemExit(f"emotion set is {cfg['emotions']}")
    if list(cfg["bursts"]) != ["laughter", "sigh", "breath"]:
        raise SystemExit(f"bursts are {cfg['bursts']}")
    if set(cfg["emotions"]) & set(cfg["bursts"]):
        raise SystemExit("a burst is listed as an emotion")
    checks = {
        "laughter": ("", "laughter"),
        "sigh": ("", "sigh"),
        "breath": ("", "breath"),
        "happy": ("", ""),
        "disgusted": ("", ""),
        "surprised": ("", ""),
        "calm": ("", ""),
        "angry": ("anger", ""),
        "fearful": ("fear", ""),
        "sad": ("sadness", ""),
        "neutral": ("neutral", ""),
        "amusement": ("amusement", ""),
    }
    for native, want in checks.items():
        got = canonical_label(native, cfg)
        if got != want:
            raise SystemExit(f"{native} mapped to {got}, want {want}")


def smoke(binary: Path) -> None:
    cfg = load_config()
    self_check_taxonomy(cfg)
    self_check_math()
    work = Path(os.environ.get("ARTIC_EVAL_SMOKE", "/tmp/articulation-v2-eval-smoke"))
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    outside_repo(work)
    specs = [
        ("low-a", 110.0, 400.0, 1400.0),
        ("low-b", 112.0, 410.0, 1420.0),
        ("high-a", 200.0, 700.0, 2100.0),
        ("high-b", 198.0, 690.0, 2080.0),
    ]
    clips = []
    for clip_id, f0, f1, f2 in specs:
        wav = work / f"{clip_id}.wav"
        synth_vowel(wav, f0, f1, f2)
        sex = "male" if f0 < 150 else "female"
        clips.append(Clip(clip_id, "low" if f0 < 150 else "high", sex, "neutral", wav))
    cache = work / "cache"
    failures = attach_profiles(clips, cache, binary, 2)
    if failures:
        raise SystemExit(f"smoke decode failed: {failures}")
    first = clips[0]
    again = work / "again.json"
    decode_job((str(binary), str(first.wav), str(again)))
    second = load_profile(again)
    if second is None or first.summary is None:
        raise SystemExit("smoke profile missing")
    for field, q in first.summary.items():
        other = second[0].get(field)
        if other is None or any(abs(a - b) > 1e-9 for a, b in zip(q, other)):
            raise SystemExit(f"repeat decode moved {field}")
    gain_wav = work / "gain.wav"
    scale_wav(first.wav, gain_wav, 0.97, None, random.Random(1))
    gain_json = work / "gain.json"
    decode_job((str(binary), str(gain_wav), str(gain_json)))
    gained = load_profile(gain_json)
    if gained is None or "f0_hz" not in gained[0] or "f0_hz" not in first.summary:
        raise SystemExit("gain profile missing f0")
    shift = abs(gained[0]["f0_hz"][Q50] - first.summary["f0_hz"][Q50])
    if shift > 1.0:
        raise SystemExit(f"0.97 gain moved F0 median by {shift:.3f} Hz")
    _source, vectors, _emotions = tract_vectors(clips, cfg)
    scores = verification(clips, vectors, False)
    auc = scores.get("auc")
    if auc is None or auc < 0.9:
        raise SystemExit(f"synthetic speaker AUC {auc} (two pitches should separate)")
    print(json.dumps({
        "smoke": "pass",
        "synthetic": True,
        "repeat_max_abs_q50": 0.0,
        "gain_f0_hz_abs_delta": shift,
        "speaker_auc": auc,
        "note": "Synthetic vowels. Not a voice result and not a dataset score.",
    }, indent=2))


def run(args: argparse.Namespace) -> None:
    cfg = load_config()
    root = Path(args.root)
    outside_repo(root)
    root.mkdir(parents=True, exist_ok=True)
    binary = ensure_binary(args.bin)
    names = args.dataset or [name for name, spec in cfg["datasets"].items() if spec.get("enabled")]
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        if name == "ravdess":
            ensure_ravdess(root, cfg)
            clips = index_ravdess(root, cfg)
        elif name == "crema-d":
            ensure_crema(root, cfg)
            clips = index_crema(root, cfg)
        else:
            raise SystemExit(f"no fetcher for {name}; it is recorded in config.toml and was not run")
        if args.limit:
            clips = clips[: args.limit]
        report = evaluate_dataset(name, clips, root, binary, cfg, args.jobs)
        dest = out_dir / f"{name}.json"
        dest.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        sex = report["speaker_part"]["sex"]
        emo = report["emotion_part"]["pooled"]
        print(
            f"{name}: decoded {report['n_decoded']}/{report['n_clips_indexed']} "
            f"sex_auc={sex.get('auc')} same_emotion_eer={report['speaker_part']['same_vs_different_same_emotion'].get('eer')} "
            f"emotion_macro_f1={emo.get('macro_f1')}",
            file=sys.stderr,
            flush=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Articulation loop eval")
    sub = parser.add_subparsers(dest="cmd", required=True)
    smoke_p = sub.add_parser("smoke", help="synthetic signals, no datasets")
    smoke_p.add_argument("--bin", default=os.environ.get("ARTIC_BIN"))
    run_p = sub.add_parser("run", help="decode public datasets and write aggregate JSON")
    run_p.add_argument("--bin", default=os.environ.get("ARTIC_BIN"))
    run_p.add_argument("--root", default=os.environ.get("ARTIC_EVAL_ROOT", "/tmp/articulation-v2-eval"))
    run_p.add_argument("--dataset", action="append", default=[])
    run_p.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 2))
    run_p.add_argument("--out-dir", default=str(EVAL_DIR / "reports"))
    run_p.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()
    if args.cmd == "smoke":
        smoke(ensure_binary(args.bin))
    else:
        run(args)


if __name__ == "__main__":
    main()
