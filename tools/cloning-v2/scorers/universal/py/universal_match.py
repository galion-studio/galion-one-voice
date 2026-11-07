#!/usr/bin/env python3
"""Universal emotion-match: moments + leaderboard + blend (existing renders only).

Backends:
  - emotion2vec+  → trained 9-class + 768-d emb (joyful/aggressive/disgust/fear/sad/neutral/surprise)
  - EfficientAT   → PRIMARY for yelling / crying / laughter (AudioSet, source label 'efficientat')
  - LAION CLAP    → zero-shot for love / seductive / logic / reason

Formula (documented):
  For each measurable category C with a base moment (strength ≥ presence):
    score_C = 0.55·max_i match_i + 0.45·mean_i match_i
  category_blend = Σ strength_C·score_C / Σ strength_C
  emb_sim = mean_i [ max_j mean_centred_cosine(emb_line_i, emb_window_j) ]
            (EmoCtrlTTS-Eval / Emo-DPO mean-centred cosine on emotion2vec 768-d)
  emotion_match = 0.70·category_blend + 0.30·emb_sim

  match_i for trained-class categories: max(0, 1 − RMS_9(e2v_line, e2v_moment))
  match_i for EfficientAT states: max(0, 1 − |eat_C_line − eat_C_moment|)  (max-pooled)
  match_i for CLAP zero-shot categories: max(0, 1 − |clap_C_line − clap_C_moment|)

Pitch/energy proxies are NOT used in the gate strengths (diagnostics only).
"""
from __future__ import annotations
import argparse, json, os, subprocess, sys, tempfile, time
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "py"))
from score_universal import (  # noqa
    Emotion2VecORT, ClapZeroShot, load_wav, load_wav_16k, mean_centred_cosine, mem_avail_mb, wait_mem, softmax
)

PRESENCE = 0.20  # slightly lower: e2v peaks are softer than SUPERB; EfficientAT AudioSet peaks are mid
E2V_TRAINED = {
    "joyful": "happy",
    "happy": "happy",
    "aggressive": "angry",
    "disgust": "disgusted",
    "fear": "fearful",
    "sad": "sad",
    "neutral": "neutral",
    "surprise": "surprised",
}
EAT_STATES = ["yelling", "crying", "laughter"]
CLAP_ZERO = ["love", "seductive", "logic", "reason"]  # yelling moved to EfficientAT
BLEND_IDS = [
    "joyful", "aggressive", "disgust", "fear", "sad", "neutral", "surprise",
    "love", "seductive", "yelling", "crying", "laughter", "logic",
]  # reason alias of logic

EAT_BIN = ROOT / "bin" / "galion-efficientat"
EAT_BIN_FALLBACK = ROOT / "efficientat" / "target" / "release" / "galion-efficientat"


def eat_binary() -> Path:
    if EAT_BIN.exists():
        return EAT_BIN
    if EAT_BIN_FALLBACK.exists():
        return EAT_BIN_FALLBACK
    raise FileNotFoundError("galion-efficientat binary missing (build scorers/universal/efficientat)")


def source_of(cat: str) -> str:
    if cat in EAT_STATES:
        return "efficientat"
    if cat in ("love", "seductive", "logic", "reason", "yelling") and cat in CLAP_ZERO:
        return "zero-shot text prompt (CLAP)"
    if cat in CLAP_ZERO:
        return "zero-shot text prompt (CLAP)"
    return "trained class (emotion2vec+)"


def strength(cat, e2v_probs, clap_probs, eat_states=None):
    if cat in EAT_STATES:
        eat_states = eat_states or {}
        st = eat_states.get(cat) or {}
        if isinstance(st, dict):
            return float(st.get("max", 0.0) or 0.0)
        return float(st or 0.0)
    if cat in E2V_TRAINED:
        return float(e2v_probs.get(E2V_TRAINED[cat], 0.0))
    if cat in clap_probs:
        return float(clap_probs.get(cat, 0.0))
    return 0.0


def match_trained(a_probs, b_probs, labels):
    va = np.array([a_probs.get(l, 0.0) for l in labels], dtype=np.float64)
    vb = np.array([b_probs.get(l, 0.0) for l in labels], dtype=np.float64)
    d = np.sqrt(np.mean((va - vb) ** 2))
    return float(max(0.0, 1.0 - d))


def match_zeroshot(a, b):
    return float(max(0.0, 1.0 - abs(float(a) - float(b))))


def eat_max(rec, cat):
    eat = rec.get("efficientat") or {}
    st = eat.get(cat) or {}
    if isinstance(st, dict):
        return float(st.get("max", 0.0) or 0.0)
    return float(st or 0.0)


