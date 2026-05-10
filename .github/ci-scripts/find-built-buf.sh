#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Locate the buf (or buf.exe) binary emitted by buf-tools build.rs under
# target/<profile>/build/buf-tools-*/out/bin/. Matches README manual workflow.
# Prints one absolute path to stdout, or exits non-zero with a message on stderr.
# -----------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -n "${GITHUB_WORKSPACE:-}" ]]; then
  ROOT="$(cd "${GITHUB_WORKSPACE}" && pwd)"
else
  ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi
cd "$ROOT"

# Primary layout (Unix + Git Bash on Windows): .../out/bin/buf or buf.exe
emit_abs() {
  local line="$1"
  if [[ -z "${line}" ]]; then
    return 1
  fi
  if [[ "${line}" != /* ]]; then
    line="${ROOT}/${line}"
  fi
  printf '%s\n' "${line}"
}

while IFS= read -r line; do
  if [[ -n "${line}" ]]; then
    emit_abs "${line}"
    exit 0
  fi
done < <(
  find target -type f \( \
    -path '*/build/buf-tools-*/out/bin/buf' \
    -o -path '*/build/buf-tools-*/out/bin/buf.exe' \
  \) 2>/dev/null | head -n 1
)

# Fallback: any buf / buf.exe under buf-tools build out/ (layout drift)
while IFS= read -r line; do
  if [[ -n "${line}" ]]; then
    emit_abs "${line}"
    exit 0
  fi
done < <(
  find target -type f \( -name 'buf' -o -name 'buf.exe' \) -path '*/build/buf-tools-*/out/*' 2>/dev/null | head -n 1
)

echo "find-built-buf: could not locate buf under ${ROOT}/target — build buf-tools first (e.g. cargo build -p buf-tools)." >&2
exit 1
