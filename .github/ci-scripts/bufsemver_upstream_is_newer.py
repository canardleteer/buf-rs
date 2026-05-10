#!/usr/bin/env python3
"""Exit 0 if upstream core semver is strictly greater than current (plain X.Y.Z cores)."""
import sys


def core_tuple(s: str) -> tuple[int, ...]:
    base = s.split("+", 1)[0].split("-", 1)[0]
    parts = base.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        print(f"error: expected X.Y.Z cores, got current={s!r}", file=sys.stderr)
        sys.exit(2)
    return tuple(int(p) for p in parts)


def main() -> None:
    if len(sys.argv) != 3:
        print("usage: bufsemver_upstream_is_newer.py <current_X.Y.Z> <upstream_X.Y.Z>", file=sys.stderr)
        sys.exit(2)
    cur, up = core_tuple(sys.argv[1]), core_tuple(sys.argv[2])
    sys.exit(0 if up > cur else 1)


if __name__ == "__main__":
    main()
