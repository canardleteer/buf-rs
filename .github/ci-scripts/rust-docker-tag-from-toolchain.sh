#!/usr/bin/env bash
# Print rust:<tag> suffix for official library/rust images, e.g. 1.95-slim-bookworm,
# from repo-root rust-toolchain.toml channel = "X.Y.Z" (single source of truth).
# Usage: rust-docker-tag-from-toolchain.sh [path/to/rust-toolchain.toml]
set -euo pipefail

FILE="${1:-rust-toolchain.toml}"
if [[ ! -f "$FILE" ]]; then
  echo "error: not a file: ${FILE}" >&2
  exit 1
fi

CHANNEL="$(
  sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "${FILE}" | head -n1
)"
if [[ -z "${CHANNEL}" ]]; then
  echo "error: no channel = \"…\" in ${FILE}" >&2
  exit 1
fi
if [[ ! "${CHANNEL}" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "error: expected channel X.Y.Z for Docker base mapping, got: ${CHANNEL}" >&2
  exit 1
fi

MAJOR_MINOR="${CHANNEL%.*}"
echo "${MAJOR_MINOR}-slim-bookworm"
