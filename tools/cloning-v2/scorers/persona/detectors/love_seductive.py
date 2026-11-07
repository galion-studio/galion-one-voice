#!/usr/bin/env python3
"""Love and seductive detector plugin (Agent 2).

Discovered by scorers/persona/detectors/__init__.py via the love_seductive*.py
glob. Exports LoveSeductiveDetector with a no-arg constructor.

Method (chosen after measuring the original-voice probe set):
  Gender-conditioned LAION-CLAP prompt ensemble (Apache-2.0, local weights)
  contrasted against a neutral-speech prompt bank. Score is a pairwise
  softmax of (mean cosine over target prompts, mean cosine over
  neutral-speech prompts) at temperature 20.

  Seductive: that contrast multiplied by (1 - emotion2vec+ sad), so
  crying/sad lines are not counted as seductive. Whisper-blend and
  max-prompt variants were measured and rejected (they detect neutrals
  at 0.2). See CHOSEN_SEDUCTIVE.

  Love: the same contrast, multiplied by (1 - emotion2vec+ sad), but
  only if the mean cosine vs the love prompt bank is > LOVE_ALIGN_MIN
  (0.20). Below that, love is reported as 0 (no presence). This fires
  on Samantha's original love window (w03) and not on her original
  neutral window (w00) or the probe neutrals. Male-tender prompt
  ensembles did not make Jarvis's synthesized love lines positively
  align, so those stay undetected at 0.2. emotion2vec+ happy is not
  used (it misses Jarvis lov02, which is e2v-neutral 1.0, and fires
  on both joyful lines; Samantha is scored on love and joyful).

  Jarvis seductive and Jarvis love values are still emitted (info).
  own-jarvis.toml marks both n/a; own-samantha.toml includes both.

This file does not edit scorers/universal/ (old scorer stays default).
"""
from __future__ import annotations

import argparse
import json
import math
import os
import sys
import time
from pathlib import Path
from typing import Any, Mapping, Optional

# Plugin is loaded via importlib.util.spec_from_file_location, so relative
# imports fail. Fall back to same-directory import of Agent 1's base.py.
try:
    from .base import DetectorScore
except ImportError:  # plugin discovery or `python love_seductive.py`
    _DET_DIR = Path(__file__).resolve().parent
    if str(_DET_DIR) not in sys.path:
        sys.path.insert(0, str(_DET_DIR))
    from base import DetectorScore

HERE = Path(__file__).resolve().parent
PROMPTS_PATH = HERE / "love_seductive_prompts.json"
UNI = HERE.parents[1] / "universal"
CLAP_DIR = UNI / "clap" / "laion-clap-htsat-fused"
SUMMARY = Path(
    "/workspace/swarm/voice-v2-sprint/cloning-v2/scorer-probe/results/SUMMARY.json"
)
PRESENCE = 0.20
CONTRAST_TEMP = 20.0
SOURCE = (
    "love_seductive clap-prompt-ensemble "
    "(gender-conditioned mean-target vs mean-neutral, sad-gated; "
    "love also requires mean cosine > 0.20)"
)

# Locked after --measure on the original-voice probe. Do not silently change.
CHOSEN_LOVE = "gender_mean_vs_neu_mean_t20_sadgate_align"
CHOSEN_SEDUCTIVE = "gender_mean_vs_neu_mean_t20_sadgate"
WHISPER_BLEND = 0.35  # measured; not used by the chosen seductive method
LOVE_ALIGN_MIN = 0.20  # minimum mean cosine vs the love prompt bank
MALE_PERSONAS = {"jarvis", "own-jarvis", "male"}
FEMALE_PERSONAS = {"samantha", "own-samantha", "female"}

_CLAP = None


def softmax2(a: float, b: float, temp: float = CONTRAST_TEMP) -> float:
    m = max(a, b)
    ea = math.exp((a - m) * temp)
    eb = math.exp((b - m) * temp)
    return float(ea / (ea + eb))


def sigmoid(x: float) -> float:
    if x >= 0:
        z = math.exp(-x)
        return float(1.0 / (1.0 + z))
    z = math.exp(x)
    return float(z / (1.0 + z))


