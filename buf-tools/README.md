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

## What this crate does

The crates.io tarball does not contain the executables (they exceed the registry
size limit). On first build, `build.rs` downloads official release assets from
`bufbuild/buf` on GitHub, verifies `sha256.txt` with
[minisign](https://jedisct1.github.io/minisign/), and places binaries under
Cargo’s `OUT_DIR`. The Buf release is pinned by this crate’s semver core (see
`CARGO_PKG_VERSION` in `build.rs`).

## Layout mode (`BUF_RS_LAYOUT_MODE`)

Compile-time selector for where binaries are exposed:

- `cache` (default): binaries under `OUT_DIR`, persistent cache for downloads.
- `cache-link`: cache plus symlinks (or copies) under
  `target/buf-tools/<semver-core>/<TARGET>/bin`.
- `cache-verified-link`: like `cache-link`, re-verifies cache contents before
  link/copy.
- `target`: artifacts under `target/buf-tools/<semver-core>/<TARGET>/...`
  without the shared cache layout.

Build output policy: default `build_log=warn` keeps happy paths quiet; set
`BUF_RS_BUILD_LOG=verbose` or `silent` as needed.

```bash
BUF_RS_LAYOUT_MODE=cache-link cargo build -p buf-tools
```

## Build-script logging (`BUF_RS_BUILD_LOG`)

- `warn` (default; `true` aliases this): warnings and failures only.
- `verbose`: full progress and diagnostics.
- `silent` (`false` aliases this): suppress warnings from the build script.

Build scripts only surface output via `cargo:warning=` lines.

## Source-controlled configuration

Defaults can live in `Cargo.toml` metadata (overridden by env vars, highest
precedence):

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

Supported keys: `layout_mode`, `build_log`, `cache_dir`, `release_base_url`,
`source_base_url`. Resolution order: built-in defaults → workspace metadata →
package metadata → environment (and optional `.cargo/config.toml` `[env]`).

## Network

HTTPS GET to `github.com` only; no GitHub token required for release downloads.

## Cache layout

Artifacts live under `$BUF_RS_CACHE_DIR/<semver-core>/<TARGET>/` when set,
otherwise under the platform cache dir (via the `dirs` crate), e.g.
`XDG_CACHE_HOME/buf-tools/...`. A successful download survives `cargo clean`
for that cache root.

## Optional source tree (`BUF_RS_INCLUDE_SOURCE`)

When `BUF_RS_INCLUDE_SOURCE=1`, `build.rs` can fetch the tagged source archive
from GitHub. Source tarballs are not covered by the same `sha256.txt` manifest
as binaries; use for inspection, not as the primary integrity story.

## URL overrides

- `BUF_RS_RELEASE_BASE_URL`: prefix for `sha256.txt`, signatures, and binaries
  (default `https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`).
- `BUF_RS_SOURCE_BASE_URL`: prefix for optional source fetches (default
  `https://github.com/bufbuild/buf/archive/refs/tags/`).

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
