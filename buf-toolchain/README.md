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
[repo-publish-channels]: https://github.com/canardleteer/buf-rs#cratesio-publish-channels-manual-workflow

## Crate version tracks Buf

The published crate semver core is the upstream Buf release. After a
stable `X.Y.Z` is on crates.io, buf-rs-only follow-ups ship as
`{core}-hotfix.N`. Check for a hotfix if a build fails against a Buf
version you already pinned.

`1.70.0-hotfix.1` repairs `buf-tools` under `cargo install` when it is
a build dependency. The same Buf core is published for `buf-toolchain`.

See [crates.io publish channels][repo-publish-channels] in the
repository README.

## What this crate does

On build (including `cargo install buf-toolchain` or as a build
dependency), `build.rs` does the following.

1. Resolves the compilation target to a Buf release asset suffix.
2. Downloads release files (or reuses a verified cache entry under lock).
3. Verifies `sha256.txt` with minisign and checks each binary hash.
4. Installs `buf`, `protoc-gen-buf-breaking`, and `protoc-gen-buf-lint` (with
   `.exe` on Windows) into one directory using atomic writes.

The default install directory is `$CARGO_HOME/bin` (often `~/.cargo/bin`).
Override with `BUF_RS_TOOLCHAIN_BIN_DIR`, or set `CARGO_INSTALL_ROOT`
(Cargo's `install.root`; binaries go in `<root>/bin`). `cargo install
--root` is not visible to `build.rs`.

`cargo install` also places `validate-cargo-buf-toolchain` on `PATH` for
post-install checks. That helper is gated on the default `validate-cli`
feature. Pass `--no-default-features` to skip it.

Per-target minimum Buf versions match `buf-tools`; unsupported combinations fail
before any large download. See `build_support/targets.rs` in the repo for the
authoritative table.

## Environment variables

Install location uses this order. Non-empty `BUF_RS_TOOLCHAIN_BIN_DIR`
wins. Else `$CARGO_INSTALL_ROOT/bin` if that env is set. Else
`$CARGO_HOME/bin`. `cargo install --root` is not visible to `build.rs`.

`BUF_RS_CACHE_DIR` is an optional cache root
(`<semver-core>/<target>/` under it). `BUF_RS_RELEASE_BASE_URL` prefixes
release assets. The default is
`https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`.

`validate-cargo-buf-toolchain` is selected by the default `validate-cli`
feature. Set `BUF_RS_VALIDATE_OFFLINE=1` to skip GitHub and crates.io.
Pass `--yaml` for a machine-readable report (install env, bin dir rule,
per-binary status, GitHub / crates.io).

Options that apply only when depending on `buf-tools` directly (layout, build
log, source bundles) are documented in the [buf-tools docs][docs-buf-tools].

## Documentation builds (docs.rs)

docs.rs sets `DOCS_RS=1` and blocks network. `build.rs` returns before any
download or install into `$CARGO_HOME/bin`. The library does not expose
install paths via `env!`; rustdoc only needs the build script to succeed.

The workspace test suite already runs this check with no network.

```bash
DOCS_RS=1 CARGO_NET_OFFLINE=true \
  cargo doc -p buf-toolchain --locked --offline --no-deps
```

## Concurrent cache writers

The same cache-slot lock as `buf-tools` serializes writers under
`<cache-root>/<semver-core>/<target>`.

## Install

```bash
cargo install buf-toolchain
validate-cargo-buf-toolchain
validate-cargo-buf-toolchain --yaml
```

`validate-cargo-buf-toolchain` re-checks installed binaries against the pinned
GitHub release, optionally compares `releases/latest`, and can query crates.io
unless `BUF_RS_VALIDATE_OFFLINE=1`. `--yaml` writes one machine-readable
document (crate pin, resolved bin dir and which env rule won, install env
snapshot, per-binary status, and network / crates.io results). Human text
stays the default.

Put the helper and the Buf binaries in one directory with
`BUF_RS_TOOLCHAIN_BIN_DIR` or `CARGO_INSTALL_ROOT`.

```bash
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" cargo install buf-toolchain
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" validate-cargo-buf-toolchain
```

## Build dependency

```toml
[build-dependencies]
# Example only: pin to the Buf release you need (authoritative: workspace root).
# Set default-features = false if you only need VERSION and the installer.
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