def load_wav(path: str | Path, target_sr: int):
    import numpy as np
    import soundfile as sf

    x, sr = sf.read(str(path), always_2d=False)
    if getattr(x, "ndim", 1) > 1:
        x = x.mean(axis=-1)
    x = np.asarray(x, dtype=np.float32)
    if sr != target_sr:
        n = int(round(len(x) * target_sr / sr))
        if n <= 0 or len(x) == 0:
            return x
        xp = np.linspace(0.0, 1.0, len(x), endpoint=False)
        tt = np.linspace(0.0, 1.0, n, endpoint=False)
        x = np.interp(tt, xp, x).astype(np.float32)
    return x


def acoustic_features(path: str | Path) -> dict[str, float]:
    """Whisper / intimacy proxies. No extra model (numpy only)."""
    import numpy as np

    x = load_wav(path, 16000)
    sr = 16000
    if x.size == 0:
        return {
            "rms_db": -120.0,
            "peak": 0.0,
            "zcr": 0.0,
            "centroid_hz": 0.0,
            "flatness": 0.0,
            "hf_ratio": 0.0,
            "voicing": 0.0,
            "whisper_raw": 0.0,
        }
    rms = float(np.sqrt(np.mean(x * x) + 1e-15))
    rms_db = float(20.0 * math.log10(rms + 1e-12))
    peak = float(np.max(np.abs(x)))
    zcr = float(np.mean(np.abs(np.diff(np.signbit(x).astype(np.int8)))))
    nfft, hop = 1024, 256
    window = np.hanning(nfft).astype(np.float32)
    frames = []
    for i in range(0, max(0, x.size - nfft + 1), hop):
        frames.append(x[i : i + nfft] * window)
    if frames:
        mag = np.abs(np.fft.rfft(np.stack(frames), axis=1)).mean(axis=0) + 1e-12
        freqs = np.fft.rfftfreq(nfft, 1.0 / sr)
        centroid = float(np.sum(freqs * mag) / np.sum(mag))
        flatness = float(np.exp(np.mean(np.log(mag))) / np.mean(mag))
        hf_ratio = float(np.sum(mag[freqs >= 4000.0]) / np.sum(mag))
    else:
        centroid = 0.0
        flatness = 0.0
        hf_ratio = 0.0
    # voicing: peak normalized autocorr in 70–350 Hz
    voicing = 0.0
    min_lag = max(1, int(sr / 350))
    max_lag = min(x.size - 1, int(sr / 70))
    if max_lag > min_lag + 2:
        chunk = x[: min(x.size, sr * 3)]
        energy = float(np.dot(chunk, chunk) + 1e-12)
        best = 0.0
        for lag in range(min_lag, max_lag + 1, 2):
            acc = float(np.dot(chunk[:-lag], chunk[lag:])) / energy
            if acc > best:
                best = acc
        voicing = float(max(0.0, best))
    # Whisper-like: quieter, noisier (flat spectrum, high ZCR), weakly voiced.
    # Fixed (not fit-on-probe) logistic mix.
    whisper_raw = float(
        sigmoid((-rms_db - 28.0) / 6.0) * 0.35
        + sigmoid((zcr - 0.08) / 0.04) * 0.25
        + sigmoid((flatness - 0.15) / 0.08) * 0.20
        + sigmoid((0.25 - voicing) / 0.10) * 0.20
    )
    return {
        "rms_db": rms_db,
        "peak": peak,
        "zcr": zcr,
        "centroid_hz": centroid,
        "flatness": flatness,
        "hf_ratio": hf_ratio,
        "voicing": voicing,
        "whisper_raw": whisper_raw,
    }


def _persona_gender(persona: Optional[str]) -> str:
    if not persona:
        return "unknown"
    p = str(persona).strip().lower()
    if p in MALE_PERSONAS or p.endswith("jarvis") or p.startswith("male"):
        return "male"
    if p in FEMALE_PERSONAS or p.endswith("samantha") or p.startswith("female"):
        return "female"
    return "unknown"


def _wav_path(rec: Mapping[str, Any]) -> Optional[Path]:
    p = rec.get("path") or rec.get("file")
    if not p:
        return None
    path = Path(p)
    return path if path.exists() else None


def _rec_persona(rec: Mapping[str, Any]) -> Optional[str]:
    for key in ("persona", "voice", "speaker", "voice_id"):
        v = rec.get(key)
        if v:
            return str(v)
    return None


