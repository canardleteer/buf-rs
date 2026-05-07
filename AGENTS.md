# Agent / maintainer operations

Concise rules for coding agents. User-facing commands stay in [`README.md`](README.md).

## Two different “versions”

1. **Upstream Buf release** — GitHub tag `v1.69.0`, assets under `bufbuild/buf`
   releases. The `buf` binary’s `--version` reports this line (no crate
   pre-release suffix).
2. **Crate semver** — `[workspace.package].version` and `version = "=…"` pins in
   the root [`Cargo.toml`](Cargo.toml), and what you publish to crates.io.

For **canary** publishes, crate semver must **not** equal the final stable slot
(`1.69.0`) until you intentionally ship stable. Use a **pre-release** such as
`1.69.0-rc.<n>.sys.testing` so a failed publish never burns the stable
version Buf “owns” semantically.

**Crate semver tracks the Buf binary, not an independent series.** The
**core** semver (before any `-` pre-release) **MUST** match the upstream Buf release
that `buf --version` corresponds to. Never invent a crates-only patch
such as `1.69.1` when Buf is still shipping `1.69.0`. If a bad publish consumes
a semver slot on crates.io, recover with **pre-release identifiers** so the
published version still reflects the same Buf release, not a fabricated crate
lineage.

## `buf-sys` packaging (download at build + crates.io cap)

- **Published `.crate`:** Rust sources, **`build.rs`**, and **`build_support/**`** only.
  **No** Buf executables ship in the tarball — they exceed crates.io’s ~10 MiB upload limit.
  Consumers’ **`build.rs`** downloads official release assets, verifies **minisign** +
  **`sha256.txt`**, and caches under **`BUF_SYS_CACHE_DIR`** or **`~/.cache/buf-sys/`**
  (see [`buf-sys/README.md`](buf-sys/README.md)).

## Publishing

The workspace sets **`publish = true`** for the publishable crate. To ship **`buf-sys`**
to crates.io from a clean tree: **`cargo publish -p buf-sys`** (after **`cargo publish --dry-run`**
if you want a no-upload rehearsal). **`cargo publish --dry-run`** does not consume a
version on crates.io.

## Linting

Before merging risky changes:

- **`cargo fmt --all -- --check`** — formatting gate (use **`cargo fmt --all`** to apply).
- **`cargo clippy --workspace --locked --all-targets`** — static analysis gate (narrow
  with `-p` when iterating on one crate).

## Examples

Runnable examples live in [`examples/`](examples/) as the **`buf-sys-examples`** package.
Run with **`cargo run -p buf-sys-examples --example <name>`**.

For **`protoc_with_buf_plugins`**, generate the gitignored Buf image under
[`examples/proto/`](examples/proto/) first (see [`README.md`](README.md)).
