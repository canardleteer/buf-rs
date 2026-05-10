# Agent / maintainer operations

Concise rules for coding agents. User-facing commands stay in [`README.md`](README.md).

## Two different “versions”

1. **Upstream Buf release** — GitHub tag `v1.69.0`, assets under `bufbuild/buf`
   releases. The `buf` binary’s `--version` reports this line (no crate
   pre-release suffix).
2. **Crate semver** — `[workspace.package].version` and `version = "=…"` pins in
   the root [`Cargo.toml`](Cargo.toml), and what you publish to crates.io.

For **canary** publishes, crate semver must **not** equal the final stable slot
(`1.69.0`) until you intentionally ship stable. CI and the manual workflow use
**`1.69.0-rc.<github.run_id>`** (unique per Actions run). Older examples such as
`1.69.0-rc.<n>.tools.testing` are equivalent in intent: never burn the stable
slot until you intentionally ship stable.

**Crate semver tracks the Buf binary, not an independent series.** The
**core** semver (before any `-` pre-release) **MUST** match the upstream Buf release
that `buf --version` corresponds to. Never invent a crates-only patch
such as `1.69.1` when Buf is still shipping `1.69.0`. If a bad publish consumes
a semver slot on crates.io, recover with **pre-release identifiers** so the
published version still reflects the same Buf release, not a fabricated crate
lineage.

## `buf-tools` packaging (download at build + crates.io cap)

- **Published `.crate`:** Rust sources, **`build.rs`**, and **`build_support/**`** only.
  **No** Buf executables ship in the tarball — they exceed crates.io’s ~10 MiB upload limit.
  Consumers’ **`build.rs`** downloads official release assets, verifies **minisign** +
  **`sha256.txt`**, and caches under **`BUF_RS_CACHE_DIR`** or **`~/.cache/buf-tools/`**
  (see [`buf-tools/README.md`](buf-tools/README.md)).

### Maintainer note — buf-tools configuration sources

When changing `buf-tools` configuration behavior, keep all related config sites,
call sites, and docs aligned:

1. **Environment variables** (highest precedence): `BUF_RS_LAYOUT_MODE`,
   `BUF_RS_BUILD_LOG`, `BUF_RS_CACHE_DIR`, `BUF_RS_RELEASE_BASE_URL`,
   `BUF_RS_SOURCE_BASE_URL`.
2. **Workspace metadata defaults**:
   `[workspace.metadata.buf-tools.config]` in the consumer `Cargo.toml`.
3. **Package metadata overrides**:
   `[package.metadata.buf-tools.config]` in the consumer `Cargo.toml`.
4. **Optional env injection** via `.cargo/config.toml` `[env]`.

Primary config resolution/consumption call sites:

- [`buf-tools/build.rs`](buf-tools/build.rs) (effective value resolution and usage).
- [`buf-tools/build_support/config.rs`](buf-tools/build_support/config.rs)
  (metadata parsing + precedence merge).

Documentation that must stay in sync when keys/preference/precedence change:

- [`buf-tools/README.md`](buf-tools/README.md)
- workspace [`README.md`](README.md)

## Publishing

**Publishable crates:** **`buf-tools`** and **`buf-toolchain`** (see root [`Cargo.toml`](Cargo.toml)
`[workspace.package]` and `[workspace.dependencies]` pins — keep them in sync when bumping Buf).

- **`cargo publish --dry-run`** does not consume a version on crates.io (safe rehearsal).
- **Stable `X.Y.Z` is irreversible:** a first successful publish of that version cannot be
  replaced with different crate contents; **yank** does not free the semver slot. Double-check
  Buf upstream alignment and run **`rust-tests`** (includes dry-run) before shipping stable.

### Manual publish workflow ([`.github/workflows/publish-crates.yml`](.github/workflows/publish-crates.yml))

