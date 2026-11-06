#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# lat240 / agent 01 (EVAL ONLY): Piper VITS onnx inference time vs onnxruntime
# intra-op threads / spinning / core pinning. Phoneme ids come from the real
# Piper 2023.11.14-2 binary (--debug), so input lengths match the shipped path.
# Note: uses the Python onnxruntime wheel on this host (not the 1.14.1 that
# Piper links) - the version is printed in every row.
import argparse, json, os, re, subprocess, time
from pathlib import Path

REPO = Path("/workspace/swarm/voice-v2-sprint/repo")
TEXTS = {"1w": "Yes.", "3w": "Oh, I see.", "4w": "I hear you, Mate.", "8w": "That sounds hard, tell me what happened next."}

def ids_for(model, text):
    p = subprocess.run([str(REPO / "weights/runtime/piper/piper"), "--model", model, "--json-input", "--output_raw", "--debug"],
                       input=(json.dumps({"text": text}) + "\n").encode(), capture_output=True)
    m = re.search(rb"phoneme id\(s\): ([0-9, ]+)", p.stderr)
    return [int(x) for x in m.group(1).decode().split(",") if x.strip()]

def absent(reason):
    print(json.dumps({
        "engine": "piper",
        "measured": False,
        "product_claim": False,
        "reason": reason,
    }, separators=(",", ":")))
    raise SystemExit(2)


def pct(v, q):
    import numpy as np
    return float(np.percentile(v, q))

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=str(REPO / "weights/tts/en_US-ljspeech-medium.onnx"))
    ap.add_argument("--threads", default="1,2,4,8")
    ap.add_argument("--spin", default="1,0")
    ap.add_argument("--cpus", default="")
    ap.add_argument("--reps", type=int, default=10)
    ap.add_argument("--piper-opts", action="store_true", help="mimic Piper 2023.11.14 session opts: no cpu mem arena, no mem pattern")
    a = ap.parse_args()
    model = Path(a.model)
    runtime = REPO / "weights/runtime/piper/piper"
    if not model.is_file() or not runtime.is_file():
        absent("Piper runtime or voice pack is not on this machine; no TTFA number")
    try:
        import numpy as np
        import onnxruntime as ort
    except ModuleNotFoundError as exc:
        absent(f"onnx runtime import failed ({exc}); no TTFA number")
    if a.cpus:
        s = set()
        for part in a.cpus.split(","):
            lo, _, hi = part.partition("-"); s.update(range(int(lo), int(hi or lo) + 1))
        os.sched_setaffinity(0, s)
    cfg = json.load(open(a.model + ".json"))
    ids = {k: ids_for(a.model, t) for k, t in TEXTS.items()}
    for th in [int(x) for x in a.threads.split(",")]:
        for spin in a.spin.split(","):
            so = ort.SessionOptions()
            so.intra_op_num_threads = th; so.inter_op_num_threads = 1
            so.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            so.add_session_config_entry("session.intra_op.allow_spinning", spin)
            so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            if a.piper_opts: so.enable_cpu_mem_arena = False; so.enable_mem_pattern = False
            s = ort.InferenceSession(a.model, so, providers=["CPUExecutionProvider"])
            scales = np.array([0.667, 1.0, 0.6], dtype=np.float32)
            def run(x):
                inp = {"input": np.array([x], dtype=np.int64), "input_lengths": np.array([len(x)], dtype=np.int64), "scales": scales}
                return s.run(None, inp)[0]
            for _ in range(3): run(ids["4w"])
            row = {"ort": ort.__version__, "model": Path(a.model).name, "threads": th, "spin": spin, "cpus": a.cpus or "all", "piper_opts": a.piper_opts,
                   "load": round(os.getloadavg()[0], 1)}
            for k, x in ids.items():
                v = []
                for _ in range(a.reps):
                    t0 = time.perf_counter(); y = run(x); v.append((time.perf_counter() - t0) * 1000)
                row[k] = [round(pct(v, 50), 1), round(pct(v, 95), 1)]
            row["audio_4w_ms"] = round(run(ids["4w"]).size * 1000 / cfg["audio"]["sample_rate"], 1)
            print(json.dumps(row), flush=True)

main()