def plan_windows(duration_s, window_s=2.5):
    out = []
    t = 0.0
    while t + 1.2 <= duration_s + 1e-9:
        end = min(t + window_s, duration_s)
        if end - t >= 1.2:
            out.append((t, end))
        t += window_s
    return out


def cut_wav(src, dest, start_s, end_s, sr_out=16000):
    import soundfile as sf
    x, sr = sf.read(src, always_2d=False)
    if getattr(x, "ndim", 1) > 1:
        x = x.mean(-1)
    s = int(round(start_s * sr)); e = int(round(end_s * sr))
    e = min(e, len(x)); s = min(s, e)
    clip = np.asarray(x[s:e], dtype=np.float32)
    dest.parent.mkdir(parents=True, exist_ok=True)
    sf.write(str(dest), clip, sr)


def score_efficientat_batch(wav_paths, out_json: Path, threads=2):
    """Run galion-efficientat score on wavs → map path -> states dict."""
    if not wav_paths:
        return {}
    bin_path = eat_binary()
    out_json.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "nice", "-n", "10",
        str(bin_path), "score",
        "--threads", str(threads),
        "--out", str(out_json),
    ] + [str(p) for p in wav_paths]
    env = os.environ.copy()
    env.setdefault(
        "ORT_DYLIB_PATH",
        "/workspace/swarm/voice-v2-sprint/repo/weights/runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so",
    )
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        raise RuntimeError(f"efficientat failed rc={r.returncode}\n{r.stderr[-2000:]}")
    doc = json.loads(out_json.read_text())
    by = {}
    for f in doc.get("files", []):
        # normalize to resolved path when possible
        p = f.get("file", "")
        try:
            key = str(Path(p).resolve())
        except Exception:
            key = p
        by[key] = f.get("states") or {}
        by[p] = by[key]
        by[Path(p).name] = by[key]
    return by


def attach_efficientat(records, path_key="path", work_dir: Path = None, tag="eat"):
    """Ensure each record has efficientat states; score missing ones via Rust ort binary."""
    need = []
    for rec in records:
        if rec.get("efficientat"):
            continue
        p = rec.get(path_key) or rec.get("file")
        if p:
            need.append(Path(p))
    if not need:
        return records
    work_dir = work_dir or Path(tempfile.mkdtemp(prefix="eat-"))
    work_dir.mkdir(parents=True, exist_ok=True)
    # unique preserve order
    uniq = []
    seen = set()
    for p in need:
        rp = str(p.resolve()) if p.exists() else str(p)
        if rp not in seen:
            seen.add(rp); uniq.append(p)
    outj = work_dir / f"{tag}_efficientat.json"
    by = score_efficientat_batch(uniq, outj)
    for rec in records:
        if rec.get("efficientat"):
            continue
        p = rec.get(path_key) or rec.get("file")
        if not p:
            continue
        pp = Path(p)
        st = by.get(str(pp.resolve()) if pp.exists() else str(pp)) or by.get(str(pp)) or by.get(pp.name)
        if st is None:
            raise RuntimeError(f"efficientat missing score for {p}")
        rec["efficientat"] = st
    return records


def score_path(e2v, clap, path):
    audio16 = load_wav(path, 16000)
    rec = {"path": str(path), "e2v": e2v.score(audio16) if e2v else None}
    if clap is not None:
        rec["clap"] = clap.score(load_wav(path, 48000), sr=48000)
    return rec


def find_moments(windows):
    """windows: list of dicts with e2v/clap/efficientat and start/end."""
    cats = [
        "joyful", "aggressive", "disgust", "fear", "sad", "neutral", "surprise",
        "love", "seductive", "yelling", "crying", "laughter", "logic",
    ]
    moments = []
    for cat in cats:
        best = None
        for w in windows:
            e2v_p = (w.get("e2v") or {}).get("probs") or {}
            clap_p = (w.get("clap") or {}).get("probs") or {}
            eat = w.get("efficientat") or {}
            s = strength(cat, e2v_p, clap_p, eat)
            if s + 1e-12 >= PRESENCE and (best is None or s > best[0]):
                best = (s, w)
        if best:
            moments.append({"category": cat, "strength": best[0], "window": best[1], "source": source_of(cat)})
    return moments


