# Docker integration (Debian + Alpine)

Two sibling images run the **same** [`entrypoint.sh`](entrypoint.sh):

| File | `RUST_DOCKER_TAG` flavor | `library/rust` label when channel is `stable` |
|------|--------------------------|-----------------------------------------------|
| [`Dockerfile`](Dockerfile) | `debian` | `slim-bookworm` |
| [`Dockerfile.alpine`](Dockerfile.alpine) | `alpine` | `alpine` |

Do not hardcode `FROM rust:…`. Both files take `ARG RUST_DOCKER_TAG` and
`FROM rust:${RUST_DOCKER_TAG}`. Alpine adds `gcompat` so official glibc
`buf-Linux-*` binaries can exec on musl.

Manual publish ([`.github/workflows/publish-crates.yml`](../../workflows/publish-crates.yml))
runs **`post-publish-integration`** after **`upload`**: it builds **both**
images from a **minimal context** (no workspace root `Cargo.toml`, no `path`
deps) and runs **`entrypoint.sh`** with **`TEST_CRATE_VERSION`** set to the
published crates.io semver. Dockerfiles must not `COPY path-src`: a
`dev` publish often uses the workflow file from `main` and checks out
the PR, so staging may omit that directory.

## Isolation (registry mode)

- Only **`cargo add buf-tools`** / **`cargo install buf-toolchain`** from the
  registry at **`TEST_CRATE_VERSION`**, plus sources copied from
  **`examples/`** (see staging below).
- Integration **`Cargo.toml`** is maintained next to these Dockerfiles —
  keep it aligned with **[`examples/Cargo.toml`](../../../examples/Cargo.toml)**
  (see **[`AGENTS.md`](../../../AGENTS.md)**).

## Shared checks

Both images, both install modes:

1. Install `buf-tools` + `buf-toolchain` (registry version **or** path-preinstall).
2. `validate-cargo-buf-toolchain --yaml`
3. `buf --version` vs crate semver core
4. `buf build` for the sample proto
5. `cargo run` both examples

## Staged build context

Same layout for CI, [`run-integration-docker.sh`](../../ci-scripts/run-integration-docker.sh),
and `cargo xtask image`:

| Artifact | Source |
|----------|--------|
| `rust-toolchain.toml` | Repo root |
| `Cargo.toml`, `Dockerfile`, `Dockerfile.alpine`, `entrypoint.sh` | This directory |
| `buf_lint.rs`, `protoc_with_buf_plugins.rs` | [`examples/`](../../../examples/) |
| `proto/**` | [`examples/proto/`](../../../examples/proto/) |
| `path-src/` | Optional. Registry builds ignore it. `cargo xtask image` bind-mounts the workspace members at build and at run. |

## Docker base image (`RUST_DOCKER_TAG`)

```bash
# Debian
RUST_DOCKER_TAG="$(bash .github/ci-scripts/rust-docker-tag-from-toolchain.sh rust-toolchain.toml debian)"
# Alpine
RUST_DOCKER_TAG="$(bash .github/ci-scripts/rust-docker-tag-from-toolchain.sh rust-toolchain.toml alpine)"
```

When `channel` is `X.Y.Z`, the script prints `X.Y-slim-bookworm` or
`X.Y-alpine` (Docker Hub has no patch tags on `library/rust`). The publish
workflow sets **`pull: true`**.

## Local checks (no GitHub Actions)

Path-preinstall this worktree (unreleased `--yaml` helper):

```bash
cargo xtask image all
# or: cargo xtask image debian
# or: cargo xtask image alpine
```

Registry mode (`TEST_CRATE_VERSION` must already exist on crates.io):

```bash
TEST_CRATE_VERSION="$(bash .github/ci-scripts/read-workspace-version.sh)" \
  bash .github/ci-scripts/run-integration-docker.sh
# DISTRO=debian|alpine|all (default all)
```

## Environment

| Variable | Meaning |
|----------|---------|
| **`TEST_CRATE_VERSION`** | Full published semver for registry `cargo add` / `cargo install`. |
| **`EXPECT_BUF_CORE`** | Buf `X.Y.Z` when path-preinstall is used (`cargo xtask image`). |
