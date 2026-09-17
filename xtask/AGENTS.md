# xtask instructions for agents

These instructions govern the standard development commands and the
repository-specific publish and Buf-pin tasks. The crate is a root workspace
member. [`.cargo/config.toml`](../.cargo/config.toml) maps `cargo xtask` to
`cargo run --locked --package xtask --`.

## Command policy

- `check` registers `fmt`, `check`, `clippy`, and `test` in that order. It is
  fail-fast, runs all four by default, and supports mutually exclusive
  `--only` and `--exclude`. Feature-aware commands default to
  `--all-features` when you pass no feature option. `buf-toolchain`
  defines `validate-cli` for the helper binary. `fmt` does not receive
  feature flags.
  `check` and `clippy` use `--locked --workspace --all-targets`. `test` uses
  `--locked --workspace` without `--all-targets`, matching
  [`.github/workflows/rust-tests.yml`](../.github/workflows/rust-tests.yml).
  Clippy does not pass `-D warnings`. `ci` is a visible alias over exactly
  the same implementation.
- When `BUF_EXPECT_VERSION` is unset, `check` (test step) and `coverage`
  inject the workspace Buf core into the child Cargo command. When the
  caller already set it, the child inherits that value.
- Keep CI, workflow, and script callers aligned with `check` when its
  registered steps or flags change.
  [`.github/workflows/rust-tests.yml`](../.github/workflows/rust-tests.yml)
  runs `cargo xtask check`, then
  [`.github/ci-scripts/run-examples.sh`](../.github/ci-scripts/run-examples.sh)
  for examples only. The xtask must remain provider-agnostic and must not
  inspect those declarations itself.
- `coverage` supports `llvm-cov` (default) and `tarpaulin`. Reports are
  `target/coverage/llvm-cov/html/index.html` and
  `target/coverage/tarpaulin/tarpaulin-report.html`. Both `coverage --open`
  and `coverage-open` generate a fresh report first.
- `image` builds the sibling Debian and Alpine integration images
  (`debian`, `alpine`, `all`) with `auto|docker|buildah`. It bind-mounts
  this worktree at run and path-installs `buf-toolchain` from that mount,
  then runs the shared
  [`entrypoint.sh`](../.github/ci/integration/entrypoint.sh) **with
  network**. Do not pass `--network none`. Keep `image` off `check` /
  `ci`. Default tags: `buf-rs-integration:debian` and
  `buf-rs-integration:alpine`. Never push.
- Repository-specific commands stay on the same CLI: `expected-buf-version`,
  `publish resolve|apply-version|verify-summary`, and
  `workspace set-buf-version`. Keep those aligned with
  [`.github/workflows/publish-crates.yml`](../.github/workflows/publish-crates.yml)
  and both `build.rs` files.

## Omitted standard handles

- `profile` and `profile-open` are omitted. There is no representative
  no-network workload: published crates download and verify Buf assets,
  and examples need a generated Buf image.
- `mcp-test` is omitted. This repository does not expose a stdio MCP
  server. Add that loop only when the user asks for it.

## Retained orchestration

- Prefer `cargo xtask` over new Python scripts for typed, cross-platform,
  Cargo-aware development orchestration.
- Retain
  [`.github/ci-scripts/bufsemver_upstream_is_newer.py`](../.github/ci-scripts/bufsemver_upstream_is_newer.py)
  for the upstream-watch version compare. Do not rewrite it without user
  approval.
- Retain the CI shell scripts under [`.github/ci-scripts/`](../.github/ci-scripts/)
  for example runs, Buf path lookup, Docker staging, and release-tag
  helpers. [`run-integration-docker.sh`](../.github/ci-scripts/run-integration-docker.sh)
  stays the registry-mode local/CI twin (`DISTRO=debian|alpine|all`). Propose
  a migration before replacing callers.

## Tool guidance

Probe only tools selected by the command. A failed launch is an unusable
tool, not an absent one. Recommend these exact Cargo installs when
applicable:

```text
cargo install --locked cargo-llvm-cov
cargo install --locked cargo-tarpaulin
```

Use `rustup component add rustfmt` or `rustup component add clippy` for
missing Rust components. Do not guess a platform package-manager command.

## Implementation and validation

Represent subprocesses as a program plus OS argument vector. Keep
`main.rs` thin, use Clap derive types, and retain parser, selection,
feature, alias, and failure-path tests. Commands are synchronous. Do not
add `tokio` or `tracing` as ceremony. `env_logger` stays at the binary
boundary for publish-channel warnings.

When you change publish or version-summary behavior, update
`publish.rs` / `publish_inputs.rs`, the publish workflow header comments,
and both `build.rs` files together if the Buf-resolution rule changes.

Run from the repository root:

```text
cargo xtask check --only fmt,check
cargo xtask expected-buf-version
```

`cargo xtask check` is the default local quality loop. Coverage depends
on a selected local engine and should be exercised when its
implementation changes.
