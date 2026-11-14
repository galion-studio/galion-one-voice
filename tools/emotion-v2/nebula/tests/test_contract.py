# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""UI contract: explicit absence, validation, reconnect, and the bridge.

No audio is read or written. Frames are numbers.
"""

from __future__ import annotations

import contextlib
import io
import json
import socket
import sys
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nebula.cli import main
from nebula.contract import (
    ELEMENT_IDS,
    ESTIMATE_DISTRIBUTION,
    ESTIMATE_NONE,
    ESTIMATE_PARTIAL,
    UI_SCHEMA,
    project_text,
    validate_live,
    validate_ui,
)
from nebula.feed import SCHEMA, FeedError, NebulaFeed, Posterior, encode
from nebula.gas import GAS_LINES, emotions_for
from nebula.session import (
    BACKOFF_CAP_MS,
    BACKOFF_START_MS,
    FeedClient,
    FeedServer,
    backoff_ms,
)

DOC = Path(__file__).resolve().parents[4] / "docs" / "galion-voice-v2" / "EMOTION-NEBULA-FEED.md"
SCHEMA_JSON = Path(__file__).resolve().parents[4] / "docs" / "galion-voice-v2" / "emotion-nebula-feed.schema.json"


def _one_hot(voice: str, emotion: str) -> dict[str, float]:
    return {key: (1.0 if key == emotion else 0.0) for key in emotions_for(voice)}


def _absence_line(voice: str = "jarvis") -> str:
    return encode(NebulaFeed(voice).hold(0))


def _distribution_line(voice: str = "samantha", emotion: str = "neutral") -> str:
    feed = NebulaFeed(voice)
    return encode(
        feed.push(
            Posterior.from_mapping(
                {
                    "t_ms": 0,
                    "weights": _one_hot(voice, emotion),
                    "valence": 0.0,
                    "arousal": 0.0,
                    "dominance": 0.0,
                    "energy": 0.2,
                }
            )
        )
    )


def _partial_line() -> str:
    feed = NebulaFeed("jarvis")
    feed.hold(0)
    return encode(feed.push(Posterior.from_mapping({"t_ms": 80, "energy": 0.4})))


class EstimateTests(unittest.TestCase):
    def test_absence_is_named_none_and_is_not_neutral(self) -> None:
        message = project_text(_absence_line("jarvis"), seq=1)
        self.assertEqual(message["schema"], UI_SCHEMA)
        self.assertEqual(message["source_schema"], SCHEMA)
        self.assertEqual(message["estimate"], ESTIMATE_NONE)
        self.assertFalse(message["present"])
        self.assertFalse(message["distribution"])
        self.assertIsNone(message["reason"])
        self.assertNotIn("seductive", message["weights"])
        neutral = message["weights"]["neutral"]
        self.assertIsNone(neutral["weight"])
        self.assertNotEqual(neutral["weight"], 1)
        self.assertEqual(neutral["gas_line"], "HeII-541.2")
        self.assertEqual(neutral["element_id"], "HeII-541.2")
        self.assertEqual(message["weights"]["calm"]["element_id"], "H-beta")
        self.assertEqual(message["weights"]["calm"]["gas_line"], "Hbeta-486.1")
        self.assertIsNone(message["valence"])

    def test_distribution_carries_picture_element_ids(self) -> None:
        message = project_text(_distribution_line("samantha", "happy"), seq=2)
        self.assertEqual(message["estimate"], ESTIMATE_DISTRIBUTION)
        self.assertTrue(message["distribution"])
        self.assertEqual(message["weights"]["happy"]["weight"], 1)
        self.assertEqual(message["weights"]["happy"]["gas_line"], "[NII]-575.5")
        self.assertEqual(message["weights"]["happy"]["element_id"], "NII-575.5")
        self.assertEqual(message["weights"]["fearful"]["element_id"], "H-delta")
        self.assertEqual(message["weights"]["seductive"]["element_id"], "H-alpha")
        self.assertEqual(message["weights"]["seductive"]["gas_line"], "Halpha-656.3")
        self.assertEqual(message["seq"], 2)
        self.assertEqual(message["origin"], "server")

    def test_partial_is_not_rewritten_into_a_simplex(self) -> None:
        message = project_text(_partial_line(), seq=1)
        self.assertEqual(message["estimate"], ESTIMATE_PARTIAL)
        self.assertTrue(message["present"])
        self.assertFalse(message["distribution"])
        self.assertIsNone(message["weights"]["neutral"]["weight"])
        self.assertEqual(message["energy"], 0.4)
        self.assertIsNone(message["valence"])

    def test_element_ids_cover_every_feed_gas_line(self) -> None:
        self.assertEqual(set(ELEMENT_IDS), set(GAS_LINES))
        self.assertEqual(ELEMENT_IDS["surprised"], "OIII-495.9")
        self.assertEqual(ELEMENT_IDS["angry"], "SIII-631.2")
        self.assertEqual(ELEMENT_IDS["disgusted"], "ClIII-553.8")
        self.assertNotEqual(ELEMENT_IDS["calm"], GAS_LINES["calm"])
        self.assertNotEqual(ELEMENT_IDS["seductive"], GAS_LINES["seductive"])


class ValidationTests(unittest.TestCase):
    def test_rejects_a_number_hiding_inside_an_absence(self) -> None:
        raw = json.loads(_absence_line("jarvis"))
        raw["weights"]["neutral"]["weight"] = 1
        with self.assertRaises(FeedError) as caught:
            validate_live(raw)
        self.assertIn("present false", str(caught.exception))

    def test_rejects_jarvis_seductive_and_a_wrong_gas_line(self) -> None:
        raw = json.loads(_distribution_line("jarvis", "calm"))
        raw["weights"]["seductive"] = {"weight": 0, "gas_line": "Halpha-656.3"}
        with self.assertRaises(FeedError) as caught:
            validate_live(raw)
        self.assertIn("Samantha", str(caught.exception))

        raw = json.loads(_distribution_line("samantha", "calm"))
        raw["weights"]["calm"]["gas_line"] = "H-beta"
        with self.assertRaises(FeedError) as caught:
            validate_live(raw)
        self.assertIn("gas_line", str(caught.exception))

    def test_rejects_unknown_fields_and_a_coarse_head(self) -> None:
        raw = json.loads(_absence_line())
        raw["mood"] = "calm"
        with self.assertRaises(FeedError):
            validate_live(raw)
        with self.assertRaises(FeedError):
            project_text('{"labels":["neu","hap"],"posterior":[0.5,0.5]}', seq=1)

    def test_ui_message_cannot_claim_none_and_carry_neutral(self) -> None:
        message = project_text(_absence_line(), seq=1)
        message["weights"]["neutral"]["weight"] = 1
        with self.assertRaises(FeedError):
            validate_ui(message)


class BackoffTests(unittest.TestCase):
    def test_schedule_starts_at_one_heartbeat_and_caps(self) -> None:
        self.assertEqual(BACKOFF_START_MS, 80)
        self.assertEqual([backoff_ms(i) for i in range(6)], [80, 160, 320, 640, 1280, 1280])
        self.assertEqual(backoff_ms(20), BACKOFF_CAP_MS)
        with self.assertRaises(FeedError):
            backoff_ms(-1)


class SessionTests(unittest.TestCase):
    def test_server_sends_only_validated_lines(self) -> None:
        server = FeedServer("127.0.0.1", 0)
        server.start()
        client = FeedClient("127.0.0.1", server.port, timeout_s=2)
        try:
            self.assertTrue(client.open_attempt())
            server.wait_clients(1, 2)
            first = server.accept_text(_absence_line("jarvis"))
            got = client.read()
            self.assertEqual(got["estimate"], ESTIMATE_NONE)
            self.assertEqual(got["seq"], first["seq"])
            self.assertIsNone(got["weights"]["neutral"]["weight"])
            second = server.accept_text(_distribution_line("jarvis", "calm"))
            got = client.read()
            self.assertEqual(got["estimate"], ESTIMATE_DISTRIBUTION)
            self.assertEqual(got["weights"]["calm"]["element_id"], "H-beta")
            self.assertEqual(got["seq"], second["seq"])
            marker = 0.123456
            bad = json.loads(_distribution_line("jarvis", "neutral"))
            bad["weights"]["neutral"]["weight"] = marker
            with self.assertRaises(FeedError):
                server.accept_text(json.dumps(bad))
            rejected = client.read()
            self.assertEqual(rejected["estimate"], ESTIMATE_NONE)
            self.assertEqual(rejected["reason"], "rejected")
            self.assertIsNone(rejected["source_schema"])
            self.assertNotIn(str(marker), json.dumps(rejected))
            self.assertIsNone(rejected["weights"]["neutral"]["weight"])
        finally:
            client.close()
            server.close()

    def test_drop_is_an_absence_and_reconnect_backs_off(self) -> None:
        sleeps: list[float] = []
        holder = socket.socket()
        holder.bind(("127.0.0.1", 0))
        port = holder.getsockname()[1]
        holder.close()
        client = FeedClient("127.0.0.1", port, sleeper=sleeps.append, timeout_s=0.3)
        try:
            self.assertFalse(client.open_attempt())
            self.assertAlmostEqual(sleeps[-1], 0.08)
            gap = client.gap_message()
            self.assertEqual(gap["estimate"], ESTIMATE_NONE)
            self.assertEqual(gap["origin"], "client")
            self.assertEqual(gap["reason"], "disconnected")
            self.assertIsNone(gap["seq"])
            self.assertIsNone(gap["voice_id"])
            self.assertIsNone(gap["weights"]["neutral"]["weight"])
            self.assertNotIn("seductive", gap["weights"])
            self.assertFalse(client.open_attempt())
            self.assertAlmostEqual(sleeps[-1], 0.16)
            server = FeedServer("127.0.0.1", port)
            server.start()
            try:
                self.assertTrue(client.open_attempt())
                self.assertEqual(client.attempt, 0)
                server.wait_clients(1, 2)
                server.accept_text(_distribution_line("samantha", "tender"))
                got = client.read()
                self.assertEqual(got["weights"]["tender"]["element_id"], "HeI-667.8")
                server.close()
                server = None
                self.assertIsNone(client.read())
                gap = client.gap_message()
                self.assertEqual(gap["estimate"], ESTIMATE_NONE)
                self.assertEqual(gap["voice_id"], "samantha")
                self.assertEqual(gap["reason"], "disconnected")
                self.assertNotEqual(gap["weights"]["tender"]["weight"], 1)
                self.assertIsNone(gap["weights"]["tender"]["weight"])
            finally:
                if server is not None:
                    server.close()
        finally:
            client.close()

    def test_pump_stops_after_one_server_frame(self) -> None:
        server = FeedServer("127.0.0.1", 0)
        server.start()
        output = io.StringIO()
        client = FeedClient("127.0.0.1", server.port, timeout_s=2)

        def run() -> None:
            client.pump(output, frames=1)

        thread = threading.Thread(target=run)
        thread.start()
        try:
            server.wait_clients(1, 2)
            server.accept_text(_partial_line())
            thread.join(3)
            self.assertFalse(thread.is_alive())
            payload = json.loads(output.getvalue())
            self.assertEqual(payload["estimate"], ESTIMATE_PARTIAL)
            self.assertEqual(payload["energy"], 0.4)
        finally:
            client.close()
            server.close()
            thread.join(1)


class BridgeCliTests(unittest.TestCase):
    def test_bridge_projects_encoder_output_and_keeps_going(self) -> None:
        bad = "{"
        good = _absence_line("jarvis")
        output = io.StringIO()
        error = io.StringIO()
        with contextlib.redirect_stderr(error):
            status = main(
                ["bridge"],
                input_stream=io.StringIO(bad + "\n" + good + "\n"),
                output_stream=output,
            )
        self.assertEqual(status, 0)
        lines = output.getvalue().splitlines()
        self.assertEqual(len(lines), 2)
        first = json.loads(lines[0])
        second = json.loads(lines[1])
        self.assertEqual(first["estimate"], ESTIMATE_NONE)
        self.assertEqual(first["reason"], "rejected")
        self.assertIsNone(first["voice_id"])
        self.assertEqual(second["estimate"], ESTIMATE_NONE)
        self.assertIsNone(second["reason"])
        self.assertEqual(second["voice_id"], "jarvis")
        self.assertEqual(second["weights"]["fearful"]["element_id"], "H-delta")
        self.assertIn("not JSON", error.getvalue())

    def test_bridge_refuses_a_public_listener(self) -> None:
        error = io.StringIO()
        with contextlib.redirect_stderr(error):
            status = main(["bridge", "--tcp", "8.8.8.8:8766"], input_stream=io.StringIO(""), output_stream=io.StringIO())
        self.assertEqual(status, 2)
        self.assertIn("local only", error.getvalue())

    def test_documented_examples_match_the_schema(self) -> None:
        text = DOC.read_text(encoding="utf-8")
        self.assertIn(UI_SCHEMA, text)
        self.assertIn('"none"', text)
        self.assertIn("80", text)
        self.assertIn("1280", text)
        self.assertIn("H-beta", text)
        self.assertIn("Hbeta-486.1", text)
        schema = json.loads(SCHEMA_JSON.read_text(encoding="utf-8"))
        self.assertIn(ESTIMATE_NONE, schema["properties"]["estimate"]["enum"])
        self.assertEqual(schema["properties"]["schema"]["const"], UI_SCHEMA)
        blocks = _json_fences(text)
        self.assertGreaterEqual(len(blocks), 2)
        estimates = set()
        for block in blocks:
            payload = json.loads(block)
            validate_ui(payload)
            estimates.add(payload["estimate"])
            for emotion, slot in payload["weights"].items():
                self.assertEqual(slot["gas_line"], GAS_LINES[emotion])
                self.assertEqual(slot["element_id"], ELEMENT_IDS[emotion])
        self.assertIn(ESTIMATE_NONE, estimates)
        self.assertIn(ESTIMATE_DISTRIBUTION, estimates)


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
