# W20-13 STATUS — Soft blocker: VPS R0–R3
*2026-11-02 ~12:40 CET (UTC+1)*

- Read `w3-briefs/12-DEPLOY.md`; inventoried deploy scripts; produced `R0-R3-DRY-RUN.md`.
- Verified `harden-dryrun.sh` / `release-dryrun.sh`: exit 0 print-only, `--execute` → exit 2.
- **Finding:** `remote-r0-dryrun.sh` claimed in brief but absent on disk.
- **Finding:** `run-gates.sh` will SSH if `GALION_VPS_HOST` is set (single env-var gate) — suggest double opt-in.
- No SSH, no VPS contact, no git push. No files besides these two docs changed.
- Blocker: owner steps (root pw rotation, key-only SSH, `galion` user, K4).
