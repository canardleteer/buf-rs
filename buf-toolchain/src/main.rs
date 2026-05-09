//! Installed as `validate-cargo-buf-toolchain` alongside `cargo install buf-toolchain`.
//! Cargo requires a `[[bin]]`; this tool sanity-checks local installs and optionally GitHub.

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use buf_toolchain::targets::from_rust_triple;
use buf_toolchain::upstream::{
    extract_installed_buf_core, report_newer_and_crates_io, verify_binaries_against_github_release,
};

fn main() -> std::process::ExitCode {
    let pkg = env!("CARGO_PKG_VERSION");
    let bin_dir = resolve_canonical_bin_dir();

    println!("validate-cargo-buf-toolchain (buf-toolchain crate {pkg})");
    println!();
    println!(
        "This program ships only because `cargo install` must install an executable. \
         The real payloads — buf and protoc-gen-buf-* — are written by this crate’s \
         build.rs (verified upstream via minisign + sha256.txt at install time)."
    );
    println!();
    println!("Canonical bin directory (same rules as install):");
    println!("  {}", bin_dir.display());
    println!();
    println!("Local checks (crate semver pin) …");
    println!();

    let mut ok = true;
    let mut buf_stdout: Option<String> = None;

    let buf_path = bin_dir.join(if cfg!(windows) { "buf.exe" } else { "buf" });
    match buf_version_output(&buf_path) {
        Some(stdout) => {
            buf_stdout = Some(stdout.clone());
            if buf_stdout_matches_expect(&stdout, pkg) {
                println!("  buf                     OK   {}", stdout.trim());
            } else {
                ok = false;
                println!("  buf                     FAIL reports: {}", stdout.trim());
                println!(
                    "           expected stdout to contain pin `{pkg}` (or semver core if pre-release)."
                );
            }
        }
        None => {
            if buf_path.is_file() {
                ok = false;
                println!(
                    "  buf                     FAIL exists at {} but `buf --version` failed",
                    buf_path.display()
                );
            } else {
                ok = false;
                println!(
                    "  buf                     FAIL missing — expected {}",
                    buf_path.display()
                );
            }
        }
    }

    for (label, name) in [
        ("protoc-gen-buf-breaking", protoc_breaking_name()),
        ("protoc-gen-buf-lint", protoc_lint_name()),
    ] {
        let p = bin_dir.join(name);
        if p.is_file() {
            let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if len > 256 {
                println!("  {label:24}OK   present ({len} bytes)");
            } else {
                ok = false;
                println!("  {label:24}FAIL present but unexpectedly small ({len} bytes)");
            }
        } else {
            ok = false;
            println!("  {label:24}FAIL missing — expected {}", p.display());
        }
    }

    let offline = env::var("BUF_RS_VALIDATE_OFFLINE")
        .map(|v| v == "1")
        .unwrap_or(false);

    if offline {
        println!();
        println!("Network checks skipped (BUF_RS_VALIDATE_OFFLINE=1).");
    } else if let Some(rt) = host_triple().as_deref().and_then(from_rust_triple) {
        if let Some(ref stdout) = buf_stdout {
            if let Some(core) = extract_installed_buf_core(stdout) {
                println!();
                println!(
                    "Upstream GitHub release v{core} (download sha256.txt + minisign, verify files) …"
                );
                match verify_binaries_against_github_release(&bin_dir, &rt, &core) {
                    Ok(()) => {}
                    Err(e) => {
                        ok = false;
                        eprintln!("  FAIL {e}");
                    }
                }
                println!();
                println!("GitHub latest tag & crates.io buf-toolchain …");
                report_newer_and_crates_io(&core);
            } else {
                println!();
                println!(
                    "Upstream verification skipped (could not parse X.Y.Z from buf --version output)."
                );
            }
        } else {
            println!();
            println!("Upstream verification skipped (buf --version unavailable).");
        }
    } else {
        println!();
        println!(
            "Upstream verification skipped (unsupported TARGET `{}` for asset mapping).",
            host_triple().as_deref().unwrap_or("unknown")
        );
    }

    println!();
    if ok {
        println!("All checks passed.");
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!("One or more checks failed.");
        std::process::ExitCode::from(1)
    }
}

fn protoc_breaking_name() -> &'static str {
    if cfg!(windows) {
        "protoc-gen-buf-breaking.exe"
    } else {
        "protoc-gen-buf-breaking"
    }
}

fn protoc_lint_name() -> &'static str {
    if cfg!(windows) {
        "protoc-gen-buf-lint.exe"
    } else {
        "protoc-gen-buf-lint"
    }
}

fn buf_version_output(buf_exe: &Path) -> Option<String> {
    let mut child = Command::new(buf_exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = String::new();
    let mut pipe = child.stdout.take()?;
    pipe.read_to_string(&mut stdout).ok()?;
    let status = child.wait().ok()?;
    status.success().then_some(stdout)
}

/// Aligns with [`buf_tools::tests::buf_stdout_matches_expect`] / buf `--version` smoke tests.
fn buf_stdout_matches_expect(stdout: &str, expect_pkg_version: &str) -> bool {
    let stdout = stdout.trim();
    let expect = expect_pkg_version.trim();
    if stdout.contains(expect) {
        return true;
    }
    if let Some((core, rest)) = expect.split_once('-')
        && !rest.is_empty()
        && stdout.contains(core)
    {
        return true;
    }
    false
}

fn resolve_canonical_bin_dir() -> PathBuf {
    if let Ok(dir) = env::var("BUF_RS_TOOLCHAIN_BIN_DIR") {
        let d = dir.trim();
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    cargo_home_bin()
}

fn cargo_home_bin() -> PathBuf {
    if let Ok(home) = env::var("CARGO_HOME") {
        return PathBuf::from(home).join("bin");
    }
    match dirs::home_dir() {
        Some(h) => h.join(".cargo").join("bin"),
        None => PathBuf::from(".cargo").join("bin"),
    }
}

/// Rust host triple for mapping to Buf asset suffixes (matches nested integration tests).
fn host_triple() -> Option<String> {
    if let Some(t) = option_env!("TARGET") {
        return Some(t.to_string());
    }
    let out = Command::new("rustc").args(["-vV"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("host: ") {
            return Some(h.trim().to_string());
        }
    }
    None
}
