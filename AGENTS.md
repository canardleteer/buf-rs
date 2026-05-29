# Agent / maintainer operations

Concise rules for coding agents. User-facing commands stay in [`README.md`](README.md).

## Two different “versions”

**Authoritative Buf pin:** `[workspace.package].version` and the `version = "=…"` pins on
**`buf-tools`** / **`buf-toolchain`** in the root [`Cargo.toml`](Cargo.toml). That plain
**`X.Y.Z`** core selects the upstream GitHub tag **`vX.Y.Z`**. Any concrete version called out
elsewhere in this file or in [`README.md`](README.md) is an **example** unless it matches
that manifest — always trust the workspace `Cargo.toml` when they disagree.

**Set the Buf pin locally (outside CI):** after confirming
[`bufbuild/buf`](https://github.com/bufbuild/buf/releases) has tag **`vX.Y.Z`**:

1. **`cargo xtask workspace set-buf-version X.Y.Z`**
2. **`cargo generate-lockfile`**
3. **`BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"`**
4. **`echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"`**
5. **`cargo test --workspace --locked`**

Read the current core with **`cargo xtask expected-buf-version`** (from **`[workspace.package].version`**
in the root **`Cargo.toml`** — same **`X.Y.Z`** as tests expect via **`BUF_EXPECT_VERSION`**).

Those fields track two related notions:

- **Upstream Buf release** — GitHub tag **`vX.Y.Z`** (illustrative example: **`v1.40.0`**),
  assets under **`bufbuild/buf`** releases. The **`buf`** binary’s **`--version`** reports
  this line (no crate pre-release suffix).
- **Crate semver** — the workspace manifest fields above, and what you publish to crates.io.

For **canary** publishes, crate semver must **not** equal the final stable slot
(e.g. **`1.40.0`**) until you intentionally ship stable. The manual publish workflow defaults to
**`dev`**: **`{core}-dev.<github.run_id>`** (pipeline validation). Use **`rc`** with
**`rc_number`** for **`{core}-rc.N`** when testing with others. Never burn the stable
slot until you intentionally ship **`stable`**.

**Crate semver tracks the Buf binary, not an independent series.** The
**core** semver (before any `-` pre-release) **MUST** match the upstream Buf release
that `buf --version` corresponds to. Never invent a crates-only patch
such as **`1.40.1`** when Buf is still shipping **`1.40.0`**. If a bad publish consumes
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
`[workspace.package]` and `[workspace.dependencies]` pins — keep them in sync when changing the pinned Buf version).

- **`cargo publish --dry-run`** does not consume a version on crates.io (safe rehearsal).
- **Stable `X.Y.Z` is irreversible:** a first successful publish of that version cannot be
  replaced with different crate contents; **yank** does not free the semver slot. Double-check
  Buf upstream alignment and run **`rust-tests`** (includes dry-run) before shipping stable.

### Manual publish workflow ([`.github/workflows/publish-crates.yml`](.github/workflows/publish-crates.yml))

