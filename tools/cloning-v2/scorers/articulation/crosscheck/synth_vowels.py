"""Synthetic 5-formant vowels (impulse train -> cascade resonators) with known F0/F1-F3, for ground-truth checks."""
import numpy as np, wave, sys
from scipy.signal import lfilter
fs = 16000
def vowel(f0, F, B, path, dur=1.0):
    n = int(fs*dur); ph = np.cumsum(np.full(n, f0)/fs); y = np.diff(np.floor(ph), prepend=0).astype(float)
    for f, b in zip(F, B):
        r = np.exp(-np.pi*b/fs); th = 2*np.pi*f/fs; y = lfilter([1-r], [1, -2*r*np.cos(th), r*r], y)
    y = lfilter([1], [1, -0.9], y); y = y/np.abs(y).max()*0.5
    w = wave.open(path, 'w'); w.setnchannels(1); w.setsampwidth(2); w.setframerate(fs); w.writeframes((y*32767).astype('<i2').tobytes())
V = {"a_120": (120, [700, 1220, 2600, 3500, 4400]), "i_220": (220, [300, 2300, 3000, 3900, 4800]), "u_150": (150, [350, 800, 2500, 3400, 4300]),
     "ae_200": (200, [850, 1900, 2900, 3800, 4700]), "o_110": (110, [500, 900, 2450, 3300, 4200])}
for k, (f0, F) in V.items(): vowel(f0, F, [70, 90, 120, 200, 250], f"{sys.argv[1]}/{k}.wav")
print(__import__("json").dumps({k: {"f0": v[0], "F": v[1][:3]} for k, v in V.items()}))
