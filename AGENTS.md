# Agent / maintainer operations

Concise rules for coding agents. User-facing commands stay in [`README.md`](README.md).

## Two different “versions”

1. **Upstream Buf release** — GitHub tag `v1.69.0`, assets under `bufbuild/buf`
   releases. The `buf` binary’s `--version` reports this line (no crate
   pre-release suffix).
2. **Crate semver** — `[workspace.package].version` and `version = "=…"` pins in
   the root [`Cargo.toml`](Cargo.toml), and what you publish to crates.io.

For **canary** publishes, crate semver must **not** equal the final stable slot
(`1.69.0`) until you intentionally ship stable. Use a **pre-release** such as
`1.69.0-rc.<n>.sys.testing` so a failed publish never burns the stable
version Buf “owns” semantically.

**Crate semver tracks the Buf binary, not an independent series.** The
**core** semver (before any `-` pre-release) **MUST** match the upstream Buf release
that `buf --version` corresponds to. Never invent a crates-only patch
such as `1.69.1` when Buf is still shipping `1.69.0`. If a bad publish consumes
a semver slot on crates.io, recover with **pre-release identifiers** so the
published version still reflects the same Buf release, not a fabricated crate
lineage.

## `buf-sys` packaging (download at build + crates.io cap)

- **Published `.crate`:** Rust sources, **`build.rs`**, and **`build_support/**`** only.
  **No** Buf executables ship in the tarball — they exceed crates.io’s ~10 MiB upload limit.
  Consumers’ **`build.rs`** downloads official release assets, verifies **minisign** +
  **`sha256.txt`**, and caches under **`BUF_SYS_CACHE_DIR`** or **`~/.cache/buf-sys/`**
  (see [`buf-sys/README.md`](buf-sys/README.md)).

## Publishing

The workspace sets **`publish = true`** for the publishable crate. To ship **`buf-sys`**
to crates.io from a clean tree: **`cargo publish -p buf-sys`** (after **`cargo publish --dry-run`**
if you want a no-upload rehearsal). **`cargo publish --dry-run`** does not consume a
version on crates.io.

## Linting

Before merging risky changes:

- **`cargo fmt --all -- --check`** — formatting gate (use **`cargo fmt --all`** to apply).
- **`cargo clippy --workspace --locked --all-targets`** — static analysis gate (narrow
  with `-p` when iterating on one crate).

## Examples

Runnable examples live in [`examples/`](examples/) as the **`buf-sys-examples`** package.
Run with **`cargo run -p buf-sys-examples --example <name>`**.

For **`protoc_with_buf_plugins`**, generate the gitignored Buf image under
[`examples/proto/`](examples/proto/) first (see [`README.md`](README.md)).

## Buf release asset pattern (v1.0.0+)

All `bufbuild/buf` release assets for the v1.x line follow:

    <bin>-<Os>-<Arch>[.exe]

- `<bin>` ∈ `buf` | `protoc-gen-buf-lint` | `protoc-gen-buf-breaking`
- `<Os>`  ∈ `Linux` | `Darwin` | `Windows` | `FreeBSD` | `OpenBSD`
- `<Arch>` ∈ `x86_64` | `aarch64` | `arm64` | `armv7` | `ppc64le` | `riscv64` | `s390x`
- `.exe` only when `<Os>` = `Windows`. Optional `.tar.gz` / `.zip` wrappers are ignored.
- `Darwin` uses `arm64`; `Linux` uses `aarch64`; `Windows` uses both (`x86_64`, `arm64`).

The minisign signing key (`BUF_MINISIGN_PUBLIC_KEY_B64`) is **unchanged** since
v1.0.0 (key id `3f8bdc6c799c0154`). The in-signature algorithm flag flipped at
**v1.12.0**: v1.0.0–v1.11.0 use raw Ed25519 (`Ed`/RWQ); v1.12.0+ use
Ed25519+BLAKE2b-512 prehash (`ED`/RUQ). Both verify against the same public key.
[`build.rs`](buf-sys/build.rs) sets `allow_legacy = core_ver < PREHASHED_MINISIGN_MIN_VERSION`
(`"1.12.0"`) so the legacy gate is opened **only** for releases that need it; v1.12.0+
keeps the strict path that `minisign-verify` defaults to.

### Per-target minimum Buf version

- `Linux-x86_64`, `Linux-aarch64`, `Darwin-x86_64`, `Darwin-arm64`, `Windows-x86_64`, `Windows-arm64` — v1.0.0
- `Linux-armv7` — v1.47.0
- `Linux-ppc64le`, `Linux-riscv64` — v1.54.0
- `Linux-s390x` — v1.56.0
- `FreeBSD-x86_64`, `FreeBSD-arm64`, `OpenBSD-x86_64`, `OpenBSD-arm64` — v1.67.0

Encoded as `min_version` on each [`ReleaseTarget`](buf-sys/build_support/targets.rs);
[`build.rs`](buf-sys/build.rs) fast-fails before any HTTP if the crate's pinned
Buf version predates the target's floor. The same table is mirrored in
`[package.metadata.buf-sys.targets]` of [`buf-sys/Cargo.toml`](buf-sys/Cargo.toml)
for `cargo metadata` / crates.io discovery, and a `#[test]` enforces the two
stay in sync.

### MAINTAINER NOTE — when adding/removing a target or changing a floor

The per-target floor table lives in **three** places by design (Rust drives
behavior; manifest metadata and README mirror it for tooling and humans).
**Update all three** in the same change:

1. `pub const ALL` and `from_rust_triple` in [`buf-sys/build_support/targets.rs`](buf-sys/build_support/targets.rs)
   (Rust source of truth — what `build.rs` actually checks).
2. `[package.metadata.buf-sys.targets.<asset_suffix>]` in [`buf-sys/Cargo.toml`](buf-sys/Cargo.toml)
   (rendered on crates.io; readable via `cargo metadata`).
3. The "Supported targets" matrix in top-level [`README.md`](README.md)
   (front-of-listing visibility for `cargo add` users).

The `cargo_metadata_matches_rust_const` `#[test]` catches drift between (1) and
(2), but **NOT** README drift — keep (3) in sync by hand. If you bump a `min_version`,
also confirm whether [`PREHASHED_MINISIGN_MIN_VERSION`](buf-sys/build_support/verify.rs)
still describes the upstream signing-algorithm boundary; if Buf flips again,
update that constant and add fixtures + tests for the new mode.

When the **crate's own pinned Buf version** moves (e.g. `1.69.0` → `1.70.0`):

- Update `[workspace.package].version` in the root [`Cargo.toml`](Cargo.toml) (and the `=` pin in `[workspace.dependencies]`).
- Update README/AGENTS prose that names the version explicitly.
- Run `cargo generate-lockfile` so [`Cargo.lock`](Cargo.lock) reflects the bump.
- Confirm `BUF_EXPECT_VERSION=<new> cargo test --workspace --locked` is still green.
