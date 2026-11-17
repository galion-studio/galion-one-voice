# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""emotion2vec+ gate glue: selection, refusal without the binary, gate unchanged.

No weights and no audio files: a scripted stand-in binary prints the JSON
shape galion-emotion12 writes.
"""

from __future__ import annotations

import os
import sys
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path

import support  # noqa: F401

from harness import scorer12, sweep
from harness.audio import synthesize_tone, write_wav
from harness.backends import BackendUnavailable, StubBackend, resolve_backend
from harness.config import load_thresholds

FAKE = textwrap.dedent('''\
    #!{py}
    import json, sys
    a = sys.argv[1:]
    out = a[a.index("--out") + 1]
    files = [x for x in a if x.endswith(".wav")]
    labels = ["neutral","calm","happy","amused","excited","tender","sad","disappointed",
              "angry","fearful","surprised","disgusted","laughter","seductive"]
    post = [0.0] * 14; post[6] = 0.5; post[7] = 0.5
    json.dump({{"schema": "galion-emotion12/1", "device": "cpu", "labels": labels,
               "files": [{{"file": f, "posterior": post, "embedding": [3.0, 4.0]}} for f in files]}},
              open(out, "w"))
''')


class Env:
    def __init__(self, **values):
        self.values = values
        self.old = {}

    def __enter__(self):
        for key, value in self.values.items():
            self.old[key] = os.environ.get(key)
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def __exit__(self, *exc):
        for key, value in self.old.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


class GateSetTests(unittest.TestCase):
    def test_fourteen_emotions_each_read_from_an_emotion2vec_class(self):
        data = tomllib.loads(scorer12.gate_emotions_path().read_text(encoding="utf-8"))
        self.assertEqual(data["gate"], 0.83)
        ids = [row["id"] for row in data["emotions"]]
        self.assertEqual(len(ids), 14)
        self.assertEqual(ids[-2:], ["laughter", "seductive"])
        classes = {"angry", "disgusted", "fearful", "happy", "neutral", "sad", "surprised"}
        for row in data["emotions"]:
            self.assertIn(row["emotion2vec"], classes)
        native = [row["id"] for row in data["emotions"] if row.get("emotion2vec_in_set")]
        self.assertEqual(sorted(native), sorted(classes))


class SelectionTests(unittest.TestCase):
    def test_missing_binary_refuses_and_auto_falls_back(self):
        with Env(GALION_EMOTION12_BIN="/nonexistent/galion-emotion12", EMOTION_V2_BACKEND=None):
            self.assertFalse(scorer12.ready())
            with self.assertRaises(BackendUnavailable):
                resolve_backend("emotion2vec+").analyse(synthesize_tone(432.0, 0.2, 16000), 16000)
            self.assertNotIsInstance(resolve_backend("stub"), scorer12.Emotion2VecPlusBackend)
            analyse, _name, detail, ready = sweep._resolve_emotion("emotion2vec+", None, None, None)
            self.assertFalse(ready)
            self.assertIsNone(analyse)
            self.assertIn("unmeasured", detail)

    @unittest.skipIf(os.name == "nt", "scripted stand-in uses a shebang")
    def test_stand_in_binary_gives_fourteen_labels(self):
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / "galion-emotion12"
            fake.write_text(FAKE.format(py=sys.executable), encoding="utf-8")
            fake.chmod(0o755)
            models = Path(tmp) / "models" / "emotion2vec_plus_base_onnx"
            models.mkdir(parents=True)
            (models / "emotion2vec_plus_base.onnx").write_bytes(b"")
            with Env(GALION_EMOTION12_BIN=str(fake), ORT_DYLIB_PATH="x",
                     EMOTION_V2_MODELS_DIR=str(Path(tmp) / "models"), EMOTION_V2_BACKEND=None):
                self.assertTrue(scorer12.ready())
                backend = resolve_backend(None)
                self.assertIsInstance(backend, scorer12.Emotion2VecPlusBackend)
                result = backend.analyse(synthesize_tone(432.0, 0.2, 16000), 16000)
                self.assertEqual(len(result.labels), 14)
                self.assertAlmostEqual(sum(result.posterior), 1.0)
                self.assertAlmostEqual(result.embedding[0], 0.6)
                analyse, name, _detail, ready = sweep._resolve_emotion(None, None, None, None)
                self.assertTrue(ready)
                self.assertEqual(name, "emotion2vec+")
                clip = Path(tmp) / "a.wav"
                write_wav(clip, synthesize_tone(432.0, 0.2, 16000), 16000)
                self.assertEqual(len(analyse(clip).posterior), 14)


class GateUnchangedTests(unittest.TestCase):
    def test_gate_value_is_still_0_83(self):
        self.assertEqual(sweep.EMOTION_MIN, 0.83)
        self.assertEqual(sweep.WER_MAX, 0.018)
        self.assertEqual(sweep.ECAPA_MIN, 0.7057)
        self.assertEqual((sweep.CATEGORY_W, sweep.EMB_W), (0.70, 0.30))
        thresholds = load_thresholds()
        self.assertEqual(thresholds.emotion_target, 0.83)
        self.assertTrue(all(v == 0.83 for v in thresholds.emotion_targets.values()))
        self.assertEqual(thresholds.backend_id, "emotion2vec+")


if __name__ == "__main__":
    unittest.main()
