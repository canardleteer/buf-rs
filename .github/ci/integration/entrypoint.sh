#!/usr/bin/env bash
# Post-publish integration: registry-only buf-tools + buf-toolchain at TEST_CRATE_VERSION, then examples.
set -euo pipefail

: "${TEST_CRATE_VERSION:?TEST_CRATE_VERSION is required}"

cd /app

echo "Integration test using TEST_CRATE_VERSION=${TEST_CRATE_VERSION}"

cargo add "buf-tools@=${TEST_CRATE_VERSION}"
cargo install buf-toolchain --version "${TEST_CRATE_VERSION}"

export PATH="${HOME}/.cargo/bin:${PATH}"

if [[ "${TEST_CRATE_VERSION}" =~ ^([0-9]+\.[0-9]+\.[0-9]+) ]]; then
  EXPECT_CORE="${BASH_REMATCH[1]}"
else
  echo "::error::Could not parse semver core from TEST_CRATE_VERSION=${TEST_CRATE_VERSION}" >&2
  exit 1
fi

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

echo "Post-publish integration checks passed."
