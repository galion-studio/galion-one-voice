#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section F. Runbooks exist, name real commands, and the short commands run.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail() { echo "F FAIL — $*"; exit 1; }
DIR="docs/galion-voice-v2/symphony"
files=(
  README.md
  LOCAL-DEMO.md
  CLONE-WORKER.md
  ONE-DOT.md
  CLIP-STORE.md
  GATES.md
  LATENCY.md
  CPU.md
)
for f in "${files[@]}"; do
  [ -f "$DIR/$f" ] || fail "missing $DIR/$f"
done
for f in LOCAL-DEMO.md CLONE-WORKER.md ONE-DOT.md CLIP-STORE.md GATES.md LATENCY.md CPU.md; do
  grep -q '```bash' "$DIR/$f" || fail "$f has no bash commands"
done
needles=(
  "EPYC 9655P"
  "CX23"
  "CX43"
  "800"
  "80 ms"
  "no GPU"
  "not measured"
  "FRAME_MS"
  "scripts/run-live-demo.sh"
  "scripts/clone/chatterbox_worker.py"
  "scripts/run-v1-screen.sh"
  "GALION_CLIP_STORE"
  "scripts/symphony/self-eval.sh"
  "galion-gates/g5-gq-synthetic.sh"
  "scripts/eval/v2-persona-selftest.sh"
  "clone_worker.rs"
  "1%"
  "Do not import GGUF"
  "protocol-stub"
  "galion-heartbeat"
  "late"
)
blob="$(cat "$DIR"/*.md)"
for n in "${needles[@]}"; do
  grep -q -F "$n" <<<"$blob" || fail "runbooks missing: $n"
done
python3 - <<'PY'
import os
root = "docs/galion-voice-v2/symphony"
for name in os.listdir(root):
    if not name.endswith(".md"):
        continue
    for line in open(os.path.join(root, name), encoding="utf-8"):
        if "gguf" not in line.lower():
            continue
        low = line.lower()
        if not any(tok in low for tok in ("not", "never", "no ", "refuse")):
            raise SystemExit(f"gguf mentioned without a refusal in {name}: {line.strip()}")
PY
scripts=(
  scripts/run-live-demo.sh
  scripts/run-dot-console-demo.sh
  scripts/run-v1-screen.sh
  scripts/one-ui.sh
  scripts/clone/chatterbox_worker.py
  scripts/check-locks.sh
  scripts/eval/v2-persona-selftest.sh
  galion-gates/g5-gq-synthetic.sh
  galion-gates/sy-frame.sh
  scripts/symphony/self-eval.sh
  scripts/symphony/check-g-latency.sh
)
for s in "${scripts[@]}"; do
  [ -f "$s" ] || fail "command target missing: $s"
  case "$s" in
    *.sh) bash -n "$s" || fail "bash -n $s" ;;
    *.py) python3 -m py_compile "$s" || fail "py_compile $s" ;;
  esac
done
bash scripts/one-ui.sh >/dev/null
bash scripts/check-locks.sh >/dev/null
echo "F PASS — runbooks list local demo, clone worker, one-dot, clip store, and self-eval; one-ui and check-locks ran; bash -n clean"
