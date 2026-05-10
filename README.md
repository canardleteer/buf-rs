# buf-rs

[![buf-tools crate version][badge-buf-tools]][crates-buf-tools]
[![buf-toolchain crate version][badge-buf-toolchain]][crates-buf-toolchain]

[badge-buf-tools]: https://img.shields.io/crates/v/buf-tools.svg
[badge-buf-toolchain]: https://img.shields.io/crates/v/buf-toolchain.svg
[crates-buf-tools]: https://crates.io/crates/buf-tools
[crates-buf-toolchain]: https://crates.io/crates/buf-toolchain

> [!WARNING]
> Clanker generated code, running an auto-release pipeline on auto-pilot from an
> external release trigger.
>
> Decide if that degree of automation is appropriate for your requirements.

Rust workspace distributing the official [Buf][buf-github] CLI plus
`protoc-gen-buf-breaking` and `protoc-gen-buf-lint` via two crates:

- [buf-tools][crates-buf-tools] for Rust dependency integration
- [buf-toolchain][crates-buf-toolchain] for `cargo install buf-toolchain`

Build-time download is required: official Buf binaries are too large for the
crates.io package size limit (~10 MiB), so `buf-tools` does not vendor them like
smaller binary crates.

Repository: [github.com/canardleteer/buf-rs][repo-github]

[buf-github]: https://github.com/bufbuild/buf
[repo-github]: https://github.com/canardleteer/buf-rs

## Usage

Use `buf-tools` when Rust code needs resolved paths to the binaries, or
`buf-toolchain` when you want `cargo install` (or a `[build-dependencies]` hook)
to place `buf` and `protoc-gen-buf-*` in your bin directory.

### `buf-tools` (Cargo.toml dependency)

```toml
[dependencies]
# Example only — match Buf release to root Cargo.toml [workspace.package].version.
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

The `build.rs` shares `buf-tools`’ `build_support` (verify, lock, targets) and
installs `buf` and `protoc-gen-buf-*` with plain names (`*.exe` on Windows). By
default those binaries go to `$CARGO_HOME/bin` (atomic install). `cargo install`
also installs `validate-cargo-buf-toolchain` — run it after install for local
checks plus GitHub / crates.io checks, or set `BUF_RS_VALIDATE_OFFLINE=1` to skip
network I/O.

Alternatively, add `buf-toolchain` under `[build-dependencies]` so `cargo build`
runs the same `build.rs` without `cargo install`:

```toml
[build-dependencies]
# Example only — match root [workspace.package].version or your pin.
buf-toolchain = "1.40.0"
```

- `BUF_RS_TOOLCHAIN_BIN_DIR` (optional) — install into this directory instead
  of `$CARGO_HOME/bin`.
- Otherwise binaries go to `$CARGO_HOME/bin` (or `~/.cargo/bin`).
- `BUF_RS_CACHE_DIR` (optional) overrides the download cache root.
- `BUF_RS_RELEASE_BASE_URL` (optional) overrides the release asset base URL for
  both crates (and runtime validation in `validate-cargo-buf-toolchain`).
- `BUF_RS_VALIDATE_OFFLINE` (optional, `validate-cargo-buf-toolchain`) — set to
  `1` to skip GitHub / crates.io (local checks only).
- `BUF_RS_SOURCE_BASE_URL` (optional, `buf-tools` only) overrides optional
  upstream source tarball base URL.
- `BUF_RS_BUILD_LOG` (optional, `buf-tools` only) controls build-script logging
  policy (`warn`, `verbose`, `silent`).
- `buf-tools` also supports source-controlled defaults in
  `[workspace.metadata.buf-tools.config]` and
  `[package.metadata.buf-tools.config]` (env vars still take precedence).

See [buf-toolchain/README.md](buf-toolchain/README.md) for full env-var
precedence and examples.

### Additional information

For crate-specific variants (including source bundle behavior), see
[buf-tools/README.md](buf-tools/README.md) and
[buf-toolchain/README.md](buf-toolchain/README.md).

### CI prewarm then offline builds

Use a shared cache directory in CI to prewarm online, then run offline:

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo build -p buf-tools -p buf-toolchain
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" CARGO_NET_OFFLINE=true \
  cargo build -p buf-tools -p buf-toolchain
```

