#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section D. Cites the persona docs, the empathy map, and the HTTP self-test.
# The HTTP half needs a live page. This check does not start one.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail() { echo "D FAIL — $*"; exit 1; }
for f in \
  docs/galion-voice-v2/personas/JARVIS.md \
  docs/galion-voice-v2/personas/SAMANTHA.md \
  docs/galion-voice-v2/EMPATHY.md \
  docs/galion-voice-v2/personas/voice/jarvis.toml \
  docs/galion-voice-v2/personas/voice/samantha.toml \
  scripts/eval/v2-persona-selftest.sh \
  galion-emotion/src/express.rs
do
  [ -f "$f" ] || fail "missing $f"
done
grep -q '## Emotions' docs/galion-voice-v2/personas/JARVIS.md || fail "Jarvis emotions section missing"
grep -q '## Emotions' docs/galion-voice-v2/personas/SAMANTHA.md || fail "Samantha emotions section missing"
grep -q 'valence / 0.6' docs/galion-voice-v2/EMPATHY.md || fail "empathy weight formula missing"
grep -q 'oklch(0.72 0.09 310)' docs/galion-voice-v2/EMPATHY.md || fail "OKLCH wash anchor missing"
grep -q 'fn empathy_weight' galion-emotion/src/express.rs || fail "empathy_weight missing"
grep -q 'fn voice_color' galion-emotion/src/express.rs || fail "voice_color missing"
grep -q 'EMPATHY_OKLCH' galion-emotion/src/express.rs || fail "EMPATHY_OKLCH missing"
grep -q 'EMPATHY_WASH_AMBER' galion-emotion/src/express.rs || fail "wash mix missing"
grep -q 'empathy_at_one_is_the_profile_and_heather' galion-emotion/src/express.rs || fail "heather test not cited"
grep -q '\[register.prosody.e0\]' docs/galion-voice-v2/personas/voice/jarvis.toml || fail "Jarvis prosody table missing"
grep -q '\[register.prosody.e0\]' docs/galion-voice-v2/personas/voice/samantha.toml || fail "Samantha prosody table missing"
grep -q 'v2-persona-selftest' scripts/eval/v2-persona-selftest.sh || fail "persona self-test script missing its name"
python3 - <<'PY'
import os
phrases = (
    "I know this is difficult.",
    "I'm worried about you.",
    "Take a deep breath for me.",
)
roots = ("crates", "scripts", "galion-demo", "galion-emotion", "galion-render")
hits = []
for root in roots:
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in (".git", "target")]
        for name in files:
            if not name.endswith((".rs", ".py", ".html", ".js", ".toml")):
                continue
            path = os.path.join(dirpath, name)
            try:
                text = open(path, encoding="utf-8", errors="replace").read()
            except OSError:
                continue
            for phrase in phrases:
                if phrase in text:
                    hits.append(f"{path}: {phrase}")
if hits:
    raise SystemExit("scripted empathy line in code:\n" + "\n".join(hits))
PY
echo "D PASS — cited scripts/eval/v2-persona-selftest.sh (HTTP half not run), EMPATHY.md, and galion-emotion voice_color; prosody and OKLCH mapped; scripted empathy lines absent from code"
