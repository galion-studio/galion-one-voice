#!/usr/bin/env bash
# DRY RUN ONLY. Prints the release gate + install steps. Never connects anywhere.
set -euo pipefail
for a in "$@"; do [[ "$a" == "--execute" ]] && { echo "refusing: dry-run only (VPS blocked)"; exit 2; }; done
SHA="${1:-<commit-sha>}"; VER="${2:-<version>}"
cat <<STEPS
[dry-run] release $VER @ $SHA  (no connection will be made)
gate: [ ] pushed  [ ] CI green on $SHA  [ ] artifact sha256 matches CI  [ ] human approval
1. copy prebuilt artifact galion-voice-$VER.tar.gz to /opt/galion/$VER/ (no build on VPS)
2. sha256sum -c galion-voice-$VER.sha256
3. ln -sfn /opt/galion/$VER /opt/galion/current
4. sudo systemctl restart galion-voice   (scoped sudo)
5. rollback: ln -sfn /opt/galion/<prev> /opt/galion/current && restart
6. append LEDGER entry: who, $SHA, artifact hash, time
STEPS