## Which Buf version does this repo pin?

Authoritative: [Cargo.toml](Cargo.toml) `[workspace.package].version` (plain
`X.Y.Z`) and the matching `=X.Y.Z` pins on `buf-tools` / `buf-toolchain` under
`[workspace.dependencies]`. Each crate’s `build.rs` downloads the `bufbuild/buf`
tag `vX.Y.Z` from that core.

Examples in this file use a concrete version for copy-paste only — if they
drift from the manifest, trust `Cargo.toml`.

Reading the pinned core:

```bash
cargo xtask expected-buf-version
```

That prints `X.Y.Z` from `[workspace.package].version` (same rule as tests via
`BUF_EXPECT_VERSION`).

### `cargo xtask workspace set-buf-version`

Maintainers use this to set which upstream Buf release the workspace tracks
(plain `X.Y.Z` in the root `Cargo.toml`: `[workspace.package].version` plus
`=X.Y.Z` pins on `buf-tools` and `buf-toolchain`). That can be an older or newer
Buf release, not only “moving forward.”

It is not the same as `cargo xtask publish apply-version`, which the publish
workflow uses on CI to apply `-dev.*` / `-rc.*` crate pre-release suffixes for
`dev` / `rc` channels.

Change the pin (maintainers, outside CI): confirm the release exists on
[bufbuild/buf releases][buf-releases], then:

```bash
cargo xtask workspace set-buf-version X.Y.Z
cargo generate-lockfile
BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"
echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"
cargo test --workspace --locked
```

[buf-releases]: https://github.com/bufbuild/buf/releases

## Supported targets

`buf-tools`’s `build.rs` resolves the compilation target to one of the official
`bufbuild/buf` release asset suffixes and downloads three binaries (`buf`,
`protoc-gen-buf-lint`, `protoc-gen-buf-breaking`). If the crate’s pinned Buf
version predates a target’s introduction, the build fails fast — before any HTTP
fetch — with a clear error.

| Asset suffix      | Rust target triples                                        | Min Buf |
| ----------------- | ---------------------------------------------------------- | ------- |
| `Linux-x86_64`    | `x86_64-unknown-linux-{gnu,musl}`                          | 1.0.0   |
| `Linux-aarch64`   | `aarch64-unknown-linux-{gnu,musl}`                         | 1.0.0   |
| `Linux-armv7`     | `arm-unknown-linux-{gnueabihf,musleabihf}`                 | 1.47.0  |
| `Linux-ppc64le`   | `powerpc64le-unknown-linux-gnu`                            | 1.54.0  |
| `Linux-riscv64`   | `riscv64gc-unknown-linux-gnu`, `riscv64-unknown-linux-gnu` | 1.54.0  |
| `Linux-s390x`     | `s390x-unknown-linux-gnu`                                  | 1.56.0  |
| `Darwin-x86_64`   | `x86_64-apple-darwin`                                      | 1.0.0   |
| `Darwin-arm64`    | `aarch64-apple-darwin`                                     | 1.0.0   |
| `Windows-x86_64`  | `x86_64-pc-windows-{gnu,msvc}`                             | 1.0.0   |
| `Windows-arm64`   | `aarch64-pc-windows-{gnu,msvc}`                            | 1.0.0   |
| `FreeBSD-x86_64`  | `x86_64-unknown-freebsd`                                   | 1.67.0  |
| `FreeBSD-arm64`   | `aarch64-unknown-freebsd`                                  | 1.67.0  |
| `OpenBSD-x86_64`  | `x86_64-unknown-openbsd`                                   | 1.67.0  |
| `OpenBSD-arm64`   | `aarch64-unknown-openbsd`                                  | 1.67.0  |

Tooling can read the same data from:

```bash
cargo metadata --format-version 1 -p buf-tools
```

Look under `packages[].metadata."buf-tools".targets`.

### Examples

Run these from the repository root (examples use [`examples/`](examples/) as
cwd so paths like `proto/` resolve).

[`buf_lint`](examples/buf_lint.rs) runs `buf lint` on the sample under
[`examples/proto/`](examples/proto/):

```bash
cargo run -p buf-tools-examples --example buf_lint
```

