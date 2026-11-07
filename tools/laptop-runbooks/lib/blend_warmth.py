#!/usr/bin/env python3
"""Blend one PR #86 warmth/love pass into a fixed JSON report.

CLAP love/neutral/seductive numbers come from clap_observation.json (one CUDA
pass). The nine-class posterior comes from e2v-cpu.json (emotion2vec+ on
CPUExecutionProvider). Prosody comes from profile.json. A field the inputs
do not contain stays null. Mixture distance is not computed here.

The love decision is stored beside the blend and is not the score.
emotion2vec+ labels are not renamed into love or warmth.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
from pathlib import Path

E2V_LABELS = (
    "angry",
    "disgusted",
    "fearful",
    "happy",
    "neutral",
    "other",
    "sad",
    "surprised",
    "unknown",
)


def load_json(path: Path | None) -> dict | None:
    if path is None or not path.is_file():
        return None
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return doc if isinstance(doc, dict) else None


def num(value: object) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def prosody_of(profile: dict | None) -> dict[str, float | None]:
    empty = {"f0_sd_st": None, "rms_db": None, "spectral_tilt_db_oct": None}
    if not profile:
        return empty
    channels = profile.get("channels")
    if not isinstance(channels, list) or not channels or not isinstance(channels[0], dict):
        return empty
    prosody = channels[0].get("prosody")
    if not isinstance(prosody, dict):
        return empty
    return {
        "f0_sd_st": num(prosody.get("f0_sd_st")),
        "rms_db": num(prosody.get("rms_db")),
        "spectral_tilt_db_oct": num(prosody.get("spectral_tilt_db_oct")),
    }


def posterior_of(e2v_doc: dict | None) -> tuple[dict[str, float] | None, str | None, str | None]:
    if not e2v_doc:
        return None, None, None
    files = e2v_doc.get("files")
    if not isinstance(files, list) or not files or not isinstance(files[0], dict):
        return None, None, None
    e2v = files[0].get("e2v")
    if not isinstance(e2v, dict):
        return None, None, None
    probs = e2v.get("probs")
    if not isinstance(probs, dict):
        return None, None, None
    posterior: dict[str, float] = {}
    for label in E2V_LABELS:
        value = num(probs.get(label))
        if value is None:
            return None, None, None
        posterior[label] = value
    top = e2v.get("top")
    model = e2v.get("model")
    return posterior, top if isinstance(top, str) else None, model if isinstance(model, str) else None


def particle(name: str, source: str, value: float | None) -> dict:
    return {
        "name": name,
        "source": source,
        "value": value,
        "status": "measured" if value is not None else "unknown",
    }


def build(args: argparse.Namespace) -> dict:
    clap = load_json(args.clap)
    profile = load_json(args.profile)
    e2v_doc = load_json(args.e2v)
    diagnostic = load_json(args.diagnostic)
    posterior, top, model = posterior_of(e2v_doc)
    prosody = prosody_of(profile)

    love = num(clap.get("love_mean_cos")) if clap else None
    neutral = num(clap.get("neutral_mean_cos")) if clap else None
    sed = num(clap.get("sed_mean_cos")) if clap else None
    margin = (love - neutral) if love is not None and neutral is not None else None
    love_decision = num(clap.get("love")) if clap else None
    if love_decision is None and clap:
        love_decision = num(clap.get("decision"))
    sed_decision = num(clap.get("seductive")) if clap else None

    particles = [
        particle("love", "clap love_mean_cos", love),
        particle("neutral", "clap neutral_mean_cos", neutral),
        particle("seductive", "clap sed_mean_cos", sed),
        particle(
            "warmth",
            "clap love_mean_cos minus neutral_mean_cos; not an emotion2vec class",
            margin,
        ),
        particle(
            "anger",
            "emotion2vec+ angry posterior; not renamed from another class",
            None if posterior is None else posterior.get("angry"),
        ),
    ]
    for label in E2V_LABELS:
        particles.append(
            particle(
                f"emotion2vec.{label}",
                "emotion2vec_plus_base ONNX posterior on CPUExecutionProvider",
                None if posterior is None else posterior[label],
            )
        )
    particles.extend(
        [
            particle("f0_sd_st", "profile.json channels[0].prosody", prosody["f0_sd_st"]),
            particle("rms_db", "profile.json channels[0].prosody", prosody["rms_db"]),
            particle(
                "spectral_tilt_db_oct",
                "profile.json channels[0].prosody",
                prosody["spectral_tilt_db_oct"],
            ),
        ]
    )

    wav = Path(args.wav) if args.wav else None
    verdict = None
    if diagnostic and isinstance(diagnostic.get("verdict"), str):
        verdict = diagnostic["verdict"]

    unknown = [p["name"] for p in particles if p["status"] == "unknown"]
    return {
        "schema": "galion-laptop-warmth-blend/1",
        "pr": 86,
        "voice": "own-samantha",
        "role": args.role,
        "label": args.label,
        "wav_basename": wav.name if wav else None,
        "wav_sha256": sha256_file(wav) if wav and wav.is_file() else None,
        "wav_path_stored": False,
        "audio_committed": False,
        "clap": {
            "device": "cuda",
            "gpu_mem_gb": 8,
            "model": "laion/clap-htsat-fused",
            "one_file": True,
            "love_mean_cos": love,
            "neutral_mean_cos": neutral,
            "sed_mean_cos": sed,
            "margin_love_minus_neutral": margin,
            "love_decision": love_decision,
            "seductive_decision": sed_decision,
            "love_decision_is_the_score": False,
        },
        "emotion2vec_plus": {
            "provider": "CPUExecutionProvider",
            "model_reported": model,
            "model_expected": "emotion2vec/emotion2vec_plus_base (ONNX)",
            "not_the_large_checkpoint": True,
            "posterior": posterior,
            "top": top,
            "labels": list(E2V_LABELS),
        },
        "prosody": prosody,
        "particles": particles,
        "mixture_distance": None,
        "mixture_distance_note": "unknown. These commands do not produce it, and it is not invented.",
        "superb_posterior": None,
        "superb_posterior_note": "unknown. Not produced by this pass.",
        "committed_fixture_diagnostic": verdict,
        "committed_fixture_note": (
            "diagnostic.json is the committed samantha_love_gate.json case. "
            "eval --score does not insert this wav, so that verdict is not this blend."
        ),
        "unknown_particles": unknown,
    }


def print_summary(doc: dict, result: Path) -> None:
    print(f"pr86 samantha warmth blend: {result}")
    print(
        f"  label={doc['label']} role={doc['role']} "
        f"wav={doc['wav_basename']} sha256={doc['wav_sha256']}"
    )
    clap = doc["clap"]
    print(
        "  clap cuda one file: "
        f"love_mean_cos={_fmt(clap['love_mean_cos'])} "
        f"neutral_mean_cos={_fmt(clap['neutral_mean_cos'])} "
        f"margin={_fmt(clap['margin_love_minus_neutral'])} "
        f"love_decision={_fmt(clap['love_decision'])} (decision is not the score)"
    )
    posterior = doc["emotion2vec_plus"]["posterior"]
    if posterior is None:
        print("  emotion2vec+ CPU posterior: unknown")
    else:
        joined = " ".join(f"{k}={posterior[k]:.6f}" for k in E2V_LABELS)
        print(f"  emotion2vec+ CPU posterior: {joined} top={doc['emotion2vec_plus']['top']}")
    prosody = doc["prosody"]
    print(
        "  prosody: "
        f"f0_sd_st={_fmt(prosody['f0_sd_st'])} "
        f"rms_db={_fmt(prosody['rms_db'])} "
        f"spectral_tilt_db_oct={_fmt(prosody['spectral_tilt_db_oct'])}"
    )
    print("  warmth and anger stay in the particle list")
    print("  mixture_distance: unknown")
    print("  superb posterior: unknown")
    if doc["unknown_particles"]:
        print("  unknown: " + ", ".join(doc["unknown_particles"]))


def _fmt(value: object) -> str:
    if value is None:
        return "unknown"
    return str(value)


def self_check() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        wav = root / "clip.wav"
        wav.write_bytes(b"not-audio-schema-check")
        clap_path = root / "clap_observation.json"
        # e2v_sad here is the empty-map default the love plugin writes.
        # The blend must not treat it as a posterior.
        clap_path.write_text(
            json.dumps(
                {
                    "love_mean_cos": 0.5,
                    "neutral_mean_cos": 0.25,
                    "sed_mean_cos": 0.125,
                    "love": 1.0,
                    "seductive": 0.0,
                    "e2v_sad": 0.0,
                }
            ),
            encoding="utf-8",
        )
        e2v_path = root / "e2v-cpu.json"
        probs = {label: 0.0 for label in E2V_LABELS}
        probs["angry"] = 0.5
        probs["happy"] = 0.5
        e2v_path.write_text(
            json.dumps(
                {
                    "files": [
                        {
                            "file": "/local/clip.wav",
                            "e2v": {
                                "probs": probs,
                                "top": "happy",
                                "model": "emotion2vec/emotion2vec_plus_base (ONNX ziyu12345 export)",
                            },
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )
        profile_path = root / "profile.json"
        profile_path.write_text(
            json.dumps(
                {
                    "channels": [
                        {
                            "prosody": {
                                "f0_sd_st": 1.5,
                                "rms_db": -20.0,
                                "spectral_tilt_db_oct": -3.0,
                            }
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )
        args = argparse.Namespace(
            wav=str(wav),
            label="schema-check",
            role="unspecified",
            clap=clap_path,
            profile=profile_path,
            e2v=e2v_path,
            diagnostic=None,
        )
        doc = build(args)
        assert doc["wav_path_stored"] is False
        assert doc["audio_committed"] is False
        assert "/local/clip.wav" not in json.dumps(doc)
        assert doc["clap"]["margin_love_minus_neutral"] == 0.25
        assert doc["clap"]["love_decision_is_the_score"] is False
        assert doc["emotion2vec_plus"]["posterior"]["angry"] == 0.5
        assert doc["emotion2vec_plus"]["posterior"]["sad"] == 0.0
        assert doc["mixture_distance"] is None
        assert doc["superb_posterior"] is None
        by = {p["name"]: p for p in doc["particles"]}
        assert by["warmth"]["value"] == 0.25
        assert by["anger"]["value"] == 0.5
        assert by["emotion2vec.sad"]["value"] == 0.0
        assert by["emotion2vec.sad"]["source"].startswith("emotion2vec_plus_base")

        empty = build(
            argparse.Namespace(
                wav=None,
                label="empty",
                role="unspecified",
                clap=None,
                profile=None,
                e2v=None,
                diagnostic=None,
            )
        )
        assert all(p["value"] is None for p in empty["particles"])
        assert empty["emotion2vec_plus"]["posterior"] is None
        assert empty["mixture_distance"] is None
        assert empty["clap"]["love_decision"] is None
    print("blend_warmth schema_check=pass measured=false")
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Write the Samantha warmth blend JSON")
    parser.add_argument("--self-check", action="store_true")
    parser.add_argument("--wav")
    parser.add_argument("--label", default="unspecified")
    parser.add_argument("--role", default="unspecified")
    parser.add_argument("--clap", type=Path)
    parser.add_argument("--profile", type=Path)
    parser.add_argument("--e2v", type=Path)
    parser.add_argument("--diagnostic", type=Path)
    parser.add_argument("--out", type=Path)
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if args.self_check:
        return self_check()
    if args.out is None:
        print("pass --out, or --self-check", file=sys.stderr)
        return 2
    doc = build(args)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print_summary(doc, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
