# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
"""Synthetic tests for the Emotion V2 capture-kit format + sha256 writer.

Fixtures are silence packed as 24-bit mono 48 kHz WAV. They are not a voice.
No consent record is written.
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import write_manifest as wm  # noqa: E402

WAVE_FORMAT_EXTENSIBLE = 0xFFFE


def pack_pcm24(samples: list[int]) -> bytes:
    out = bytearray()
    for s in samples:
        if s < -0x800000:
            s = -0x800000
        if s > 0x7FFFFF:
            s = 0x7FFFFF
        out.extend(int(s).to_bytes(3, "little", signed=True))
    return bytes(out)


def write_wav(
    path: Path,
    *,
    samples: list[int] | None = None,
    sample_rate: int = 48000,
    bits: int = 24,
    channels: int = 1,
    format_tag: int = 1,
    block_align: int | None = None,
    float_data: bool = False,
    extensible: bool = False,
    frames: int = 4800,
) -> bytes:
    if samples is None:
        samples = [0] * (frames * channels)
    if float_data:
        body = b"".join(struct.pack("<f", float(s) / 0x7FFFFF) for s in samples)
        bits = 32
        format_tag = 3
        block_align = 4 * channels
    elif bits == 16:
        body = b"".join(struct.pack("<h", max(-32768, min(32767, s))) for s in samples)
        block_align = 2 * channels
    elif bits == 24:
        scaled = []
        for s in samples:
            if abs(s) <= 32768:
                scaled.append(s << 8)
            else:
                scaled.append(s)
        body = pack_pcm24(scaled)
        if block_align is None:
            block_align = 3 * channels
    elif bits == 32:
        body = b"".join(struct.pack("<i", int(s)) for s in samples)
        block_align = 4 * channels
    else:
        raise AssertionError("unsupported bits in fixture")

    byte_rate = sample_rate * block_align
    fmt_body = struct.pack(
        "<HHIIHH",
        WAVE_FORMAT_EXTENSIBLE if extensible else format_tag,
        channels,
        sample_rate,
        byte_rate,
        block_align,
        bits if not (extensible and bits == 32) else 24,
    )
    if extensible:
        fmt_body += struct.pack("<HHI", 22, 24, 0x4) + wm.PCM_SUBFORMAT
    fmt_chunk = b"fmt " + struct.pack("<I", len(fmt_body)) + fmt_body
    data_chunk = b"data" + struct.pack("<I", len(body)) + body
    riff = b"WAVE" + fmt_chunk + data_chunk
    file_bytes = b"RIFF" + struct.pack("<I", len(riff)) + riff
    path.write_bytes(file_bytes)
    return file_bytes


class WavFormatTests(unittest.TestCase):
    def test_packed_24_mono_48k_passes_and_hashes_file_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            raw = write_wav(path)
            info = wm.inspect_wav(path)
            self.assertEqual(info["format_verdict"], "PASS")
            self.assertEqual(info["sample_rate"], 48000)
            self.assertEqual(info["bit_depth"], 24)
            self.assertEqual(info["channels"], 1)
            self.assertEqual(info["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertEqual(info["bytes"], len(raw))

    def test_extensible_pcm_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, extensible=True)
            info = wm.inspect_wav(path)
            self.assertEqual(info["format_verdict"], "PASS")

    def test_44100_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, sample_rate=44100)
            with self.assertRaises(wm.FormatError) as ctx:
                wm.inspect_wav(path)
            self.assertIn("sample_rate", str(ctx.exception))

    def test_16_bit_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, bits=16)
            with self.assertRaises(wm.FormatError) as ctx:
                wm.inspect_wav(path)
            self.assertIn("bit_depth", str(ctx.exception))

    def test_stereo_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, channels=2, samples=[0, 0] * 4800)
            with self.assertRaises(wm.FormatError) as ctx:
                wm.inspect_wav(path)
            self.assertIn("channels", str(ctx.exception))

    def test_24_in_32_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, bits=24, block_align=4, frames=100)
            with self.assertRaises(wm.FormatError) as ctx:
                wm.inspect_wav(path)
            self.assertIn("block_align", str(ctx.exception))

    def test_float_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "take-01.wav"
            write_wav(path, float_data=True)
            with self.assertRaises(wm.FormatError) as ctx:
                wm.inspect_wav(path)
            self.assertIn("format tag", str(ctx.exception))



class SessionAndRefsTests(unittest.TestCase):
    def _session_tree(self, tmp: Path) -> Path:
        session = tmp / "samantha" / "2026-11-14-studio"
        (session / "calibration").mkdir(parents=True)
        write_wav(session / "calibration" / "take-01.wav")
        for emo in ("neutral", "calm"):
            folder = session / emo
            folder.mkdir()
            for n in range(1, 4):
                write_wav(folder / f"take-{n:02d}.wav")
        return session

    def test_session_manifest_fields_and_relative_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = self._session_tree(Path(tmp))
            manifest = wm.build_session_manifest(
                session, line="samantha", session_id=None, persona=None
            )
            self.assertEqual(manifest["tool"], "capture-kit")
            self.assertEqual(manifest["standard"], "galion-voice-v2-432")
            self.assertEqual(manifest["consent"], "owner-own-recordings-only")
            self.assertEqual(manifest["originals"], "read-only")
            self.assertEqual(manifest["persona"], "samantha")
            self.assertEqual(manifest["line"], "samantha")
            self.assertEqual(manifest["session_id"], "2026-11-14-studio")
            paths = {f["path"]: f for f in manifest["files"]}
            self.assertIn("calibration/take-01.wav", paths)
            self.assertIsNone(paths["calibration/take-01.wav"]["emotion"])
            self.assertEqual(paths["calibration/take-01.wav"]["role"], "calibration")
            self.assertEqual(paths["neutral/take-01.wav"]["emotion"], "neutral")
            self.assertEqual(paths["neutral/take-01.wav"]["role"], "emotion")
            self.assertIsNone(paths["neutral/take-01.wav"]["verdict"])
            self.assertEqual(paths["neutral/take-01.wav"]["format_verdict"], "PASS")
            self.assertEqual(len(paths["neutral/take-01.wav"]["sha256"]), 64)
            self.assertNotIn("format_errors", manifest)

    def test_session_records_format_fail_without_dropping_the_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = self._session_tree(Path(tmp))
            bad = session / "neutral" / "take-04.wav"
            write_wav(bad, bits=16)
            manifest = wm.build_session_manifest(
                session, line="samantha", session_id=None, persona=None
            )
            row = next(f for f in manifest["files"] if f["path"] == "neutral/take-04.wav")
            self.assertEqual(row["format_verdict"], "FAIL")
            self.assertEqual(len(row["sha256"]), 64)
            self.assertTrue(manifest["format_errors"])

    def test_refs_map_uses_line_emotion_key(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            wav = tmp_path / "originals" / "samantha" / "2026-11-14-studio" / "neutral" / "take-03.wav"
            wav.parent.mkdir(parents=True)
            raw = write_wav(wav)
            out = tmp_path / "per-sector-refs.json"
            payload = wm.update_refs(out, line="samantha", emotion="neutral", wav=wav)
            key = "samantha/neutral"
            self.assertEqual(payload["schema"], "galion-emotion-v2-per-sector-refs/1")
            self.assertIn(key, payload["refs"])
            entry = payload["refs"][key]
            self.assertEqual(entry["line"], "samantha")
            self.assertEqual(entry["emotion"], "neutral")
            self.assertEqual(entry["path"], str(wav))
            self.assertEqual(entry["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertEqual(payload["refs_list"][0]["sha256"], entry["sha256"])
            wav2 = tmp_path / "originals" / "jarvis" / "2026-11-14-studio" / "calm" / "take-01.wav"
            wav2.parent.mkdir(parents=True)
            write_wav(wav2)
            wm.dump_json(out, payload)
            payload2 = wm.update_refs(out, line="jarvis", emotion="calm", wav=wav2)
            self.assertIn("samantha/neutral", payload2["refs"])
            self.assertIn("jarvis/calm", payload2["refs"])

    def test_refs_accepts_list_form_on_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            wav = tmp_path / "take-01.wav"
            write_wav(wav)
            out = tmp_path / "refs.json"
            out.write_text(
                json.dumps(
                    {
                        "schema": "galion-emotion-v2-per-sector-refs/1",
                        "refs": [
                            {
                                "line": "samantha",
                                "emotion": "sad",
                                "path": "old.wav",
                                "sha256": "abc",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            payload = wm.update_refs(out, line="samantha", emotion="neutral", wav=wav)
            self.assertIn("samantha/sad", payload["refs"])
            self.assertIn("samantha/neutral", payload["refs"])
            self.assertEqual(payload["refs"]["samantha/sad"]["path"], "old.wav")

    def test_jarvis_seductive_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            wav = Path(tmp) / "take-01.wav"
            write_wav(wav)
            out = Path(tmp) / "refs.json"
            with self.assertRaises(wm.FormatError) as ctx:
                wm.update_refs(out, line="jarvis", emotion="seductive", wav=wav)
            self.assertIn("not recorded for jarvis", str(ctx.exception).lower())

    def test_cli_check_and_session_and_refs(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = self._session_tree(Path(tmp))
            wav = session / "neutral" / "take-01.wav"
            self.assertEqual(
                wm.main(["check", "--wav", str(wav), "--role", "emotion"]), 0
            )
            out = session / "MANIFEST.json"
            self.assertEqual(
                wm.main(
                    [
                        "session",
                        "--dir",
                        str(session),
                        "--line",
                        "samantha",
                        "--out",
                        str(out),
                    ]
                ),
                0,
            )
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertIn(payload["files"][0]["role"], {"calibration", "emotion"})
            refs_out = Path(tmp) / "per-sector-refs.json"
            self.assertEqual(
                wm.main(
                    [
                        "refs",
                        "--line",
                        "samantha",
                        "--emotion",
                        "neutral",
                        "--wav",
                        str(wav),
                        "--out",
                        str(refs_out),
                    ]
                ),
                0,
            )
            refs = json.loads(refs_out.read_text(encoding="utf-8"))
            self.assertEqual(
                refs["refs"]["samantha/neutral"]["sha256"],
                hashlib.sha256(wav.read_bytes()).hexdigest(),
            )

    def test_unfilled_consent_template_is_refused(self):
        template = (
            Path(__file__).resolve().parents[4]
            / "docs"
            / "emotion-v2"
            / "capture-kit"
            / "consent.template.json"
        )
        self.assertTrue(template.is_file(), "consent template must be in the kit")
        raw = json.loads(template.read_text(encoding="utf-8"))
        self.assertFalse(raw["recording_consent"]["granted"])
        self.assertFalse(raw["training_consent"]["granted"])
        self.assertEqual(raw["recording_consent"]["signer_name"], "")
        self.assertEqual(raw["recording_consent"]["signed_date"], "")
        self.assertEqual(raw["recording_consent"]["signature"], "")
        self.assertEqual(raw["training_consent"]["signer_name"], "")
        self.assertEqual(raw["training_consent"]["signed_date"], "")
        self.assertEqual(raw["training_consent"]["signature"], "")
        with self.assertRaises(wm.ConsentTemplateError):
            wm.refuse_consent_template(template)
        with tempfile.TemporaryDirectory() as tmp:
            wav = Path(tmp) / "take-01.wav"
            write_wav(wav)
            rc = wm.main(
                ["check", "--wav", str(wav), "--consent", str(template)]
            )
            self.assertEqual(rc, 2)

    def test_cli_does_not_write_a_consent_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = self._session_tree(Path(tmp))
            consent_shaped = Path(tmp) / "consent.json"
            consent_shaped.write_text(
                json.dumps(
                    {
                        "recording_consent": {"granted": False},
                        "training_consent": {"granted": False},
                    }
                ),
                encoding="utf-8",
            )
            rc = wm.main(
                [
                    "session",
                    "--dir",
                    str(session),
                    "--line",
                    "samantha",
                    "--out",
                    str(consent_shaped),
                ]
            )
            self.assertEqual(rc, 2)
            leftover = json.loads(consent_shaped.read_text(encoding="utf-8"))
            self.assertIn("recording_consent", leftover)
            self.assertNotIn("files", leftover)


if __name__ == "__main__":
    unittest.main()
