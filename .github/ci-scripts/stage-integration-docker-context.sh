#!/usr/bin/env bash
# Stage the registry-mode integration Docker context into DEST.
# Call after checkout of the publish ref so the recipe matches that tree's
# Dockerfiles (crates-io-publish often dispatches the workflow file from
# main while checking out another ref).
# Usage: stage-integration-docker-context.sh DEST
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEST="${1:?destination directory}"

rm -rf "${DEST}"
mkdir -p "${DEST}/proto" "${DEST}/path-src"
cp "${ROOT}/rust-toolchain.toml" "${DEST}/"
cp "${ROOT}/.github/ci/integration/Cargo.toml" \
    "${ROOT}/.github/ci/integration/Dockerfile" \
    "${ROOT}/.github/ci/integration/Dockerfile.alpine" \
    "${ROOT}/.github/ci/integration/entrypoint.sh" \
    "${DEST}/"
chmod +x "${DEST}/entrypoint.sh"
cp "${ROOT}/examples/buf_lint.rs" "${ROOT}/examples/protoc_with_buf_plugins.rs" "${DEST}/"
cp -r "${ROOT}/examples/proto/"* "${DEST}/proto/"
# Optional for registry builds. Path-preinstall (`cargo xtask image`) replaces
# this with workspace members. Dockerfiles must not require the tree.
: >"${DEST}/path-src/.keep"
