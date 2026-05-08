# buf-toolchain

Install official Buf release executables using Cargo-managed packaging and version pinning.

## What this crate does

When this crate is built (including `cargo install buf-toolchain`), its `build.rs`:

1. Resolves the current compilation target.
2. Downloads Buf release artifacts for the crate semver core from `bufbuild/buf`.
3. Verifies `sha256.txt` with minisign, then validates executable checksums.
4. Installs available binaries into a dedicated managed directory.

This installer is best-effort per target/version:

- `buf` is required.
- `protoc-gen-buf-lint` and `protoc-gen-buf-breaking` are installed when available in the selected upstream release for that target.

## Directory and environment variables

Environment variable precedence:

- `BUF_RS_TOOLCHAIN_BIN_DIR`: if set, install executables directly into this directory.
- otherwise install under `BUF_RS_TOOLCHAIN_HOME/<version-core>/<target>/bin`.
- `BUF_RS_TOOLCHAIN_HOME`: default install root override.
- default install root: `$CARGO_HOME/buf-toolchain`, then `~/.cargo/buf-toolchain` when `CARGO_HOME` is unset.

Cache:

- `BUF_RS_CACHE_DIR`: optional cache root override.
- default cache root: `$XDG_CACHE_HOME/buf-toolchain` with platform fallback via `dirs::cache_dir()`.
- `BUF_RS_RELEASE_BASE_URL`: optional release asset base override for
  `sha256.txt`, `sha256.txt.minisig`, and binary downloads. Default:
  `https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`.

## Concurrent cache writers

`build.rs` uses a lock file per cache slot (`<cache-root>/<semver-core>/<target>`)
to avoid two jobs writing the same artifact concurrently. If another process
already holds the lock, the current process waits and logs lock activity, then
re-validates expected cached artifacts after lock release.

## Install

```bash
cargo install buf-toolchain
```

To place the executables in a custom path:

```bash
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" cargo install buf-toolchain
```

## CI prewarm then offline install/build

```bash
# Online prewarm
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo build -p buf-toolchain

# Offline rebuild with warm cache
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  CARGO_NET_OFFLINE=true \
  cargo build -p buf-toolchain
```