- **Trigger:** `workflow_dispatch` only (no automatic upload).
- **Jobs:** **`verify`** runs packaging **`cargo publish --dry-run`** for both crates; **`upload`**
  uses environment **`crates-io-publish`** and runs after **`verify`** only if **`CRATES_IO_TOKEN`**
  is set. While the repo stays **private**, leave that secret unset so **`upload`** never runs; after
  the repo is **public**, add the secret and optionally **Required reviewers** on **`crates-io-publish`**
  (Free/Pro/Team: [public repos only](https://docs.github.com/en/actions/reference/deployments-and-environments#required-reviewers))
  for a pause before **`upload`**.
- **Pre-release (default):** bumps workspace version on the runner to **`{core}-rc.${{ github.run_id }}`**,
  runs **`cargo generate-lockfile`**, then **`cargo publish --allow-dirty --locked`** (ephemeral
  `Cargo.toml` / `Cargo.lock` changes are **not** committed to git).
- **Stable:** requires **`channel=stable`**, dispatch from **`main`**, **`inputs.ref`** set to **`main`** (guards
  against dispatching from **`main`** while checking out another ref), and **`confirm_stable_version`**
  matching **`[workspace.package].version`**; no `--allow-dirty`; no in-runner version bump. Use **prerelease**
  to exercise non-**`main`** refs.
- **Upload skipped (green run):** if **`CRATES_IO_TOKEN`** is unset, **`upload`** is skipped — safe for
  private repos and dry testing. Remove the **`upload`** job’s **`if:`** gate (see TEMP comment in the
  YAML) once the token is always configured.
- **Publish version bumps:** [`xtask`](xtask/) — **`cargo xtask publish resolve`**, **`apply-prerelease`**,
  **`workspace-core`** (alias in [`.cargo/config.toml`](.cargo/config.toml); uses **`--locked`**).

### GitHub settings (before upload works)

1. **Secret:** Settings → Secrets and variables → Actions → **`CRATES_IO_TOKEN`** (crates.io token with
   publish rights for **`buf-tools`** and **`buf-toolchain`**). Omit until the repo is **public** if you
   want **`upload`** disabled while private.
2. **Environment:** Settings → Environments → **`crates-io-publish`** → **Configure environment** →
   under **Deployment protection rules**, add **Required reviewers** if you want an approval gate before
   **`upload`** ([public only on Free/Pro/Team](https://docs.github.com/en/actions/reference/deployments-and-environments#required-reviewers)).
   Optional: **Deployment branches / tags** (e.g. **`main`** only — [Pro/Team private](https://docs.github.com/en/actions/reference/deployments-and-environments#deployment-branches-and-tags)).
   With no reviewer rule, **`upload`** runs immediately after **`verify`** once the token is set.
3. **Actions:** enabled; workflow default read-only permissions.

Local one-off: **`cargo publish -p buf-tools`** / **`buf-toolchain`** from a clean tree after
**`cargo publish -p … --dry-run`**.

## CI (GitHub Actions)

- **Tests:** [`.github/workflows/rust-tests.yml`](.github/workflows/rust-tests.yml) — on **`push`** and
  **`pull_request`** to **`main`**, matrix (linux amd64/arm64, macos arm64, windows amd64). Runs
  **`cargo fmt --check`**, **`cargo clippy`**, **`cargo test`**, both **`buf-tools-examples`** examples
  via [`.github/ci-scripts/run-examples.sh`](.github/ci-scripts/run-examples.sh), then
  **`cargo publish -p buf-tools --dry-run --locked`** and **`buf-toolchain`** (no token; packaging gate).
- **Publish:** [`.github/workflows/publish-crates.yml`](.github/workflows/publish-crates.yml) — manual only
  (see **Publishing** above).

## Linting

Before merging risky changes:

- **`cargo fmt --all -- --check`** — formatting gate (use **`cargo fmt --all`**
  to apply).
- **`cargo clippy --workspace --locked --all-targets`** — static analysis gate
  (narrow with `-p` when iterating on one crate).
- **`buf-toolchain` + `buf-tools/build_support`** — **`buf-toolchain/build_support`**
  is a **symlink** to **[`buf-tools/build_support`](buf-tools/build_support)** so
  **`cargo package -p buf-toolchain`** packs the shared **`*.rs`** sources.
  **`build.rs`** and **`src/lib.rs`** include them via **`#[path]`** under
  **`build_support/`**. Changes to verify, lock, `write_executable`, `targets`,
  or `fetch` in **`buf-tools/build_support/`** affect **both** crates; run
  workspace tests when touching that tree.
- **`buf-toolchain` layout contract** — workspace **`cargo test`** does not
  prove nested-install behavior in isolation. Run:
  **`cargo test -p buf-toolchain --locked --test managed_bin_layout -- --ignored`**
  when changing installer logic (requires network unless the nested build’s temp
  cache is warm).
- **`buf-toolchain` `[[bin]]`** — Cargo only **`cargo install`**s crates that
  expose a binary (or installable example). The installed binary is
  **`validate-cargo-buf-toolchain`** (package name remains **`buf-toolchain`**);
  it re-verifies **`sha256.txt`** / **minisign** against GitHub for the
  installed Buf core, optionally compares **`releases/latest`**, and probes
  crates.io for **`buf-toolchain`** when an upgrade exists
  (**`BUF_RS_VALIDATE_OFFLINE=1`** skips network). **`buf`** /
  **`protoc-gen-buf-*`** are installed by **`build.rs`**, not by that helper.

## Examples

Runnable examples live in [`examples/`](examples/) as the **`buf-tools-examples`** package.
Run with **`cargo run -p buf-tools-examples --example <name>`**.

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
[`build.rs`](buf-tools/build.rs) sets `allow_legacy = core_ver < PREHASHED_MINISIGN_MIN_VERSION`
(`"1.12.0"`) so the legacy gate is opened **only** for releases that need it; v1.12.0+
keeps the strict path that `minisign-verify` defaults to.

### Per-target minimum Buf version

- `Linux-x86_64`, `Linux-aarch64`, `Darwin-x86_64`, `Darwin-arm64`, `Windows-x86_64`, `Windows-arm64` — v1.0.0
- `Linux-armv7` — v1.47.0
- `Linux-ppc64le`, `Linux-riscv64` — v1.54.0
- `Linux-s390x` — v1.56.0
- `FreeBSD-x86_64`, `FreeBSD-arm64`, `OpenBSD-x86_64`, `OpenBSD-arm64` — v1.67.0

Encoded as `min_version` on each [`ReleaseTarget`](buf-tools/build_support/targets.rs);
[`build.rs`](buf-tools/build.rs) fast-fails before any HTTP if the crate's pinned
Buf version predates the target's floor. The same table is mirrored in
`[package.metadata.buf-tools.targets]` of [`buf-tools/Cargo.toml`](buf-tools/Cargo.toml)
for `cargo metadata` / crates.io discovery, and a `#[test]` enforces the two
stay in sync.

### MAINTAINER NOTE — when adding/removing a target or changing a floor

The per-target floor table lives in **three** places by design (Rust drives
behavior; manifest metadata and README mirror it for tooling and humans).
**Update all three** in the same change:

1. `pub const ALL` and `from_rust_triple` in [`buf-tools/build_support/targets.rs`](buf-tools/build_support/targets.rs)
   (Rust source of truth — what `build.rs` actually checks).
2. `[package.metadata.buf-tools.targets.<asset_suffix>]` in [`buf-tools/Cargo.toml`](buf-tools/Cargo.toml)
   (rendered on crates.io; readable via `cargo metadata`).
3. The "Supported targets" matrix in top-level [`README.md`](README.md)
   (front-of-listing visibility for `cargo add` users).

The `cargo_metadata_matches_rust_const` `#[test]` catches drift between (1) and
(2), but **NOT** README drift — keep (3) in sync by hand. If you bump a `min_version`,
also confirm whether [`PREHASHED_MINISIGN_MIN_VERSION`](buf-tools/build_support/verify.rs)
still describes the upstream signing-algorithm boundary; if Buf flips again,
update that constant and add fixtures + tests for the new mode.

When the **crate's own pinned Buf version** moves (e.g. `1.69.0` → `1.70.0`):

- Update `[workspace.package].version` in the root [`Cargo.toml`](Cargo.toml) (and the `=` pin in `[workspace.dependencies]`).
- Update README/AGENTS prose that names the version explicitly.
- Run `cargo generate-lockfile` so [`Cargo.lock`](Cargo.lock) reflects the bump.
- Confirm `BUF_EXPECT_VERSION=<new> cargo test --workspace --locked` is still green.
