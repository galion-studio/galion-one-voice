# Artifact-first board repair

Run `nice -n 10 scoreboard/galion-board/target/release/galion-board` from any directory.
The Rust entrypoint invokes `aggregate.py`, which performs only JSON/filesystem
aggregation. Metrics are produced exclusively by the existing Rust binaries.

The previous board assumed one universal CLI (`--wav-dir --text-dir --json`)
and searched only release trees, missing articulation's READY `bin/galion-artic`.
It also discovered WAVs using voice asset names, missing experiment directories,
then overwrote useful historical scores with scorer invocation failures.

The replacement consumes historical Rust JSON first, resolves WAV paths relative
to the job directory, deduplicates identical audio sets, and fills missing metrics
with scorer-specific invocations. ECAPA receives explicit WAV paths; emotion
uses `score` and fp32 defaults; articulation uses `profile -j 2`; quality uses
`bin/galion-{wer,mos,hygiene}`. Articulation output is honestly labelled as
profile articulation rate rather than falsely claiming MCD against unmatched text.

No rendering, voice asset writes, GPU use, commits, pushes, or deletions occur.
Refprep and merged-production workers own their score files: these are read only.
Repair scorer outputs live separately in `scoreboard/repair-scores/`.
The old watch-duration argument is no longer used: each execution is one pass.

Ranking is gate-first and full-12-line evidence first, then matched-baseline
emotion improvement and ECAPA. Exact component/merged ties prefer the merged
production row. MOS is not a ranking tiebreaker because historical Jarvis jobs
used different SQUIM non-matching references; the board preserves and flags that
provenance rather than presenting it as a fair MOS gain.

Both merged-production rows are consumed from completed `merge-v2/scores/`
with `merge-v2/results12.json` and `merge-v2/REPORT.md` as validation context.
The board never changes the production voices itself.
