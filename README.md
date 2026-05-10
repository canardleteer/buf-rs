# buf-rs

[![crates.io buf-tools](https://img.shields.io/crates/v/buf-tools.svg)](https://crates.io/crates/buf-tools)
[![crates.io buf-toolchain](https://img.shields.io/crates/v/buf-toolchain.svg)](https://crates.io/crates/buf-toolchain)

> [!WARNING]
> Clanker generated code, running an auto-release pipeline on auto-pilot from an
> external release trigger.
>
> Decide if that degree of automation is appropriate for your requirements.

Rust workspace distributing the official
[`buf`](https://github.com/bufbuild/buf) CLI plus `protoc-gen-buf-breaking` and
`protoc-gen-buf-lint` via two crates:

- **`buf-tools`** for Rust dependency integration
- **`buf-toolchain`** for **`cargo install buf-toolchain`**

**Why build-time download is required:** official Buf binaries are too large to
ship inside a crates.io package (the registry cap is about 10 MiB per crate
upload), so `buf-tools` cannot vendor binaries the way some smaller
binary-vendoring crates do.

**Repository home:** [github.com/canardleteer/buf-rs](https://github.com/canardleteer/buf-rs)

## Usage

Use **`buf-tools`** when Rust code needs resolved paths to the binaries, or
**`buf-toolchain`** when you want **`cargo install`** (or a
**`[build-dependencies]`** hook) to place **`buf`** and **`protoc-gen-buf-*`**
into your canonical bin directory.

### `buf-tools` (Cargo.toml dependency)

```toml
[dependencies]
# Example only — use the same X.Y.Z as the Buf release you depend on (see root Cargo.toml).
buf-tools = "1.40.0"
```

```rust
use std::process::Command;

let buf = buf_tools::buf_bin_path();
let _ = Command::new(buf).arg("--version").status();
```

### `buf-toolchain` (`cargo install` or build dependency)

Primary workflow:

```bash
cargo install buf-toolchain
```

The **`build.rs`** shares **`buf-tools`’ `build_support`** (verify, lock,
targets) and installs **`buf`** and **`protoc-gen-buf-*`** with plain names
(`*.exe` on Windows). **By default** those binaries are written **directly** to
**`$CARGO_HOME/bin`** (atomic install). **`cargo install`** also copies
**`validate-cargo-buf-toolchain`** — run it after install for local checks plus
GitHub / crates.io checks, or set **`BUF_RS_VALIDATE_OFFLINE=1`** to skip
network I/O.

Alternatively, add **`buf-toolchain`** under **`[build-dependencies]`** so
**`cargo build`** runs the same **`build.rs`** without using **`cargo
install`**:

```toml
[build-dependencies]
# Example only — align with root [workspace.package].version in this repo or your chosen pin.
buf-toolchain = "1.40.0"
```

- `BUF_RS_TOOLCHAIN_BIN_DIR` (optional) — install into this directory instead of
  **`$CARGO_HOME/bin`**.
- Otherwise binaries go to **`$CARGO_HOME/bin`** (or `~/.cargo/bin`).
- `BUF_RS_CACHE_DIR` (optional) overrides the download cache root.
- `BUF_RS_RELEASE_BASE_URL` (optional) overrides the release asset base URL for
  both crates (and runtime validation in **`validate-cargo-buf-toolchain`**).
- `BUF_RS_VALIDATE_OFFLINE` (optional, **`validate-cargo-buf-toolchain`**) — set
  to **`1`** to skip GitHub / crates.io (local checks only).
- `BUF_RS_SOURCE_BASE_URL` (optional, `buf-tools` only) overrides optional
  upstream source tarball base URL.
- `BUF_RS_BUILD_LOG` (optional, `buf-tools` only) controls build-script logging
  policy (`warn`, `verbose`, `silent`).
- `buf-tools` also supports source-controlled defaults in
  `[workspace.metadata.buf-tools.config]` and
  `[package.metadata.buf-tools.config]` (env vars still take precedence).

See [`buf-toolchain/README.md`](buf-toolchain/README.md) for full env-var precedence and examples.

### Additional information

For crate-specific variants (including source bundle behavior), see
[`buf-tools/README.md`](buf-tools/README.md) and
[`buf-toolchain/README.md`](buf-toolchain/README.md).

### CI prewarm then offline builds

Use a shared cache directory in CI to prewarm online, then run offline:

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo build -p buf-tools -p buf-toolchain
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" CARGO_NET_OFFLINE=true \
  cargo build -p buf-tools -p buf-toolchain
```

## Which Buf version does this repo pin?

**Authoritative:** [`Cargo.toml`](Cargo.toml)'s **`[workspace.package].version`**
(plain **`X.Y.Z`**) and the matching **`=X.Y.Z`** pins on **`buf-tools`** /
**`buf-toolchain`** under **`[workspace.dependencies]`**. **`build.rs`**
downloads **`bufbuild/buf`** tag **`vX.Y.Z`** from that core.

**Examples**: in this file use a concrete version for copy-paste only — if they
drift from the manifest, **trust the `Cargo.toml`**.

**Reading the pinned core:** `cargo xtask expected-buf-version`
(prints **`X.Y.Z`** from **`[workspace.package].version`**).

## Supported targets

`buf-tools`'s `build.rs` resolves the compilation target to one of the official
`bufbuild/buf` release asset suffixes and downloads three binaries (`buf`,
`protoc-gen-buf-lint`, `protoc-gen-buf-breaking`). If the crate's pinned Buf
version predates a target's introduction, the build fails fast — *before* any
HTTP fetch — with a clear error.

| Asset suffix      | Rust target triples                                                        | Min Buf version |
| ----------------- | -------------------------------------------------------------------------- | --------------- |
| `Linux-x86_64`    | `x86_64-unknown-linux-{gnu,musl}`                                          | 1.0.0           |
| `Linux-aarch64`   | `aarch64-unknown-linux-{gnu,musl}`                                         | 1.0.0           |
| `Linux-armv7`     | `arm-unknown-linux-{gnueabihf,musleabihf}`                                 | 1.47.0          |
| `Linux-ppc64le`   | `powerpc64le-unknown-linux-gnu`                                            | 1.54.0          |
| `Linux-riscv64`   | `riscv64gc-unknown-linux-gnu`, `riscv64-unknown-linux-gnu`                 | 1.54.0          |
| `Linux-s390x`     | `s390x-unknown-linux-gnu`                                                  | 1.56.0          |
| `Darwin-x86_64`   | `x86_64-apple-darwin`                                                      | 1.0.0           |
| `Darwin-arm64`    | `aarch64-apple-darwin`                                                     | 1.0.0           |
| `Windows-x86_64`  | `x86_64-pc-windows-{gnu,msvc}`                                             | 1.0.0           |
| `Windows-arm64`   | `aarch64-pc-windows-{gnu,msvc}`                                            | 1.0.0           |
| `FreeBSD-x86_64`  | `x86_64-unknown-freebsd`                                                   | 1.67.0          |
| `FreeBSD-arm64`   | `aarch64-unknown-freebsd`                                                  | 1.67.0          |
| `OpenBSD-x86_64`  | `x86_64-unknown-openbsd`                                                   | 1.67.0          |
| `OpenBSD-arm64`   | `aarch64-unknown-openbsd`                                                  | 1.67.0          |

Tooling can read the same data programmatically from
`cargo metadata --format-version 1 -p buf-tools` (look under
`packages[].metadata."buf-tools".targets`).

### Examples

Run these from the repository root (the examples set their working directory to
[`examples/`](examples/) so paths like `proto/` resolve correctly).

**[`buf_lint`](examples/buf_lint.rs)** — runs `buf lint` on the sample module under
[`examples/proto/`](examples/proto/).

```bash
cargo run -p buf-tools-examples --example buf_lint
```

First build downloads Buf release binaries (HTTPS to GitHub). Optionally pin the
cache under the workspace:

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo run -p buf-tools-examples --example buf_lint
```

**[`protoc_with_buf_plugins`](examples/protoc_with_buf_plugins.rs)** — runs
`protoc` from **`protoc-bin-vendored`** (see [`examples/Cargo.toml`](examples/Cargo.toml))
and wires **`protoc-gen-buf-lint`** and **`protoc-gen-buf-breaking`** from
**`buf-tools`**. It checks
[`weather.proto`](examples/proto/acme/weather/v1/weather.proto) with lint and
breaking detection against a **Buf binary image** baseline.

Generate that baseline once under [`examples/proto/`](examples/proto/) (the file
is **gitignored**). Build **`buf-tools`** first so the official `buf` CLI is
available under `target/`:

```bash
cargo build -p buf-tools
BUF=$(find target -type f -path '*/build/buf-tools-*/out/bin/buf' | head -n 1)
( cd examples/proto && "$BUF" build -o breaking_against.binpb . )
```

Then run the example:

```bash
cargo run -p buf-tools-examples --example protoc_with_buf_plugins
```

Use the same optional **`BUF_RS_CACHE_DIR`** as above if you want Buf artifacts
under the workspace instead of the default cache directory.

### `cargo xtask workspace set-buf-version`

Maintainers use this to **set** which upstream Buf release the workspace tracks
(plain **`X.Y.Z`** in the **root** [`Cargo.toml`](Cargo.toml):
**`[workspace.package].version`** plus **`=X.Y.Z`** pins on **`buf-tools`** and
**`buf-toolchain`**). That can be an **older or newer** Buf line — it is not
only “moving forward.”

It is **not** the same as **`cargo xtask publish apply-version`**, which the
publish workflow uses on CI to apply **`-test.*`** / **`-rc.*`** crate
pre-release suffixes for **`dev`** / **`rc`** channels.

**Change the pin (maintainers, outside CI):** confirm the release exists on
[bufbuild/buf releases](https://github.com/bufbuild/buf/releases), then:

```bash
cargo xtask workspace set-buf-version X.Y.Z
cargo generate-lockfile
BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"
echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"
cargo test --workspace --locked
```

## Tests

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache"
BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"
echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"
cargo test --workspace --locked
```

- `BUF_EXPECT_VERSION` must match the workspace Buf core **`X.Y.Z`**.
- **`cargo xtask expected-buf-version`** reads **`[workspace.package].version`**
  in the root **`Cargo.toml`**.
- You can set **`BUF_EXPECT_VERSION`** manually instead if you prefer.

## License

Rust sources in this repository are licensed under the **MIT** license — see
[`LICENSE`](LICENSE).

The workspace **does not** vendor or redistribute Buf binaries inside crates.io
packages. The official [`buf`](https://github.com/bufbuild/buf) CLI and
`protoc-gen-buf-*` plugins are downloaded at build time from upstream
`bufbuild/buf` GitHub releases (Apache-2.0) on the consumer machine and verified
via `sha256.txt` + minisign. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
