# VoiceCheck report: own-jarvis

- **When:** 2026-11-05T22:41:02+0100 (Europe/Zagreb (UTC+1))
- **Persona:** jarvis
- **Dry-run:** true
- **Profile:** /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-jarvis.json
- **Base recording:** `/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/original.wav`
- **Baselines:** {"ecapa_min":0.7331,"wer_max":0.019}
- **Kept fix:** *(none)*

## Attempts

| Label | Voice | WER | ECAPA | Emo dist | ΔF0 Hz | Gate |
|---|---|---:|---:|---:|---:|---|
| baseline | `own-jarvis` | 0.0190 | 0.7331 | 0.5020 | +9.42 | reject |

### Gate reasons

- **baseline:** REJECT: ECAPA 0.7331 <= 0.7331 (WER 0.0190 ok vs 0.0190)

## Notes

- profile loaded from /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/analysis/profile-jarvis.json (galion-voicecheck-profile/1)
- reused merge-v2 score JSON for baseline ( --reuse-scores )
- fix a_ref_expressive skipped: dry-run: would extract high-energy 8s window from base and cbx voice add (no TTS, no voice write)
- fix b_prosody_light skipped: dry-run: would copy voice profile + write prosody sidecar (pace=1.03, pitch=+0.4st)
- fix c_combined skipped: dry-run: would run fix-a then fix-b into combined check voice

