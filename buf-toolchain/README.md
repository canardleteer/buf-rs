# buf-toolchain

Install official [Buf](https://github.com/bufbuild/buf) release executables using
Cargo-managed packaging and version pinning. This file is the **buf-toolchain**
package README on [crates.io](https://crates.io/crates/buf-toolchain) and in the
repo [tree](https://github.com/canardleteer/buf-rs/tree/main/buf-toolchain).

Shared helpers live in the workspace
([`build_support` source tree](https://github.com/canardleteer/buf-rs/tree/main/buf-tools/build_support));
**`buf-tools`** documents the same verify/cache/target behaviour.

## What this crate does

When this crate is built (including **`cargo install buf-toolchain`** or as
**`[build-dependencies]`**), its `build.rs` compiles those helpers (same
verify/write/lock/target code as [`buf-tools`](https://crates.io/crates/buf-tools))
and then:

1. Resolves the current compilation target.
2. Downloads Buf release artifacts for the crate semver core from `bufbuild/buf`
   (or uses a valid cache copy under lock).
3. Verifies `sha256.txt` with minisign, then validates each executable against
   that manifest.
4. Writes **`buf`**, **`protoc-gen-buf-breaking`**, **`protoc-gen-buf-lint`**
   (and **`*.exe`** on Windows) into a **single canonical directory** using
   atomic installs (`write_executable`).

**Default:** **`$CARGO_HOME/bin`** (or **`~/.cargo/bin`** when `CARGO_HOME` is unset),
alongside other **`cargo install`** binaries. **`build.rs`** writes **`buf`** and
**`protoc-gen-buf-*`** there; **`cargo install`** also installs **`validate-cargo-buf-toolchain`**.

**Override:** set **`BUF_RS_TOOLCHAIN_BIN_DIR`** to install into a directory of
your choice instead of **`$CARGO_HOME/bin`**.

Per-target requirements match **`buf-tools`**: after downloading
**`sha256.txt`**, the build checks that **all three** upstream filenames for
your asset suffix (`buf-…`, both `protoc-gen-buf-*-…`) appear in the manifest.
If any are missing (or your Rust triple maps to a platform **Buf did not ship
for that release**), the build fails **before** fetching blobs — same
**`target_supported`** guard as
[`buf-tools` `build.rs`](https://github.com/canardleteer/buf-rs/blob/main/buf-tools/build.rs).
Older Buf releases list **fewer platforms** than today’s matrix; this crate pins
a minimum Buf **core** per target in
[`targets.rs`](https://github.com/canardleteer/buf-rs/blob/main/buf-tools/build_support/targets.rs)
so you do not select a release predating assets for that OS/arch.

## Directory and environment variables

Where binaries are written:

1. **`BUF_RS_TOOLCHAIN_BIN_DIR`** — if set (non-empty), install into this
   directory only.
2. Else **`$CARGO_HOME/bin`** (typically **`~/.cargo/bin`**).

Cache:

- **`BUF_RS_CACHE_DIR`**: optional cache root override (same semantics as
  `buf-tools`: this path is the root for `<semver-core>/<rust-target>/` slots).
- Default cache root: **`$XDG_CACHE_HOME/buf-toolchain`** (via
  `dirs::cache_dir()`).
- **`BUF_RS_RELEASE_BASE_URL`**: optional release asset base for `sha256.txt`,
  `sha256.txt.minisig`, and binaries. Default:
  `https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`.
- **`BUF_RS_VALIDATE_OFFLINE`**: set to **`1`** so
  **`validate-cargo-buf-toolchain`** skips GitHub / crates.io (local pin checks
  only).

## Concurrent cache writers

`build.rs` uses **`buf-tools`’ cache-slot lock** on
`<cache-root>/<semver-core>/<target>` so two jobs do not write the same artifact
concurrently. The lock is held until cached blobs are valid **and** canonical
installs have finished.

## Install

```bash
cargo install buf-toolchain
validate-cargo-buf-toolchain
```

**`validate-cargo-buf-toolchain`** (installed because **`cargo install`** must
ship a binary) re-checks local **`buf`** / plugins against the official GitHub
release for your installed Buf core (**minisign** + **`sha256.txt`**), compares
**`releases/latest`**, and probes crates.io for a matching **`buf-toolchain`**
when a newer Buf exists. Set **`BUF_RS_VALIDATE_OFFLINE=1`** to skip network.
See the repo **[README](https://github.com/canardleteer/buf-rs#readme)** for the
full env-var list.

Custom flat directory:

```bash
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" cargo install buf-toolchain
BUF_RS_TOOLCHAIN_BIN_DIR="$HOME/.local/bin" validate-cargo-buf-toolchain
```

## Build dependency (optional)

If you prefer not to use **`cargo install`**, list **`buf-toolchain`** under
**`[build-dependencies]`** so **`cargo build`** runs **`build.rs`** and installs
the same binaries:

```toml
[build-dependencies]
buf-toolchain = "1.69.0"
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

## Integration tests (maintainers)

Nested `cargo build` with isolated temp `CARGO_HOME` / cache (needs network if
cache is cold):

```bash
cargo test -p buf-toolchain --locked --test managed_bin_layout -- --ignored
```

## crates.io note

Shared logic lives in the
**[`buf-tools` `build_support` tree](https://github.com/canardleteer/buf-rs/tree/main/buf-tools/build_support)**.
In this checkout, **`buf-toolchain/build_support`** is a **symlink** to that
directory so
**[`include`](https://github.com/canardleteer/buf-rs/blob/main/buf-toolchain/Cargo.toml)**
packs the **`*.rs`** files into the **`.crate`**. **`build.rs`** and **`src/`**
wire modules via **`#[path]`** under **`build_support/`**.

Before **`cargo publish`**, run **`cargo package -p buf-toolchain`** (it verifies
the tarball by building in isolation). On Windows, enable symlink checkouts if
the symlink is missing (**`git config core.symlinks true`**) or recreate it.
