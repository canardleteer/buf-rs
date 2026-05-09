//! Install verified Buf release binaries via **`build.rs`** when this crate is built.
//!
//! Typical use is **`cargo install buf-toolchain`** (Cargo requires a `[[bin]]`; the installed
//! **`validate-cargo-buf-toolchain`** binary re-checks **`buf`** / **`protoc-gen-buf-*`** against the
//! official GitHub release (**minisign** + **`sha256.txt`**), compares **`releases/latest`** for newer
//! Buf, and probes crates.io for **`buf-toolchain`** when an upgrade exists — **`buf`** and plugins
//! themselves are installed by **`build.rs`**).
//! Alternatively add **`buf-toolchain`** under **`[build-dependencies]`** so **`cargo build`** runs
//! the same installer without **`cargo install`**.

#![forbid(unsafe_code)]

#[path = "../build_support/targets.rs"]
pub mod targets;

#[path = "../build_support/verify.rs"]
pub mod verify;

pub mod upstream;

/// Crate version string (matches this crate’s semver pin to upstream Buf).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
