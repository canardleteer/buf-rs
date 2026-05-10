# buf-toolchain

Install official [Buf](https://github.com/bufbuild/buf) release binaries through
Cargo (`cargo install` or `[build-dependencies]`).

- [crates.io/crates/buf-toolchain][crates-buf-toolchain]
- [docs.rs/buf-toolchain][docs-buf-toolchain]

Companion crate [buf-tools][docs-buf-tools] shares download, verify, and
target-selection logic via the workspace `build_support` tree. The
[repository README][repo-readme] describes the whole workspace; this file ships
in the published crate.

[crates-buf-toolchain]: https://crates.io/crates/buf-toolchain
[docs-buf-toolchain]: https://docs.rs/buf-toolchain
[docs-buf-tools]: https://docs.rs/buf-tools
[repo-readme]: https://github.com/canardleteer/buf-rs#readme

## What this crate does

On build (including `cargo install buf-toolchain` or as a build dependency),
`build.rs`:

1. Resolves the compilation target to a Buf release asset suffix.
2. Downloads release files (or reuses a verified cache entry under lock).
3. Verifies `sha256.txt` with minisign and checks each binary hash.
4. Installs `buf`, `protoc-gen-buf-breaking`, and `protoc-gen-buf-lint` (with
   `.exe` on Windows) into one directory using atomic writes.

Default install directory: `$CARGO_HOME/bin` (often `~/.cargo/bin`). Override
with `BUF_RS_TOOLCHAIN_BIN_DIR`.

`cargo install` also places the `validate-cargo-buf-toolchain` binary on `PATH`
for post-install checks.

Per-target minimum Buf versions match `buf-tools`; unsupported combinations fail
before any large download. See `build_support/targets.rs` in the repo for the
authoritative table.

## Environment variables

Install location:

- `BUF_RS_TOOLCHAIN_BIN_DIR` — if non-empty, install only here; otherwise
  `$CARGO_HOME/bin`.

Cache and downloads:

- `BUF_RS_CACHE_DIR` — optional cache root (`<semver-core>/<target>/` under it).
- `BUF_RS_RELEASE_BASE_URL` — optional prefix for release assets (default
  `https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`).

Validation helper (`validate-cargo-buf-toolchain` binary):

- `BUF_RS_VALIDATE_OFFLINE=1` — skip GitHub and crates.io network calls.

Options that apply only when depending on `buf-tools` directly (layout, build
log, source bundles) are documented in the [buf-tools docs][docs-buf-tools].

## Concurrent cache writers

The same cache-slot lock as `buf-tools` serializes writers under
`<cache-root>/<semver-core>/<target>`.

## Install

```bash
cargo install buf-toolchain
validate-cargo-buf-toolchain
```

`validate-cargo-buf-toolchain` re-checks installed binaries against the pinned
GitHub release, optionally compares `releases/latest`, and can query crates.io
unless `BUF_RS_VALIDATE_OFFLINE=1`.

Custom directory:

```bash
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" cargo install buf-toolchain
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" validate-cargo-buf-toolchain
```

## Build dependency

```toml
[build-dependencies]
# Example only — pin to the Buf release you need (authoritative: workspace root).
buf-toolchain = "1.40.0"
```

## CI: online prewarm then offline build

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" cargo build -p buf-toolchain
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" CARGO_NET_OFFLINE=true \
  cargo build -p buf-toolchain
```

## Maintainer integration tests

Nested Cargo with isolated `CARGO_HOME` (needs network on cold cache):

```bash
cargo test -p buf-toolchain --locked --test managed_bin_layout -- --ignored
```

## crates.io packaging

Published sources include `build_support` via `#[path]` includes. In the git
workspace, `buf-toolchain/build_support` is a symlink to `buf-tools/build_support`;
`cargo package -p buf-toolchain` expands that into the tarball. On Windows,
ensure symlinks are enabled or recreate the link if packaging fails.
