//! Install verified Buf release binaries via **`build.rs`** when this crate is built.
//!
//! Typical use is **`cargo install buf-toolchain --features validate-cli`** (Cargo requires a
//! `[[bin]]`; the installed **`validate-cargo-buf-toolchain`** binary re-checks **`buf`** /
//! **`protoc-gen-buf-*`** against the
//! official GitHub release (**minisign** + **`sha256.txt`**), compares **`releases/latest`** for newer
//! Buf, and probes crates.io for **`buf-toolchain`** when an upgrade exists — **`buf`** and plugins
//! themselves are installed by **`build.rs`**).
//! Alternatively add **`buf-toolchain`** under **`[build-dependencies]`** so **`cargo build`** runs
//! the same installer without **`cargo install`**.

#![forbid(unsafe_code)]

/// Crate version string (matches this crate’s semver pin to upstream Buf).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Shared with `build.rs`. `test` keeps the same unit tests as buf-tools without
// enabling validate-cli (HTTP / clap) on a default `cargo test --lib`.
#[cfg(any(feature = "validate-cli", test))]
#[path = "../build_support/targets.rs"]
pub mod targets;

#[cfg(any(feature = "validate-cli", test))]
#[path = "../build_support/verify.rs"]
pub mod verify;

#[cfg(feature = "validate-cli")]
pub mod upstream;
