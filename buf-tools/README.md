# buf-tools

Rust paths to the official **[Buf](https://github.com/bufbuild/buf)** CLI and
bundled `protoc-gen-buf-*` plugins. This file is the **buf-tools** package
README on [crates.io](https://crates.io/crates/buf-tools) and in the repo
[tree](https://github.com/canardleteer/buf-rs/tree/main/buf-tools).

## What this crate does (read this first)

The **crates.io package does not contain** the executables. They are larger
than the crates.io upload cap (~10 MiB per package), so on **first build**
this crate’s **`build.rs`**
downloads them from **`bufbuild/buf` GitHub releases**, verifies
**[minisign](https://jedisct1.github.io/minisign/)** + **`sha256.txt`**, then
installs them under Cargo’s **`OUT_DIR`**. Same **upstream artifacts** you would
get from the official release — **pinned by this crate’s semver** — but
**fetched at compile time**, not shipped inside the `.crate` file.

## Layout mode (`BUF_RS_LAYOUT_MODE`)

`BUF_RS_LAYOUT_MODE` is a compile-time selector for where `buf-tools` exposes
executables:

- `cache` (default; also used when unset/empty): current behavior, binaries land
  under Cargo `OUT_DIR` and downloads are backed by persistent cache.
- `cache-link`: populate/use persistent cache, then expose binaries at
  `target/buf-tools/<semver-core>/<TARGET>/bin` via symlink (fallback to copy
  when symlink is unavailable).
- `cache-verified-link`: same as `cache-link`, but explicitly re-verifies cache
  artifacts against trusted release metadata before linking/copying.
- `target`: bypass persistent cache and keep downloaded artifacts under
  `target/buf-tools/<semver-core>/<TARGET>/...` (convenient but more re-download
  prone after cleanups).

By default (`build_log=warn`), happy-path builds stay quiet while edge-case
diagnostics still emit as warnings. Set `BUF_RS_BUILD_LOG=verbose` for full
detail, or `BUF_RS_BUILD_LOG=silent` to suppress warning output entirely.

Example:

```bash
BUF_RS_LAYOUT_MODE=cache-link cargo build -p buf-tools
```

## Build-script logging (`BUF_RS_BUILD_LOG`)

- `warn` (default; also used by unset/empty and `true`): emit edge-case
  diagnostics and build failures, suppress normal info/progress.
- `verbose`: emit full informational/progress output.
- `silent` (also used by `false`): suppress build-script warnings entirely.
- Cargo caveat: build scripts only expose user-visible output through
  `cargo:warning=`. These levels control emission policy, not Cargo severity
  classes.

## Source-controlled configuration

`buf-tools` also supports source-controlled defaults in `Cargo.toml` metadata:

```toml
[workspace.metadata.buf-tools.config]
layout_mode = "cache-link"
build_log = "warn"
cache_dir = "target/buf-rs-cache"
# Use the same Buf vX.Y.Z as this crate’s semver core (e.g. v1.69.0 for 1.69.0):
release_base_url = "https://github.com/bufbuild/buf/releases/download/v1.69.0/"
source_base_url = "https://github.com/bufbuild/buf/archive/refs/tags/"
```

You can also override per package:

```toml
[package.metadata.buf-tools.config]
layout_mode = "target"
build_log = "verbose"
```

Supported keys:

- `layout_mode`: `cache`, `cache-link`, `cache-verified-link`, or `target`
- `build_log`: `warn`, `verbose`, `silent` (`true` aliases `warn`; `false`
  aliases `silent`)
- `cache_dir`: cache root directory path
- `release_base_url`: release asset base URL
- `source_base_url`: source archive base URL

Resolution order is:

1. Built-in defaults
2. `[workspace.metadata.buf-tools.config]`
3. `[package.metadata.buf-tools.config]`
4. Environment variables (**highest precedence**)

Environment variables may also be set via `.cargo/config.toml` `[env]`.

## Network and authentication

- **HTTPS GET** to **`github.com`** only — **no GitHub token / PAT** required.
- Progress lines use **`cargo:warning=`** (often **~10% steps** per large file
  when `Content-Length` is present).

## Cache (default: no surprise re-downloads)

Downloaded blobs are stored under:

- **`$BUF_RS_CACHE_DIR/<semver-core>/<TARGET>/`** if **`BUF_RS_CACHE_DIR`**
  is set, else
- **`$XDG_CACHE_HOME/buf-tools/<semver-core>/<TARGET>/`** (with platform fallbacks
  via the **`dirs`** crate — see implementation).

After a successful download, **`cargo clean`** does **not** wipe this cache;
routine rebuilds reuse verified files.

## Optional upstream source (`BUF_RS_INCLUDE_SOURCE`)

When **`BUF_RS_INCLUDE_SOURCE=1`** (or `true` / `yes`), **`build.rs`** also
downloads the **tagged source archive** from GitHub
(`archive/refs/tags/v{X.Y.Z}.tar.gz`), extracts it under the cache slot, and
sets **`BUF_RS_SOURCE_ROOT`** for this build.

**Integrity note:** Binaries are verified with Buf’s **`sha256.txt`** +
**minisign**. GitHub-generated **source tarballs are not covered by that
manifest** — treat them as an **audit / inspection convenience**, not the same
assurance level as the binary pipeline.

## Mirror/base URL overrides

Use these when your environment cannot reach GitHub directly:

- `BUF_RS_RELEASE_BASE_URL` overrides the release asset base used for
  `sha256.txt`, `sha256.txt.minisig`, and binary downloads.
  - Default:
    `https://github.com/bufbuild/buf/releases/download/v{X.Y.Z}/`
- `BUF_RS_SOURCE_BASE_URL` overrides the base used for optional source tarball
  fetches (`BUF_RS_INCLUDE_SOURCE=1`).
  - Default:
    `https://github.com/bufbuild/buf/archive/refs/tags/`

Both values should be URL prefixes. A trailing slash is optional.

## Concurrent cache writers

`build.rs` coordinates concurrent jobs with a slot lock file under the resolved
cache slot (`<cache-root>/<semver-core>/<target>`). If a writer is already
active, subsequent jobs wait, log what happened, then validate cached artifacts
after lock release.

If the lock wait succeeds but expected artifacts are still missing or invalid,
the build fails with an explicit error instead of silently re-fetching.

## CI prewarm then offline build

```bash
# Online prewarm (fills cache)
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  cargo build -p buf-tools

# Offline repeatable build using warmed cache
BUF_RS_CACHE_DIR="$PWD/target/buf-rs-cache" \
  CARGO_NET_OFFLINE=true \
  cargo build -p buf-tools
```