First build downloads Buf binaries (HTTPS to GitHub). Optional cache pin:

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo run -p buf-tools-examples --example buf_lint
```

[`protoc_with_buf_plugins`](examples/protoc_with_buf_plugins.rs) runs `protoc`
from `protoc-bin-vendored` (see [`examples/Cargo.toml`](examples/Cargo.toml)) and
wires `protoc-gen-buf-lint` and `protoc-gen-buf-breaking` from `buf-tools`. It
checks [`weather.proto`](examples/proto/acme/weather/v1/weather.proto) with lint
and breaking detection against a Buf binary image baseline.

Generate that baseline once under [`examples/proto/`](examples/proto/) (output
is gitignored). Build `buf-tools` first so the `buf` CLI exists under `target/`:

```bash
cargo build -p buf-tools
BUF=$(find target -type f -path '*/build/buf-tools-*/out/bin/buf' | head -n 1)
( cd examples/proto && "$BUF" build -o breaking_against.binpb . )
```

Then:

```bash
cargo run -p buf-tools-examples --example protoc_with_buf_plugins
```

Use the same optional `BUF_RS_CACHE_DIR` as above to keep Buf artifacts under the
workspace.

## Tests

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache"
BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"
echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"
cargo test --workspace --locked
```

- `BUF_EXPECT_VERSION` must match the workspace Buf core `X.Y.Z`.
- `cargo xtask expected-buf-version` reads `[workspace.package].version` in the
  root `Cargo.toml`.
- You can set `BUF_EXPECT_VERSION` manually instead if you prefer.

### Post-publish testing

After a version is on crates.io, you can run the same **registry-only** smoke
the manual publish workflow uses (minimal Docker context, no workspace `path`
deps): [`.github/ci-scripts/run-integration-docker.sh`](.github/ci-scripts/run-integration-docker.sh)
stages [`rust-toolchain.toml`](rust-toolchain.toml), the integration manifest
under [`.github/ci/integration/`](.github/ci/integration/), and mirrored
[`examples/`](examples/) sources, then builds an image and runs the integration
entrypoint (`cargo add buf-tools`, `cargo install buf-toolchain`, `buf --version`
vs crate semver core, `buf build` for the example baseline, both examples).

`TEST_CRATE_VERSION` must be a **published** semver (whatever you just shipped),
not only the value in `Cargo.toml`:

```bash
TEST_CRATE_VERSION=1.41.0-rc.1 bash .github/ci-scripts/run-integration-docker.sh
```

Or pass the same string as the first argument. Requires Docker (default) or set
`DOCKER=podman`. More detail: [`.github/ci/integration/README.md`](.github/ci/integration/README.md).

## GitHub workflows

Workflow YAML files live under [`.github/workflows/`](.github/workflows/). **Repository
settings, tokens, and operator notes** for each workflow are documented in **comment
headers at the top of those files** (not duplicated here).

| Workflow | Role |
|----------|------|
| [**rust-tests.yml**](.github/workflows/rust-tests.yml) | On **push** / **pull_request** to **main**: fmt, clippy, tests, examples, **`cargo publish --dry-run`** for both crates (matrix: Linux amd64/arm64, macOS arm64, Windows amd64). |
| [**publish-crates.yml**](.github/workflows/publish-crates.yml) | Manual **workflow_dispatch** for crates.io **dev** / **rc** / **stable**; includes **post-publish integration** (Docker) after a successful upload. |
| [**buf-upstream-watch.yml**](.github/workflows/buf-upstream-watch.yml) | **Schedule** (6h), **workflow_dispatch**, **repository_dispatch**: proposes a bump PR when [bufbuild/buf](https://github.com/bufbuild/buf) **releases/latest** is newer than the workspace pin. |

Maintainer-oriented detail: [`AGENTS.md`](AGENTS.md).

## License

Rust sources in this repository are licensed under the MIT license — see
[LICENSE](LICENSE).

The workspace does not vendor Buf binaries inside crates.io packages. The
official [Buf][buf-github] CLI and `protoc-gen-buf-*` plugins are downloaded at
build time from upstream `bufbuild/buf` GitHub releases (Apache-2.0) on the
consumer machine and verified via `sha256.txt` + minisign. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
