# VoiceCheck report: own-samantha

- **When:** 2026-11-05T22:57:13+0100 (Europe/Zagreb (UTC+1))
- **Persona:** samantha
- **Dry-run:** true
- **Profile:** /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-samantha.json
- **Base recording:** `/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav`
- **Baselines:** {"ecapa_min":0.7057,"wer_max":0.018}
- **Kept fix:** *(none)*

## Attempts

| Label | Voice | WER | ECAPA | Emo dist | ΔF0 Hz | Gate |
|---|---|---:|---:|---:|---:|---|
| baseline | `own-samantha` | 0.0150 | 0.7081 | 0.4373 | +2.98 | KEEP |

### Gate reasons

- **baseline:** KEEP: WER 0.0150 <= 0.0180 and ECAPA 0.7081 > 0.7057

## Notes

- profile loaded from /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-samantha.json (galion-voicecheck-profile/1)
- fixes skipped (--skip-fixes)

