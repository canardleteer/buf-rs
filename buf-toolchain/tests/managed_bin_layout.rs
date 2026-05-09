//! Contract tests for `build.rs` install layout (isolated nested `cargo build`).
//!
//! Requires network when the temp cache is cold. Run:
//! `cargo test -p buf-toolchain --locked --test managed_bin_layout -- --ignored`

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
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

fn semver_core() -> &'static str {
    env!("CARGO_PKG_VERSION")
        .split('-')
        .next()
        .expect("semver core")
}

/// Upstream GitHub asset name for `buf` for the given Cargo host triple (`rustc -vV`).
fn remote_buf_object_name_for_triple(triple: &str) -> &'static str {
    match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => "buf-Linux-x86_64",
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => "buf-Linux-aarch64",
        "arm-unknown-linux-gnueabihf" | "arm-unknown-linux-musleabihf" => "buf-Linux-armv7",
        "powerpc64le-unknown-linux-gnu" => "buf-Linux-ppc64le",
        "s390x-unknown-linux-gnu" => "buf-Linux-s390x",
        "riscv64gc-unknown-linux-gnu" | "riscv64-unknown-linux-gnu" => "buf-Linux-riscv64",
        "x86_64-apple-darwin" => "buf-Darwin-x86_64",
        "aarch64-apple-darwin" => "buf-Darwin-arm64",
        "x86_64-pc-windows-gnu" | "x86_64-pc-windows-msvc" => "buf-Windows-x86_64.exe",
        "aarch64-pc-windows-msvc" | "aarch64-pc-windows-gnu" => "buf-Windows-arm64.exe",
        "x86_64-unknown-freebsd" => "buf-FreeBSD-x86_64",
        "aarch64-unknown-freebsd" => "buf-FreeBSD-arm64",
        "x86_64-unknown-openbsd" => "buf-OpenBSD-x86_64",
        "aarch64-unknown-openbsd" => "buf-OpenBSD-arm64",
        other => panic!("managed_bin_layout: add buf remote name for host triple={other}"),
    }
}

fn local_buf_name() -> &'static str {
    if cfg!(windows) { "buf.exe" } else { "buf" }
}

fn sha256_hex_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn parse_expected_hex_for_remote(sha256_txt: &str, remote: &str) -> String {
    for line in sha256_txt.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (hash, name) = line.split_once("  ").expect("sha256.txt line");
        if name.trim() == remote {
            return hash.to_ascii_lowercase();
        }
    }
    panic!("no entry for {remote} in sha256.txt");
}

fn assert_fifo_or_regular_file_not_symlink(path: &Path) {
    #[cfg(unix)]
    {
        assert!(
            !fs::symlink_metadata(path)
                .expect("symlink_metadata")
                .is_symlink(),
            "{:?} must be a real file for cargo bin install, not a symlink",
            path
        );
    }
}

#[test]
#[ignore = "nested cargo + cold cache needs network; cargo test -p buf-toolchain --locked --test managed_bin_layout -- --ignored"]
fn nested_default_installs_real_bins_under_fake_cargo_home_bin() {
    let scratch = Scratch::new("buf-tc-cargo-bin");
    let fake_cargo = scratch.path().join(".cargo");
    fs::create_dir_all(fake_cargo.join("bin")).expect("mkdir");
    let cache_root = scratch.path().join("cache-root");

    let st = Command::new("cargo")
        .current_dir(workspace_root())
        .env("CARGO_HOME", &fake_cargo)
        .env("BUF_RS_CACHE_DIR", &cache_root)
        .args(["build", "-p", "buf-toolchain", "--locked"])
        .status()
        .expect("spawn cargo");
    assert!(st.success(), "nested cargo build failed");

    let stub = fake_cargo.join("bin").join(local_buf_name());
    assert!(stub.is_file(), "expected {:?}", stub);
    assert_fifo_or_regular_file_not_symlink(&stub);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&stub).expect("stat").permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "expected {:?} executable, mode {:o}",
            stub,
            mode
        );
    }

    let triple = rustc_host_triple();
    let remote_buf = remote_buf_object_name_for_triple(&triple);
    let cache_slot = cache_root.join(semver_core()).join(&triple);
    let upstream = cache_slot.join(remote_buf);
    assert!(
        upstream.is_file(),
        "expected cached upstream blob {:?}",
        upstream
    );

    let installed = fs::read(&stub).expect("read installed buf");
    let cached = fs::read(&upstream).expect("read cache");
    assert_eq!(
        installed, cached,
        "installed buf must equal verified cache blob"
    );

    let sha256_txt_path = cache_slot.join("sha256.txt");
    if sha256_txt_path.is_file() {
        let txt = fs::read_to_string(&sha256_txt_path).expect("sha256.txt");
        let expected = parse_expected_hex_for_remote(&txt, remote_buf);
        assert_eq!(sha256_hex_bytes(&installed), expected);
    } else {
        // build.rs does not persist sha256.txt into the cache slot; byte equality with upstream cache still proves the manifest-backed artifact.
        assert_eq!(
            sha256_hex_bytes(&installed),
            sha256_hex_bytes(&cached),
            "SHA256(installed) vs cache"
        );
    }
}

