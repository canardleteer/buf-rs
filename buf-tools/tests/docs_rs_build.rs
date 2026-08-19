//! Regression tests for the docs.rs `DOCS_RS=1` build path (no network).
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
        .expect("buf-tools crate must live one level below workspace root")
        .to_path_buf()
}

fn docs_rs_cargo(scratch: &Scratch) -> Command {
    let target_dir = scratch.target_dir();
    fs::create_dir_all(&target_dir).expect("mkdir target");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(workspace_root())
        .env("DOCS_RS", "1")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", &target_dir);
    cmd
}

fn assert_success(mut cmd: Command, label: &str) {
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawn {label}: {e}"));
    if !output.status.success() {
        panic!(
            "{label} failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn docs_rs_lib_tests_and_rustdoc() {
    let scratch = Scratch::new("buf-tools-docs-rs");

    let mut test_cmd = docs_rs_cargo(&scratch);
    test_cmd.args(["test", "-p", "buf-tools", "--locked", "--offline", "--lib"]);
    assert_success(test_cmd, "DOCS_RS=1 cargo test -p buf-tools --lib");

    let mut doc_cmd = docs_rs_cargo(&scratch);
    doc_cmd.args([
        "doc",
        "-p",
        "buf-tools",
        "--locked",
        "--offline",
        "--no-deps",
    ]);
    assert_success(doc_cmd, "DOCS_RS=1 cargo doc -p buf-tools --no-deps");
}
