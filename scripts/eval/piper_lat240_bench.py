#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# lat240 / agent 01: warm Piper first-audio bench (EVAL ONLY, not shipped).
# Real wall clock around a real Piper 2023.11.14-2 child: JSON line written ->
# first raw PCM byte on stdout. No stubs. Run under the shared bench lock:
#   flock latency-240/BENCH.lock python3 scripts/eval/piper_lat240_bench.py ...
import argparse, json, os, select, subprocess, sys, time
from pathlib import Path

ROOT = Path(os.environ.get("GALION_REPO", Path(__file__).resolve().parents[2]))
if not (ROOT / "weights/runtime/piper/piper").exists():
    ROOT = Path("/workspace/swarm/voice-v2-sprint/repo")
TEXTS = {
    "1w": "Yes.",
    "2w": "Oh, nice.",
    "3w": "Oh, I see.",
    "4w": "I hear you, Mate.",
    "5w": "That's an interesting article, really.",
    "8w": "That sounds hard, tell me what happened next.",
    "20w": "I was thinking about what you said this morning, and I think you were right about the plan for the weekend trip.",
}

def pct(v, p):
    s = sorted(v); k = (len(s) - 1) * p / 100.0; f = int(k); c = min(f + 1, len(s) - 1)
    return s[f] + (s[c] - s[f]) * (k - f)

def summ(v):
    return {"n": len(v), "p50": round(pct(v, 50), 1), "p95": round(pct(v, 95), 1), "max": round(max(v), 1), "min": round(min(v), 1)}

class Piper:
    def __init__(self, runtime, model, cpus=None, noise=("0.667", "0.6"), env_extra=None):
        cmd = [str(runtime), "--model", str(model), "--config", str(model) + ".json", "--output_raw", "--json-input",
               "--sentence_silence", "0", "--noise_scale", noise[0], "--noise_w", noise[1]]
        env = dict(os.environ); env.update(env_extra or {})
        pre = (lambda: os.sched_setaffinity(0, cpus)) if cpus else None
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, preexec_fn=pre, env=env)
        self.out, self.err = self.p.stdout.fileno(), self.p.stderr.fileno()
        os.set_blocking(self.out, False); os.set_blocking(self.err, False)
        self.errbuf = b""

    def threads(self):
        try: return len(os.listdir(f"/proc/{self.p.pid}/task"))
        except OSError: return None

    def say(self, text):
        """returns (ttfa_ms, done_ms, n_samples, first_chunk_samples)"""
        mark = self.errbuf.count(b"Real-time factor")
        t0 = time.perf_counter()
        self.p.stdin.write((json.dumps({"text": text}) + "\n").encode()); self.p.stdin.flush()
        first = None; first_n = 0; n = 0
        while True:
            r, _, _ = select.select([self.out, self.err], [], [], 30)
            if not r: raise RuntimeError("piper timeout")
            if self.out in r:
                try: b = os.read(self.out, 1 << 20)
                except BlockingIOError: b = b""
                if b:
                    if first is None: first = (time.perf_counter() - t0) * 1000; first_n = len(b) // 2
                    n += len(b)
            if self.err in r:
                try: self.errbuf += os.read(self.err, 65536)
                except BlockingIOError: pass
            if self.errbuf.count(b"Real-time factor") > mark:
                done = (time.perf_counter() - t0) * 1000
                try:
                    b = os.read(self.out, 1 << 20); n += len(b)
                except BlockingIOError: pass
                return first, done, n // 2, first_n

    def close(self):
        self.p.stdin.close(); self.p.kill(); self.p.wait()

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=str(ROOT / "weights/tts/en_US-ljspeech-medium.onnx"))
    ap.add_argument("--runtime", default=str(ROOT / "weights/runtime/piper/piper"))
    ap.add_argument("--cpus", default="", help="e.g. 6-7 or 7; empty = inherit")
    ap.add_argument("--words", default="1w,2w,3w,4w,8w")
    ap.add_argument("--reps", type=int, default=12)
    ap.add_argument("--warm", type=int, default=3, help="throwaway lines after spawn")
    ap.add_argument("--idle-ms", type=int, default=0, help="sleep before each measured line (call idle gap)")
    ap.add_argument("--keepalive-ms", type=int, default=0, help="if >0, synth a throwaway 'Mm.' this long before each measured line")
    ap.add_argument("--label", default="")
    a = ap.parse_args()
    cpus = None
    if a.cpus:
        cpus = set()
        for part in a.cpus.split(","):
            lo, _, hi = part.partition("-"); cpus.update(range(int(lo), int(hi or lo) + 1))
    t_spawn = time.perf_counter()
    pp = Piper(a.runtime, a.model, cpus)
    cold, _, _, _ = pp.say("Hello.")
    cold_total = (time.perf_counter() - t_spawn) * 1000
    for _ in range(a.warm): pp.say("Warming up the voice.")
    rate = json.load(open(a.model + ".json"))["audio"]["sample_rate"]
    res = {"label": a.label, "model": Path(a.model).name, "rate": rate, "cpus": a.cpus or "all", "threads": pp.threads(),
           "idle_ms": a.idle_ms, "keepalive_ms": a.keepalive_ms, "cold_spawn_to_done_ms": round(cold_total, 1),
           "loadavg": os.getloadavg()[0], "by_words": {}}
    for k in a.words.split(","):
        ttfa, first_ms = [], []
        for _ in range(a.reps):
            if a.idle_ms: time.sleep(a.idle_ms / 1000)
            if a.keepalive_ms:
                pp.say("Mm."); time.sleep(a.keepalive_ms / 1000)
            f, d, n, fn = pp.say(TEXTS[k])
            ttfa.append(f); first_ms.append(fn * 1000 / rate)
        s = summ(ttfa); s["first_chunk_audio_ms"] = round(pct(first_ms, 50), 1); s["text"] = TEXTS[k]
        res["by_words"][k] = s
    pp.close()
    print(json.dumps(res))

if __name__ == "__main__":
    main()
