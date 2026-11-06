# VoiceCheck profile format (`galion-voicecheck-profile/1`)

Worker B (`autocheck`) reads `emotion-blend/analysis/profile-<persona>.json` when present.
Worker A may also publish `A-global/profile.json` (`galion-emotion-blend/1`); voicecheck accepts both.

## Preferred path

`emotion-blend/analysis/profile-samantha.json` / `profile-jarvis.json`

```json
{
  "schema": "galion-voicecheck-profile/1",
  "persona": "samantha",
  "base_recording": "/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav",
  "reference_wav": "/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav",
  "baselines": {
    "wer_max": 0.018,
    "ecapa_min": 0.7057,
    "note": "Samantha merge-v2 production; keep fix only if WER<=wer_max AND ECAPA>ecapa_min"
  },
  "base": {
    "f0_mean_hz": 190.08,
    "f0_sd_st": 2.956,
    "f0_range_st": 9.112,
    "energy_sd_db": 9.320,
    "rms_dbfs": -20.956,
    "pace_syl_s": 2.879,
    "ser": { "neu": 0.350, "hap": 0.213, "ang": 0.119, "sad": 0.318 },
    "avd": { "arousal": 0.499, "valence": 0.553, "dominance": 0.569 }
  }
}
```

Fields under `base` mirror `galion-emotion` proxies + SER/AVD so line-by-line deltas are comparable.
If only `galion-emotion-blend/1` exists, voicecheck maps `voices.<persona>.original.whole` → `base`.
