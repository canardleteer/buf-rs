#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Print [workspace.package].version from the repository-root Cargo.toml.
# Writes only the semver string to stdout (single line).
# Used to set BUF_EXPECT_VERSION for buf-tools integration tests.
# -----------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
export ROOT

python3 -c "
import os, tomllib
from pathlib import Path
root = Path(os.environ['ROOT'])
cargo = root / 'Cargo.toml'
with open(cargo, 'rb') as f:
    data = tomllib.load(f)
print(data['workspace']['package']['version'])
"
