# Clone weights for CloudAgents

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.

The spelling is **Galion**. The product is Galion Voice.

## Policy

Cloning is Rust-first. This note is the on-demand fetch for the Rust slot. It does not train a voice. Piper training on own-voice clips is forbidden. Reference wavs stay outside git and are read-only. They are not release assets, not pin rows, and not Git LFS objects. The deployed Jarvis voice must not change. Fetch does not run generate. WER, ECAPA, MOS, and timings are not measured here. Those gates stay **BLOCKED**. No audio file is added. `LOCKS.toml` is not edited.

`weights/clone/` holds the Chatterbox safetensors. `weights/.gitignore` is `*`, so a CloudAgent clone has the pins and the fetch script and none of the bytes. Putting the directory in the git tree would copy about 4.58 GiB into every VM. This note is the on-demand fetch. The bytes stay gitignored.

LOCKS.toml is unchanged. `reference_clips` stays false. Samantha and Jarvis reference clips stay on the admin box. They are not release assets, not pin rows, and not Git LFS objects.

## BLOCKED — admin upload

This checkout does not publish the release. The safetensors are not in git, and this PR does not add them. GitHub also rejects one archive of the directory: [each release asset must be under 2 GiB](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases), and the five files together are 4,912,434,696 bytes (4.575 GiB). The Nano four alone are 2,996,954,644 bytes (2.791 GiB), which is already over that cap. Upload five separate assets. Do not tar, zip, or concatenate them.

Until Maciej Grajczyk runs the upload below, a release fetch exits 1 and prints `BLOCKED`:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano
```

The same hashes are still on Hugging Face. That path is explicit:

```bash
GALION_CLONE_FETCH=1 GALION_CLONE_WEIGHTS_SOURCE=hf scripts/clone/fetch-clone-weights.sh --nano
```

## Release

| | |
|---|---|
| Repository | `galion-studio/galion-one-voice` |
| Tag | `clone-weights-v1` |
| Latest | false. Product release `v2.0.0` stays Latest. |
| Target | `main`. The tag's git tree has no weight blobs. |
| Fetch | `https://github.com/galion-studio/galion-one-voice/releases/download/clone-weights-v1/<asset>` |

`git checkout clone-weights-v1` does not materialize the files. The fetch script does, after `GALION_CLONE_FETCH=1`. CI does not set that flag.

A replaced asset is a failed sha check. A new hash is a new tag, `clone-weights-v2`, and a one-line change of `CLONE_RELEASE_TAG` in `scripts/clone/weights-release.sh`. Do not edit `v2.0.0` to carry these files.

Curl follows redirects. An egress allowlist needs `github.com`, `release-assets.githubusercontent.com`, and `objects.githubusercontent.com`. The Hugging Face path needs `huggingface.co` and the LFS CDN it redirects to.

### Assets and pins

Content sha256, read 2026-11-03 from the Hugging Face tree API (LFS oids). Rows live in `scripts/clone/PINS.sha256`. Columns: `rel sha256 size hf_repo hf_path release_asset`.

| rel under `weights/clone/` | bytes | release asset | sha256 |
|---|---:|---|---|
| `shared/s3gen.safetensors` | 1056484620 | `galion-clone-shared-s3gen.safetensors` | `2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e` |
| `shared/s3gen_meanflow.safetensors` | 1064875036 | `galion-clone-shared-s3gen_meanflow.safetensors` | `d65cb687a2ed581ee6cc297e919ffefa63386944f42364ae13b78a594945514f` |
| `shared/ve.safetensors` | 5695784 | `galion-clone-shared-ve.safetensors` | `f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c` |
| `nano/t3_nano_v1.safetensors` | 869899204 | `galion-clone-nano-t3_nano_v1.safetensors` | `72b110185087d945dbdf54dee4e333848e1811bdd5fd6cb16ceb8da50006f0c9` |
| `turbo/t3_turbo_v1.safetensors` | 1915480052 | `galion-clone-turbo-t3_turbo_v1.safetensors` | `fcf1f8c1d651bb7e3acd69ee5be269b4ac10c02980b7708213d598bc9f7cdf87` |

Nano is the first four rows: 2,996,954,644 bytes. All five: 4,912,434,696 bytes. The largest single file is Turbo at 1,915,480,052 bytes, 232,003,596 bytes under the 2 GiB line (2,147,483,648).

Hugging Face repos, same bytes: `ResembleAI/chatterbox-nano` tree `71ccd1d0081b430592cea481f4307e764e07bc64` for the shared files and Nano, `ResembleAI/chatterbox-turbo` tree `749d1c1a46eb10492095d68fbcf55691ccf137cd` for Turbo. Shared tensor hashes match across both repos. `.pt` is not pinned.

Upstream Chatterbox code is MIT, copyright 2025 Resemble AI. These rows are the safetensors already named in the pin file.

