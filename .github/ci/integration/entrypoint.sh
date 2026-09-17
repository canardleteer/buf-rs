#!/usr/bin/env bash
# Shared Debian/Alpine integration: install buf-tools + buf-toolchain, then
# validate-cargo-buf-toolchain --yaml, buf --version, buf build, both examples.
set -euo pipefail

cd /app
export PATH="/usr/local/bin:/usr/local/cargo/bin:${HOME}/.cargo/bin:${PATH}"

if [[ -n "${TEST_CRATE_VERSION:-}" ]]; then
    echo "Integration test using TEST_CRATE_VERSION=${TEST_CRATE_VERSION}"
    cargo add "buf-tools@=${TEST_CRATE_VERSION}"
    cargo install buf-toolchain --version "${TEST_CRATE_VERSION}" --features validate-cli
    if [[ "${TEST_CRATE_VERSION}" =~ ^([0-9]+\.[0-9]+\.[0-9]+) ]]; then
        EXPECT_CORE="${BASH_REMATCH[1]}"
    else
        echo "::error::Could not parse semver core from TEST_CRATE_VERSION=${TEST_CRATE_VERSION}" >&2
        exit 1
    fi
elif [[ -n "${EXPECT_BUF_CORE:-}" && -d /opt/path-src/buf-tools ]]; then
    echo "Integration test using path-preinstalled buf-toolchain; EXPECT_BUF_CORE=${EXPECT_BUF_CORE}"
    cargo add --path /opt/path-src/buf-tools
    EXPECT_CORE="${EXPECT_BUF_CORE}"
else
    echo "error: set TEST_CRATE_VERSION (registry) or EXPECT_BUF_CORE with path-src buf-tools" >&2
    exit 1
fi

echo "==> validate-cargo-buf-toolchain --yaml"
validate-cargo-buf-toolchain --yaml

BUF_LINE="$(buf --version)"
echo "buf --version output: ${BUF_LINE}"
if [[ "${BUF_LINE}" =~ ([0-9]+\.[0-9]+\.[0-9]+) ]]; then
    GOT_CORE="${BASH_REMATCH[1]}"
else
    echo "::error::Could not parse buf version from: ${BUF_LINE}" >&2
    exit 1
fi

if [[ "${EXPECT_CORE}" != "${GOT_CORE}" ]]; then
    echo "::error::Expected buf semver core ${EXPECT_CORE} (from crate), got ${GOT_CORE} from buf --version" >&2
    exit 1
fi

buf build -o proto/breaking_against.binpb proto

cargo build
cargo run --example buf_lint
cargo run --example protoc_with_buf_plugins

echo "Integration checks passed."
