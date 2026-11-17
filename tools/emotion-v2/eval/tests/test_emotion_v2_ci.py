# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# Samantha · Galion Voice V2. 432 Hz tuning standard.
"""Glue test for the #182 scorer plugin: fake tool binaries, a synthetic tone, no weights, no audio files."""
import os
import stat
import sys
import tempfile
import types
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import emotion_v2_ci as ci  # noqa: E402

FAKE = {
    # galion-wer: last arg is the wav, --out FILE
    "galion-wer": 'out=""; while [ $# -gt 0 ]; do [ "$1" = --out ] && out="$2"; shift; done; echo \'{"summary":{"mean":0.0}}\' > "$out"',
    "galion-ecapa": 'out=""; while [ $# -gt 0 ]; do [ "$1" = --out ] && out="$2"; shift; done; echo \'{"mean":0.81}\' > "$out"',
    "galion-emotion": (
        'o=""; c=""; while [ $# -gt 0 ]; do case "$1" in --out-dir) o="$2";; --config) c="$2";; esac; shift; done; '
        'n=$(grep -c "^\\[\\[emotion\\]\\]" "$c"); e=$(sed -n \'s/^id = "\\(.*\\)"/\\1/p\' "$c"); '
        '[ "$n" = 1 ] || exit 3; '
        'echo "{\\"rows\\":[{\\"emotion\\":\\"$e\\",\\"emotion_match\\":0.9,\\"wer\\":0.0,\\"ecapa\\":0.81}]}" > "$o/matrix.json"; '
        'exit 1'  # like the real tool: exit 1 when the gate row FAILs, report still written
    ),
}


def clip(emotion="calm"):
    t = np.arange(16000) / 16000.0
    audio = types.SimpleNamespace(samples=0.2 * np.sin(2 * np.pi * 432.0 * t), sample_rate=16000)
    return types.SimpleNamespace(audio=audio, text="hello there", emotion=emotion, voice_id="samantha", intensity=1.0)


class PluginTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.bins = Path(self.tmp.name)
        for name, body in FAKE.items():
            p = self.bins / name
            p.write_text("#!/usr/bin/env bash\nset -e\n" + body + "\n")
            p.chmod(p.stat().st_mode | stat.S_IEXEC)
        self.cfg = {"bin_dir": str(self.bins), "models_root": str(self.bins), "speaker_ref": "spk.wav", "reference_dir": str(self.bins)}

    def tearDown(self):
        self.tmp.cleanup()

    def test_three_gate_scorers_one_run_per_clip(self):
        scorers = ci.build_scorers(self.cfg)
        self.assertEqual([s.name for s in scorers], ["wer", "ecapa", "emotion"])
        c = clip("seductive")
        self.assertEqual([s.score(c) for s in scorers], [0.0, 0.81, 0.9])

    def test_every_eval_emotion_is_a_single_row_config(self):
        src = ci.ROOT / "tools/emotion-v2/eval/emotions.toml"
        ids = [l.split('"')[1] for l in src.read_text().splitlines() if l.startswith('id = "')]
        self.assertEqual(len(ids), 14)
        for e in ids:
            out = self.bins / f"{e}.toml"
            ci._one_emotion_config(src, e, out)
            text = out.read_text()
            self.assertEqual(text.splitlines().count("[[emotion]]"), 1)
            self.assertIn(f'id = "{e}"', text)
            self.assertIn("emotion_min = 0.83", text)

    def test_missing_tool_never_passes(self):
        (self.bins / "galion-ecapa").unlink()
        with self.assertRaises(Exception):
            ci.build_scorers(self.cfg)[1].score(clip())

    def test_unmeasured_value_raises(self):
        (self.bins / "galion-emotion").write_text(
            '#!/usr/bin/env bash\no=""; while [ $# -gt 0 ]; do [ "$1" = --out-dir ] && o="$2"; shift; done\n'
            'echo \'{"rows":[{"emotion":"calm","emotion_match":null,"wer":0.0,"ecapa":0.8}]}\' > "$o/matrix.json"\n')
        with self.assertRaises(ci.ScorerUnavailable):
            ci.build_scorers(self.cfg)[2].score(clip())


if __name__ == "__main__":
    unittest.main()
