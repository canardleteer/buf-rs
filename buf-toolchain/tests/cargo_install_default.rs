//! Default `cargo install` of `buf-toolchain` must select the helper binary.
//!
//! Cargo prints
//! `warning: none of the package's binaries are available for install using the
//! selected features` when no `[[bin]]` matches the enabled features. Nested
//! install (needs network when the temp cache is cold):
//! `cargo test -p buf-toolchain --locked --test cargo_install_default -- --ignored`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MISSING_BINARIES_WARNING: &str =
    "none of the package's binaries are available for install using the selected features";

struct Scratch(PathBuf);

impl Scratch {
    fn new(prefix: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        Self(std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id())))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("buf-toolchain crate must live one level below workspace root")
        .to_path_buf()
}

fn helper_name() -> &'static str {
    if cfg!(windows) {
        "validate-cargo-buf-toolchain.exe"
    } else {
        "validate-cargo-buf-toolchain"
    }
}

#[test]
#[ignore = "nested cargo install + cold cache needs network; cargo test -p buf-toolchain --locked --test cargo_install_default -- --ignored"]
fn default_cargo_install_does_not_warn_missing_binaries() {
    let scratch = Scratch::new("buf-tc-install-default");
    let fake_cargo = scratch.path().join(".cargo");
    fs::create_dir_all(fake_cargo.join("bin")).expect("mkdir cargo home bin");
    let install_root = scratch.path().join("install-root");
    fs::create_dir_all(install_root.join("bin")).expect("mkdir install-root bin");
    let cache_root = std::env::var_os("BUF_RS_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.path().join("cache-root"));
    fs::create_dir_all(&cache_root).expect("mkdir cache");
    let target_dir = scratch.path().join("install-target");
    fs::create_dir_all(&target_dir).expect("mkdir install target");

    let crate_path = workspace_root().join("buf-toolchain");
    let output = Command::new("cargo")
        .current_dir(workspace_root())
        .args([
            "install",
            "--path",
            crate_path.to_str().expect("utf-8 crate path"),
            "--force",
        ])
        .env("CARGO_HOME", &fake_cargo)
        .env("CARGO_INSTALL_ROOT", &install_root)
        .env("TMPDIR", scratch.path())
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("BUF_RS_CACHE_DIR", &cache_root)
        .output()
        .expect("spawn cargo install");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "default cargo install buf-toolchain failed (status {:?})\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains(MISSING_BINARIES_WARNING),
        "default cargo install (no --features / --no-default-features) must not emit `{MISSING_BINARIES_WARNING}`\nstderr:\n{stderr}"
    );

    let helper = install_root.join("bin").join(helper_name());
    assert!(
        helper.is_file(),
        "expected helper at {:?} after default cargo install",
        helper
    );
}
