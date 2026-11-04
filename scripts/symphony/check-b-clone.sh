#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section B. Cites the clone-worker contract and runs the protocol smoke.
# protocol-stub PCM is not a landed voice. Ref hashes are computed only when the wav is on disk.
set -euo pipefail
cd "$(dirname "$0")/../.."
WORKER="scripts/clone/chatterbox_worker.py"
RUST="crates/galion-ml-voice/src/speech/clone_worker.rs"
fail() { echo "B FAIL — $*"; exit 1; }
[ -f "$WORKER" ] || fail "missing $WORKER"
[ -f "$RUST" ] || fail "missing $RUST"
grep -q 'two_turns_reuse_one_clone_process_and_the_clock_is_not_a_product_claim' "$RUST" || fail "clone clock test not cited in clone_worker.rs"
grep -q 'reference clips are on the rights hold' "$WORKER" || fail "worker does not refuse a reference clip"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
printf '%s\n' '{"text":"Hello.","model":"nano","persona":"jarvis","tags":true}' \
  | GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$TMP/pcm" 2>"$TMP/err" || fail "stub worker exited non-zero"
[ -s "$TMP/pcm" ] || fail "stub worker wrote no PCM"
PCM_BYTES="$(wc -c <"$TMP/pcm" | tr -d ' ')"
grep -q 'galion-clone-engine: protocol-stub' "$TMP/err" || fail "engine was not protocol-stub"
grep -q 'galion-clone-persona: jarvis' "$TMP/err" || fail "persona marker missing"
grep -q 'galion-clone-utt:' "$TMP/err" || fail "utt marker missing"

set +e
printf '%s\n' '{"text":"Hello.","reference":"/tmp/galion-not-a-clip.wav"}' \
  | GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$TMP/pcm2" 2>"$TMP/err2"
RC=$?
set -e
[ "$RC" -eq 3 ] || fail "reference clip must exit 3, got $RC"
grep -q 'galion-clone-refuse: reference clips are on the rights hold' "$TMP/err2" || fail "refuse line missing"
[ ! -s "$TMP/pcm2" ] || fail "refused request wrote PCM"

REF_NOTE="$(python3 - <<'PY'
import hashlib, os, sys
pins = {
    "samantha": "bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693",
    "jarvis": "c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60",
}
store = os.environ.get("GALION_CLIP_STORE", "").strip()
found = 0
for persona, expect in pins.items():
    paths = [f"docs/galion-voice-v2/personas/refs/{persona}.wav"]
    if store:
        paths.append(os.path.join(store, persona, "reference.wav"))
    for path in paths:
        if not os.path.isfile(path):
            continue
        found += 1
        digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
        if digest != expect:
            print(f"mismatch {path} measured={digest} expected={expect}", file=sys.stderr)
            sys.exit(1)
        print(f"measured {persona} {digest}")
if found == 0:
    print("ref_sha256=not-measured")
else:
    print(f"ref_sha256=measured files={found}")
PY
)" || fail "reference sha256 mismatch"
REF_ONE="$(printf '%s\n' "$REF_NOTE" | tail -n 1)"
echo "B PASS — cited scripts/clone/chatterbox_worker.py and crates/galion-ml-voice/src/speech/clone_worker.rs (clock is not a product claim); protocol-stub pcm_bytes=${PCM_BYTES}; ${REF_ONE}; landing=not-a-voice"
