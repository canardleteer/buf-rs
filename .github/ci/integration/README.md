# Post-publish Docker integration

Manual publish ([`.github/workflows/publish-crates.yml`](../../workflows/publish-crates.yml)) runs a **`post-publish-integration`** job after **`upload`** succeeds: it builds this image from a **minimal context** (no workspace root `Cargo.toml`, no `path` deps) and runs **`entrypoint.sh`** with **`TEST_CRATE_VERSION`** set to the published crates.io semver.

## Isolation

- Only **`cargo add buf-tools`** / **`cargo install buf-toolchain`** from the registry at **`TEST_CRATE_VERSION`**, plus sources copied from **`examples/`** (see staging below).
- Integration **`Cargo.toml`** is maintained next to this **`Dockerfile`** — keep it aligned with **[`examples/Cargo.toml`](../../../examples/Cargo.toml)** (see **[`AGENTS.md`](../../../AGENTS.md)**).

## Staged build context (same for CI and [`run-integration-docker.sh`](../../ci-scripts/run-integration-docker.sh))

| Artifact | Source |
|----------|--------|
| `rust-toolchain.toml` | Repo root |
| `Cargo.toml`, `Dockerfile`, `entrypoint.sh` | This directory |
| `buf_lint.rs`, `protoc_with_buf_plugins.rs` | [`examples/`](../../../examples/) |
| `proto/**` | [`examples/proto/`](../../../examples/proto/) |

## Docker base image (`RUST_DOCKER_TAG`)

**`Dockerfile`** uses **`ARG RUST_DOCKER_TAG`** then **`FROM rust:${RUST_DOCKER_TAG}`** (no hardcoded Rust line). Before **`docker build`**, compute the tag from repo-root **`rust-toolchain.toml`**:

```bash
RUST_DOCKER_TAG="$(bash .github/ci-scripts/rust-docker-tag-from-toolchain.sh rust-toolchain.toml)"
docker build --build-arg "RUST_DOCKER_TAG=${RUST_DOCKER_TAG}" …
```

**[`run-integration-docker.sh`](../../ci-scripts/run-integration-docker.sh)** and **`post-publish-integration`** in **[`publish-crates.yml`](../../workflows/publish-crates.yml)** do this for you. Inside the image, **`rust-toolchain.toml`** still selects the exact **`channel`** (patch + components); the base image only needs the matching **major.minor** line.

## Caching

- **GitHub Actions:** [`docker/build-push-action`](https://github.com/docker/build-push-action) with **`cache-from` / `cache-to: type=gha`**. Cache inputs include **`rust-toolchain.toml`**, this tree, mirrored **`examples/`** sources, and the resolved **`RUST_DOCKER_TAG`** build-arg — **not** **`TEST_CRATE_VERSION`** (that is only passed at **`docker run`**).
- **Dockerfile:** **`cargo fetch`** warms the Cargo cache for **`protoc-bin-vendored`** before **`buf-tools`** is added at runtime.

## Environment

| Variable | Meaning |
|----------|---------|
| **`TEST_CRATE_VERSION`** | Full published semver for **`buf-tools`** / **`buf-toolchain`** (same string CI passes from **`verify`** **`publish_version`**). |

## Local smoke

From the repo root (requires Docker or Podman). **`TEST_CRATE_VERSION` must already exist on crates.io** (same semver CI passes after **`upload`**). The workspace version from **`read-workspace-version.sh`** only works once that release has been published.

```bash
TEST_CRATE_VERSION="$(bash .github/ci-scripts/read-workspace-version.sh)" bash .github/ci-scripts/run-integration-docker.sh
```

Or pass an explicit published version:

```bash
TEST_CRATE_VERSION=1.41.0 bash .github/ci-scripts/run-integration-docker.sh
```