class _ClapEnsemble:
    def __init__(self):
        import numpy as np
        import torch
        from transformers import ClapModel, ClapProcessor

        self.np = np
        self.torch = torch
        self.processor = ClapProcessor.from_pretrained(str(CLAP_DIR), local_files_only=True)
        self.model = ClapModel.from_pretrained(str(CLAP_DIR), local_files_only=True)
        self.model.eval()
        # CPU unless a laptop eval sets GALION_CLAP_DEVICE=cuda. One model, one clip.
        self.device = os.environ.get("GALION_CLAP_DEVICE", "cpu").strip() or "cpu"
        if self.device != "cpu":
            self.model.to(self.device)
        bank = json.loads(PROMPTS_PATH.read_text())
        self.raw_bank = bank
        self.prompt_lists: dict[str, list[str]] = {
            k: list(v) for k, v in bank.items() if isinstance(v, list)
        }
        self.text_emb: dict[str, Any] = {}
        self.single_emb: dict[str, dict[str, Any]] = {}
        with torch.no_grad():
            for name, prompts in self.prompt_lists.items():
                inputs = self.processor(text=prompts, return_tensors="pt", padding=True)
                if self.device != "cpu":
                    inputs = {
                        k: v.to(self.device) if hasattr(v, "to") else v
                        for k, v in inputs.items()
                    }
                out = self.model.get_text_features(
                    input_ids=inputs["input_ids"],
                    attention_mask=inputs.get("attention_mask"),
                )
                emb = out.pooler_output  # [N, D] L2-normalized
                mean = emb.mean(dim=0, keepdim=True)
                mean = mean / mean.norm(dim=-1, keepdim=True)
                self.text_emb[name] = mean
                self.single_emb[name] = {
                    p: emb[i : i + 1] for i, p in enumerate(prompts)
                }

    def audio_emb(self, wav48):
        torch = self.torch
        with torch.no_grad():
            inputs = self.processor(audio=wav48, sampling_rate=48000, return_tensors="pt")
            if self.device != "cpu":
                inputs = {
                    k: v.to(self.device) if hasattr(v, "to") else v
                    for k, v in inputs.items()
                }
            kw = {"input_features": inputs["input_features"]}
            if "is_longer" in inputs and inputs["is_longer"] is not None:
                kw["is_longer"] = inputs["is_longer"]
            return self.model.get_audio_features(**kw).pooler_output

    def cos_mean(self, audio_emb, bank: str) -> float:
        te = self.text_emb[bank]
        return float((audio_emb @ te.T).squeeze().item())

    def cos_max(self, audio_emb, bank: str) -> tuple[float, str]:
        best = -2.0
        best_p = ""
        for p, te in self.single_emb[bank].items():
            v = float((audio_emb @ te.T).squeeze().item())
            if v > best:
                best = v
                best_p = p
        return best, best_p

    def gender_banks(self, kind: str, gender: str) -> list[str]:
        """Return bank names to merge for a target kind (love/seductive/neutral)."""
        names = [f"{kind}_shared"]
        if gender == "male":
            names.append(f"{kind}_male")
        elif gender == "female":
            names.append(f"{kind}_female")
        else:
            names.append(f"{kind}_male")
            names.append(f"{kind}_female")
        return [n for n in names if n in self.text_emb]

    def merge_max(self, audio_emb, banks: list[str]) -> tuple[float, str]:
        best = -2.0
        best_p = ""
        for b in banks:
            v, p = self.cos_max(audio_emb, b)
            if v > best:
                best = v
                best_p = f"{b}::{p}"
        return best, best_p

    def merge_mean(self, audio_emb, banks: list[str]) -> float:
        vals = [self.cos_mean(audio_emb, b) for b in banks]
        return float(sum(vals) / len(vals)) if vals else 0.0


def get_clap() -> _ClapEnsemble:
    global _CLAP
    if _CLAP is None:
        _CLAP = _ClapEnsemble()
    return _CLAP


def _clip01(x: float) -> float:
    return float(min(1.0, max(0.0, x)))


