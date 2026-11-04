"""Premium Piper voice bench: SQUIM quality proxy per (model, speaker); warm TTFA
p50/p95 and 1-core RTF per model under flock BENCH.lock. Legacy piper CLI
2023.11.14-2 for every candidate (same runtime, supports speaker_id)."""
import json, os, subprocess, threading, time, sys, fcntl, statistics as st
import numpy as np, torch, torchaudio
P = '/workspace/swarm/voice-v2-sprint/repo/weights/runtime/piper/piper'
M = '/workspace/swarm/voice-v2-sprint/models/piper-premium/'
LOCK = '/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock'
OUT = os.path.dirname(os.path.abspath(__file__))
SHORT = "Sure, I can help."
LONG = ("I have moved the meeting to Thursday at four, and the notes from this morning "
        "are already in your inbox, so you can read them whenever you like.")
QUAL = ["Good evening. I have checked the schedule, and everything is ready for this afternoon.",
        "That sounds wonderful, I am really happy for you.",
        "I'm sorry to hear that. Take your time, I'm right here.",
        "The weather tomorrow looks clear, with a high of twenty one degrees."]
C = [  # (persona, model, speaker name or None)
 ('samantha','en_US-ljspeech-medium',None), ('samantha','en_US-ljspeech-high',None),
 ('samantha','en_GB-cori-high',None), ('samantha','en_US-kristin-medium',None),
 ('samantha','en_GB-alba-medium',None),
 *[('samantha','en_GB-vctk-medium',s) for s in ['p225','p228','p229','p231','p257','p268']],
 *[('samantha','en_US-libritts_r-medium',s) for s in ['2882','288','3118']],
 *[('samantha','en_US-libritts-high',s) for s in ['p3989','p7245','p5604','p3003']],
 *[('jarvis','en_GB-vctk-medium',s) for s in ['p226','p232','p243','p254','p258','p274','p278']],
 *[('jarvis','en_GB-aru-medium',s) for s in ['02','01','05','12','10','06']],
 ('jarvis','en_US-norman-medium',None), ('jarvis','en_US-john-medium',None), ('jarvis','en_US-joe-medium',None),
 *[('jarvis','en_US-libritts_r-medium',s) for s in ['1748','7833']],
]
def cfg(model): return json.load(open(M + model + '.onnx.json'))
def sid_of(model, name):
    return None if name is None else cfg(model)['speaker_id_map'][name]

class Piper:
    def __init__(self, model, pin=None):
        cmd = [P, '--model', M + model + '.onnx', '--output_raw', '--json-input']
        if pin is not None: cmd = ['taskset', '-c', str(pin)] + cmd
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)
        self.sr = cfg(model)['audio']['sample_rate']
        self.buf = bytearray(); self.first = None; self.done = threading.Event(); self.lk = threading.Lock()
        threading.Thread(target=self._out, daemon=True).start()
        threading.Thread(target=self._err, daemon=True).start()
    def _out(self):
        while True:
            b = self.p.stdout.read1(65536) if hasattr(self.p.stdout, 'read1') else os.read(self.p.stdout.fileno(), 65536)
            if not b: return
            with self.lk:
                if self.first is None: self.first = time.perf_counter()
                self.buf += b
    def _err(self):
        for line in self.p.stderr:
            if b'Real-time factor' in line: self.done.set()
    def say(self, text, sid=None):
        with self.lk: self.buf = bytearray(); self.first = None
        self.done.clear()
        d = {"text": text}
        if sid is not None: d["speaker_id"] = sid
        t0 = time.perf_counter()
        self.p.stdin.write((json.dumps(d) + "\n").encode())
        self.done.wait(120); time.sleep(0.03)
        t1 = time.perf_counter()
        with self.lk:
            pcm = np.frombuffer(bytes(self.buf[:len(self.buf)//2*2]), dtype=np.int16).copy(); first = self.first
        return pcm, (first - t0) if first else None, t1 - t0
    def close(self):
        self.p.stdin.close(); self.p.wait(10)

def quality():
    model = torchaudio.pipelines.SQUIM_OBJECTIVE.get_model().eval()
    rows = []
    for persona, m, name in C:
        sid = sid_of(m, name); pp = Piper(m)
        stoi, pesq, sisdr, f0s = [], [], [], []
        for k, t in enumerate(QUAL):
            pcm, _, _ = pp.say(t, sid)
            if k == 0:
                import wave
                os.makedirs(OUT + '/samples', exist_ok=True)
                w = wave.open(f"{OUT}/samples/{persona}-{m}-{name or 'single'}.wav", 'wb'); w.setnchannels(1); w.setsampwidth(2); w.setframerate(pp.sr); w.writeframes(pcm.tobytes()); w.close()
            x = torch.from_numpy(pcm.astype(np.float32) / 32768).unsqueeze(0)
            x = torchaudio.functional.resample(x, pp.sr, 16000)
            with torch.no_grad(): s, p_, d = model(x)
            stoi.append(float(s)); pesq.append(float(p_)); sisdr.append(float(d))
        pp.close()
        rows.append(dict(persona=persona, model=m, speaker=name, sid=sid, stoi=np.mean(stoi), pesq=np.mean(pesq), sisdr=np.mean(sisdr)))
        print('Q', rows[-1], flush=True)
    return rows

def timing(models):
    # Caller holds BENCH.lock for the whole run (quality phase is CPU-heavy too).
    rows = {}
    if True:
        load = os.getloadavg()
        for m in models:
            pp = Piper(m)
            sid = 0 if cfg(m).get('num_speakers', 1) > 1 else None
            for _ in range(3): pp.say(SHORT, sid)
            tt = [pp.say(SHORT, sid)[1] * 1000 for _ in range(20)]
            pp.close()
            p1 = Piper(m, pin=7)
            p1.say(LONG, sid)
            rtf = []
            for _ in range(4):
                pcm, _, wall = p1.say(LONG, sid)
                rtf.append(wall / (len(pcm) / p1.sr))
            p1.close()
            size = os.path.getsize(M + m + '.onnx') / 1e6
            rows[m] = dict(ttfa_p50=float(np.percentile(tt, 50)), ttfa_p95=float(np.percentile(tt, 95)), rtf1=float(np.median(rtf)), size_mb=size,
                           sr=cfg(m)['audio']['sample_rate'], quality=cfg(m)['audio'].get('quality'), speakers=cfg(m).get('num_speakers', 1))
            print('T', m, rows[m], flush=True)
        load2 = os.getloadavg()
    return rows, load, load2

if __name__ == '__main__':
    lf = open(LOCK, 'a'); t_wait = time.time()
    fcntl.flock(lf, fcntl.LOCK_EX)  # same lock the latency benches use
    print(f'lock acquired after {time.time()-t_wait:.0f}s', flush=True)
    torch.set_num_threads(2)
    q = quality()
    models = sorted({m for _, m, _ in C})
    t, l1, l2 = timing(models)
    json.dump(dict(quality=q, timing=t, load_before=l1, load_after=l2, nproc=os.cpu_count(), date=time.strftime('%Y-%m-%d %H:%M %Z')),
              open(OUT + '/bench.json', 'w'), indent=1, default=float)
    print('DONE', flush=True)
