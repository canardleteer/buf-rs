#!/usr/bin/env bash
# Local smoke: same Docker context assembly as publish-crates post-publish-integration.
# Usage:
#   TEST_CRATE_VERSION=<semver> [DISTRO=debian|alpine|all] [DOCKER=docker|podman] \
#     bash .github/ci-scripts/run-integration-docker.sh
#   bash .github/ci-scripts/run-integration-docker.sh <semver> [debian|alpine|all]
# Registry mode: TEST_CRATE_VERSION must exist on crates.io.
# For path-preinstall of this worktree (no published crate), use `cargo xtask image`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DOCKER="${DOCKER:-docker}"
DISTRO="${DISTRO:-all}"

if [[ -n "${1:-}" && "${1}" != debian && "${1}" != alpine && "${1}" != all ]]; then
    TEST_CRATE_VERSION="$1"
    shift
fi
if [[ -n "${1:-}" ]]; then
    DISTRO="$1"
fi
if [[ -z "${TEST_CRATE_VERSION:-}" ]]; then
    echo "Usage: TEST_CRATE_VERSION=X.Y.Z[-pre.release] DISTRO=debian|alpine|all $0" >&2
    echo "   or: $0 X.Y.Z [debian|alpine|all]" >&2
    echo "Example: TEST_CRATE_VERSION=\$(bash .github/ci-scripts/read-workspace-version.sh) bash $0" >&2
    echo "Path-preinstall (unreleased helper): cargo xtask image all" >&2
    exit 1
fi
case "${DISTRO}" in
    debian | alpine | all) ;;
    *)
        echo "error: DISTRO must be debian, alpine, or all (got ${DISTRO})" >&2
        exit 1
        ;;
esac

CTX="$(mktemp -d)"
cleanup() {
    rm -rf "${CTX}"
}
trap cleanup EXIT

bash "${ROOT}/.github/ci-scripts/stage-integration-docker-context.sh" "${CTX}"

run_one() {
    local flavor="$1"
    local dockerfile="$2"
    local image_tag="$3"
    local rust_tag
    rust_tag="$(
        bash "${ROOT}/.github/ci-scripts/rust-docker-tag-from-toolchain.sh" \
            "${ROOT}/rust-toolchain.toml" "${flavor}"
    )"
    echo "Building ${image_tag} (${DOCKER}), RUST_DOCKER_TAG=${rust_tag}, file=${dockerfile}…"
    "${DOCKER}" build \
        --build-arg "RUST_DOCKER_TAG=${rust_tag}" \
        -f "${CTX}/${dockerfile}" \
        -t "${image_tag}" \
        "${CTX}"
    echo "Running ${image_tag} with TEST_CRATE_VERSION=${TEST_CRATE_VERSION}…"
    "${DOCKER}" run --rm -e "TEST_CRATE_VERSION=${TEST_CRATE_VERSION}" "${image_tag}"
}

if [[ "${DISTRO}" == "debian" || "${DISTRO}" == "all" ]]; then
    run_one debian Dockerfile buf-rs-integration:debian
fi
if [[ "${DISTRO}" == "alpine" || "${DISTRO}" == "all" ]]; then
    run_one alpine Dockerfile.alpine buf-rs-integration:alpine
fi
