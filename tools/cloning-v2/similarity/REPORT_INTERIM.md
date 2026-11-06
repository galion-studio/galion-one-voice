# Similarity V2 interim (Europe/Zagreb)

## Delivered so far
- **Rust binary** `cloning-v2/similarity/simsel-rs/target/release/simsel`: `prep-cleaned`, `prep-multiclip`, `prep-all`, `best-of-n` (emotion distance primary w=0.65, ECAPA w=0.35), `score-pair`, `scorers-status`, `wait-scorers`.
- **Voices** (V1 untouched): `own-{samantha,jarvis}-v2-sim-{cleaned,multi}` from Python refs; `own-{samantha,jarvis}-v2-sim-rs-{cleaned,multi}` from Rust refs.
- **Synth grid** running under `nice -n 10`, `--intra-threads 2` (heavy box contention from 10 workers → ~60–250s/line).

## Rust vs Python timings
| Task | Rust | Python | Speedup |
|---|---|---|---|
| Ref prep-all (2 personas × cleaned+multi+phrases) | **625 ms** | 8814 ms | **14.1×** |
| ECAPA one file (galion-ecapa vs ref_score.py) | **4451 ms** | 25963 ms | **5.8×** |
| Agreement | cosine **0.6878 = 0.6878** | | exact |

Emotion (`galion-emotion`) one-file wall ~16.3 s (no Python cross-check timed yet). Scorer `READY` markers not written yet; simsel soft-discovers `target/release/galion-*`.

## Early ECAPA (sam-cleaned rp1.2, n=7 vs original ref)
Mean ≈ **0.69** (baseline 0.707±0.030). **Not a clear win** (within/near sd; n incomplete). Cleaned+loudnorm alone does not close the gap — need multi-clip + rp A/B + best-of-N + possibly HF agent’s restored refs.

## Limits from original recordings
- Samantha HF hole (0.4% energy >4 kHz) remains the ceiling driver (0.828 half-vs-half needs presence the recording lacks).
- Short single-take refs (~16–17 s) limit multi-clip diversity.
- Jarvis already near/above short-half ceiling (0.725).

## Next
- Finish priority synth (cleaned 1.0/1.2, multi 1.2 × both voices × 12 lines).
- Batch Rust ECAPA+emotion; best-of-N on weak lines.
- Copy old-vs-new samples to `/workspace/samples/v2/`.
