# buf-sys

[![crates.io](https://img.shields.io/crates/v/buf-sys.svg)](https://crates.io/crates/buf-sys)

> [!WARNING]
> Clanker generated code, running an auto-release pipeline on auto-pilot from an
> external release trigger.
>
> Decide if that degree of automation is appropriate for your requirements.

Rust workspace distributing the official [`buf`](https://github.com/bufbuild/buf)
CLI plus `protoc-gen-buf-breaking` and `protoc-gen-buf-lint` via the
**`buf-sys`** crate. Binaries are **pinned** to upstream Buf releases
(**minisign** + **`sha256.txt`**) but **downloaded at compile time** — see
**[`buf-sys/README.md`](buf-sys/README.md)** for cache layout,
**`BUF_VENDOR_INCLUDE_SOURCE`**, and honest wording about “vendored in spirit.”

Repository home: **[github.com/canardleteer/buf-sys](https://github.com/canardleteer/buf-sys)**
(rename may lag the crate; URLs in **`Cargo.toml`** should match the canonical repo).

## Usage

```toml
[dependencies]
buf-sys = "1.69.0"
```

```rust
use std::process::Command;

let buf = buf_sys::buf_bin_path();
let _ = Command::new(buf).arg("--version").status();
```

## Supported targets

`buf-sys`'s `build.rs` resolves the compilation target to one of the official
`bufbuild/buf` release asset suffixes and downloads three binaries (`buf`,
`protoc-gen-buf-lint`, `protoc-gen-buf-breaking`). If the crate's pinned Buf
version predates a target's introduction, the build fails fast — *before* any
HTTP fetch — with a clear error.

| Asset suffix      | Rust target triples                                                        | Min Buf version |
| ----------------- | -------------------------------------------------------------------------- | --------------- |
| `Linux-x86_64`    | `x86_64-unknown-linux-{gnu,musl}`                                          | 1.0.0           |
| `Linux-aarch64`   | `aarch64-unknown-linux-{gnu,musl}`                                       | 1.0.0           |
| `Linux-armv7`     | `arm-unknown-linux-{gnueabihf,musleabihf}`                               | 1.47.0          |
| `Linux-ppc64le`   | `powerpc64le-unknown-linux-gnu`                                          | 1.54.0          |
| `Linux-riscv64`   | `riscv64gc-unknown-linux-gnu`, `riscv64-unknown-linux-gnu`               | 1.54.0          |
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
`cargo metadata --format-version 1 -p buf-sys` (look under
`packages[].metadata."buf-sys".targets`).

### Examples

Run these from the repository root (the examples set their working directory to
[`examples/`](examples/) so paths like `proto/` resolve correctly).

**[`buf_lint`](examples/buf_lint.rs)** — runs `buf lint` on the sample module under
[`examples/proto/`](examples/proto/).

```bash
cargo run -p buf-sys-examples --example buf_lint
```

First build downloads Buf release binaries (HTTPS to GitHub). Optionally pin the
cache under the workspace:

```bash
BUF_SYS_CACHE_DIR="$PWD/target/buf-sys-cache" \
  cargo run -p buf-sys-examples --example buf_lint
```

**[`protoc_with_buf_plugins`](examples/protoc_with_buf_plugins.rs)** — runs
`protoc` from **`protoc-bin-vendored`** (see [`examples/Cargo.toml`](examples/Cargo.toml))
and wires **`protoc-gen-buf-lint`** and **`protoc-gen-buf-breaking`** from
**`buf-sys`**. It checks
[`weather.proto`](examples/proto/acme/weather/v1/weather.proto) with lint and
breaking detection against a **Buf binary image** baseline.

Generate that baseline once under [`examples/proto/`](examples/proto/) (the file
is **gitignored**). Build **`buf-sys`** first so the official `buf` CLI is
available under `target/`:

```bash
cargo build -p buf-sys
BUF=$(find target -type f -path '*/build/buf-sys-*/out/bin/buf' | head -n 1)
( cd examples/proto && "$BUF" build -o breaking_against.binpb . )
```

Then run the example:

```bash
cargo run -p buf-sys-examples --example protoc_with_buf_plugins
```

Use the same optional **`BUF_SYS_CACHE_DIR`** as above if you want Buf artifacts
under the workspace instead of the default cache directory.

## Tests

```bash
BUF_SYS_CACHE_DIR="$PWD/target/buf-sys-cache"
BUF_EXPECT_VERSION=1.69.0 cargo test --workspace --locked
```

## License

This crate's source is licensed under the **MIT** license — see [`LICENSE`](LICENSE).

The crate does **not** redistribute Buf. The official [`buf`](https://github.com/bufbuild/buf)
CLI and `protoc-gen-buf-*` plugins are downloaded at build time from upstream
`bufbuild/buf` GitHub releases (Apache-2.0) by the consumer's machine and
verified via `sha256.txt` + minisign. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
