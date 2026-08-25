#!/usr/bin/env bash
# Print rust:<tag> suffix for official library/rust images from
# rust-toolchain.toml [toolchain].channel.
#   stable → slim-bookworm
#   X.Y.Z  → X.Y-slim-bookworm (Docker Hub has no X.Y.Z patch tags)
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

if [[ "${CHANNEL}" == "stable" ]]; then
  echo "slim-bookworm"
  exit 0
fi

if [[ ! "${CHANNEL}" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "error: expected channel \"stable\" or X.Y.Z for Docker base mapping, got: ${CHANNEL}" >&2
  exit 1
fi

MAJOR_MINOR="${CHANNEL%.*}"
echo "${MAJOR_MINOR}-slim-bookworm"
