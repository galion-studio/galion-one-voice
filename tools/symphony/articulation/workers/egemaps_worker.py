# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""eGeMAPSv02 via openSMILE, called as-is. No feature is reimplemented here.

usage: python egemaps_worker.py IN.wav OUT.json
Exits 0 with a gap JSON when the `opensmile` package is not installed.
"""
import json
import sys


def main():
    wav, out = sys.argv[1], sys.argv[2]
    try:
        import opensmile
    except ImportError:
        json.dump(
            {
                "source": "opensmile",
                "gap": "opensmile is not installed; eGeMAPSv02 was not computed",
                "features": {},
            },
            open(out, "w"),
        )
        return
    smile = opensmile.Smile(
        feature_set=opensmile.FeatureSet.eGeMAPSv02,
        feature_level=opensmile.FeatureLevel.Functionals,
    )
    frame = smile.process_file(wav)
    features = {str(k): float(v) for k, v in frame.iloc[0].items()}
    json.dump(
        {
            "source": "opensmile eGeMAPSv02",
            "features": features,
            "bricks": [
                {
                    "name": "openSMILE",
                    "version": getattr(opensmile, "__version__", "unknown"),
                    "licence": "audEERING (research binary; wrapper as packaged)",
                    "lang": "Python",
                    "process": "out of process",
                }
            ],
        },
        open(out, "w"),
    )


if __name__ == "__main__":
    main()
