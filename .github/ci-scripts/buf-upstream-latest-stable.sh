#!/usr/bin/env bash
# Print Buf stable release version X.Y.Z from bufbuild/buf GitHub releases/latest (no leading v).
# Uses gh when GITHUB_TOKEN/GH_TOKEN is set; otherwise curl (lower rate limits).
set -euo pipefail

if command -v gh >/dev/null 2>&1 && { [[ -n "${GITHUB_TOKEN:-}" ]] || [[ -n "${GH_TOKEN:-}" ]]; }; then
  gh api repos/bufbuild/buf/releases/latest --jq '.tag_name' | sed 's/^v//'
else
  curl -fsSL -H "Accept: application/vnd.github+json" "https://api.github.com/repos/bufbuild/buf/releases/latest" \
    | python3 -c "import sys, json; print(json.load(sys.stdin)['tag_name'].lstrip('v'))"
fi
