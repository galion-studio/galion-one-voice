# HORBM research data

Perpetual Open MIT License. (c) 2026 Galion Labs.

The open chat corpora used for pre-alpha HORBM retrieval stay on the laptop. This repository is perpetual open MIT. Dataset text, cards, samples, and derived fixtures are not committed.

The corpora are EmpatheticDialogues, KdConv, PersonaChat, ConvAI2, Blended Skill Talk, LCCC, DailyDialog, and OpenSubtitles.

## Environment

Retrieval may read those corpora only through `GALION_HORBM_RESEARCH_DIR`. The variable points at a private directory outside this repository. When it is unset or empty, that read is skipped. No file in the tree supplies a default path.

## What may be committed

- The names of those corpora, in documentation, in code identifiers, and in licence notes.
- This page and `scripts/check-research-boundary.sh`.
- Product fixtures that are not those corpora and not research cards. The kettle note and the clause fixture are product fixtures.

## What may not be committed

- A dataset file. That is a `jsonl`, `parquet`, `csv`, or `tsv` of 64 KiB or more outside a `test`, `tests`, or `testdata` directory, including the same file with a `.gz`, `.bz2`, or `.xz` suffix. It is also any data file whose name, or a parent directory, is one of the corpora. Data suffixes here are `json`, `jsonl`, `csv`, `tsv`, `txt`, `parquet`, `zip`, `tar`, `tgz`, `gz`, `bz2`, `xz`, and `arrow`.
- Copied dialogue. The checker looks for a short list of release-specific strings. Those strings are listed only in `scripts/check-research-boundary.sh`.
- A hard-coded path to the research directory, or a filesystem path whose segment names a corpus. Reading `GALION_HORBM_RESEARCH_DIR` is the allowed form.
- A test fixture that is a research card, or a made-up card standing in for one.

Research cards are our own private research and development cards. They will be published later through knowledge.galion.app. Until that publication they stay off this repository. A test does not invent a substitute card.

`scripts/check-research-boundary.sh` is the gate. `.github/workflows/gates.yml` runs it.

## False-positive policy

The marker list is small on purpose. Each string is a long or oddly spelled span from a public sample: a comma escape left inside an EmpatheticDialogues utterance, the DailyDialog space-before-punctuation and its end-of-utterance token, the ungrammatical PersonaChat sentence also used by ConvAI2, one PersonaChat persona sentence carried by Blended Skill Talk, the KdConv music-domain example question, and one spaced LCCC reply. OpenSubtitles has no quotation marker. Ordinary film lines collide with prose, so that corpus is caught by file name and by the size rule.

Matches are exact. The PersonaChat sentence and the bow-hunting persona sentence are matched without regard to case, because the paper and the release spell them differently. The other markers are case-sensitive, so the same English sentence with normal punctuation is not a hit.

A hit fails in a comment, a test, a doc, or a fixture. Quoting the utterance is a copy. Naming the corpus is not a copy. The bare comma-escape token is not a marker: an unrelated test already contains that character sequence inside a longer identifier. The marker is the escape as it sits in the fireworks utterance.

This page does not repeat the marker strings. The checker holds them and is excluded from the search.

If a marker hits a sentence we wrote, rephrase the sentence. If a marker is genuinely too broad, replace it with a longer span of the same public sample and note that change here. Do not add a per-file allowlist. Do not delete a marker to clear a hit. Do not shorten one.

The size rule uses 64 KiB. Every tracked `jsonl`, `csv`, and `tsv` on main at `02f8ac90` is under 16 KiB. The largest is the EfficientAT class-label table. Do not raise the threshold to admit a corpus. A table that is not a corpus and must grow past 64 KiB stays out of git. A threshold change is a separate edit, it says why, and it stays far below a corpus split.

Directories named `test`, `tests`, and `testdata` skip only the size rule. A corpus file name, a dialogue marker, a hard-coded path, and a research-card fixture still fail there.

A citation on a line that contains `http://` or `https://` is not treated as a filesystem path. A path on a line with no URL still fails. Write a corpus link as an `https` URL, or name the corpus with no slash.

`docs/horbm/research/` is the in-repo note path (the R-series pages). It is not the laptop research root. The gate ignores that prefix. Any other path into a research directory under horbm still fails, including on a line that also cites the note. Do not hide a laptop path by writing only the note prefix.

A voice profile whose persona value is a single id, such as the Samantha autocheck profile, is not a research card. A persona value that contains a space is a sentence and fails when it sits in a fixture.

A markdown or source file may use a corpus name as its own file name. A data file may not. A directory named for a corpus fails when a data file sits inside it. A markdown note in that directory is not, by itself, a failure. Do not use that gap to store utterances.

Research-card fixtures are `json`, `jsonl`, markdown, text, `csv`, or `tsv` under `test`, `tests`, `testdata`, `fixture`, or `fixtures`, and any of those files whose name contains `research-card`, `research_card`, `sample-card`, or `sample_card`. They fail when they carry corpus fields (a conversation id, an utterance index, a persona line, a knowledge-graph attribute) or when they are a dumped knowledge card (`said_by` together with `claim_type`). The kettle note and `horbm/crates/horbm-brain/fixtures/clauses.json` do not carry those fields. Loader tests that build a kettle card in Rust source are not fixture files.

## Pre-publish removal

Every trace of the corpora is removed and replaced before any public release.

Before a release of retrieval, of cards, or of a build that used these corpora:

1. Remove every copied utterance, corpus file, sample, and derived fixture from the release commit.
2. Replace each research card with the card published at knowledge.galion.app, or remove the card if it is not published yet. These are our own private research and development cards.
3. Leave `GALION_HORBM_RESEARCH_DIR` unset in the release. Do not write a path.
4. Run `scripts/check-research-boundary.sh` on that commit. It must print `OK`.
5. Do not tag or ship while any other trace remains.

The marker list inside the checker is the gate. It stays so the gate can see a later copy. It is not a sample pack. Every other trace is removed and replaced.
