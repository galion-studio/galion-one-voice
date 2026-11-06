# Agent 3 inventory — persona-aware rescore harness

Generated: 2026-11-06 Europe/Zagreb. Read-only inventory; no clone re-renders; no deploy.

## Sets

### (a) Original-voice probe lines
- Manifest: `scorer-probe/lines.json`
- WAVs: `scorer-probe/wav/{samantha,jarvis}/*.wav`
- Samantha: 20 wavs (cry×4, sed×4, neu×2, joy×2, fea×2, sad×2, sur×2, dis×2). Missing agg/lov/lau renders for Samantha.
- Jarvis: 26 wavs (above + agg×2, lov×2, lau×2).
- Existing universal raw scores: `scorer-probe/results/probe_universal_all.json` (46 files, schema `galion-universal-emotion/1`) and `SUMMARY.json` (presence 0.2). **Reuse as OLD for probe.**

### (b) Ceilings (read-only originals)
- Samantha: `/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav` (17.407188 s, 44100 Hz stereo)
- Jarvis: `/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/original.wav` (15.9520625 s, 48000 Hz stereo)
- Existing universal match: `scorers/universal/results/{samantha,jarvis}-universal/RESULTS.json` → `BASE-self-ceiling`. **Reuse as OLD.**

### (c) Clone merge-v2 (12-line renders)
- Samantha voice: `own-samantha-v2-emo-r2` → dirs:
  - `cloning-v2/merge-v2/wav/samantha/t01..t12.wav`
  - identical sha of `t01` to `/workspace/share/cloning-v2-listen/pkg/renders/samantha-merge-v2/`
- Jarvis voice: `own-jarvis` → `cloning-v2/merge-v2/wav/jarvis/` (= share `jarvis-merge-v2`)
- Report: `share/.../pkg/reports/merge-v2-results12.json` and `cloning-v2/merge-v2/results12.json`
- Existing universal match set name `merge-v2` in RESULTS.json. **Reuse as OLD.**

### (d) Clone coherent (blendfix-coherent)
- Samantha: `own-samantha-v2-blendfix-coherent` → `cloning-v2/emotion-blend/global-fix/wav/samantha/` (= share `samantha-coherent`)
- Jarvis: `own-jarvis-v2-blendfix-coherent` → `.../global-fix/wav/jarvis/`
- Report: `share/.../pkg/reports/coherent-results12.json`
- Existing universal match set name `global-fix-coherent` in RESULTS.json. **Reuse as OLD.**

## Universal formula (unchanged scorer)
`emotion_match = 0.70 × category_blend + 0.30 × emb_sim`, presence ≥ 0.2.
Gate: emotion_match ≥ 0.83 (persona emotions only once persona scorer lands).

## OLD emotion_match (from RESULTS.json, same inputs)

| voice | set | emotion_match | category_blend | emb_sim | gate 0.83 |
|---|---|---|---|---|---|
| samantha | ceiling | 0.9043674822327858 | 0.8633821174754084 | 1.0 | PASS |
| samantha | merge-v2 | 0.7738271410896993 | 0.7279109709606062 | 0.8809648713909163 | FAIL |
| samantha | coherent | 0.750903466379144 | 0.716687174659952 | 0.8307414803905919 | FAIL |
| jarvis | ceiling | 0.9475098448700927 | 0.9250140641001326 | 1.0 | PASS |
| jarvis | merge-v2 | 0.880208326849959 | 0.892812455907453 | 0.8507986923824734 | PASS |
| jarvis | coherent | 0.8900818729989053 | 0.8807917200834441 | 0.9117588964683153 | PASS |

Samantha moments present (≥0.2): aggressive, neutral, love, yelling, laughter, logic.
Jarvis moments present (≥0.2): neutral, yelling, logic.
Missing from originals (so not in universal blend): joyful/disgust/fear/sad/surprise/seductive/crying (both); also aggressive/love/laughter for Jarvis.

## Persona scorer wait
- Expect: `scorers/persona/INTERFACE.md`
- Expect: `pipeline/personas/own-samantha.toml`, `own-jarvis.toml`
- Expect: `scorers/persona/detectors/love_seductive*`
- Own files for Agent 3: this directory + `rescore.py` / `rescore.sh` only.
