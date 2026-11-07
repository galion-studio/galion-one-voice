#!/usr/bin/env python3
"""Prompt-ensemble experiment for love/seductive (no training, probe wavs only)."""
from __future__ import annotations
import json, sys, time
from pathlib import Path
import numpy as np

UNI = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/universal")
CLAP_DIR = UNI / "clap" / "laion-clap-htsat-fused"
SUMMARY = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/scorer-probe/results/SUMMARY.json")
OUT = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/persona/experiments/prompt_trial.json")

BANKS = {
    "old_love": [
        "a tender loving voice",
        "a warm affectionate speaking voice",
        "someone speaking with love and care",
    ],
    "old_seductive": [
        "a seductive flirtatious voice",
        "a soft sensual speaking voice",
        "someone speaking in a seductive tone",
    ],
    "new_love": [
        "a tender loving voice",
        "a warm affectionate speaking voice",
        "someone speaking with love and care",
        "a man speaking with love and affection",
        "a tender male voice speaking lovingly",
        "a warm male voice saying I love you",
        "a woman speaking with love and affection",
        "a whispered affectionate voice",
        "someone whispering words of love",
        "a soft whispered loving voice",
        "a low gentle male voice full of love",
        "an intimate loving speaking voice",
    ],
    "new_seductive": [
        "a seductive flirtatious voice",
        "a soft sensual speaking voice",
        "someone speaking in a seductive tone",
        "a man speaking in a seductive whisper",
        "a low sensual male voice",
        "a flirtatious male whisper",
        "a woman whispering seductively",
        "a breathy feminine seductive voice",
        "a whispered seductive voice",
        "someone whispering seductively",
        "a soft breathy whispered sensual voice",
        "a low intimate whispered speaking voice",
    ],
    "neg_neutral": [
        "a person speaking in a neutral tone",
        "a calm ordinary speaking voice",
        "someone speaking normally without emotion",
        "a plain conversational voice",
        "a man speaking in a neutral conversational voice",
        "a woman speaking in a neutral conversational voice",
    ],
}

def load_wav(path, target_sr):
    import soundfile as sf
    x, sr = sf.read(path, always_2d=False)
    if getattr(x, "ndim", 1) > 1:
        x = x.mean(axis=-1)
    x = np.asarray(x, dtype=np.float32)
    if sr != target_sr:
        n = int(round(len(x) * target_sr / sr))
        xp = np.linspace(0, 1, len(x), endpoint=False)
        tt = np.linspace(0, 1, n, endpoint=False)
        x = np.interp(tt, xp, x).astype(np.float32)
    return x

def sigmoid(x):
    return 1.0 / (1.0 + np.exp(-x))

def softmax2(a, b, temp=20.0):
    m = max(a, b)
    ea = np.exp((a - m) * temp)
    eb = np.exp((b - m) * temp)
    return float(ea / (ea + eb))

def mean_sep(pos, neu):
    if not pos or not neu:
        return None
    mp, mn = float(np.mean(pos)), float(np.mean(neu))
    return {
        "pos_mean": mp,
        "neu_mean": mn,
        "delta": mp - mn,
        "pos_min": float(np.min(pos)),
        "neu_max": float(np.max(neu)),
        "gap_minpos_maxneu": float(np.min(pos) - np.max(neu)),
        "n_pos": len(pos),
        "n_neu": len(neu),
        "detect_pos_0.2": sum(1 for x in pos if x >= 0.2) / len(pos),
        "detect_neu_0.2": sum(1 for x in neu if x >= 0.2) / len(neu),
    }