## Fetch

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check --nano
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh          # Nano and Turbo
```

`--nano` is the CloudAgent pack: `s3gen`, `s3gen_meanflow`, `ve`, `t3_nano_v1`. The default also fetches `t3_turbo_v1`. Files land in `weights/clone/` and stay gitignored. A sha256 or size mismatch deletes the partial. Generate is not run.

The script refuses a pin row whose name ends in `.pt`, `.wav`, `.wave`, `.mp3`, `.flac`, `.ogg`, `.opus`, `.m4a`, `.aac`, `.mp4`, `.mkv`, `.mov`, or `.webm`, and any row whose path contains `samantha` or `jarvis`. The Rust reader in `clone_worker.rs` still uses only `rel`, sha256, and size. Columns after size are fetch metadata.

## One-shot upload (admin box)

Run this where `weights/clone/` already contains the five files. A clean CloudAgent VM cannot do this step. From an up-to-date checkout of `main`:

```bash
git pull
GALION_CLONE_PUBLISH=1 scripts/clone/publish-clone-weights.sh
GALION_CLONE_PUBLISH=1 scripts/clone/publish-clone-weights.sh --upload
```

The first command hashes every pin, refuses any other file under `weights/clone/` (including a reference clip), and prints the `gh release create` line. The second publishes tag `clone-weights-v1` with `--latest=false` and `--target main`. It does not commit. `--upload` refuses `GALION_CLONE_PINS` and `GALION_CLONE_WEIGHTS` overrides, so a test directory cannot be published by accident.

`weights/clone/` at publish time may contain only:

- `shared/s3gen.safetensors`
- `shared/s3gen_meanflow.safetensors`
- `shared/ve.safetensors`
- `nano/t3_nano_v1.safetensors`
- `turbo/t3_turbo_v1.safetensors`
- `PINS.sha256` (the sidecar the fetch script rewrites)

Move any reference clip out of that directory before the publish. The clip store is `GALION_CLIP_STORE` outside the work tree (`CLIP-INGEST.md`). The publish walks the directory and exits 2 if it sees audio, a symlink, or an extra file.

If `gh` stops halfway, delete the draft release in the GitHub UI before a retry. The script refuses to upload when `clone-weights-v1` already exists.

Hand upload, if you run `gh` yourself after the verify command has passed. Stage links so the asset names are the basenames (a 4.6 GiB copy is unnecessary when `ln` works):

```bash
stage=$(mktemp -d)
ln weights/clone/shared/s3gen.safetensors "$stage/galion-clone-shared-s3gen.safetensors"
ln weights/clone/shared/s3gen_meanflow.safetensors "$stage/galion-clone-shared-s3gen_meanflow.safetensors"
ln weights/clone/shared/ve.safetensors "$stage/galion-clone-shared-ve.safetensors"
ln weights/clone/nano/t3_nano_v1.safetensors "$stage/galion-clone-nano-t3_nano_v1.safetensors"
ln weights/clone/turbo/t3_turbo_v1.safetensors "$stage/galion-clone-turbo-t3_turbo_v1.safetensors"
gh release create clone-weights-v1 \
  --repo galion-studio/galion-one-voice \
  --target main \
  --latest=false \
  --title "Clone weights v1" \
  "$stage/galion-clone-shared-s3gen.safetensors" \
  "$stage/galion-clone-shared-s3gen_meanflow.safetensors" \
  "$stage/galion-clone-shared-ve.safetensors" \
  "$stage/galion-clone-nano-t3_nano_v1.safetensors" \
  "$stage/galion-clone-turbo-t3_turbo_v1.safetensors"
```

Write release notes that list the five sha256 lines from `scripts/clone/PINS.sha256` and state that reference audio is absent. Sign the notes.

After the tag exists, on a clean VM:

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check --nano
```

`git status` stays clean. The new files match `weights/.gitignore`.

## Git LFS

This repo does not gain a `.gitattributes` LFS rule.

`weights/.gitignore` ignores every file under `weights/` except the gitignore itself. Git LFS tracks files that are in the index. Tracking these safetensors means un-ignoring them, so a clone that smudges LFS writes the bytes into the VM. That is the disk and startup cost this fetch avoids. The fetch stays behind `GALION_CLONE_FETCH=1`, which CI leaves unset. Twenty CloudAgents do not each pull the pack at boot.

GitHub's per-file LFS cap depends on the plan: 2 GB on Free and Pro, 4 GB on Team, 5 GB on Enterprise Cloud ([About Git Large File Storage](https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-git-large-file-storage)). One 4,912,434,696-byte object is over the Free, Pro, and Team caps. Release assets have no total-size cap and no bandwidth cap; the 2 GiB rule applies per file, and each pin is under it.

Film audio stays out of Git LFS (`CLIP-INGEST.md`). An LFS pattern on `weights/` would be one mistake away from uploading a reference clip. The publish script's allowlist is the gate for the release instead.

## What this file does not do

The worker still does not call generate from this note. Piper remains the speaking fallback when the pack is absent. Piper training on own-voice clips is forbidden. Reference wavs stay outside git and are read-only. The deployed Jarvis voice must not change. WER, ECAPA, MOS, and timings stay **BLOCKED**. Rights for the two film voices stay on the checklist in `RIGHTS-CLEARANCE.md`.

— Maciej Grajczyk
