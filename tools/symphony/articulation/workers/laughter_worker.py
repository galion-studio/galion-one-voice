# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Optional laughter detector. Calls jrgillick/laughter-detection only when
GALION_LAUGHTER_REPO points at a local checkout. Nothing is cloned or downloaded.

usage: python laughter_worker.py IN.wav OUT.json
"""
import json
import os
import sys


def main():
    _wav, out = sys.argv[1], sys.argv[2]
    repo = os.environ.get("GALION_LAUGHTER_REPO", "")
    if not repo or not os.path.isdir(repo):
        json.dump(
            {
                "source": "gap",
                "gap": "GALION_LAUGHTER_REPO is unset; laughter is not detected. Expected a local jrgillick/laughter-detection checkout.",
                "events": [],
            },
            open(out, "w"),
        )
        return
    json.dump(
        {
            "source": "gap",
            "gap": "laughter repo is present but this glue does not import its model weights; run that repo's own detect script and pass events JSON",
            "events": [],
            "repo": repo,
        },
        open(out, "w"),
    )


if __name__ == "__main__":
    main()
