# Missing-emotion targets (opt-in pipeline stage)

Universal stage every voice can go through the same way:

1. Score the original (2.5 s windows) with the **unmodified** universal scorer.
2. Detect which scorer categories fall below presence **0.2**.
3. Synthesize configured probe lines for each missing emotion (`cbx` batch, special tokens + `--pace`).
4. Score those probes with the same unmodified scorer.
5. Write `TARGETS.json`: per-emotion reference targets marked `original-present` or `synthesized`.

Default behaviour is unchanged. Nothing runs unless `--missing-emotion-targets` is passed to `pipeline/bin/cbx-add-voice`, or a *new* per-voice config sets `missing_emotion_targets.enabled=true`. Deployed `own-samantha` / `own-jarvis` configs are not touched.

The scorer, its models and thresholds, existing clones and `.cbxvoice` files, and locked files are not modified. Originals are read-only. Probe profiles use a new name `emo-tgt-<voice>` and `--out-file` under the work directory, then a create_new copy into the cbx voice cache so `cbx batch --voice` can find them. Existing cache voices are never overwritten. `--resume` reuses files already in `--out-dir` instead of refusing (no deletes).

## Standalone

```
nice -n 10 pipeline/missing-emotion-targets/target/release/galion-emotion-targets \
  --voice own-samantha \
  --original /path/to/original.wav \
  --out-dir NEW_DIR
```

Opt-in persona-aware scoring (default remains `universal`; does not edit `scorers/`):

```
.../galion-emotion-targets --voice own-samantha --original ORIGINAL.wav --out-dir NEW_DIR \
  --scorer persona [--persona-file pipeline/personas/own-samantha.toml]
```

`cbx-add-voice --missing-emotion-targets --targets-work-dir NEW_DIR --targets-scorer persona`
(or config `missing_emotion_targets.scorer: "persona"`). Without that flag the stage still
invokes `scorers/universal/py/score_universal.py`.

## Voice-add hook (opt-in)

```
pipeline/bin/cbx-add-voice --missing-emotion-targets --targets-work-dir NEW_DIR \
  --name own-PERSONA-v2-... --voice-wav ORIGINAL.wav --dtype fp32
```

`--no-missing-emotion-targets` overrides a config opt-in. Without the flag, `cbx-add-voice` still execs the existing cbx voice-add path only.

## Target file

`NEW_DIR/TARGETS.json` (`schema: galion-emotion-targets/1`):

- `original-present`: `target` = detector strength on the best original window (same `strength()` as `universal_match.py`).
- `synthesized`: `target` = detector strength on the best probe line; `best_probe` / `moment_wav` point at that wav so a later gate can use it as the missing moment instead of skipping the category.

Aliases: `happy` → `joyful`, `reason` → `logic` (not synthesized separately).

## What a deploy would change (not done; needs explicit yes)

1. Build the release binary (if not already built on this box):
   ```
   nice -n 10 cargo test --offline -j 2 --manifest-path pipeline/missing-emotion-targets/Cargo.toml
   nice -n 10 cargo build --release --offline -j 2 --manifest-path pipeline/missing-emotion-targets/Cargo.toml
   ln -sfn ../missing-emotion-targets/target/release/galion-emotion-targets pipeline/bin/galion-emotion-targets
   ```
2. Keep default off. To enable for a *new* candidate voice only, copy `pipeline/voice-defaults/missing-emotion-targets.example.json` into that candidate’s `voice-defaults/<new-name>.json` (never into `own-samantha.json` / `own-jarvis.json` without a separate yes).
3. Future gate hook (not implemented): `galion-voicecheck` would grow an opt-in `--emotion-targets TARGETS.json` and treat each `moment_wav` as the base moment for that category. Scorer code, `emo_min=0.83`, and presence `0.2` would stay unchanged.
4. No git merge, push, or live voice switch is part of this stage.

## Extension hooks (persona scorer is opt-in; default remains universal)

This crate invokes the current unmodified universal scorer and does not edit `scorers/`. A new scorer version living *next to* the current one, or a persona-specific probe set, can plug in here without changing default behaviour:

- **Scorer selection.** `--scorer universal` (default) or `--scorer persona`. Resolver: `scorer.rs` (`parse_scorer_kind`, `resolve_scorer`). Universal still uses `scorers/universal/py/score_universal.py`. Persona reads `scorers/persona/INTERFACE.md` when present (script / python / `--persona` vs `--persona-file`); otherwise the conventional stub path `scorers/persona/py/score_persona.py`. `--scorer-script` overrides the script (tests). Invocation is `invoke::score_with()` (windows, then probes).
- **Score JSON adapter.** `detect.rs` (`parse_score_file`, `parse_probe_scores`, `strength`) expects the current `score_universal` shape: `files[].e2v.probs`, `files[].clap.probs`, `files[].efficientat.<cat>.max`. A persona-aware scorer should emit that schema, or a thin adapter belongs in `detect.rs` — not in the existing scorer files.
- **Persona target-set / probe texts.** `--catalog` loads `probe_catalog.json` (texts, `--pace`, tokenizer tags per emotion). A persona set is another catalog file passed with `--catalog`, or resolved from `--voice`. `catalog.rs` (`BLEND_IDS`, `source_of`, `e2v_trained_key`) is the category map.
- **Voice-add hook.** `pipeline/bin/cbx-add-voice` already runs this binary when opted in (`EMOTION_TARGETS --voice --original --out-dir`). Per-voice `missing_emotion_targets.catalog` / `missing_emotion_targets.scorer` can be forwarded there later; today only `enabled` is read. Default remains off.
- **Target file.** `TARGETS.json` (`schema: galion-emotion-targets/1`) already records `voice`, per-emotion `origin` / `target` / `moment_wav`. A `scorer` or `target_set` field can be added when a second backend is selected.
