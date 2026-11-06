#!/usr/bin/env python3
"""Universal emotion scorer: emotion2vec+ (ORT) + LAION CLAP zero-shot (transformers).

Outputs galion-compatible JSON with:
  files[].e2v: {probs, top, embedding[768]}
  files[].clap: {label -> score (softmax over prompt-bank labels)}
  files[].ser: mapped 4-class from e2v for legacy readers (optional)
"""
from __future__ import annotations
import argparse, json, os, sys, time
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
E2V_DIR = ROOT / "emotion2vec"
CLAP_DIR = ROOT / "clap" / "laion-clap-htsat-fused"
PROMPTS = ROOT / "prompts" / "prompt_bank.json"

EAT_BIN = ROOT / "bin" / "galion-efficientat"
EAT_BIN_FALLBACK = ROOT / "efficientat" / "target" / "release" / "galion-efficientat"

def score_efficientat_files(paths, threads=2):
    """Call Rust galion-efficientat; return {resolved_path: states}."""
    import json, os, subprocess, tempfile
    bin_path = EAT_BIN if EAT_BIN.exists() else EAT_BIN_FALLBACK
    if not bin_path.exists():
        return None
    out = Path(tempfile.mkstemp(prefix="eat-", suffix=".json")[1])
    cmd = ["nice","-n","10", str(bin_path), "score", "--threads", str(threads), "--out", str(out)] + [str(x) for x in paths]
    env = os.environ.copy()
    env.setdefault("ORT_DYLIB_PATH",
        "/workspace/swarm/voice-v2-sprint/repo/weights/runtime/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so")
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        print(f"[efficientat] warn: rc={r.returncode} {r.stderr[-500:]}", file=sys.stderr)
        return None
    doc = json.loads(out.read_text()); out.unlink(missing_ok=True)
    by = {}
    for f in doc.get("files", []):
        fp = f.get("file","")
        try: key = str(Path(fp).resolve())
        except Exception: key = fp
        by[key] = f.get("states") or {}
        by[fp] = by[key]
    return by

E2V_LABELS = ["angry", "disgusted", "fearful", "happy", "neutral", "other", "sad", "surprised", "unknown"]

def mem_avail_mb() -> int:
    try:
        for line in open("/proc/meminfo"):
            if line.startswith("MemAvailable"):
                return int(line.split()[1]) // 1024
    except Exception:
        return 0
    return 0

def wait_mem(min_mb=3000):
    while mem_avail_mb() < min_mb:
        print(f"[wait] MemAvailable={mem_avail_mb()} MB < {min_mb}", file=sys.stderr)
        time.sleep(10)

def load_wav(path: str, target_sr: int) -> np.ndarray:
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

def load_wav_16k(path: str) -> np.ndarray:
    return load_wav(path, 16000)

def softmax(x: np.ndarray) -> np.ndarray:
    e = np.exp(x - np.max(x))
    return e / e.sum()

class Emotion2VecORT:
    def __init__(self, threads=2):
        import onnxruntime as ort
        so = ort.SessionOptions()
        so.intra_op_num_threads = threads
        so.inter_op_num_threads = 1
        self.sess = ort.InferenceSession(
            str(E2V_DIR / "emotion2vec_plus_base.onnx"),
            sess_options=so,
            providers=["CPUExecutionProvider"],
        )
        self.inp = self.sess.get_inputs()[0].name
        head = json.loads((E2V_DIR / "emotion2vec_head.json").read_text())
        self.W = np.array(head["weight"], dtype=np.float32)
        self.B = np.array(head["bias"], dtype=np.float32)
        self.labels = list(head["labels"])

    def score(self, audio: np.ndarray):
        feats = self.sess.run(None, {self.inp: audio.reshape(1, -1)})[0]  # [1,T,768]
        pooled = feats[0].mean(axis=0).astype(np.float32)
        logits = self.W @ pooled + self.B
        probs = softmax(logits)
        return {
            "probs": {lab: float(p) for lab, p in zip(self.labels, probs)},
            "top": self.labels[int(np.argmax(probs))],
            "top_conf": float(np.max(probs)),
            "embedding": pooled.tolist(),
            "embedding_dim": int(pooled.shape[0]),
            "source": "trained class (emotion2vec+)",
            "model": "emotion2vec/emotion2vec_plus_base (ONNX ziyu12345 export)",
        }

class ClapZeroShot:
    def __init__(self, device="cpu"):
        import torch
        from transformers import ClapModel, ClapProcessor
        self.torch = torch
        self.processor = ClapProcessor.from_pretrained(str(CLAP_DIR), local_files_only=True)
        self.model = ClapModel.from_pretrained(str(CLAP_DIR), local_files_only=True)
        self.model.eval()
        self.device = device
        self.model.to(device)
        bank = json.loads(PROMPTS.read_text())
        self.bank = bank["labels"]
        # Precompute text embeddings (mean over paraphrases)
        self.text_emb = {}
        with torch.no_grad():
            for lab, prompts in self.bank.items():
                inputs = self.processor(text=prompts, return_tensors="pt", padding=True)
                # transformers≥5: get_text_features returns BaseModelOutputWithPooling;
                # normalized projection lives in .pooler_output
                out = self.model.get_text_features(
                    input_ids=inputs["input_ids"].to(device),
                    attention_mask=inputs.get("attention_mask"),
                )
                emb = out.pooler_output  # [N,D] already L2-normalized
                self.text_emb[lab] = emb.mean(dim=0, keepdim=True)
                self.text_emb[lab] = self.text_emb[lab] / self.text_emb[lab].norm(dim=-1, keepdim=True)

    def score(self, audio_48k: np.ndarray, sr=48000):
        torch = self.torch
        with torch.no_grad():
            inputs = self.processor(audio=audio_48k, sampling_rate=sr, return_tensors="pt")
            kw = {"input_features": inputs["input_features"].to(self.device)}
            if "is_longer" in inputs and inputs["is_longer"] is not None:
                kw["is_longer"] = inputs["is_longer"].to(self.device)
            out = self.model.get_audio_features(**kw)
            a = out.pooler_output  # [1,D] normalized
            sims = {}
            for lab, te in self.text_emb.items():
                sims[lab] = float((a @ te.T).squeeze().item())
            # softmax over labels for comparable probs
            keys = list(sims.keys())
            logits = np.array([sims[k] for k in keys], dtype=np.float64)
            # temperature: CLAP cosine often in [-0.1,0.4]; scale up
            probs = softmax(logits * 20.0)
            out_probs = {k: float(p) for k, p in zip(keys, probs)}
            return {
                "cosine": sims,
                "probs": out_probs,
                "top": keys[int(np.argmax(probs))],
                "source": "zero-shot text prompt (CLAP)",
                "model": "laion/clap-htsat-fused (Apache-2.0)",
                "prompts": self.bank,
            }