def score_from_audio(
    path: Path,
    persona: Optional[str],
    e2v: Optional[Mapping[str, float]] = None,
) -> dict[str, Any]:
    clap = get_clap()
    gender = _persona_gender(persona)
    audio = load_wav(path, 48000)
    a = clap.audio_emb(audio)
    ac = acoustic_features(path)
    e2v = dict(e2v or {})

    love_banks = clap.gender_banks("love", gender)
    sed_banks = clap.gender_banks("seductive", gender)
    neu_banks = clap.gender_banks("neutral", gender)

    love_max, love_prompt = clap.merge_max(a, love_banks)
    sed_max, sed_prompt = clap.merge_max(a, sed_banks)
    love_mean = clap.merge_mean(a, love_banks)
    sed_mean = clap.merge_mean(a, sed_banks)
    neu_mean = clap.merge_mean(a, neu_banks)
    neu_max, neu_prompt = clap.merge_max(a, neu_banks)
    joy_mean = clap.cos_mean(a, "joyful_shared") if "joyful_shared" in clap.text_emb else 0.0
    love_old = clap.cos_mean(a, "love_old")
    sed_old = clap.cos_mean(a, "seductive_old")

    methods = {
        "old_raw_mean": {"love": love_old, "seductive": sed_old},
        "gender_mean_raw": {"love": love_mean, "seductive": sed_mean},
        "gender_max_raw": {"love": love_max, "seductive": sed_max},
        "gender_mean_vs_neu_mean_t20": {
            "love": softmax2(love_mean, neu_mean, 20.0),
            "seductive": softmax2(sed_mean, neu_mean, 20.0),
        },
        "gender_max_vs_neu_mean_t20": {
            "love": softmax2(love_max, neu_mean, 20.0),
            "seductive": softmax2(sed_max, neu_mean, 20.0),
        },
        "gender_max_vs_neu_max_t20": {
            "love": softmax2(love_max, neu_max, 20.0),
            "seductive": softmax2(sed_max, neu_max, 20.0),
        },
        "gender_max_vs_neu_mean_t10": {
            "love": softmax2(love_max, neu_mean, 10.0),
            "seductive": softmax2(sed_max, neu_mean, 10.0),
        },
        "gender_max_vs_neu_mean_t40": {
            "love": softmax2(love_max, neu_mean, 40.0),
            "seductive": softmax2(sed_max, neu_mean, 40.0),
        },
        "sig15_max_minus_neu": {
            "love": sigmoid((love_max - neu_mean) * 15.0),
            "seductive": sigmoid((sed_max - neu_mean) * 15.0),
        },
        "love_vs_joyful_t20": {
            "love": softmax2(love_max, joy_mean, 20.0),
            "seductive": softmax2(sed_max, joy_mean, 20.0),
        },
    }

    happy = float(e2v.get("happy", 0.0) or 0.0)
    sad = float(e2v.get("sad", 0.0) or 0.0)
    angry = float(e2v.get("angry", 0.0) or 0.0)
    neu_e2v = float(e2v.get("neutral", 0.0) or 0.0)

    love_contrast_max = methods["gender_max_vs_neu_mean_t20"]["love"]
    sed_contrast_max = methods["gender_max_vs_neu_mean_t20"]["seductive"]
    love_contrast_mean = methods["gender_mean_vs_neu_mean_t20"]["love"]
    sed_contrast_mean = methods["gender_mean_vs_neu_mean_t20"]["seductive"]

    # Combined love: do NOT OR with e2v happy (fires on joyful; misses
    # Jarvis lov02 which is e2v-neutral 1.0). Keep contrastive CLAP.
    methods["love_or_happy"] = {
        "love": max(love_contrast_max, happy),
        "seductive": sed_contrast_max,
    }
    methods["love_blend_happy"] = {
        "love": 0.55 * love_contrast_max + 0.45 * happy,
        "seductive": sed_contrast_max,
    }

    # Seductive whisper blend: measured; rejected (detects neutrals at 0.2).
    allow_whisper = (sad < 0.5) and (angry < 0.5)
    whisper = ac["whisper_raw"] if allow_whisper else 0.0
    sed_blend = _clip01((1.0 - WHISPER_BLEND) * sed_contrast_max + WHISPER_BLEND * whisper)
    methods["gender_max_vs_neu_mean_t20_plus_whisper_blend"] = {
        "love": love_contrast_max,
        "seductive": sed_blend,
    }
    methods["whisper_raw"] = {"love": 0.0, "seductive": ac["whisper_raw"]}

    sed_sadgate = float(sed_contrast_mean * (1.0 - sad))
    love_sadgate = float(love_contrast_mean * (1.0 - sad))
    love_align = love_sadgate if love_mean > LOVE_ALIGN_MIN else 0.0
    methods["gender_mean_vs_neu_mean_t20_sadgate"] = {
        "love": love_sadgate,
        "seductive": sed_sadgate,
    }
    methods["gender_mean_vs_neu_mean_t20_sadgate_align"] = {
        "love": love_align,
        "seductive": sed_sadgate,
    }

    chosen_love = methods[CHOSEN_LOVE]["love"]
    chosen_sed = methods[CHOSEN_SEDUCTIVE]["seductive"]

    return {
        "persona": persona,
        "gender": gender,
        "path": str(path),
        "love": float(chosen_love),
        "seductive": float(chosen_sed),
        "chosen_love_method": CHOSEN_LOVE,
        "chosen_seductive_method": CHOSEN_SEDUCTIVE,
        "love_max_cos": love_max,
        "love_mean_cos": love_mean,
        "love_best_prompt": love_prompt,
        "sed_max_cos": sed_max,
        "sed_mean_cos": sed_mean,
        "sed_best_prompt": sed_prompt,
        "neu_mean_cos": neu_mean,
        "neu_max_cos": neu_max,
        "neu_best_prompt": neu_prompt,
        "joy_mean_cos": joy_mean,
        "love_old_cos": love_old,
        "sed_old_cos": sed_old,
        "e2v_happy": happy,
        "e2v_sad": sad,
        "e2v_angry": angry,
        "e2v_neutral": neu_e2v,
        "acoustic": ac,
        "whisper_used": allow_whisper,
        "methods": methods,
    }


