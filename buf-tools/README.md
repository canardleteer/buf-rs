# buf-tools

Rust API for resolving paths to the official
[Buf](https://github.com/bufbuild/buf) CLI and `protoc-gen-buf-*` plugins.

- [crates.io/crates/buf-tools][crates-buf-tools]
- [docs.rs/buf-tools][docs-buf-tools]

The repository overview is in the
[repo root README][repo-readme]; this file ships in the published crate.

[crates-buf-tools]: https://crates.io/crates/buf-tools
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

The crates.io tarball does not contain the executables (they exceed the registry
size limit). On first build, `build.rs` downloads official release assets from
`bufbuild/buf` on GitHub, verifies `sha256.txt` with
[minisign](https://jedisct1.github.io/minisign/), and places binaries under
Cargo’s `OUT_DIR`. The Buf release is pinned by this crate’s semver core (see
`CARGO_PKG_VERSION` in `build.rs`).

## Layout mode (`BUF_RS_LAYOUT_MODE`)

`BUF_RS_LAYOUT_MODE` selects where binaries are exposed.

- `cache` (default) puts binaries under `OUT_DIR` and keeps a persistent
  download cache.
- `cache-link` adds symlinks (or copies) under
  `target/buf-tools/<semver-core>/<TARGET>/bin`.
- `cache-verified-link` matches `cache-link` and re-verifies cache
  contents before the link or copy.
- `target` writes artifacts under
  `target/buf-tools/<semver-core>/<TARGET>/...` without the shared cache
  layout.

Default `build_log=warn` keeps happy paths quiet. Set
`BUF_RS_BUILD_LOG=verbose` or `silent` when you need more or less
output.

```bash
BUF_RS_LAYOUT_MODE=cache-link cargo build -p buf-tools
```

## `cargo install` and build dependencies

When your crate lists `buf-tools` as a `[build-dependencies]` entry and end
users install it with `cargo install`, Cargo builds dependencies under a temp
tree (for example `/tmp/cargo-install…/release/build/…/out`) rather than your
project `target/` directory.

**Default `cache` mode** works in that layout. Binaries are exposed via
compile-time `env!` paths under the dependency `OUT_DIR/bin`, and downloads use
the shared cache (`BUF_RS_CACHE_DIR` or the platform cache dir). No project
`target/` ancestor is required.

**Non-cache modes** (`cache-link`, `cache-verified-link`, `target`) need a
layout root. Resolution walks this order.

1. `CARGO_TARGET_DIR` when set → `$CARGO_TARGET_DIR/buf-tools/<core>/<TARGET>/`
2. nearest `OUT_DIR` ancestor named `target` → `target/buf-tools/<core>/<TARGET>/`
3. parent of nearest `OUT_DIR` ancestor named `build` (cargo-install temps) →
   `<profile>/buf-tools/<core>/<TARGET>/` (for example `…/release/buf-tools/…`)

During `cargo install`, step 3 applies when there is no project `target/`.
Linked or copied binaries land under Cargo’s install temp profile directory, not
your repo `target/`. They are not a substitute for project-local
`target/buf-tools/…` from `cargo build`. For install flows, prefer
`BUF_RS_LAYOUT_MODE=cache` unless you explicitly need linked bins in the install
tree.

**Custom Cargo profiles** (`[profile.foo]` in `Cargo.toml`) use step 3 with
whatever directory parents `build/` (for example `foo/buf-tools/<core>/<TARGET>/`),
including under `cargo install` temps.

### Recovery without republishing your crate

Environment variables are read when `buf-tools` compiles and override
`Cargo.toml` metadata. Set them when building or installing the consumer crate.

| Symptom | What to try |
|--------|-------------|
| `could not locate Cargo target dir from OUT_DIR` on a `buf-tools` release from before the layout fix | Upgrade `buf-tools` to a release containing the fix. No env var bypasses the unconditional target walk in those versions. Alternatives: use [`buf-toolchain`](../buf-toolchain/README.md) as a build dependency instead, or `cargo install buf-toolchain` if you only need the CLI. |
| `cargo install` fails with non-default `layout_mode` in workspace metadata | `BUF_RS_LAYOUT_MODE=cache cargo install …` |
| Need a stable writable layout root for non-cache modes | `CARGO_TARGET_DIR=/path/to/writable/dir cargo install …` |
| Network flake or air-gapped retry | Prewarm with `BUF_RS_CACHE_DIR=… cargo build` (any crate using `buf-tools`), then `BUF_RS_CACHE_DIR=… CARGO_NET_OFFLINE=1 cargo install …` |
| Diagnose resolution or downloads | `BUF_RS_BUILD_LOG=verbose cargo install …` |
| Override download mirrors | `BUF_RS_RELEASE_BASE_URL`, `BUF_RS_SOURCE_BASE_URL` (see below) |

Repo-local defaults can live in `.cargo/config.toml` instead of the
shell.

```toml
# .cargo/config.toml
[env]
BUF_RS_LAYOUT_MODE = "cache"
# BUF_RS_CACHE_DIR = "target/buf-rs-cache"
```

Contract tests need network on a cold cache. CI runs them on linux-amd64.
Locally they are opt-in.

```bash
cargo test -p buf-tools --locked --test cargo_install_layout -- --ignored
```

## Build-script logging (`BUF_RS_BUILD_LOG`)

- `warn` (default; `true` aliases this) prints warnings and failures only.
- `verbose` prints full progress and diagnostics.
- `silent` (`false` aliases this) suppresses warnings from the build script.

Build scripts only surface output via `cargo:warning=` lines.

## Source-controlled configuration

Defaults can live in `Cargo.toml` metadata (overridden by env vars, highest
precedence).

```toml
[workspace.metadata.buf-tools.config]
layout_mode = "cache-link"
build_log = "warn"
cache_dir = "target/buf-rs-cache"
# Example only: align with [workspace.package].version (authoritative).
release_base_url = "https://github.com/bufbuild/buf/releases/download/v1.40.0/"
source_base_url = "https://github.com/bufbuild/buf/archive/refs/tags/"
```

Per-package overrides use `[package.metadata.buf-tools.config]`.

Supported keys are `layout_mode`, `build_log`, `cache_dir`,
`release_base_url`, and `source_base_url`. Resolution walks built-in
defaults, then workspace metadata, then package metadata, then the
environment (and optional `.cargo/config.toml` `[env]`).

## Network

HTTPS GET to `github.com` only; no GitHub token required for release downloads.

## Documentation builds (docs.rs)

docs.rs sets `DOCS_RS=1` and blocks network. `build.rs` then skips GitHub
downloads, writes 12 KiB ELF or MZ placeholders under `OUT_DIR/bin`, and
emits cache-mode layout metadata (`resolved_layout_mode()` is `"cache"`,
`bin_layout_root()` is `None`). It does not walk `OUT_DIR` for a `target/`
ancestor.

`compiled_for_docs_rs()` is `true` in that build. Path accessors still
compile, but the files are not the Buf CLI. Do not execute them from a
consumer `build.rs` during documentation builds. Package a generated
descriptor (or skip live `buf`) when `compiled_for_docs_rs()` is true.

The workspace test suite already runs this check with no network.

```bash
DOCS_RS=1 CARGO_NET_OFFLINE=true \
  cargo test -p buf-tools --locked --offline --lib
DOCS_RS=1 CARGO_NET_OFFLINE=true \
  cargo doc -p buf-tools --locked --offline --no-deps
```

## Cache layout

Artifacts live under `$BUF_RS_CACHE_DIR/<semver-core>/<TARGET>/` when set,
otherwise under the platform cache dir (`XDG_CACHE_HOME`, `%LOCALAPPDATA%` on
Windows, `~/Library/Caches` on macOS, or `~/.cache` elsewhere), e.g.
`XDG_CACHE_HOME/buf-tools/...`. A successful download survives `cargo clean`
for that cache root.

## Optional source tree (`BUF_RS_INCLUDE_SOURCE`)

When `BUF_RS_INCLUDE_SOURCE=1`, `build.rs` can fetch the tagged source archive
from GitHub. Source tarballs are not covered by the same `sha256.txt` manifest
as binaries. Treat that tree as inspection-only. Do not treat it as the
primary integrity check.

## URL overrides

`BUF_RS_RELEASE_BASE_URL` prefixes `sha256.txt`, signatures, and
binaries. The default is
`https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`.
`BUF_RS_SOURCE_BASE_URL` prefixes optional source fetches. The default
is `https://github.com/bufbuild/buf/archive/refs/tags/`.

Trailing slash optional.

## Concurrent writers

`build.rs` uses a lock file under the cache slot so parallel builds do not
corrupt downloads. With `CARGO_NET_OFFLINE=true`, a cold cache fails fast
instead of downloading.

## CI: online prewarm then offline build

```bash
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" cargo build -p buf-tools
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" CARGO_NET_OFFLINE=true \
  cargo build -p buf-tools
```

For supported targets and `min_version` metadata, see the crate API docs and
`buf-tools` `build_support` sources in the repository.
