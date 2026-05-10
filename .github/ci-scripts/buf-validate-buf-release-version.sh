#!/usr/bin/env bash
# Exit 0 if bufbuild/buf has git tag vX.Y.Z. Usage: buf-validate-buf-release-version.sh X.Y.Z
set -euo pipefail

VER="${1:?expected X.Y.Z}"
if [[ ! "${VER}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: expected plain X.Y.Z, got: ${VER}" >&2
  exit 1
fi

if command -v gh >/dev/null 2>&1 && { [[ -n "${GITHUB_TOKEN:-}" ]] || [[ -n "${GH_TOKEN:-}" ]]; }; then
  gh api "repos/bufbuild/buf/git/refs/tags/v${VER}" --silent
else
  code="$(curl -sS -o /dev/null -w '%{http_code}' -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/bufbuild/buf/git/refs/tags/v${VER}")"
  [[ "${code}" == "200" ]]
fi
