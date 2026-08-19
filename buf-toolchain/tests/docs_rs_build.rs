//! Regression test that `buf-toolchain` rustdoc succeeds with `DOCS_RS=1` (no network).
//!
//! Isolated `CARGO_TARGET_DIR` avoids deadlocking with the parent `cargo test`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(prefix: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        Self(std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id())))
    }

    fn target_dir(&self) -> PathBuf {
        self.0.join("target")
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

#[test]
fn docs_rs_rustdoc() {
    let scratch = Scratch::new("buf-toolchain-docs-rs");
    let target_dir = scratch.target_dir();
    fs::create_dir_all(&target_dir).expect("mkdir target");

    let output = Command::new("cargo")
        .current_dir(workspace_root())
        .env("DOCS_RS", "1")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", &target_dir)
        .args([
            "doc",
            "-p",
            "buf-toolchain",
            "--locked",
            "--offline",
            "--no-deps",
        ])
        .output()
        .expect("spawn cargo doc");

    if !output.status.success() {
        panic!(
            "DOCS_RS=1 cargo doc -p buf-toolchain --no-deps failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
