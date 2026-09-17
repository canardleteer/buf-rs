//! Regression test that `buf-toolchain` rustdoc succeeds with `DOCS_RS=1` (no network).
//!
//! 1.72.0-hotfix.1: `build.rs` must return before HTTP or bin-dir writes.
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

    fn path(&self) -> &std::path::Path {
        &self.0
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
    let managed_bin = scratch.path().join("managed-bin");
    fs::create_dir_all(&target_dir).expect("mkdir target");
    fs::create_dir_all(&managed_bin).expect("mkdir managed-bin");

    let output = Command::new("cargo")
        .current_dir(workspace_root())
        .env("DOCS_RS", "1")
        .env("CARGO_NET_OFFLINE", "true")
        .env("BUF_RS_TOOLCHAIN_BIN_DIR", &managed_bin)
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("canonical bin dir"),
        "DOCS_RS=1 must return before install; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("buf-toolchain: installed"),
        "DOCS_RS=1 must not install binaries; stderr:\n{stderr}"
    );

    let buf_name = if cfg!(windows) { "buf.exe" } else { "buf" };
    let leaked = managed_bin.join(buf_name);
    assert!(
        !leaked.exists(),
        "DOCS_RS=1 must not write {buf_name} under BUF_RS_TOOLCHAIN_BIN_DIR"
    );
}
