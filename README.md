# buf-rs

[![crates.io](https://img.shields.io/crates/v/buf-tools.svg)](https://crates.io/crates/buf-tools)

> [!WARNING]
> Clanker generated code, running an auto-release pipeline on auto-pilot from an
> external release trigger.
>
> Decide if that degree of automation is appropriate for your requirements.

Rust workspace distributing the official
[`buf`](https://github.com/bufbuild/buf) CLI plus `protoc-gen-buf-breaking` and
`protoc-gen-buf-lint` via two crates: **`buf-tools`** for Rust dependency
integration and **`buf-toolchain`** for **`cargo install buf-toolchain`** (and
optionally **`[build-dependencies]`**) so upstream binaries land in a single
canonical directory (default **`$CARGO_HOME/bin`**). Cargo requires every
installable crate to ship at least one executable — this crate installs
**`validate-cargo-buf-toolchain`**, which (unless
**`BUF_RS_VALIDATE_OFFLINE=1`**) re-downloads **`sha256.txt`** +
**`sha256.txt.minisign`** from GitHub for your installed Buf core, verifies
**minisign**, compares file hashes to the manifest, compares
**`releases/latest`** to **`buf --version`**, and checks crates.io for
**`buf-toolchain`** when a newer Buf exists. **`buf`** and
**`protoc-gen-buf-*`** themselves come from **`build.rs`**. Binaries are
**pinned** to upstream Buf releases (**minisign** + **`sha256.txt`**) and
downloaded on the consumer machine.

Why build-time download is required: official Buf binaries are too large to ship
inside a crates.io package (the registry cap is about 10 MiB per crate upload),
so `buf-tools` cannot vendor binaries the way some smaller binary-vendoring
crates do.

**Repository home:** [github.com/canardleteer/buf-rs](https://github.com/canardleteer/buf-rs)

## Usage

Use **`buf-tools`** when Rust code needs resolved paths to the binaries, or
**`buf-toolchain`** when you want **`cargo install`** (or a
**`[build-dependencies]`** hook) to place **`buf`** and **`protoc-gen-buf-*`**
into your canonical bin directory.

### `buf-tools` (Cargo.toml dependency)

```toml
[dependencies]
buf-tools = "1.69.0"
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
buf-toolchain = "1.69.0"
```

Upstream asset names such as `buf-Linux-x86_64` exist **only under the download
cache**.

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

### CI prewarm then offline builds

Use a shared cache directory in CI to prewarm online, then run offline:

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo build -p buf-tools -p buf-toolchain
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" CARGO_NET_OFFLINE=true \
  cargo build -p buf-tools -p buf-toolchain
```

For crate-specific variants (including source bundle behavior), see
[`buf-tools/README.md`](buf-tools/README.md) and
[`buf-toolchain/README.md`](buf-toolchain/README.md).

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

## Tests

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache"
BUF_EXPECT_VERSION=1.69.0 cargo test --workspace --locked
```

## License

This crate's source is licensed under the **MIT** license — see [`LICENSE`](LICENSE).

The crate does **not** redistribute Buf. The official [`buf`](https://github.com/bufbuild/buf)
CLI and `protoc-gen-buf-*` plugins are downloaded at build time from upstream
`bufbuild/buf` GitHub releases (Apache-2.0) by the consumer's machine and
verified via `sha256.txt` + minisign. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