class LoveSeductiveDetector:
    name = "love-seductive-prompt-ensemble"
    categories = ("love", "seductive")

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        persona = _rec_persona(rec)
        path = _wav_path(rec)
        e2v = {}
        raw_e2v = rec.get("e2v") or {}
        if isinstance(raw_e2v.get("probs"), dict):
            e2v = {k: float(v) for k, v in raw_e2v["probs"].items() if isinstance(v, (int, float))}
        else:
            e2v = {k: float(v) for k, v in raw_e2v.items() if isinstance(v, (int, float))}

        if path is not None:
            detail = score_from_audio(path, persona, e2v)
            extras_common = {
                "method_love": CHOSEN_LOVE,
                "method_seductive": CHOSEN_SEDUCTIVE,
                "gender": detail["gender"],
                "love_best_prompt": detail["love_best_prompt"],
                "sed_best_prompt": detail["sed_best_prompt"],
                "love_max_cos": detail["love_max_cos"],
                "sed_max_cos": detail["sed_max_cos"],
                "neu_mean_cos": detail["neu_mean_cos"],
                "whisper_raw": detail["acoustic"]["whisper_raw"],
                "plugin": "love_seductive",
            }
            return {
                "love": DetectorScore(
                    category="love",
                    value=float(detail["love"]),
                    source=SOURCE,
                    extras=dict(extras_common),
                ),
                "seductive": DetectorScore(
                    category="seductive",
                    value=float(detail["seductive"]),
                    source=SOURCE,
                    extras=dict(extras_common),
                ),
            }

        # Degraded path: no wav on the record. Re-interpret legacy CLAP
        # cosine if present (still better than 10-way softmax vs logic);
        # otherwise pass through legacy clap_probs.
        clap = rec.get("clap") or {}
        cosine = clap.get("cosine") if isinstance(clap.get("cosine"), dict) else rec.get("clap_cosine") or {}
        probs = clap.get("probs") if isinstance(clap.get("probs"), dict) else rec.get("clap_probs") or {}
        extras = {"plugin": "love_seductive", "degraded": True, "reason": "no wav path on record"}
        out = {}
        for cat in self.categories:
            if cat in cosine and isinstance(cosine[cat], (int, float)):
                # Pairwise vs the strongest non-target old prompt if available.
                others = [
                    float(v)
                    for k, v in cosine.items()
                    if k != cat and isinstance(v, (int, float))
                ]
                other = max(others) if others else 0.0
                val = softmax2(float(cosine[cat]), other, CONTRAST_TEMP)
                src = SOURCE + " [degraded: old-cosine contrast]"
            else:
                val = float(probs.get(cat, 0.0) or 0.0)
                src = SOURCE + " [degraded: legacy clap_probs]"
            out[cat] = DetectorScore(category=cat, value=val, source=src, extras=dict(extras))
        return out


def build_detector() -> LoveSeductiveDetector:
    return LoveSeductiveDetector()


