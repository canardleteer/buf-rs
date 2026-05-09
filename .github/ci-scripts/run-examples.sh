#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# CI entrypoint: rustfmt check, clippy, workspace tests, then run buf-tools
# examples (buf_lint + protoc_with_buf_plugins). Requires network for Buf
# downloads during build.rs unless cache is warm.
#
# Environment:
#   GITHUB_WORKSPACE — if set (Actions), cd there before running (default: cwd).
#   BUF_RS_CACHE_DIR   — optional; defaults to <repo>/target/buf-rs-cache
# -----------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -n "${GITHUB_WORKSPACE:-}" ]]; then
  ROOT="$(cd "${GITHUB_WORKSPACE}" && pwd)"
else
  ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi
cd "$ROOT"

export BUF_RS_CACHE_DIR="${BUF_RS_CACHE_DIR:-${ROOT}/target/buf-rs-cache}"
mkdir -p "${BUF_RS_CACHE_DIR}"

BUF_EXPECT_VERSION="$(bash "${SCRIPT_DIR}/read-workspace-version.sh")"
export BUF_EXPECT_VERSION
echo "BUF_EXPECT_VERSION=${BUF_EXPECT_VERSION}"

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --workspace --locked --all-targets"
cargo clippy --workspace --locked --all-targets

echo "==> cargo test --workspace --locked"
cargo test --workspace --locked

echo "==> cargo build -p buf-tools --locked (ensure buf present for examples)"
cargo build -p buf-tools --locked

BUF="$(bash "${SCRIPT_DIR}/find-built-buf.sh")"
echo "Using buf CLI at: ${BUF}"

echo "==> buf build -o breaking_against.binpb (examples/proto)"
(
  cd "${ROOT}/examples/proto"
  "${BUF}" build -o breaking_against.binpb .
)

echo "==> cargo run -p buf-tools-examples --example buf_lint"
cargo run -p buf-tools-examples --locked --example buf_lint

echo "==> cargo run -p buf-tools-examples --example protoc_with_buf_plugins"
cargo run -p buf-tools-examples --locked --example protoc_with_buf_plugins

echo "==> CI script finished successfully"