def score_set(lines, moments, e2v_labels):
    lb = []
    for m in moments:
        cat = m["category"]
        matches = []
        for ln in lines:
            src = m["source"]
            if src == "efficientat":
                sc = match_zeroshot(eat_max(ln, cat), eat_max(m["window"], cat))
            elif src.startswith("trained"):
                sc = match_trained(ln["e2v"]["probs"], m["window"]["e2v"]["probs"], e2v_labels)
            else:
                sc = match_zeroshot(
                    (ln.get("clap") or {}).get("probs", {}).get(cat, 0.0),
                    (m["window"].get("clap") or {}).get("probs", {}).get(cat, 0.0),
                )
            matches.append((ln["id"], sc))
        matches.sort(key=lambda x: -x[1])
        best = matches[0]
        mean = sum(x[1] for x in matches) / len(matches)
        score = 0.55 * best[1] + 0.45 * mean
        lb.append({
            "category": cat, "source": m["source"], "strength": round(m["strength"], 4),
            "moment": f"{m['window']['start_s']:.3f}-{m['window']['end_s']:.3f}",
            "score": round(score, 4), "best_line": best[0], "best_match": round(best[1], 4),
            "mean_match": round(mean, 4),
        })
    num = den = 0.0
    for c in lb:
        if c["category"] not in BLEND_IDS:
            continue
        w = max(c["strength"], 1e-12)
        num += w * c["score"]; den += w
    cat_blend = num / den if den > 0 else None
    emb_scores = []
    win_embs = [np.array(w["e2v"]["embedding"], dtype=np.float64) for w in
                [m["window"] for m in moments] if w.get("e2v") and w["e2v"].get("embedding")]
    all_win_embs = win_embs
    for ln in lines:
        if not ln.get("e2v") or not ln["e2v"].get("embedding"):
            continue
        le = np.array(ln["e2v"]["embedding"], dtype=np.float64)
        if not all_win_embs:
            continue
        emb_scores.append(max(mean_centred_cosine(le, we) for we in all_win_embs))
    emb_sim = float(np.mean(emb_scores)) if emb_scores else None
    emotion_match = None
    if cat_blend is not None and emb_sim is not None:
        emotion_match = 0.70 * cat_blend + 0.30 * emb_sim
    elif cat_blend is not None:
        emotion_match = cat_blend
    lb.sort(key=lambda x: -x["score"])
    return emotion_match, cat_blend, emb_sim, lb


def emb_sim_vs_windows(lines, windows):
    win_embs = [np.array(w["e2v"]["embedding"], dtype=np.float64) for w in windows if w.get("e2v")]
    scores = []
    for ln in lines:
        if not ln.get("e2v"):
            continue
        le = np.array(ln["e2v"]["embedding"], dtype=np.float64)
        scores.append(max(mean_centred_cosine(le, we) for we in win_embs))
    return float(np.mean(scores)) if scores else None


