#!/usr/bin/env bash
# DRY RUN ONLY. Prints the hardening steps a human would perform. Never connects anywhere.
set -euo pipefail
for a in "$@"; do [[ "$a" == "--execute" ]] && { echo "refusing: dry-run only (VPS blocked)"; exit 2; }; done
H="${1:-<host>}"
cat <<STEPS
[dry-run] target: $H  (no connection will be made)
1. (provider console, human) rotate root password; store in vault
2. adduser --disabled-password galion
3. install operator ed25519 pubkey -> /home/galion/.ssh/authorized_keys (700/600)
4. verify: ssh galion@$H   (second session)
5. write /etc/ssh/sshd_config.d/10-galion.conf:
   PermitRootLogin no / PasswordAuthentication no / AllowUsers galion
6. sshd -t && systemctl reload ssh ; re-verify new session
7. ufw default deny incoming ; ufw allow 22/tcp ; ufw enable
8. web stays on 127.0.0.1; access: ssh -N -L 8080:127.0.0.1:8080 galion@$H
STEPS
