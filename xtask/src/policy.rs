//! Repository-owned seams for check order, coverage reports, and image targets.

use crate::{CheckStep, ImageEngine};

pub const CHECK_ORDER: [CheckStep; 4] = [
    CheckStep::Fmt,
    CheckStep::Check,
    CheckStep::Clippy,
    CheckStep::Test,
];

pub const LLVM_COV_REPORT: &str = "target/coverage/llvm-cov/html/index.html";
pub const TARPAULIN_REPORT: &str = "target/coverage/tarpaulin/tarpaulin-report.html";

pub const IMAGE_AUTO_ORDER: [ImageEngine; 2] = [ImageEngine::Docker, ImageEngine::Buildah];
pub const IMAGE_CONTEXT_DIR: &str = "target/xtask-image-context";
pub const IMAGE_DEBIAN_FILE: &str = "Dockerfile";
pub const IMAGE_ALPINE_FILE: &str = "Dockerfile.alpine";
pub const IMAGE_DEBIAN_TAG: &str = "buf-rs-integration:debian";
pub const IMAGE_ALPINE_TAG: &str = "buf-rs-integration:alpine";
pub const IMAGE_DEBIAN_FLAVOR: &str = "debian";
pub const IMAGE_ALPINE_FLAVOR: &str = "alpine";
pub const IMAGE_SMOKE_PROGRAM: &str = "/app/entrypoint.sh";
pub const IMAGE_TAG_SCRIPT: &str = ".github/ci-scripts/rust-docker-tag-from-toolchain.sh";
