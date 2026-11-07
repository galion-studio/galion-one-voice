# Laptop runbooks

Ready-to-run checks for two open pull requests. Run them on the laptop.
This directory does not contain their results. Both scripts check
prerequisites, write one JSON file under a fixed cache path, and print a
summary. They do not merge, push, or commit audio.

Own-voice and CC0 audio stay outside this repo. The default cache is

```text
${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks
```

Set `GALION_LAPTOP_OUT` to another absolute path outside the repo if you
need one. A path inside the checkout is refused.

These commands do not edit `LOCKS.toml`, `SOUL.md`, `config.yaml`, persona
TOML, `thresholds.toml`, `capture-spec.toml`, or the files on PR #97.

## PR #87 — fidelity, four clips

Branch `swarm/symphony-a-fidelity-20261106`. The script runs
`tools/cloning-v2/fidelity/scripts/eval-laptop.sh`, which scores **amy**,
**alan**, **long**, and **long-multi** as four separate clips. Amy and Alan
are not averaged. `embed_cosine` is not ECAPA. WER, ECAPA, and
`emotion_match` stay unmeasured when those models are absent. The rollout
gate is printed for reporting only.

From the repo root:

```bash
tools/laptop-runbooks/pr87-fidelity.sh
```

Prerequisite check, no download and no eval:

```bash
tools/laptop-runbooks/pr87-fidelity.sh --prereq-only
```

JSON:

```text
${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks/pr87-fidelity.json
```

CC0 audio and wavs land in `pr87-cache/` under that same directory. Point
`GALION_PR87_ROOT` at an existing checkout of the PR to skip the detached
worktree. Otherwise the script fetches `origin/swarm/symphony-a-fidelity-20261106`
into `worktrees/pr87` and does not merge it.

## PR #86 — Samantha warmth, one file, reported as a blend

Branch `swarm/symphony-b-expression-20261106`. One wav at a time. CLAP
(`laion/clap-htsat-fused`) loads once on the RTX 4060 with
`GALION_CLAP_DEVICE=cuda` and `--gpu-mem-gb 8`. After that process exits,
`score_persona.py --skip-clap --skip-efficientat` writes the emotion2vec+
nine-class posterior on `CPUExecutionProvider` (`emotion2vec_plus_base`,
not the large checkpoint named in `emotions.toml`). The blend JSON keeps
the love decision beside the particles. The decision is not the score.
Warmth is the CLAP love-minus-neutral margin. Anger is the `angry`
posterior. emotion2vec labels are not renamed into love or warmth.
Mixture distance and the four-class SUPERB posterior are left unknown.

Put the wav outside the repo, then:

```bash
tools/laptop-runbooks/pr86-samantha-warmth.sh \
  --wav /absolute/path/outside-the-repo.wav \
  --label samantha-love-window \
  --role reference
```

A clone line is the same command with `--role clone` and its own `--label`.
One file, then the next. The fixed JSON path is overwritten by the latest
run; the per-label work directory under `pr86/work/<label>/` keeps that
file's `clap_observation.json`, `profile.json`, `e2v-cpu.json`, and
`diagnostic.json`.

Prerequisite check (weights, GPU, modules, wav). It does not load CLAP or
emotion2vec+:

```bash
tools/laptop-runbooks/pr86-samantha-warmth.sh \
  --prereq-only \
  --wav /absolute/path/outside-the-repo.wav
```

JSON:

```text
${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks/pr86-samantha-warmth.json
```

Weights must already be on the machine:

- `tools/cloning-v2/scorers/universal/clap/laion-clap-htsat-fused/`
- `tools/cloning-v2/scorers/universal/emotion2vec/emotion2vec_plus_base.onnx`
- `tools/cloning-v2/scorers/universal/emotion2vec/emotion2vec_head.json`

The script does not download them and does not `pip install`. The card must
report a name containing `4060` and at least 7000 MiB (an 8 GB card reports
a bit under 8192). `GALION_ALLOW_OTHER_GPU=1` accepts another card of that
size. The committed fixture
`pipeline/expression/fixtures/samantha_love_gate.json` is passed as `--case`
and is not modified. `pipeline/expression/rust-toolchain.toml` selects Rust
1.85 for that build. `GALION_PR86_ROOT` selects an existing checkout;
otherwise the script fetches `origin/swarm/symphony-b-expression-20261106`
into a detached worktree.

Do not copy the wav, the cache, or `pr86-samantha-warmth.json` into git.
That JSON is an observation of the file you passed.