DETECTOR = LoveSeductiveDetector()


def _sep(pos: list[float], neu: list[float]) -> dict[str, Any]:
    import numpy as np

    if not pos or not neu:
        return {"note": "missing group", "n_pos": len(pos), "n_neu": len(neu)}
    pa = np.asarray(pos, dtype=np.float64)
    na = np.asarray(neu, dtype=np.float64)
    return {
        "n_pos": int(pa.size),
        "n_neu": int(na.size),
        "pos_mean": float(pa.mean()),
        "neu_mean": float(na.mean()),
        "margin": float(pa.mean() - na.mean()),
        "pos_min": float(pa.min()),
        "pos_max": float(pa.max()),
        "neu_min": float(na.min()),
        "neu_max": float(na.max()),
        "gap_minpos_maxneu": float(pa.min() - na.max()),
        "detect_pos_0.2": int(np.sum(pa >= PRESENCE)),
        "detect_neu_0.2": int(np.sum(na >= PRESENCE)),
        "detect_pos_rate_0.2": float(np.mean(pa >= PRESENCE)),
        "detect_neu_rate_0.2": float(np.mean(na >= PRESENCE)),
    }


def measure(out_path: Path) -> dict[str, Any]:
    t0 = time.time()
    summary = json.loads(SUMMARY.read_text())
    wanted = {"seductive", "love", "neutral", "joyful", "crying", "sad"}
    recs = []
    for ln in summary["lines"]:
        if ln.get("group") not in wanted:
            continue
        path = Path(ln["path"])
        print(f"[measure] {ln['persona']} {ln['id']} {ln['group']}", file=sys.stderr)
        detail = score_from_audio(path, ln.get("persona"), ln.get("e2v") or {})
        detail["id"] = ln["id"]
        detail["group"] = ln["group"]
        detail["old_clap_love"] = float((ln.get("clap_probs") or {}).get("love", 0.0))
        detail["old_clap_seductive"] = float((ln.get("clap_probs") or {}).get("seductive", 0.0))
        recs.append(detail)

    method_names = list(recs[0]["methods"].keys()) if recs else []
    evals: dict[str, Any] = {}
    for persona in ("samantha", "jarvis"):
        evals[persona] = {}
        for emotion in ("seductive", "love"):
            pos = [r for r in recs if r["persona"] == persona and r["group"] == emotion]
            neu = [r for r in recs if r["persona"] == persona and r["group"] == "neutral"]
            joy = [r for r in recs if r["persona"] == persona and r["group"] == "joyful"]
            cry = [r for r in recs if r["persona"] == persona and r["group"] == "crying"]
            sad = [r for r in recs if r["persona"] == persona and r["group"] == "sad"]
            block: dict[str, Any] = {
                "n_pos": len(pos),
                "n_neu": len(neu),
                "scored_for_persona": (
                    emotion != "seductive" or persona == "samantha"
                )
                if emotion == "seductive"
                else (persona == "samantha"),
                "note": None,
            }
            # Love is in Samantha's persona list, not Jarvis's (Agent 1 toml).
            # Seductive is Samantha-only. Jarvis numbers are info either way.
            if persona == "jarvis" and emotion == "seductive":
                block["note"] = "info only: seductive is Samantha-only (n/a for Jarvis)"
            if persona == "jarvis" and emotion == "love":
                block["note"] = (
                    "info / male-tender test: own-jarvis.toml excludes love "
                    "(n/a for Jarvis). Probe has the only labeled love lines."
                )
            if persona == "samantha" and emotion == "love":
                block["note"] = (
                    "Samantha has love in her persona list but the probe set "
                    "has no labeled samantha love lines (lov01/lov02 are Jarvis-only)."
                )
            if not pos:
                block["methods"] = {}
                evals[persona][emotion] = block
                continue
            key = "love" if emotion == "love" else "seductive"
            old_key = "old_clap_love" if emotion == "love" else "old_clap_seductive"
            methods_out = {
                "old_clap_prob": _sep([r[old_key] for r in pos], [r[old_key] for r in neu])
            }
            methods_out["old_clap_prob"]["pos_ids"] = {r["id"]: r[old_key] for r in pos}
            methods_out["old_clap_prob"]["neu_ids"] = {r["id"]: r[old_key] for r in neu}
            if joy:
                methods_out["old_clap_prob"]["joy_ids"] = {r["id"]: r[old_key] for r in joy}
                methods_out["old_clap_prob"]["joy_mean"] = float(
                    sum(r[old_key] for r in joy) / len(joy)
                )
            for m in method_names:
                ps = [r["methods"][m][key] for r in pos]
                ns = [r["methods"][m][key] for r in neu]
                ev = _sep(ps, ns)
                ev["pos_ids"] = {r["id"]: r["methods"][m][key] for r in pos}
                ev["neu_ids"] = {r["id"]: r["methods"][m][key] for r in neu}
                if joy:
                    js = [r["methods"][m][key] for r in joy]
                    ev["joy_mean"] = float(sum(js) / len(js))
                    ev["joy_ids"] = {r["id"]: r["methods"][m][key] for r in joy}
                    ev["detect_joy_0.2"] = int(sum(1 for x in js if x >= PRESENCE))
                if cry:
                    cs = [r["methods"][m][key] for r in cry]
                    ev["cry_mean"] = float(sum(cs) / len(cs))
                    ev["cry_ids"] = {r["id"]: r["methods"][m][key] for r in cry}
                    ev["detect_cry_0.2"] = int(sum(1 for x in cs if x >= PRESENCE))
                if sad:
                    ss = [r["methods"][m][key] for r in sad]
                    ev["sad_mean"] = float(sum(ss) / len(ss))
                    ev["sad_ids"] = {r["id"]: r["methods"][m][key] for r in sad}
                    ev["detect_sad_0.2"] = int(sum(1 for x in ss if x >= PRESENCE))
                methods_out[m] = ev
            # chosen method view
            chosen_name = CHOSEN_LOVE if emotion == "love" else CHOSEN_SEDUCTIVE
            block["chosen_method"] = chosen_name
            block["methods"] = methods_out
            evals[persona][emotion] = block

    out = {
        "schema": "galion-love-seductive-separation/1",
        "timing_s": time.time() - t0,
        "presence": PRESENCE,
        "chosen": {"love": CHOSEN_LOVE, "seductive": CHOSEN_SEDUCTIVE},
        "source": SOURCE,
        "model": {
            "id": "laion/clap-htsat-fused",
            "licence": "Apache-2.0",
            "path": str(CLAP_DIR),
        },
        "prompts_path": str(PROMPTS_PATH),
        "records": recs,
        "evals": evals,
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, indent=2))
    print(out_path)

    print("\n===== SEPARATION =====")
    for persona in ("samantha", "jarvis"):
        for emotion in ("seductive", "love"):
            block = evals[persona][emotion]
            print(f"\n--- {persona} {emotion} ---")
            if block.get("note"):
                print("  note:", block["note"])
            if not block.get("methods"):
                print("  no positive probe lines")
                continue
            rows = []
            for name, ev in block["methods"].items():
                if "margin" not in ev:
                    continue
                rows.append((name, ev))
            rows.sort(key=lambda kv: (-(kv[1]["detect_pos_0.2"] - kv[1]["detect_neu_0.2"]), -kv[1]["margin"]))
            for name, ev in rows:
                mark = "  <-- chosen" if name == block.get("chosen_method") or (
                    name == "old_clap_prob" and False
                ) else ""
                if name == block.get("chosen_method"):
                    mark = "  <-- chosen"
                extra = ""
                if "joy_mean" in ev:
                    extra += f" joy={ev['joy_mean']:.6f} detJ={ev.get('detect_joy_0.2', 0)}"
                if "cry_mean" in ev:
                    extra += f" cry={ev['cry_mean']:.6f} detC={ev.get('detect_cry_0.2', 0)}"
                print(
                    f"  {name:44} margin={ev['margin']:+.6f} "
                    f"pos={ev['pos_mean']:.6f} neu={ev['neu_mean']:.6f} "
                    f"gap={ev['gap_minpos_maxneu']:+.6f} "
                    f"detP={ev['detect_pos_0.2']}/{ev['n_pos']} "
                    f"detN={ev['detect_neu_0.2']}/{ev['n_neu']}"
                    f"{extra}{mark}"
                )
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--measure", action="store_true")
    ap.add_argument(
        "--out",
        default=str(HERE / "love_seductive_separation.json"),
    )
    args = ap.parse_args()
    if args.measure:
        measure(Path(args.out))
        return
    ap.error("pass --measure to run the original-voice separation experiment")


if __name__ == "__main__":
    main()
