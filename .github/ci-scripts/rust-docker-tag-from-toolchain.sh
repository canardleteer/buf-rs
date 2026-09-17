#!/usr/bin/env bash
# Print rust:<tag> suffix for official library/rust images from
# rust-toolchain.toml [toolchain].channel.
#   debian (default): stable → slim-bookworm; X.Y.Z → X.Y-slim-bookworm
#   alpine:           stable → alpine;        X.Y.Z → X.Y-alpine
# Docker Hub has no X.Y.Z patch tags on library/rust.
# Usage: rust-docker-tag-from-toolchain.sh [path/to/rust-toolchain.toml] [debian|alpine]
set -euo pipefail

FILE="${1:-rust-toolchain.toml}"
FLAVOR="${2:-debian}"
if [[ ! -f "$FILE" ]]; then
    echo "error: not a file: ${FILE}" >&2
    exit 1
fi
case "${FLAVOR}" in
    debian | alpine) ;;
    *)
        echo "error: flavor must be debian or alpine, got: ${FLAVOR}" >&2
        exit 1
        ;;
esac

CHANNEL="$(
    sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "${FILE}" | head -n1
)"
if [[ -z "${CHANNEL}" ]]; then
    echo "error: no channel = \"…\" in ${FILE}" >&2
    exit 1
fi

suffix_for_flavor() {
    if [[ "${FLAVOR}" == "alpine" ]]; then
        echo "alpine"
    else
        echo "slim-bookworm"
    fi
}

if [[ "${CHANNEL}" == "stable" ]]; then
    suffix_for_flavor
    exit 0
fi

if [[ ! "${CHANNEL}" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    echo "error: expected channel \"stable\" or X.Y.Z for Docker base mapping, got: ${CHANNEL}" >&2
    exit 1
fi

MAJOR_MINOR="${CHANNEL%.*}"
echo "${MAJOR_MINOR}-$(suffix_for_flavor)"
