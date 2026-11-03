# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Shared names for the Chatterbox clone-weight release.
# Sourced by fetch-clone-weights.sh and publish-clone-weights.sh.
# This file does not download and does not upload.

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  echo "weights-release: source this file from the fetch or publish script" >&2
  exit 2
fi

# Immutable tag. A new content hash is a new tag (clone-weights-v2).
# Do not attach these assets to the product release v2.0.0.
CLONE_RELEASE_TAG=clone-weights-v1
CLONE_RELEASE_REPO=galion-studio/galion-one-voice
CLONE_RELEASE_BASE_DEFAULT=https://github.com/galion-studio/galion-one-voice

# GitHub requires each release asset to be under 2 GiB.
# https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
CLONE_ASSET_MAX_BYTES=2147483648

clone_weight_rels() {
  printf '%s\n' \
    shared/s3gen.safetensors \
    shared/s3gen_meanflow.safetensors \
    shared/ve.safetensors \
    nano/t3_nano_v1.safetensors \
    turbo/t3_turbo_v1.safetensors
}

clone_weight_asset_for() {
  case "$1" in
    shared/s3gen.safetensors) printf '%s\n' galion-clone-shared-s3gen.safetensors ;;
    shared/s3gen_meanflow.safetensors) printf '%s\n' galion-clone-shared-s3gen_meanflow.safetensors ;;
    shared/ve.safetensors) printf '%s\n' galion-clone-shared-ve.safetensors ;;
    nano/t3_nano_v1.safetensors) printf '%s\n' galion-clone-nano-t3_nano_v1.safetensors ;;
    turbo/t3_turbo_v1.safetensors) printf '%s\n' galion-clone-turbo-t3_turbo_v1.safetensors ;;
    *) return 1 ;;
  esac
}

# Nano is the four shared+nano rows. Turbo is the fifth.
clone_weight_is_nano_rel() {
  case "$1" in
    turbo/*) return 1 ;;
    *) return 0 ;;
  esac
}
