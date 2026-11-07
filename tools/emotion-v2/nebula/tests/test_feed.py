# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Nebula live feed: EMA, gas-line ids, replay, UDP, and the 80 ms heartbeat.

No audio is synthesized or read. Posteriors are numbers.
"""

from __future__ import annotations

import contextlib
import io
import json
import math
import socket
import sys
import time
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nebula.cli import main
from nebula.feed import (
    DEFAULT_TAU_MS,
    HEARTBEAT_MS,
    SCHEMA,
    FeedError,
    NebulaFeed,
    Posterior,
    ema_alpha,
    encode,
    posterior_from_analysis,
)
from nebula.gas import EMOTIONS, GAS_LINES, emotions_for, wavelength_nm
from nebula.replay import load_replay, play
from nebula.replay import ListSink
from nebula.udp import UdpSink, parse_udp

EXAMPLE = Path(__file__).resolve().parents[1] / "examples" / "posteriors.json"
DOC = Path(__file__).resolve().parents[4] / "docs" / "emotion-v2" / "NEBULA-LIVE-FEED.md"

# The fixed ids. A change here is a visualiser contract change.
EXPECTED_GAS = {
    "neutral": "HeII-541.2",
    "calm": "Hbeta-486.1",
    "happy": "[NII]-575.5",
    "amused": "HeI-587.6",
    "excited": "NeI-594.5",
    "tender": "HeI-667.8",
    "sad": "HeII-468.6",
    "disappointed": "[ClIII]-551.7",
    "angry": "[SIII]-631.2",
    "fearful": "Hdelta-410.2",
    "surprised": "[OIII]-495.9",
    "disgusted": "[ClIII]-553.8",
    "seductive": "Halpha-656.3",
}


def _one_hot(voice: str, emotion: str) -> dict[str, float]:
    return {key: (1.0 if key == emotion else 0.0) for key in emotions_for(voice)}


def _weight(frame_json: dict, emotion: str) -> float | None:
    return frame_json["weights"][emotion]["weight"]


class GasLineTests(unittest.TestCase):
    def test_table_matches_the_visualiser_contract(self) -> None:
        self.assertEqual(GAS_LINES, EXPECTED_GAS)
        self.assertEqual(list(GAS_LINES), list(EMOTIONS) + ["seductive"])
        self.assertEqual(len(set(GAS_LINES.values())), 13)
        for emotion, gas_line in GAS_LINES.items():
            self.assertEqual(wavelength_nm(gas_line), float(gas_line.rsplit("-", 1)[1]))
            self.assertTrue(gas_line.endswith(str(wavelength_nm(gas_line))))
            self.assertIn(emotion, EXPECTED_GAS)

    def test_seductive_is_samantha_only(self) -> None:
        self.assertNotIn("seductive", emotions_for("jarvis"))
        self.assertEqual(emotions_for("samantha")[-1], "seductive")
        with self.assertRaises(KeyError):
            emotions_for("galion")


class EmaTests(unittest.TestCase):
    def test_time_constant_is_one_over_e_at_one_tau(self) -> None:
        alpha = ema_alpha(DEFAULT_TAU_MS, DEFAULT_TAU_MS)
        self.assertAlmostEqual(alpha, 1.0 - math.exp(-1.0), places=12)
        self.assertAlmostEqual(ema_alpha(HEARTBEAT_MS, DEFAULT_TAU_MS), 1.0 - math.exp(-80.0 / 240.0))

    def test_step_moves_by_alpha_and_keeps_a_simplex(self) -> None:
        feed = NebulaFeed("samantha", tau_ms=240)
        first = feed.push(
            Posterior.from_mapping(
                {
                    "t_ms": 0,
                    "kind": "frame",
                    "weights": _one_hot("samantha", "neutral"),
                    "valence": 0.0,
                    "arousal": 0.0,
                    "dominance": 0.0,
                    "energy": 0.2,
                }
            )
        )
        self.assertTrue(first.present)
        self.assertTrue(first.distribution)
        self.assertEqual(first.weights[0].weight, 1.0)
        self.assertEqual(first.weights[0].gas_line, "HeII-541.2")

        alpha = ema_alpha(80, 240)
        second = feed.push(
            Posterior.from_mapping(
                {
                    "t_ms": 80,
                    "weights": _one_hot("samantha", "happy"),
                    "valence": 0.8,
                    "arousal": 0.4,
                    "dominance": 0.2,
                    "energy": 0.7,
                }
            )
        )
        by_id = {item.id: item.weight for item in second.weights}
        self.assertAlmostEqual(by_id["happy"], alpha, places=12)
        self.assertAlmostEqual(by_id["neutral"], 1.0 - alpha, places=12)
        self.assertAlmostEqual(by_id["seductive"], 0.0, places=12)
        self.assertAlmostEqual(sum(by_id.values()), 1.0, places=12)
        self.assertAlmostEqual(second.valence, alpha * 0.8, places=12)
        self.assertAlmostEqual(second.energy, 0.2 + alpha * 0.5, places=12)
        self.assertTrue(second.distribution)
        self.assertEqual(second.tau_ms, 240.0)

    def test_first_axis_is_copied_when_it_arrives_later(self) -> None:
        feed = NebulaFeed("jarvis", tau_ms=240)
        feed.push(Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("jarvis", "calm")}))
        frame = feed.push(
            Posterior.from_mapping(
                {
                    "t_ms": 80,
                    "weights": _one_hot("jarvis", "calm"),
                    "arousal": -0.5,
                }
            )
        )
        self.assertEqual(frame.arousal, -0.5)
        self.assertIsNone(frame.valence)
        self.assertIsNone(frame.energy)

    def test_hold_repeats_the_cloud_and_absence_is_not_neutral(self) -> None:
        feed = NebulaFeed("samantha")
        absence = feed.hold(0)
        self.assertFalse(absence.present)
        self.assertFalse(absence.distribution)
        self.assertTrue(all(item.weight is None for item in absence.weights))
        self.assertEqual(absence.weights[0].gas_line, "HeII-541.2")
        self.assertIsNone(absence.valence)
        encoded = json.loads(encode(absence))
        self.assertIsNone(encoded["weights"]["neutral"]["weight"])
        self.assertNotEqual(encoded["weights"]["neutral"]["weight"], 1)

        feed.push(
            Posterior.from_mapping({"t_ms": 80, "weights": _one_hot("samantha", "tender"), "energy": 0.4})
        )
        held = feed.hold(160)
        tender = next(item.weight for item in held.weights if item.id == "tender")
        self.assertEqual(tender, 1.0)
        self.assertEqual(held.energy, 0.4)
        self.assertEqual(held.t_ms, 160)
        again = feed.hold(240)
        tender_again = next(item.weight for item in again.weights if item.id == "tender")
        self.assertEqual(tender_again, 1.0)

    def test_timestamps_must_move_forward(self) -> None:
        feed = NebulaFeed("jarvis")
        feed.push(Posterior.from_mapping({"t_ms": 80, "weights": _one_hot("jarvis", "neutral")}))
        with self.assertRaises(FeedError):
            feed.push(Posterior.from_mapping({"t_ms": 80, "weights": _one_hot("jarvis", "neutral")}))
        with self.assertRaises(FeedError):
            feed.hold(40)

    def test_utterance_smoother_is_independent(self) -> None:
        feed = NebulaFeed("samantha", tau_ms=240)
        feed.push(Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("samantha", "angry")}))
        utterance = feed.push(
            Posterior.from_mapping(
                {
                    "t_ms": 0,
                    "kind": "utterance",
                    "weights": _one_hot("samantha", "sad"),
                    "energy": 0.3,
                }
            )
        )
        self.assertEqual(utterance.kind, "utterance")
        sad = next(item.weight for item in utterance.weights if item.id == "sad")
        angry = next(item.weight for item in utterance.weights if item.id == "angry")
        self.assertEqual(sad, 1.0)
        self.assertEqual(angry, 0.0)
        frame = feed.hold(80, kind="frame")
        frame_angry = next(item.weight for item in frame.weights if item.id == "angry")
        self.assertEqual(frame.kind, "frame")
        self.assertEqual(frame_angry, 1.0)


class ContractTests(unittest.TestCase):
    def test_jarvis_omits_seductive_and_rejects_it(self) -> None:
        feed = NebulaFeed("jarvis")
        frame = feed.push(Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("jarvis", "neutral")}))
        encoded = json.loads(encode(frame))
        self.assertEqual(list(encoded["weights"]), list(EMOTIONS))
        self.assertNotIn("seductive", encoded["weights"])
        self.assertEqual(encoded["weights"]["surprised"]["gas_line"], "[OIII]-495.9")
        bad = dict(_one_hot("jarvis", "neutral"))
        bad["seductive"] = 0.0
        with self.assertRaises(FeedError) as caught:
            feed.push(Posterior.from_mapping({"t_ms": 80, "weights": bad}))
        self.assertIn("Samantha", str(caught.exception))

    def test_samantha_frame_carries_every_gas_line(self) -> None:
        feed = NebulaFeed("samantha")
        frame = feed.push(Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("samantha", "seductive")}))
        encoded = json.loads(encode(frame))
        self.assertEqual(encoded["schema"], SCHEMA)
        self.assertEqual(encoded["voice_id"], "samantha")
        self.assertEqual(list(encoded["weights"]), list(emotions_for("samantha")))
        for emotion, gas_line in GAS_LINES.items():
            self.assertEqual(encoded["weights"][emotion]["gas_line"], gas_line)
        self.assertEqual(_weight(encoded, "seductive"), 1)
        self.assertLess(len(encode(frame).encode("utf-8")), 4096)

    def test_coarse_heads_are_not_expanded(self) -> None:
        feed = NebulaFeed("samantha")
        superb = posterior_from_analysis(
            SimpleNamespace(labels=("neu", "hap", "ang", "sad"), posterior=(0.25, 0.25, 0.25, 0.25)),
            t_ms=0,
        )
        with self.assertRaises(FeedError) as caught:
            feed.push(superb)
        self.assertIn("not expanded", str(caught.exception))

        nine = ("angry", "disgusted", "fearful", "happy", "neutral", "other", "sad", "surprised", "unknown")
        with self.assertRaises(FeedError):
            feed.push(
                Posterior.from_mapping(
                    {"t_ms": 0, "labels": list(nine), "posterior": [1.0 / 9.0] * 9}
                )
            )

    def test_harness_twelve_vector_is_accepted_for_jarvis(self) -> None:
        posterior = tuple(1.0 if emotion == "fearful" else 0.0 for emotion in EMOTIONS)
        analysis = SimpleNamespace(labels=EMOTIONS, posterior=posterior)
        feed = NebulaFeed("jarvis")
        frame = feed.push(posterior_from_analysis(analysis, t_ms=0, energy=0.25))
        fearful = next(item for item in frame.weights if item.id == "fearful")
        self.assertEqual(fearful.weight, 1.0)
        self.assertEqual(fearful.gas_line, "Hdelta-410.2")
        self.assertEqual(frame.energy, 0.25)

    def test_rejects_bad_mass_and_axes(self) -> None:
        feed = NebulaFeed("jarvis")
        weights = _one_hot("jarvis", "neutral")
        weights["neutral"] = 0.4
        with self.assertRaises(FeedError):
            feed.push(Posterior.from_mapping({"t_ms": 0, "weights": weights}))
        negative = _one_hot("jarvis", "neutral")
        negative["calm"] = -0.1
        negative["neutral"] = 1.1
        with self.assertRaises(FeedError):
            feed.push(Posterior.from_mapping({"t_ms": 0, "weights": negative}))
        with self.assertRaises(FeedError):
            Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("jarvis", "neutral"), "valence": 1.5})
        with self.assertRaises(FeedError):
            Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("jarvis", "neutral"), "energy": 1.2})
        with self.assertRaises(FeedError):
            NebulaFeed("samantha", tau_ms=0)
        with self.assertRaises(FeedError):
            NebulaFeed("galion-one")

    def test_json_fields_and_rounding(self) -> None:
        feed = NebulaFeed("samantha", tau_ms=240)
        feed.push(Posterior.from_mapping({"t_ms": 0, "weights": _one_hot("samantha", "neutral"), "energy": 0.2}))
        frame = feed.push(
            Posterior.from_mapping({"t_ms": 80, "weights": _one_hot("samantha", "happy"), "energy": 0.7})
        )
        text = encode(frame)
        self.assertNotIn("\n", text)
        payload = json.loads(text)
        self.assertEqual(
            list(payload),
            [
                "schema",
                "voice_id",
                "t_ms",
                "kind",
                "present",
                "distribution",
                "tau_ms",
                "weights",
                "valence",
                "arousal",
                "dominance",
                "energy",
            ],
        )
        self.assertEqual(payload["tau_ms"], 240)
        self.assertEqual(payload["t_ms"], 80)
        self.assertTrue(payload["distribution"])
        total = sum(item["weight"] for item in payload["weights"].values())
        self.assertAlmostEqual(total, 1.0, places=4)


class ReplayAndUdpTests(unittest.TestCase):
    def test_example_replay_file(self) -> None:
        replay = load_replay(EXAMPLE)
        self.assertEqual(replay.voice_id, "samantha")
        self.assertEqual(replay.tau_ms, 240.0)
        sink = ListSink()
        self.assertEqual(play(replay, sink), 3)
        frames = [json.loads(line) for line in sink.lines]
        self.assertEqual(frames[0]["kind"], "frame")
        self.assertEqual(_weight(frames[0], "neutral"), 1)
        alpha = 1.0 - math.exp(-80.0 / 240.0)
        self.assertAlmostEqual(_weight(frames[1], "happy"), alpha, places=6)
        self.assertEqual(frames[2]["kind"], "utterance")
        self.assertEqual(_weight(frames[2], "happy"), 0.5)
        self.assertEqual(_weight(frames[2], "amused"), 0.5)
        self.assertEqual(frames[2]["weights"]["amused"]["gas_line"], "HeI-587.6")
        for frame in frames:
            self.assertEqual(frame["schema"], SCHEMA)
            self.assertIn("seductive", frame["weights"])

    def test_cli_replay_stdout_and_voice_mismatch(self) -> None:
        output = io.StringIO()
        status = main(["replay", str(EXAMPLE)], output_stream=output)
        self.assertEqual(status, 0)
        self.assertEqual(len(output.getvalue().splitlines()), 3)
        with contextlib.redirect_stderr(io.StringIO()):
            status = main(["replay", str(EXAMPLE), "--voice", "jarvis"], output_stream=io.StringIO())
        self.assertEqual(status, 2)

    def test_cli_feed_stdin(self) -> None:
        line = json.dumps(
            {"t_ms": 0, "kind": "frame", "weights": _one_hot("jarvis", "disgusted"), "energy": 0.1}
        )
        output = io.StringIO()
        status = main(["feed", "--voice", "jarvis"], input_stream=io.StringIO(line + "\n"), output_stream=output)
        self.assertEqual(status, 0)
        payload = json.loads(output.getvalue())
        self.assertEqual(payload["voice_id"], "jarvis")
        self.assertEqual(payload["weights"]["disgusted"]["gas_line"], "[ClIII]-553.8")
        self.assertNotIn("seductive", payload["weights"])

    def test_udp_loopback_and_refusal(self) -> None:
        with self.assertRaises(FeedError):
            parse_udp("8.8.8.8:8765")
        with self.assertRaises(FeedError):
            UdpSink("10.1.2.3", 8765)
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(("127.0.0.1", 0))
        sock.settimeout(2)
        port = sock.getsockname()[1]
        try:
            output = io.StringIO()
            status = main(
                ["replay", str(EXAMPLE), "--udp", f"127.0.0.1:{port}"],
                output_stream=output,
            )
            self.assertEqual(status, 0)
            packet, addr = sock.recvfrom(4096)
            self.assertEqual(addr[0], "127.0.0.1")
            self.assertFalse(packet.endswith(b"\n"))
            payload = json.loads(packet.decode("utf-8"))
            self.assertEqual(payload["schema"], SCHEMA)
            self.assertEqual(payload["weights"]["neutral"]["gas_line"], "HeII-541.2")
            self.assertEqual(len(output.getvalue().splitlines()), 3)
        finally:
            sock.close()

    def test_smooth_and_encode_stays_inside_one_heartbeat(self) -> None:
        """One hundred smoothed frames finish inside the locked 80 ms heartbeat.

        The clock is HEARTBEAT_MS from LOCKS.toml. This times the EMA and the
        JSON encode only. It does not time a twelve-way recogniser, and it
        does not authorise putting that head on the heartbeat.
        """
        self.assertEqual(HEARTBEAT_MS, 80)
        feed = NebulaFeed("samantha", tau_ms=DEFAULT_TAU_MS)
        hot = _one_hot("samantha", "neutral")
        started = time.perf_counter()
        for index in range(100):
            weights = _one_hot("samantha", "happy" if index % 2 else "neutral")
            frame = feed.push(
                Posterior.from_mapping(
                    {
                        "t_ms": index * HEARTBEAT_MS,
                        "weights": weights if index else hot,
                        "valence": 0.1,
                        "arousal": -0.2,
                        "dominance": 0.3,
                        "energy": 0.4,
                    }
                )
            )
            encoded = encode(frame)
            self.assertIn("HeII-541.2", encoded)
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        self.assertLess(elapsed_ms, HEARTBEAT_MS)
        self.assertLess(elapsed_ms / 100.0, 1.0)

    def test_package_does_not_import_audio(self) -> None:
        banned = ("harness.audio", "harness.backends", "wave", "soundfile")
        for name in banned:
            self.assertNotIn(name, sys.modules)


class SchemaDocTests(unittest.TestCase):
    def test_doc_carries_the_contract(self) -> None:
        text = DOC.read_text(encoding="utf-8")
        self.assertIn(SCHEMA, text)
        self.assertIn("240", text)
        self.assertIn("80", text)
        self.assertIn("1 - exp(-dt_ms / tau_ms)", text)
        self.assertIn("apps/nebula-voice-v2", text)
        self.assertIn("Samantha", text)
        for gas_line in GAS_LINES.values():
            self.assertIn(gas_line, text)
        self.assertIn("voice conditioning", text.lower())
        self.assertIn("replay", text.lower())
        blocks = _json_fences(text)
        self.assertGreaterEqual(len(blocks), 2)
        voices = set()
        for block in blocks:
            payload = json.loads(block)
            self.assertEqual(payload["schema"], SCHEMA)
            voices.add(payload["voice_id"])
            for emotion, slot in payload["weights"].items():
                self.assertEqual(slot["gas_line"], GAS_LINES[emotion])
        self.assertEqual(voices, {"samantha", "jarvis"})


def _json_fences(text: str) -> list[str]:
    blocks: list[str] = []
    current: list[str] | None = None
    for line in text.splitlines():
        if line.startswith("```json"):
            current = []
            continue
        if current is not None and line.startswith("```"):
            blocks.append("\n".join(current))
            current = None
            continue
        if current is not None:
            current.append(line)
    return blocks


if __name__ == "__main__":
    unittest.main()