def main():
    t0 = time.time()
    import torch
    from transformers import ClapModel, ClapProcessor
    print("[load] CLAP", file=sys.stderr)
    processor = ClapProcessor.from_pretrained(str(CLAP_DIR), local_files_only=True)
    model = ClapModel.from_pretrained(str(CLAP_DIR), local_files_only=True)
    model.eval()
    text_emb = {}
    with torch.no_grad():
        for name, prompts in BANKS.items():
            inputs = processor(text=prompts, return_tensors="pt", padding=True)
            out = model.get_text_features(
                input_ids=inputs["input_ids"],
                attention_mask=inputs.get("attention_mask"),
            )
            emb = out.pooler_output
            emb = emb.mean(dim=0, keepdim=True)
            emb = emb / emb.norm(dim=-1, keepdim=True)
            text_emb[name] = emb
    summary = json.loads(SUMMARY.read_text())
    wanted_groups = {"seductive", "love", "neutral", "joyful", "crying", "sad"}
    recs = []
    for ln in summary["lines"]:
        if ln["group"] not in wanted_groups:
            continue
        path = ln["path"]
        print(f"[audio] {ln['persona']} {ln['id']}", file=sys.stderr)
        audio = load_wav(path, 48000)
        with torch.no_grad():
            inputs = processor(audio=audio, sampling_rate=48000, return_tensors="pt")
            kw = {"input_features": inputs["input_features"]}
            if "is_longer" in inputs and inputs["is_longer"] is not None:
                kw["is_longer"] = inputs["is_longer"]
            a = model.get_audio_features(**kw).pooler_output
        sims = {name: float((a @ te.T).squeeze().item()) for name, te in text_emb.items()}
        recs.append({
            "persona": ln["persona"],
            "id": ln["id"],
            "group": ln["group"],
            "old_clap_love": ln["clap_probs"]["love"],
            "old_clap_sed": ln["clap_probs"]["seductive"],
            "e2v_happy": ln["e2v"]["happy"],
            "e2v_sad": ln["e2v"]["sad"],
            "sims": sims,
        })

    # methods
    methods = {}
    for rec in recs:
        s = rec["sims"]
        rec["methods"] = {
            "old_clap_love": rec["old_clap_love"],
            "old_clap_sed": rec["old_clap_sed"],
            "new_love_raw": s["new_love"],
            "new_sed_raw": s["new_seductive"],
            "old_love_raw": s["old_love"],
            "old_sed_raw": s["old_seductive"],
            "love_contrast20": softmax2(s["new_love"], s["neg_neutral"], 20.0),
            "sed_contrast20": softmax2(s["new_seductive"], s["neg_neutral"], 20.0),
            "love_contrast10": softmax2(s["new_love"], s["neg_neutral"], 10.0),
            "sed_contrast10": softmax2(s["new_seductive"], s["neg_neutral"], 10.0),
            "love_contrast40": softmax2(s["new_love"], s["neg_neutral"], 40.0),
            "sed_contrast40": softmax2(s["new_seductive"], s["neg_neutral"], 40.0),
            "love_sig15": float(sigmoid((s["new_love"] - s["neg_neutral"]) * 15.0)),
            "sed_sig15": float(sigmoid((s["new_seductive"] - s["neg_neutral"]) * 15.0)),
            "love_or_happy": max(softmax2(s["new_love"], s["neg_neutral"], 20.0), rec["e2v_happy"]),
            "love_blend": 0.55 * softmax2(s["new_love"], s["neg_neutral"], 20.0) + 0.45 * rec["e2v_happy"],
            # combined: clap contrast OR (happy AND contrast not worse than chance)
            "love_or_happy_if_warm": (
                max(softmax2(s["new_love"], s["neg_neutral"], 20.0), rec["e2v_happy"])
                if rec["e2v_happy"] >= 0.2 or softmax2(s["new_love"], s["neg_neutral"], 20.0) >= 0.2
                else softmax2(s["new_love"], s["neg_neutral"], 20.0)
            ),
        }

    # evaluate separation
    evals = {}
    for persona in ("samantha", "jarvis"):
        evals[persona] = {}
        groups_for = {
            "seductive": ("seductive", "neutral"),
            "love": ("love", "neutral"),
        }
        method_pairs = {
            "seductive": [
                "old_clap_sed", "old_sed_raw", "new_sed_raw",
                "sed_contrast20", "sed_contrast10", "sed_contrast40", "sed_sig15",
            ],
            "love": [
                "old_clap_love", "old_love_raw", "new_love_raw",
                "love_contrast20", "love_contrast10", "love_contrast40", "love_sig15",
                "love_or_happy", "love_blend", "love_or_happy_if_warm",
            ],
        }
        for emotion, (pg, ng) in groups_for.items():
            pos = [r for r in recs if r["persona"] == persona and r["group"] == pg]
            neu = [r for r in recs if r["persona"] == persona and r["group"] == ng]
            joy = [r for r in recs if r["persona"] == persona and r["group"] == "joyful"]
            if not pos:
                evals[persona][emotion] = {"note": "no probe lines"}
                continue
            evals[persona][emotion] = {"n_pos": len(pos), "n_neu": len(neu)}
            for m in method_pairs[emotion]:
                ps = [r["methods"][m] for r in pos]
                ns = [r["methods"][m] for r in neu]
                js = [r["methods"][m] for r in joy] if joy and m.startswith("love") else None
                ev = mean_sep(ps, ns)
                ev["pos_ids"] = {r["id"]: r["methods"][m] for r in pos}
                ev["neu_ids"] = {r["id"]: r["methods"][m] for r in neu}
                if js is not None:
                    ev["joy_mean"] = float(np.mean(js))
                    ev["joy_ids"] = {r["id"]: r["methods"][m] for r in joy}
                    ev["joy_detect_0.2"] = sum(1 for x in js if x >= 0.2) / len(js)
                evals[persona][emotion][m] = ev

    out = {
        "timing_s": time.time() - t0,
        "banks": BANKS,
        "records": recs,
        "evals": evals,
    }
    OUT.write_text(json.dumps(out, indent=2))
    print(OUT)

    # printable ranking: pick methods with largest delta and positive gap
    print("\n===== SEPARATION RANKING =====")
    for persona in ("samantha", "jarvis"):
        for emotion in ("seductive", "love"):
            block = evals[persona].get(emotion, {})
            print(f"\n--- {persona} {emotion} ---")
            if "note" in block:
                print(block["note"])
                continue
            rows = []
            for k, v in block.items():
                if not isinstance(v, dict) or "delta" not in v:
                    continue
                rows.append((k, v))
            rows.sort(key=lambda kv: -kv[1]["delta"])
            for k, v in rows:
                print(
                    f"  {k:22} delta={v['delta']:+.4f} pos={v['pos_mean']:.4f} neu={v['neu_mean']:.4f} "
                    f"gap={v['gap_minpos_maxneu']:+.4f} detP={v['detect_pos_0.2']:.2f} detN={v['detect_neu_0.2']:.2f}"
                    + (f" joy={v.get('joy_mean'):.4f}" if "joy_mean" in v else "")
                )

if __name__ == "__main__":
    main()