def mean_centred_cosine(a: np.ndarray, b: np.ndarray) -> float:
    """EmoCtrlTTS-Eval / Emo-DPO style: subtract mean dim, then cosine."""
    a = np.asarray(a, dtype=np.float64)
    b = np.asarray(b, dtype=np.float64)
    a = a - a.mean()
    b = b - b.mean()
    na = np.linalg.norm(a); nb = np.linalg.norm(b)
    if na < 1e-12 or nb < 1e-12:
        return 0.0
    return float(np.dot(a, b) / (na * nb))

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("wavs", nargs="+")
    ap.add_argument("--out", required=True)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--ref", default=None, help="reference wav for emb cosine + vs_ref")
    ap.add_argument("--skip-clap", action="store_true")
    ap.add_argument("--skip-e2v", action="store_true")
    ap.add_argument("--skip-efficientat", action="store_true")
    args = ap.parse_args()

    wait_mem(3000)
    t0 = time.time()
    e2v = None if args.skip_e2v else Emotion2VecORT(threads=args.threads)
    clap = None
    if not args.skip_clap:
        wait_mem(3000)
        clap = ClapZeroShot()

    ref_e2v = None
    if args.ref and e2v is not None:
        ref_e2v = e2v.score(load_wav(args.ref, 16000))

    files = []
    for p in args.wavs:
        wait_mem(2500)
        audio16 = load_wav_16k(p)
        rec = {"file": str(Path(p).resolve()), "duration_s": float(len(audio16) / 16000.0)}
        if e2v is not None:
            rec["e2v"] = e2v.score(audio16)
            # legacy 4-class projection for old readers
            pr = rec["e2v"]["probs"]
            rec["ser"] = {
                "probs": {
                    "ang": pr.get("angry", 0.0),
                    "hap": pr.get("happy", 0.0),
                    "neu": pr.get("neutral", 0.0) + 0.5 * pr.get("other", 0.0) + 0.5 * pr.get("unknown", 0.0),
                    "sad": pr.get("sad", 0.0),
                },
                "top": rec["e2v"]["top"],
                "note": "legacy 4-class projection from emotion2vec+; prefer e2v.probs",
            }
            if ref_e2v is not None:
                cos = mean_centred_cosine(rec["e2v"]["embedding"], ref_e2v["embedding"])
                rec["e2v"]["emb_cosine_mean_centred_vs_ref"] = cos
        if clap is not None:
            audio48 = load_wav(p, 48000)
            rec["clap"] = clap.score(audio48, sr=48000)
        files.append(rec)

    eat_by = None
    if not args.skip_efficientat:
        eat_by = score_efficientat_files([Path(p) for p in args.wavs], threads=args.threads)
        if eat_by:
            for rec in files:
                key = rec["file"]
                st = eat_by.get(key) or eat_by.get(str(Path(key)))
                if st is not None:
                    rec["efficientat"] = st
                    rec["efficientat_source"] = "efficientat"

    out = {
        "schema": "galion-universal-emotion/1",
        "backend": "universal",
        "models": {
            "emotion2vec+": {
                "id": "emotion2vec/emotion2vec_plus_base",
                "onnx": "ziyu12345/emotion2vec_plus_base_onnx",
                "licence": "FunASR Model License 1.1 (commercial OK w/ attribution)",
                "url": "https://huggingface.co/emotion2vec/emotion2vec_plus_base",
                "classes": E2V_LABELS,
                "skipped": args.skip_e2v,
            },
            "efficientat": {
                "id": "Chris-Kode/efficientat-mn10-as-onnx",
                "licence": "MIT",
                "url": "https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx",
                "sha256": "bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504",
                "attribution": "EfficientAT mn10_as © 2022 Florian Schmid; ONNX export Chris-Kode",
                "role": "PRIMARY for yelling/crying/laughter",
                "skipped": args.skip_efficientat or eat_by is None,
            },
            "clap": {
                "id": "laion/clap-htsat-fused",
                "licence": "Apache-2.0",
                "url": "https://huggingface.co/laion/clap-htsat-fused",
                "note": "Fallback for MS-CLAP (ms-pl OK) due to py3.13/numpy pin; love/seductive/logic/reason only (yell/cry/laugh → EfficientAT)",
                "skipped": args.skip_clap,
            },
        },
        "reference": args.ref,
        "files": files,
        "timing": {"wall_s": time.time() - t0},
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(out))
    print(args.out)

if __name__ == "__main__":
    main()
