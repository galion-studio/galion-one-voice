#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# emotion_match = 0.70*category_blend+0.30*emb_sim
#
# category_blend is the histogram intersection of the categorical posterior
# (softmax of the classifier log-probabilities) between the reference and a clip.
# emb_sim is the cosine of the L2-normalised utterance embeddings from the same
# open SpeechBrain encoder. Both are means over the clips. The weights above are
# applied in emotion_match(); this process does not invent a constant.
#
# Weights: speechbrain/emotion-recognition-wav2vec2-IEMOCAP (Apache-2.0).
# Cached outside the git checkout. CPU only.

import os
import sys

FORMULA = "0.70*category_blend+0.30*emb_sim"
CATEGORY_W = 0.70
EMB_W = 0.30


def _reexec():
    try:
        import torch  # noqa: F401
        return
    except ImportError:
        pass
    cand = os.environ.get("GALION_PYTHON", "/home/galion/workspace/galion/venv/bin/python")
    if os.path.isfile(cand) and os.path.realpath(cand) != os.path.realpath(sys.executable):
        os.execv(cand, [cand, *sys.argv])


_reexec()

import json  # noqa: E402

import torch  # noqa: E402
import torchaudio  # noqa: E402


def emotion_match(category_blend, emb_sim):
    """Evaluate 0.70*category_blend+0.30*emb_sim."""
    return CATEGORY_W * category_blend + EMB_W * emb_sim


def usage(code):
    sys.stdout.write(
        "emotion_match — 0.70*category_blend+0.30*emb_sim\n"
        "usage: emotion_match --ref REF.wav clip.wav... [--threads N] [--out out.json]\n"
        "       emotion_match --self-test\n"
    )
    raise SystemExit(code)


def self_test():
    got = emotion_match(0.9, 0.8)
    expect = 0.70 * 0.9 + 0.30 * 0.8
    if abs(got - expect) > 1e-12 or FORMULA != "0.70*category_blend+0.30*emb_sim":
        sys.stderr.write(f"self-test failed got={got} expect={expect}\n")
        raise SystemExit(1)
    sys.stdout.write(f"ok {got}\n")
    raise SystemExit(0)


def parse(argv):
    if "--help" in argv or "-h" in argv:
        usage(0)
    if "--self-test" in argv:
        self_test()
    ref = None
    out = None
    threads = 2
    files = []
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
        elif a.startswith("--"):
            sys.stderr.write(f"unknown flag {a}\n")
            usage(2)
        else:
            files.append(a)
        i += 1
    if not ref or not files:
        usage(2)
    return ref, files, out, max(1, min(threads, 2))


def load16(path):
    wav, sr = torchaudio.load(path)
    if wav.shape[0] > 1:
        wav = wav.mean(dim=0, keepdim=True)
    wav = wav.squeeze(0)
    if sr != 16000:
        wav = torchaudio.functional.resample(wav, sr, 16000)
    return wav.contiguous()


def as_prob(row):
    row = row.detach().float().cpu().reshape(-1)
    total = float(row.sum())
    if torch.any(row < -1e-5) or abs(total - 1.0) > 1e-2:
        row = torch.softmax(row, dim=0)
    else:
        row = row / row.sum().clamp_min(1e-12)
    return row


def as_unit(emb):
    e = emb.detach().float().cpu().reshape(-1)
    return e / e.norm().clamp_min(1e-12)


def load_classifier(threads):
    torch.set_num_threads(threads)
    try:
        torch.set_num_interop_threads(1)
    except RuntimeError:
        pass
    os.environ.setdefault("CUDA_VISIBLE_DEVICES", "")
    savedir = os.environ.get(
        "GALION_EMOTION_MODEL",
        "/home/galion/workspace/galion/cache/hf/speechbrain/emotion-recognition-wav2vec2-IEMOCAP",
    )
    os.makedirs(savedir, exist_ok=True)
    from speechbrain.inference.interfaces import foreign_class

    return foreign_class(
        source="speechbrain/emotion-recognition-wav2vec2-IEMOCAP",
        pymodule_file="custom_interface.py",
        classname="CustomEncoderWav2vec2Classifier",
        savedir=savedir,
        run_opts={"device": "cpu"},
    )


def embed_and_prob(clf, wav):
    batch = wav.unsqueeze(0)
    if not hasattr(clf, "encode_batch"):
        raise SystemExit("emotion model has no encode_batch")
    emb = clf.encode_batch(batch)
    out_prob, _score, _index, _text = clf.classify_batch(batch)
    return as_prob(out_prob), as_unit(emb)


def intersection(p, q):
    n = min(p.numel(), q.numel())
    return float(torch.minimum(p[:n], q[:n]).sum())


def cosine(a, b):
    n = min(a.numel(), b.numel())
    return float(torch.dot(a[:n], b[:n]))


def main():
    ref, files, out, threads = parse(sys.argv[1:])
    clf = load_classifier(threads)
    pref, eref = embed_and_prob(clf, load16(ref))
    rows = []
    blends = []
    sims = []
    matches = []
    for path in files:
        p, e = embed_and_prob(clf, load16(path))
        cb = intersection(pref, p)
        es = cosine(eref, e)
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
        "model": "speechbrain/emotion-recognition-wav2vec2-IEMOCAP",
        "licence": "Apache-2.0",
        "device": "cpu",
        "threads": threads,
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


if __name__ == "__main__":
    main()
