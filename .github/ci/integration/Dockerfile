# Integration image — build context is staged by CI, run-integration-docker.sh, or cargo xtask image
# (rust-toolchain.toml, this Cargo.toml, entrypoint.sh, examples *.rs + proto/, optional path-src/).
#
# RUST_DOCKER_TAG comes from rust-toolchain.toml via
# rust-docker-tag-from-toolchain.sh (`stable` → slim-bookworm).
# Callers set it so the base image is not hardcoded here.
ARG RUST_DOCKER_TAG=slim-bookworm
FROM rust:${RUST_DOCKER_TAG}

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY rust-toolchain.toml ./
# Channel + components come from rust-toolchain.toml (directory override); no rustup default.

COPY Cargo.toml ./
COPY buf_lint.rs protoc_with_buf_plugins.rs ./
COPY proto ./proto/

# Prefetch crates.io deps that do not depend on TEST_CRATE_VERSION (buf-tools is added at runtime).
RUN cargo fetch

COPY entrypoint.sh ./
RUN chmod +x entrypoint.sh

# Optional workspace tree for path-preinstall (xtask). Registry staging copies an empty dir.
COPY path-src /opt/path-src
ARG INSTALL_MODE=registry
RUN if [ "${INSTALL_MODE}" = "path" ]; then \
      cargo install --locked --root /usr/local --features validate-cli --path /opt/path-src/buf-toolchain; \
    fi

ENTRYPOINT ["/app/entrypoint.sh"]
