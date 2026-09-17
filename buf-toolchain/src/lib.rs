//! Install verified Buf release binaries via **`build.rs`** when this crate is built.
//!
//! Typical use is **`cargo install buf-toolchain`** (Cargo requires a
//! `[[bin]]`; default feature **`validate-cli`** selects it). The installed
//! **`validate-cargo-buf-toolchain`** binary re-checks **`buf`** /
//! **`protoc-gen-buf-*`** against the
//! official GitHub release (**minisign** + **`sha256.txt`**), compares **`releases/latest`** for newer
//! Buf, and probes crates.io for **`buf-toolchain`** when an upgrade exists — **`buf`** and plugins
//! themselves are installed by **`build.rs`**).
//! Alternatively add **`buf-toolchain`** under **`[build-dependencies]`** so **`cargo build`** runs
//! the same installer without **`cargo install`**.

#![forbid(unsafe_code)]

/// Crate version string (matches this crate’s semver pin to upstream Buf).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Shared with `build.rs`. `test` keeps the same unit tests as buf-tools when
// validate-cli is off (`--no-default-features`).
#[cfg(any(feature = "validate-cli", test))]
#[path = "../build_support/targets.rs"]
pub mod targets;

#[cfg(any(feature = "validate-cli", test))]
#[path = "../build_support/verify.rs"]
pub mod verify;

#[cfg(feature = "validate-cli")]
pub mod upstream;

#[cfg(test)]
mod tests {
    #[test]
    fn validate_cli_is_a_default_feature() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let value: toml::Value = toml::from_str(manifest).expect("parse Cargo.toml");
        let default = value
            .get("features")
            .and_then(|features| features.get("default"))
            .and_then(toml::Value::as_array)
            .expect("features.default");
        assert!(
            default
                .iter()
                .any(|feature| feature.as_str() == Some("validate-cli")),
            "validate-cli must stay a default feature so cargo install selects the helper binary"
        );
    }
}