def _nebula_field(persona: str, reference_recs, rendered_recs):
    """Offline distribution report. Not an input to emotion_match."""
    py = Path(__file__).resolve().parents[2] / "persona" / "py"
    if str(py) not in sys.path:
        sys.path.insert(0, str(py))
    from nebula import field_for_clips

    return field_for_clips(
        samantha=(persona == "samantha"),
        reference_recs=reference_recs,
        rendered_recs=rendered_recs,
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--persona", required=True, choices=["samantha", "jarvis"])
    ap.add_argument("--original", required=True)
    ap.add_argument("--renders", action="append", default=[], help="name=dir with t01.wav...")
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--window-s", type=float, default=2.5)
    ap.add_argument("--forced-window", default=None, help="START-END")
    ap.add_argument("--skip-clap", action="store_true")
    ap.add_argument("--skip-efficientat", action="store_true")
    args = ap.parse_args()

    work = Path(args.work); work.mkdir(parents=True, exist_ok=True)
    wait_mem(3000)
    e2v = Emotion2VecORT(threads=2)
    clap = None if args.skip_clap else ClapZeroShot()

    import soundfile as sf
    x, sr = sf.read(args.original, always_2d=False)
    dur = len(x) / sr
    spans = plan_windows(dur, args.window_s)
    if args.forced_window:
        a, b = args.forced_window.split("-")
        spans.append((float(a), float(b)))
    uniq = []
    for s, e in spans:
        if not any(abs(s - u[0]) < 0.05 and abs(e - u[1]) < 0.05 for u in uniq):
            uniq.append((s, e))
    spans = uniq

    wdir = work / "windows"; wdir.mkdir(exist_ok=True)
    windows = []
    cache = work / "base_windows_universal.json"
    if cache.exists():
        windows = json.loads(cache.read_text())
        print(f"[cache] loaded {len(windows)} windows", file=sys.stderr)
    else:
        for i, (s, e) in enumerate(spans):
            wait_mem(2500)
            dest = wdir / f"w{i:02d}_{s:.3f}_{e:.3f}.wav"
            if not dest.exists():
                cut_wav(args.original, dest, s, e)
            print(f"[base] {dest.name}", file=sys.stderr)
            rec = score_path(e2v, clap, dest)
            rec["id"] = f"w{i:02d}"; rec["start_s"] = s; rec["end_s"] = e
            windows.append(rec)
        cache.write_text(json.dumps(windows))

    if not args.skip_efficientat:
        attach_efficientat(windows, path_key="path", work_dir=work, tag="base")
        # rewrite cache with efficientat attached (keep e2v/clap)
        cache.write_text(json.dumps(windows))

    moments = find_moments(windows)
    e2v_labels = list(e2v.labels) if e2v else []

    ceil_lines = []
    for w in windows:
        ceil_lines.append({
            "id": w["id"], "e2v": w["e2v"], "clap": w.get("clap"),
            "efficientat": w.get("efficientat"),
        })
    emotion_match_c, cat_c, emb_c, lb_c = score_set(ceil_lines, moments, e2v_labels)
    emb_c = emb_sim_vs_windows(ceil_lines, windows)
    if cat_c is not None and emb_c is not None:
        emotion_match_c = 0.70 * cat_c + 0.30 * emb_c

    results = {
        "persona": args.persona,
        "formula": (
            "emotion_match = 0.70*category_blend + 0.30*emb_sim; "
            "category_blend = strength-weighted mean of per-emotion scores; "
            "score_C = 0.55*max_i match_i + 0.45*mean_i match_i; "
            "trained match = max(0,1-RMS9(e2v)); "
            "efficientat match = max(0,1-|Δeat_C|) for yelling/crying/laughter; "
            "zeroshot match = max(0,1-|Δclap_C|) for love/seductive/logic; "
            "emb_sim = mean_i max_j mean_centred_cosine(emb_i, emb_window_j) (EmoCtrlTTS-Eval/Emo-DPO); "
            "no pitch/energy proxies in gate"
        ),
        "presence": PRESENCE,
        "backends": {
            "emotion2vec+": "trained class",
            "efficientat": {
                "role": "PRIMARY for yelling/crying/laughter",
                "model": "Chris-Kode/efficientat-mn10-as-onnx",
                "url": "https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx",
                "licence": "MIT",
                "sha256": "bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504",
                "attribution": "EfficientAT mn10_as © 2022 Florian Schmid; ONNX export Chris-Kode",
            },
            "clap": "zero-shot for love/seductive/logic/reason",
        },
        "moments": [{"category": m["category"], "source": m["source"], "strength": round(m["strength"], 4),
                     "span": f"{m['window']['start_s']:.3f}-{m['window']['end_s']:.3f}",
                     "e2v_top": (m["window"].get("e2v") or {}).get("top"),
                     "clap_top": (m["window"].get("clap") or {}).get("top"),
                     "efficientat": {
                         k: round(float((m["window"].get("efficientat") or {}).get(k, {}).get("max", 0.0)), 4)
                         for k in EAT_STATES
                     } if m["window"].get("efficientat") else None,
                     } for m in moments],
        "BASE-self-ceiling": {
            "emotion_match": emotion_match_c,
            "nebula": _nebula_field(args.persona, windows, windows),
            "category_blend": cat_c, "emb_sim": emb_c,
            "leaderboard": lb_c,
        },
        "sets": {},
    }

    for spec in args.renders:
        name, directory = spec.split("=", 1)
        d = Path(directory)
        lines = []
        scache = work / f"lines_{name}.json"
        if scache.exists():
            lines = json.loads(scache.read_text())
            print(f"[cache] lines {name} n={len(lines)}", file=sys.stderr)
        else:
            wavs = sorted(d.glob("t*.wav"))
            for w in wavs:
                wait_mem(2500)
                print(f"[{name}] {w.name}", file=sys.stderr)
                rec = score_path(e2v, clap, w)
                rec["id"] = w.stem
                lines.append(rec)
            scache.write_text(json.dumps(lines))
        if not args.skip_efficientat:
            attach_efficientat(lines, path_key="path", work_dir=work, tag=f"lines_{name}")
            scache.write_text(json.dumps(lines))
        em, cat_b, emb_s, lb = score_set(lines, moments, e2v_labels)
        emb_s = emb_sim_vs_windows(lines, windows)
        if cat_b is not None and emb_s is not None:
            em = 0.70 * cat_b + 0.30 * emb_s
        results["sets"][name] = {
            "n": len(lines),
            "emotion_match": em,
            "nebula": _nebula_field(args.persona, windows, lines),
            "category_blend": cat_b, "emb_sim": emb_s,
            "leaderboard": lb, "renders": str(d),
        }
        print(f"=== {name} emotion_match={em} cat={cat_b} emb={emb_s}", file=sys.stderr)

    Path(args.out).write_text(json.dumps(results, indent=2))
    print(args.out)


if __name__ == "__main__":
    main()