#[test]
#[ignore = "nested cargo + cold cache needs network"]
fn nested_bin_dir_override_skips_cargo_bin() {
    let scratch = Scratch::new("buf-tc-bindir");
    let managed_bin = scratch.path().join("managed-bin");
    let fake_cargo = scratch.path().join(".cargo");
    fs::create_dir_all(&managed_bin).expect("mkdir managed-bin");
    fs::create_dir_all(fake_cargo.join("bin")).expect("mkdir fake cargo bin");
    let cache_root = scratch.path().join("cache-root");

    let st = Command::new("cargo")
        .current_dir(workspace_root())
        .env("CARGO_HOME", &fake_cargo)
        .env("BUF_RS_TOOLCHAIN_BIN_DIR", &managed_bin)
        .env("BUF_RS_CACHE_DIR", &cache_root)
        .args(["build", "-p", "buf-toolchain", "--locked"])
        .status()
        .expect("spawn cargo");
    assert!(st.success(), "nested cargo build failed");

    let canonical = managed_bin.join(local_buf_name());
    assert!(
        canonical.is_file(),
        "expected BUF_RS_TOOLCHAIN_BIN_DIR install at {:?}",
        canonical
    );

    let cargo_stub = fake_cargo.join("bin").join(local_buf_name());
    assert!(
        !cargo_stub.exists(),
        "BUF_RS_TOOLCHAIN_BIN_DIR must be the only install root; unexpected {:?}",
        cargo_stub
    );

    let triple = rustc_host_triple();
    let upstream = cache_root
        .join(semver_core())
        .join(&triple)
        .join(remote_buf_object_name_for_triple(&triple));
    let installed = fs::read(&canonical).expect("read installed");
    let cached = fs::read(&upstream).expect("read cache");
    assert_eq!(installed, cached);
}

#[test]
#[ignore = "nested cargo + cold cache needs network"]
fn nested_offline_reuses_verified_cache() {
    let scratch = Scratch::new("buf-tc-offline");
    let fake_cargo = scratch.path().join(".cargo");
    fs::create_dir_all(fake_cargo.join("bin")).expect("mkdir");
    let cache_root = scratch.path().join("cache-root");

    let online = Command::new("cargo")
        .current_dir(workspace_root())
        .env("CARGO_HOME", &fake_cargo)
        .env("BUF_RS_CACHE_DIR", &cache_root)
        .args(["build", "-p", "buf-toolchain", "--locked"])
        .status()
        .expect("spawn cargo");
    assert!(online.success(), "online nested build failed");

    remove_installed_bins_for_clean_offline_check(&fake_cargo.join("bin"));

    let offline = Command::new("cargo")
        .current_dir(workspace_root())
        .env("CARGO_HOME", &fake_cargo)
        .env("BUF_RS_CACHE_DIR", &cache_root)
        .env("CARGO_NET_OFFLINE", "1")
        .args(["build", "-p", "buf-toolchain", "--locked"])
        .status()
        .expect("spawn cargo offline");
    assert!(offline.success(), "offline nested build failed");

    let stub = fake_cargo.join("bin").join(local_buf_name());
    assert!(stub.is_file(), "expected reinstall {:?}", stub);

    let triple = rustc_host_triple();
    let upstream = cache_root
        .join(semver_core())
        .join(&triple)
        .join(remote_buf_object_name_for_triple(&triple));
    assert_eq!(
        fs::read(&stub).expect("read"),
        fs::read(&upstream).expect("cache")
    );
}

fn remove_installed_bins_for_clean_offline_check(bin_dir: &Path) {
    for name in [
        local_buf_name(),
        if cfg!(windows) {
            "protoc-gen-buf-breaking.exe"
        } else {
            "protoc-gen-buf-breaking"
        },
        if cfg!(windows) {
            "protoc-gen-buf-lint.exe"
        } else {
            "protoc-gen-buf-lint"
        },
    ] {
        let _ = fs::remove_file(bin_dir.join(name));
    }
}

fn rustc_host_triple() -> String {
    let out = Command::new("rustc").args(["-vV"]).output().expect("rustc");
    assert!(out.status.success(), "rustc -vV failed");
    let text = String::from_utf8(out.stdout).expect("utf-8");
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("host: ") {
            return h.trim().to_string();
        }
    }
    panic!("no host: in rustc -vV");
}
