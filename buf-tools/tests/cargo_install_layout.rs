//! Contract tests for `buf-tools` layout under `cargo install` (isolated nested install).
//!
//! Requires network when the temp cache is cold. Run:
//! `cargo test -p buf-tools --locked --test cargo_install_layout -- --ignored`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static INSTALL_LOCK: Mutex<()> = Mutex::new(());

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

    fn install_target_dir(&self) -> PathBuf {
        self.0.join("install-target")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/install-consumer")
}

fn semver_core() -> &'static str {
    env!("CARGO_PKG_VERSION")
        .split('-')
        .next()
        .expect("semver core")
}

fn rustc_host_triple() -> String {
    let out = Command::new("rustc").args(["-vV"]).output().expect("rustc");
    assert!(out.status.success(), "rustc -vV failed");
    let text = String::from_utf8(out.stdout).expect("utf-8");
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("host: ") {
            return h.to_string();
        }
    }
    panic!("host triple not found in rustc -vV");
}

fn local_buf_name() -> &'static str {
    if cfg!(windows) { "buf.exe" } else { "buf" }
}

fn run_cargo_install(scratch: &Scratch, extra_env: &[(&str, &str)]) -> std::process::ExitStatus {
    let fake_cargo = scratch.path().join(".cargo");
    fs::create_dir_all(fake_cargo.join("bin")).expect("mkdir cargo home");
    let cache_root = scratch.path().join("cache-root");
    fs::create_dir_all(&cache_root).expect("mkdir cache");
    let target_dir = scratch.install_target_dir();
    fs::create_dir_all(&target_dir).expect("mkdir install target");

    let mut cmd = Command::new("cargo");
    cmd.arg("install")
        .arg("--path")
        .arg(fixture_dir())
        .arg("--root")
        .arg(&fake_cargo)
        .arg("--force")
        .env("CARGO_HOME", &fake_cargo)
        .env("TMPDIR", scratch.path())
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("BUF_RS_CACHE_DIR", &cache_root);
    for (k, v) in extra_env {
        cmd.env(k, *v);
    }
    cmd.status().expect("spawn cargo install")
}

fn profile_layout_bin(scratch: &Scratch) -> PathBuf {
    scratch
        .install_target_dir()
        .join("buf-tools")
        .join(semver_core())
        .join(rustc_host_triple())
        .join("bin")
        .join(local_buf_name())
}

#[test]
#[ignore = "nested cargo install + cold cache needs network; cargo test -p buf-tools --locked --test cargo_install_layout -- --ignored"]
fn cargo_install_default_cache_succeeds() {
    let _guard = INSTALL_LOCK.lock().expect("install lock");
    let scratch = Scratch::new("buf-tools-install-cache");
    let st = run_cargo_install(&scratch, &[]);
    assert!(
        st.success(),
        "cargo install failed with default cache mode (regression: target layout walk in cache mode)"
    );
}

#[test]
#[ignore = "nested cargo install + cold cache needs network; cargo test -p buf-tools --locked --test cargo_install_layout -- --ignored"]
fn cargo_install_cache_link_with_profile_fallback() {
    let _guard = INSTALL_LOCK.lock().expect("install lock");
    let scratch = Scratch::new("buf-tools-install-cache-link");
    let st = run_cargo_install(&scratch, &[("BUF_RS_LAYOUT_MODE", "cache-link")]);
    assert!(
        st.success(),
        "cargo install failed with BUF_RS_LAYOUT_MODE=cache-link"
    );
    let layout_bin = profile_layout_bin(&scratch);
    assert!(
        layout_bin.is_file(),
        "expected cache-link landing pad at {:?}",
        layout_bin
    );
}

#[test]
#[ignore = "nested cargo install + cold cache needs network; cargo test -p buf-tools --locked --test cargo_install_layout -- --ignored"]
fn cargo_install_offline_after_prewarm() {
    let _guard = INSTALL_LOCK.lock().expect("install lock");
    let scratch = Scratch::new("buf-tools-install-offline");
    let online = run_cargo_install(&scratch, &[]);
    assert!(online.success(), "online cargo install failed");

    let offline = run_cargo_install(&scratch, &[("CARGO_NET_OFFLINE", "true")]);
    assert!(
        offline.success(),
        "offline cargo install failed after prewarm"
    );
}
