#!/usr/bin/env bash
# Local smoke: same Docker context assembly as publish-crates post-publish-integration.
# Usage: TEST_CRATE_VERSION=<semver> [DOCKER=docker|podman] bash .github/ci-scripts/run-integration-docker.sh
#    or: bash .github/ci-scripts/run-integration-docker.sh <semver>
# Run from the repository root (or pass through paths via ROOT below).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DOCKER="${DOCKER:-docker}"

if [[ -n "${1:-}" ]]; then
  TEST_CRATE_VERSION="$1"
fi
if [[ -z "${TEST_CRATE_VERSION:-}" ]]; then
  echo "Usage: TEST_CRATE_VERSION=X.Y.Z[-pre.release] ${DOCKER} …   OR   $0 X.Y.Z" >&2
  echo "Example: TEST_CRATE_VERSION=\$(bash .github/ci-scripts/read-workspace-version.sh) bash $0" >&2
  exit 1
fi

CTX="$(mktemp -d)"
cleanup() {
  rm -rf "${CTX}"
}
trap cleanup EXIT

mkdir -p "${CTX}/proto"
cp "${ROOT}/rust-toolchain.toml" "${CTX}/"
cp "${ROOT}/.github/ci/integration/Cargo.toml" \
  "${ROOT}/.github/ci/integration/Dockerfile" \
  "${ROOT}/.github/ci/integration/entrypoint.sh" "${CTX}/"
chmod +x "${CTX}/entrypoint.sh"
cp "${ROOT}/examples/buf_lint.rs" "${ROOT}/examples/protoc_with_buf_plugins.rs" "${CTX}/"
cp -r "${ROOT}/examples/proto/"* "${CTX}/proto/"

IMAGE_TAG="buf-rs-integration:local"

RUST_DOCKER_TAG="$(
  bash "${ROOT}/.github/ci-scripts/rust-docker-tag-from-toolchain.sh" "${ROOT}/rust-toolchain.toml"
)"
echo "Building ${IMAGE_TAG} (${DOCKER}), RUST_DOCKER_TAG=${RUST_DOCKER_TAG}…"
"${DOCKER}" build \
  --build-arg "RUST_DOCKER_TAG=${RUST_DOCKER_TAG}" \
  -t "${IMAGE_TAG}" \
  "${CTX}"

echo "Running integration with TEST_CRATE_VERSION=${TEST_CRATE_VERSION}…"
exec "${DOCKER}" run --rm -e "TEST_CRATE_VERSION=${TEST_CRATE_VERSION}" "${IMAGE_TAG}"
