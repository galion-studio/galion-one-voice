# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""The Rust const, the Python mirror, and the research doc stay identical."""

from __future__ import annotations

import importlib.util
import math
import re
import unittest
from pathlib import Path

import support

REPO = support.REPO
RUST = REPO / "crates" / "nebula-elements" / "src" / "lib.rs"
DOC = REPO / "docs" / "emotion-v2" / "UNIVERSAL-EMOTION-GAS-TABLE-20261107.md"


def _load_elements():
    path = REPO / "emotion-v2" / "elements.py"
    spec = importlib.util.spec_from_file_location("galion_emotion_elements", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _srgb_to_lab(hex_s: str) -> tuple[float, float, float]:
    hex_s = hex_s.lstrip("#")
    rgb = [int(hex_s[i : i + 2], 16) / 255 for i in (0, 2, 4)]

    def lin(c: float) -> float:
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = map(lin, rgb)
    x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375
    y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750
    z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041

    def f(t: float) -> float:
        return t ** (1 / 3) if t > 0.008856 else 7.787 * t + 16 / 116

    fx, fy, fz = f(x / 0.95047), f(y / 1.0), f(z / 1.08883)
    return 116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)


def _delta_e(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    L1, a1, b1 = a
    L2, a2, b2 = b
    C1 = math.hypot(a1, b1)
    C2 = math.hypot(a2, b2)
    Cbar = (C1 + C2) / 2
    G = 0.5 * (1 - math.sqrt(Cbar**7 / (Cbar**7 + 25**7)))
    a1p, a2p = (1 + G) * a1, (1 + G) * a2
    C1p, C2p = math.hypot(a1p, b1), math.hypot(a2p, b2)
    h1p = math.degrees(math.atan2(b1, a1p)) % 360
    h2p = math.degrees(math.atan2(b2, a2p)) % 360
    dLp = L2 - L1
    dCp = C2p - C1p
    dhp = h2p - h1p
    if C1p * C2p == 0:
        dhp = 0
    elif dhp > 180:
        dhp -= 360
    elif dhp < -180:
        dhp += 360
    dHp = 2 * math.sqrt(C1p * C2p) * math.sin(math.radians(dhp) / 2)
    Lbar = (L1 + L2) / 2
    Cbarp = (C1p + C2p) / 2
    if C1p * C2p == 0:
        hbar = h1p + h2p
    elif abs(h1p - h2p) > 180:
        hbar = (h1p + h2p + 360) / 2 if h1p + h2p < 360 else (h1p + h2p - 360) / 2
    else:
        hbar = (h1p + h2p) / 2
    T = (
        1
        - 0.17 * math.cos(math.radians(hbar - 30))
        + 0.24 * math.cos(math.radians(2 * hbar))
        + 0.32 * math.cos(math.radians(3 * hbar + 6))
        - 0.20 * math.cos(math.radians(4 * hbar - 63))
    )
    dtheta = 30 * math.exp(-((hbar - 275) / 25) ** 2)
    Rc = 2 * math.sqrt(Cbarp**7 / (Cbarp**7 + 25**7))
    Sl = 1 + (0.015 * (Lbar - 50) ** 2) / math.sqrt(20 + (Lbar - 50) ** 2)
    Sc = 1 + 0.045 * Cbarp
    Sh = 1 + 0.015 * Cbarp * T
    Rt = -math.sin(math.radians(2 * dtheta)) * Rc
    return math.sqrt((dLp / Sl) ** 2 + (dCp / Sc) ** 2 + (dHp / Sh) ** 2 + Rt * (dCp / Sc) * (dHp / Sh))


class ElementSyncTests(unittest.TestCase):
    def test_rust_python_and_doc_match(self) -> None:
        py = _load_elements()
        rust = RUST.read_text(encoding="utf-8")
        demo_rows = re.findall(
            r'row\("([^"]+)", "([^"]+)", "([^"]+)", "([^"]+)", ([0-9.]+), "(#[0-9A-Fa-f]{6})", "(#[0-9A-Fa-f]{6})"',
            rust,
        )
        univ_rows = re.findall(
            r'urow\("([^"]+)", "([^"]+)", "([^"]+)", ([0-9.]+), "(#[0-9A-Fa-f]{6})"\)',
            rust,
        )
        self.assertEqual(len(demo_rows), 13)
        self.assertEqual(len(univ_rows), 62)
        self.assertEqual(len(py.DEMO), 13)
        self.assertEqual(len(py.UNIVERSAL), 62)
        for rust_row, py_row in zip(demo_rows, py.DEMO, strict=True):
            self.assertEqual(rust_row[0], py_row[0])
            self.assertEqual(rust_row[1], py_row[1])
            self.assertEqual(rust_row[2], py_row[2])
            self.assertEqual(rust_row[3], py_row[3])
            self.assertAlmostEqual(float(rust_row[4]), py_row[4], places=3)
            self.assertEqual(rust_row[5], py_row[5])
            self.assertEqual(rust_row[6], py_row[6])
        for rust_row, py_row in zip(univ_rows, py.UNIVERSAL, strict=True):
            self.assertEqual(rust_row[0], py_row[0])
            self.assertEqual(rust_row[2], py_row[2])
            self.assertAlmostEqual(float(rust_row[3]), py_row[3], places=3)
            self.assertEqual(rust_row[4], py_row[4])
        doc = DOC.read_text(encoding="utf-8")
        demo_doc = doc.split("## Demo 13 rows v2", 1)[1].split("### Compact universal table", 1)[0]
        for line in demo_doc.splitlines():
            if not line.startswith("| ") or line.startswith("| Emotion") or line.startswith("|---"):
                continue
            cols = [c.strip() for c in line.strip().strip("|").split("|")]
            found = next(row for row in py.DEMO if row[1] == cols[0])
            hexes = re.findall(r"#[0-9A-Fa-f]{6}", cols[4] + " " + cols[5])
            self.assertEqual(found[5], hexes[0])
            self.assertEqual(found[6], hexes[1])
            self.assertAlmostEqual(found[4], float(cols[2]), places=3)

    def test_display_pairs_keep_delta_e_2000_of_at_least_10(self) -> None:
        py = _load_elements()
        labs = [(row[0], row[6], _srgb_to_lab(row[6])) for row in py.DEMO]
        worst = 999.0
        for i in range(len(labs)):
            for j in range(i + 1, len(labs)):
                worst = min(worst, _delta_e(labs[i][2], labs[j][2]))
        self.assertGreaterEqual(worst, 10.0)

    def test_distribution_labels_do_not_replace_the_posterior(self) -> None:
        from harness.audio import synthesize_tone
        from harness.backends import StubBackend
        from harness.emotions import STUB_FREQ_HZ

        tone = synthesize_tone(STUB_FREQ_HZ["angry"], sample_rate=16000, seconds=0.25)
        analysis = StubBackend().analyse(list(tone), 16000)
        angry = analysis.labels.index("angry")
        self.assertEqual(analysis.posterior.index(max(analysis.posterior)), angry)
        row = analysis.distribution()[angry]
        self.assertEqual(row["element"], "SIII-631.2")
        self.assertEqual(row["weight"], analysis.posterior[angry])


if __name__ == "__main__":
    unittest.main()
