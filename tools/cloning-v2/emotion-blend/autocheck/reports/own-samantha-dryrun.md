# VoiceCheck report: own-samantha

- **When:** 2026-11-05T22:41:02+0100 (Europe/Zagreb (UTC+1))
- **Persona:** samantha
- **Dry-run:** true
- **Profile:** /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-samantha.json
- **Base recording:** `/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav`
- **Baselines:** {"ecapa_min":0.7057,"wer_max":0.018}
- **Kept fix:** *(none)*

## Attempts

| Label | Voice | WER | ECAPA | Emo dist | ΔF0 Hz | Gate |
|---|---|---:|---:|---:|---:|---|
| baseline | `own-samantha` | 0.0180 | 0.7057 | 0.4651 | -4.35 | reject |

### Gate reasons

- **baseline:** REJECT: ECAPA 0.7057 <= 0.7057 (WER 0.0180 ok vs 0.0180)

## Notes

- profile loaded from /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-samantha.json (galion-voicecheck-profile/1)
- reused merge-v2 score JSON for baseline ( --reuse-scores )
- fix a_ref_expressive skipped: dry-run: would extract high-energy 8s window from base and cbx voice add (no TTS, no voice write)
- fix b_prosody_light skipped: dry-run: would copy voice profile + write prosody sidecar (pace=1.03, pitch=+0.4st)
- fix c_combined skipped: dry-run: would run fix-a then fix-b into combined check voice