- **Trigger:** `workflow_dispatch` only (no automatic upload).
- **Jobs:** **`verify`** runs packaging **`cargo publish --dry-run`** for both crates; **`upload`**
  runs **`cargo publish`** for both crates. Both jobs use environment **`crates-io-publish`** so
  **environment-scoped** **`CRATES_IO_TOKEN`** is visible (GitHub does not expose environment secrets
  to jobs that omit **`environment:`**). **`upload`** runs after **`verify`** only if **`verify`**’s
  token gate sees **`CRATES_IO_TOKEN`** (repository secret or environment secret). While the repo
  stays **private**, leave that secret unset so **`upload`** never runs; after the repo is **public**,
  add the secret and optionally **Required reviewers** on **`crates-io-publish`**
  (Free/Pro/Team: [public repos only](https://docs.github.com/en/actions/reference/deployments-and-environments#required-reviewers)).
  **Note:** **Required reviewers** (if enabled) apply to **every** job that references the environment, so
  **`verify`** may wait for approval before dry-run and **`upload`** may wait again — tune environment rules
  or use a **repository**-level token if you want lighter gating.
- **Channels:** **`dev`** (default) → **`{core}-dev.<run_id>`**; **`rc`** → **`{core}-rc.<rc_number>`**
  (**`rc_number`** input, integer > 0); **`stable`** → committed **`X.Y.Z`** only. **`dev`** / **`rc`**
  run **`cargo xtask publish apply-version`**, **`cargo generate-lockfile`**, then
  **`cargo publish --allow-dirty --locked`** (ephemeral `Cargo.toml` / `Cargo.lock`, not committed).
- **Stable:** requires **`channel=stable`**, dispatch from **`main`**, **`inputs.ref`** set to **`main`**, and
  **`confirm_stable_version`** matching **`[workspace.package].version`**; no **`--allow-dirty`**; no
  in-runner ephemeral manifest version (dev/rc). Use **`dev`** or **`rc`** to exercise non-**`main`** refs.
- **Crate pre-release vs Buf binary:** Crate versions may include **`-dev.*`** or **`-rc.*`** for buf-rs
  packaging only. **`build.rs`** still downloads the **stable** Buf GitHub release **`v{major}.{minor}.{patch}`**
  from **`CARGO_PKG_VERSION`** (see **`buf-tools/build.rs`** / **`buf-toolchain/build.rs`**). The verify job
  summary prints **resolved buf for buf-tools** and **resolved buf for buf-toolchain** via
  **`cargo xtask publish verify-summary`** (uses the **`semver`** crate); that output must stay aligned
  with both **`build.rs`** files.
- **Maintainer priority:** When changing publish/versioning or the verify summary, update **`xtask`**
  (`verify-summary`, **`resolve`**, **`apply-version`**), the **workflow YAML header comments** (source-of-truth
  block), and—if the rule changes—**`buf-tools/build.rs`** and **`buf-toolchain/build.rs`** together.
- **Post-publish integration:** After **`upload`** succeeds, job **`post-publish-integration`** builds **`.github/ci/integration/`**’s Docker image (staged context: repo **`rust-toolchain.toml`**, integration **`Cargo.toml`** / **`Dockerfile`** / **`entrypoint.sh`**, mirrored **`examples/`** sources — **no** workspace root) and runs **`cargo add buf-tools`**, **`cargo install buf-toolchain`**, **`buf --version`** vs crate semver core, **`buf build`** baseline, then both examples. Failure **fails the workflow**. **`verify`** exposes **`publish_version`** for **`TEST_CRATE_VERSION`**. Skipped when **`upload`** is skipped (no token).
- **Artifacts:** Each job uploads **`Cargo.toml`**, **`buf-tools/Cargo.toml`**, and **`buf-toolchain/Cargo.toml`**
  for debugging (requires **`permissions.actions: write`** on the workflow).
- **Upload skipped (green run):** if **`verify`** does not see **`CRATES_IO_TOKEN`**, **`upload`** is
  skipped — safe for private repos and dry testing. If the token lives only under **environment**
  **`crates-io-publish`**, both **`verify`** and **`upload`** must declare that **`environment`** (the
  workflow does). Remove the **`upload`** job’s **`if:`** gate (see TEMP comment in the YAML) once you
  always want **`upload`** to run.
- **Publish helpers:** [`xtask`](xtask/) — **`cargo xtask publish resolve`**, **`apply-version`**,
  **`verify-summary`**, **`cargo xtask expected-buf-version`**, and **`cargo xtask workspace set-buf-version`**
  (alias in [`.cargo/config.toml`](.cargo/config.toml); uses **`--locked`**). See **Pinning the workspace Buf release (`set-buf-version`)** below.

### Pinning the workspace Buf release (`set-buf-version`)

**`cargo xtask workspace set-buf-version X.Y.Z`** updates the **root** [`Cargo.toml`](Cargo.toml) in one step:
**`[workspace.package].version`** and the **`version = "=X.Y.Z"`** entries for **`buf-tools`** and
**`buf-toolchain`** under **`[workspace.dependencies]`**. Use a plain **`X.Y.Z`** (no pre-release) that
matches an existing **`bufbuild/buf`** tag **`vX.Y.Z`**.

This is for **maintainers changing which upstream Buf release the workspace pins** (any direction —
older or newer). It is **not** the same as **`publish apply-version`**, which only rewrites the manifest
on CI runners for **`dev`** / **`rc`** channels using **`-dev.*`** / **`-rc.*`** crate suffixes.

After running it: **`cargo generate-lockfile`**, then set **`BUF_EXPECT_VERSION`** from
**`cargo xtask expected-buf-version`** and run **`cargo test --workspace --locked`** (see the numbered
list under **Two different “versions”**).

### GitHub settings (before upload works)

1. **Secret `CRATES_IO_TOKEN`:** either **repository** (Settings → Secrets and variables → Actions) or
   **environment** **`crates-io-publish`** (Environment secrets). The publish workflow assigns **`verify`**
   and **`upload`** both to **`environment: crates-io-publish`** so environment-scoped tokens are visible
   to the verify-step gate and to **`CARGO_REGISTRY_TOKEN`**. Omit the secret until the repo is **public**
   if you want **`upload`** disabled while private.
2. **Environment `crates-io-publish`:** Settings → Environments → **`crates-io-publish`** → **Configure environment** →
   optional **Required reviewers** or deployment-branch rules
   ([public-only reviewers on Free/Pro/Team](https://docs.github.com/en/actions/reference/deployments-and-environments#required-reviewers)).
   **`verify`** and **`upload`** both reference this environment, so each job may trigger the same protection
   rules (e.g. two review rounds if reviewers are required).
   Optional: **Deployment branches / tags** (e.g. **`main`** only — [Pro/Team private](https://docs.github.com/en/actions/reference/deployments-and-environments#deployment-branches-and-tags)).
3. **Actions:** enabled. **Workflow-specific** token needs and GitHub UI settings (**Workflow
   permissions**, secrets, environments) are documented in the **comment headers** at the top of
   each workflow under [`.github/workflows/`](.github/workflows/) — see especially
   **`publish-crates.yml`** and **`buf-upstream-watch.yml`**.

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
- **Buf upstream watch:** [`.github/workflows/buf-upstream-watch.yml`](.github/workflows/buf-upstream-watch.yml) —
  scheduled / manual / **`repository_dispatch`** bump PRs when [bufbuild/buf](https://github.com/bufbuild/buf)
  **`releases/latest`** is newer than **`cargo xtask expected-buf-version`**. **Settings, `curl` example,
  dev publish from the bump branch,** and branch naming (**`automated/buf/X.Y.Z`**) are documented in
  that file’s **header comments** (and in the generated PR body).

## `rust-toolchain.toml` (pinned Rust toolchain)

The repo root **[`rust-toolchain.toml`](rust-toolchain.toml)** sets **`channel`** to an **explicit stable release** (e.g. **`1.96.0`**) — not the bare **`stable`** channel string — so resolution is reproducible. **Today’s pin is `1.96.0` because that is the current latest stable** on **[releases.rs](https://releases.rs)**; as new stables ship, maintainers should **bump `channel` (and aligned bits below) to track latest stable**, unless the project **deliberately** stays on an older compiler (document that choice).

**Whenever you edit [`rust-toolchain.toml`](rust-toolchain.toml)** — bump, tweak **`components`**, or otherwise touch the file — **check [releases.rs](https://releases.rs)** so you know whether you are staying on **latest stable**, intentionally behind, or intentionally pinning a specific release for another reason.

**`components`** (e.g. **`rustfmt`**, **`clippy`**) live in that file too. GitHub Actions uses **`dtolnay/rust-toolchain@stable`** **without** duplicate **`toolchain:`** / **`components:`** inputs; rustup applies the workspace **`rust-toolchain.toml`** when **`cargo`** / **`rustc`** run in the repo.

Keep these aligned on the **same Rust release line** when you bump the pin:

- **`rust-toolchain.toml`** **`channel`** (and **`components`**)
- **Integration Docker base:** **`Dockerfile`** **`ARG RUST_DOCKER_TAG`** → **`FROM rust:${RUST_DOCKER_TAG}`**. CI and **[`.github/ci-scripts/run-integration-docker.sh`](.github/ci-scripts/run-integration-docker.sh)** pass **`RUST_DOCKER_TAG="$(bash .github/ci-scripts/rust-docker-tag-from-toolchain.sh rust-toolchain.toml)"`** (maps **`channel = "X.Y.Z"`** → **`X.Y-slim-bookworm`**; Docker Hub has no **`X.Y.Z`** patch tags on **`library/rust`**). Bumping **`[toolchain].channel`** updates the build-arg automatically; GHA layer cache invalidates when **`rust-toolchain.toml`** / **`Dockerfile`** / context change.

## Post-publish Docker integration (`.github/ci/integration/`)

Registry-only smoke test after a successful **`upload`** (see **Publishing** → **Post-publish integration**). **Whenever you edit [`examples/Cargo.toml`](examples/Cargo.toml)** (deps, **`[[example]]`**, paths, **`edition.workspace`** / metadata), update the integration scaffold **[`.github/ci/integration/Cargo.toml`](.github/ci/integration/Cargo.toml)** next to the **`Dockerfile`**, and adjust **[`.github/ci/integration/README.md`](.github/ci/integration/README.md)** / **`COPY`** lists if paths change. Local smoke: **[`.github/ci-scripts/run-integration-docker.sh`](.github/ci-scripts/run-integration-docker.sh)** (same staging as CI).

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
(2), but **NOT** README drift — keep (3) in sync by hand. If you raise a target's `min_version`,
also confirm whether [`PREHASHED_MINISIGN_MIN_VERSION`](buf-tools/build_support/verify.rs)
still describes the upstream signing-algorithm boundary; if Buf flips again,
update that constant and add fixtures + tests for the new mode.

When the **crate's own pinned Buf version** moves (e.g. **`1.40.0` → `1.41.0`**):

- Prefer **`cargo xtask workspace set-buf-version X.Y.Z`** (updates **`[workspace.package].version`**
  and **`[workspace.dependencies]`** `=` pins together).
- Run **`cargo generate-lockfile`** so [`Cargo.lock`](Cargo.lock) reflects the new version.
- Confirm tests with **`BUF_EXPECT_VERSION="$(cargo xtask expected-buf-version)"`** (and
  **`echo "Expected Buf Version: ${BUF_EXPECT_VERSION}"`** if you want a clear log line) are still green.
- Optionally refresh **example-only** version mentions in [`README.md`](README.md) / this file so
  they stay helpful — they are **not** the source of truth.
