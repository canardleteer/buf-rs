# Post-publish integration image — build context is staged by CI or .github/ci-scripts/run-integration-docker.sh
# (rust-toolchain.toml, this Cargo.toml, entrypoint.sh, examples *.rs + proto/). No workspace root.
#
# RUST_DOCKER_TAG must match [toolchain].channel in rust-toolchain.toml (major.minor-slim-bookworm).
# CI and run-integration-docker.sh set it via .github/ci-scripts/rust-docker-tag-from-toolchain.sh
# so the base image tracks the same release line as rust-toolchain.toml without hardcoding here.
ARG RUST_DOCKER_TAG
FROM rust:${RUST_DOCKER_TAG}

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY rust-toolchain.toml ./
# Toolchain patch + components come from rust-toolchain.toml (directory override); no rustup default.

COPY Cargo.toml ./
COPY buf_lint.rs protoc_with_buf_plugins.rs ./
COPY proto ./proto/

# Prefetch crates.io deps that do not depend on TEST_CRATE_VERSION (buf-tools is added at runtime).
RUN cargo fetch

COPY entrypoint.sh ./
RUN chmod +x entrypoint.sh

ENTRYPOINT ["/app/entrypoint.sh"]
