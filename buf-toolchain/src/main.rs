//! Installed as `validate-cargo-buf-toolchain` alongside `cargo install buf-toolchain`.
//! Cargo requires a `[[bin]]`; this tool sanity-checks local installs and optionally GitHub.

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use clap::Parser;
use serde::Serialize;

#[allow(dead_code)]
#[path = "../build_support/paths.rs"]
mod paths;

use buf_toolchain::targets::from_rust_triple;
use buf_toolchain::upstream::{
    UpgradeComparison, collect_upgrade_report, extract_installed_buf_core,
    verify_binaries_against_github_release,
};

#[derive(Debug, Parser)]
#[command(
    name = "validate-cargo-buf-toolchain",
    about = "Re-check buf / protoc-gen-buf-* installed by buf-toolchain"
)]
struct Cli {
    /// Write a single YAML document to stdout instead of the human report.
    #[arg(long)]
    yaml: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let report = collect_report();
    if cli.yaml {
        match noyalib::to_string(&report) {
            Ok(yaml) => {
                print!("{yaml}");
                if !yaml.ends_with('\n') {
                    println!();
                }
            }
            Err(e) => {
                eprintln!("failed to serialize YAML report: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        print_human(&report);
    }
    if report.ok {
        ExitCode::SUCCESS
    } else {
        if !cli.yaml {
            eprintln!("One or more checks failed.");
        }
        ExitCode::from(1)
    }
}

fn collect_report() -> ValidateReport {
    let crate_version = env!("CARGO_PKG_VERSION").to_string();
    let semver_core = semver_core(&crate_version);
    let (bin_dir, bin_dir_source) = resolve_canonical_bin_dir();
    let host_triple = host_triple();
    let release_target = host_triple.as_deref().and_then(from_rust_triple);

    let mut binaries = Vec::new();
    let mut ok = true;

    let buf_name = if cfg!(windows) { "buf.exe" } else { "buf" };
    let buf_path = bin_dir.join(buf_name);
    let buf_check = check_buf(&buf_path, &crate_version);
    if buf_check.status != "ok" {
        ok = false;
    }
    let buf_stdout = buf_check.version_stdout.clone();
    binaries.push(buf_check);

    for (label, name) in [
        ("protoc-gen-buf-breaking", protoc_breaking_name()),
        ("protoc-gen-buf-lint", protoc_lint_name()),
    ] {
        let check = check_plugin(label, &bin_dir.join(name));
        if check.status != "ok" {
            ok = false;
        }
        binaries.push(check);
    }

    let offline = env::var("BUF_RS_VALIDATE_OFFLINE")
        .map(|v| v == "1")
        .unwrap_or(false);

    let mut network = NetworkSection {
        offline,
        skipped: None,
        github_sha256: None,
        upgrade: None,
    };

    if offline {
        network.skipped = Some("BUF_RS_VALIDATE_OFFLINE=1".to_string());
    } else if let Some(rt) = release_target {
        if let Some(ref stdout) = buf_stdout {
            if let Some(core) = extract_installed_buf_core(stdout) {
                match verify_binaries_against_github_release(&bin_dir, &rt, &core) {
                    Ok(assets) => {
                        network.github_sha256 = Some(GitHubSha256 {
                            ok: true,
                            installed_core: Some(core.clone()),
                            error: None,
                            assets: assets
                                .into_iter()
                                .map(|a| GitHubAsset {
                                    local_name: a.local_name,
                                    remote: a.remote,
                                    status: "ok".to_string(),
                                })
                                .collect(),
                        });
                    }
                    Err(e) => {
                        ok = false;
                        network.github_sha256 = Some(GitHubSha256 {
                            ok: false,
                            installed_core: Some(core.clone()),
                            error: Some(e),
                            assets: Vec::new(),
                        });
                    }
                }
                let upgrade = collect_upgrade_report(&core);
                network.upgrade = Some(UpgradeSection {
                    latest_github_core: upgrade.latest_github_core,
                    comparison: match upgrade.comparison {
                        UpgradeComparison::Newer => "newer",
                        UpgradeComparison::Same => "same",
                        UpgradeComparison::Older => "older",
                        UpgradeComparison::Error => "error",
                    }
                    .to_string(),
                    message: upgrade.message,
                    crates_io: upgrade.crates_io,
                });
            } else {
                network.skipped =
                    Some("could not parse X.Y.Z from buf --version output".to_string());
            }
        } else {
            network.skipped = Some("buf --version unavailable".to_string());
        }
    } else {
        network.skipped = Some(format!(
            "unsupported TARGET `{}` for asset mapping",
            host_triple.as_deref().unwrap_or("unknown")
        ));
    }

    ValidateReport {
        ok,
        crate_version,
        semver_core,
        host_triple,
        asset_suffix: release_target.map(|t| t.asset_suffix.to_string()),
        windows: cfg!(windows),
        bin_dir: bin_dir.display().to_string(),
        bin_dir_source,
        env: env_snapshot(),
        binaries,
        network,
    }
}

fn print_human(report: &ValidateReport) {
    println!(
        "validate-cargo-buf-toolchain (buf-toolchain crate {})",
        report.crate_version
    );
    println!();
    println!(
        "This program ships only because `cargo install` must install an executable. \
         The real payloads — buf and protoc-gen-buf-* — are written by this crate’s \
         build.rs (verified upstream via minisign + sha256.txt at install time)."
    );
    println!();
    println!("Canonical bin directory (same rules as install):");
    println!("  {} ({})", report.bin_dir, report.bin_dir_source);
    println!();
    println!("Local checks (crate semver pin) …");
    println!();

    for bin in &report.binaries {
        match bin.status.as_str() {
            "ok" => {
                if let Some(ref stdout) = bin.version_stdout {
                    println!("  {:24}OK   {}", bin.name, stdout.trim());
                } else if let Some(len) = bin.size_bytes {
                    println!("  {:24}OK   present ({len} bytes)", bin.name);
                } else {
                    println!("  {:24}OK", bin.name);
                }
            }
            _ => {
                if let Some(ref detail) = bin.detail {
                    println!("  {:24}FAIL {detail}", bin.name);
                } else {
                    println!("  {:24}FAIL", bin.name);
                }
            }
        }
    }

    if report.network.offline {
        println!();
        println!("Network checks skipped (BUF_RS_VALIDATE_OFFLINE=1).");
    } else if let Some(ref skipped) = report.network.skipped {
        println!();
        println!("Upstream verification skipped ({skipped}).");
    } else {
        if let Some(ref sha) = report.network.github_sha256 {
            println!();
            if let Some(ref core) = sha.installed_core {
                println!(
                    "Upstream GitHub release v{core} (download sha256.txt + minisign, verify files) …"
                );
            }
            if let Some(ref err) = sha.error {
                eprintln!("  FAIL {err}");
            } else {
                for asset in &sha.assets {
                    println!(
                        "  {:24}OK   matches GitHub sha256.txt ({})",
                        asset.local_name, asset.remote
                    );
                }
            }
        }
        if let Some(ref upgrade) = report.network.upgrade {
            println!();
            println!("GitHub latest tag & crates.io buf-toolchain …");
            println!("  {}", upgrade.message);
            if let Some(ref crates_io) = upgrade.crates_io {
                println!("  {crates_io}");
            }
        }
    }

    println!();
    if report.ok {
        println!("All checks passed.");
    }
}

fn check_buf(buf_path: &Path, pkg: &str) -> BinaryCheck {
    match buf_version_output(buf_path) {
        Some(stdout) => {
            if buf_stdout_matches_expect(&stdout, pkg) {
                BinaryCheck {
                    name: "buf".to_string(),
                    path: buf_path.display().to_string(),
                    exists: true,
                    size_bytes: std::fs::metadata(buf_path).ok().map(|m| m.len()),
                    version_stdout: Some(stdout),
                    status: "ok".to_string(),
                    detail: None,
                }
            } else {
                BinaryCheck {
                    name: "buf".to_string(),
                    path: buf_path.display().to_string(),
                    exists: true,
                    size_bytes: std::fs::metadata(buf_path).ok().map(|m| m.len()),
                    version_stdout: Some(stdout.clone()),
                    status: "fail".to_string(),
                    detail: Some(format!(
                        "reports: {}; expected stdout to contain pin `{pkg}` (or semver core if pre-release).",
                        stdout.trim()
                    )),
                }
            }
        }
        None => {
            if buf_path.is_file() {
                BinaryCheck {
                    name: "buf".to_string(),
                    path: buf_path.display().to_string(),
                    exists: true,
                    size_bytes: std::fs::metadata(buf_path).ok().map(|m| m.len()),
                    version_stdout: None,
                    status: "fail".to_string(),
                    detail: Some(format!(
                        "exists at {} but `buf --version` failed",
                        buf_path.display()
                    )),
                }
            } else {
                BinaryCheck {
                    name: "buf".to_string(),
                    path: buf_path.display().to_string(),
                    exists: false,
                    size_bytes: None,
                    version_stdout: None,
                    status: "fail".to_string(),
                    detail: Some(format!("missing — expected {}", buf_path.display())),
                }
            }
        }
    }
}

fn check_plugin(label: &str, path: &Path) -> BinaryCheck {
    if path.is_file() {
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if len > 256 {
            BinaryCheck {
                name: label.to_string(),
                path: path.display().to_string(),
                exists: true,
                size_bytes: Some(len),
                version_stdout: None,
                status: "ok".to_string(),
                detail: None,
            }
        } else {
            BinaryCheck {
                name: label.to_string(),
                path: path.display().to_string(),
                exists: true,
                size_bytes: Some(len),
                version_stdout: None,
                status: "fail".to_string(),
                detail: Some(format!("present but unexpectedly small ({len} bytes)")),
            }
        }
    } else {
        BinaryCheck {
            name: label.to_string(),
            path: path.display().to_string(),
            exists: false,
            size_bytes: None,
            version_stdout: None,
            status: "fail".to_string(),
            detail: Some(format!("missing — expected {}", path.display())),
        }
    }
}

fn env_snapshot() -> Vec<EnvEntry> {
    [
        ("BUF_RS_TOOLCHAIN_BIN_DIR", "buf-toolchain"),
        ("BUF_RS_CACHE_DIR", "buf-toolchain"),
        ("BUF_RS_RELEASE_BASE_URL", "buf-toolchain"),
        ("BUF_RS_VALIDATE_OFFLINE", "buf-toolchain"),
        ("CARGO_HOME", "cargo"),
        ("BUF_RS_LAYOUT_MODE", "buf-tools-only"),
        ("BUF_RS_BUILD_LOG", "buf-tools-only"),
        ("BUF_RS_SOURCE_BASE_URL", "buf-tools-only"),
    ]
    .into_iter()
    .map(|(name, applies_to)| match env::var(name) {
        Ok(value) => EnvEntry {
            name: name.to_string(),
            set: true,
            value: Some(value),
            applies_to: applies_to.to_string(),
        },
        Err(_) => EnvEntry {
            name: name.to_string(),
            set: false,
            value: None,
            applies_to: applies_to.to_string(),
        },
    })
    .collect()
}

fn semver_core(v: &str) -> String {
    let base = v.split('+').next().unwrap_or(v);
    base.split('-').next().unwrap_or(base).to_string()
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

fn resolve_canonical_bin_dir() -> (PathBuf, String) {
    if let Ok(dir) = env::var("BUF_RS_TOOLCHAIN_BIN_DIR") {
        let d = dir.trim();
        if !d.is_empty() {
            return (PathBuf::from(d), "BUF_RS_TOOLCHAIN_BIN_DIR".to_string());
        }
    }
    if env::var("CARGO_HOME").is_ok() {
        return (cargo_home_bin(), "CARGO_HOME".to_string());
    }
    (cargo_home_bin(), "default_home".to_string())
}

fn cargo_home_bin() -> PathBuf {
    if let Ok(home) = env::var("CARGO_HOME") {
        return PathBuf::from(home).join("bin");
    }
    match paths::home_dir() {
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

#[derive(Debug, Serialize)]
struct ValidateReport {
    ok: bool,
    crate_version: String,
    semver_core: String,
    host_triple: Option<String>,
    asset_suffix: Option<String>,
    windows: bool,
    bin_dir: String,
    bin_dir_source: String,
    env: Vec<EnvEntry>,
    binaries: Vec<BinaryCheck>,
    network: NetworkSection,
}

#[derive(Debug, Serialize)]
struct EnvEntry {
    name: String,
    set: bool,
    value: Option<String>,
    applies_to: String,
}

#[derive(Debug, Serialize)]
struct BinaryCheck {
    name: String,
    path: String,
    exists: bool,
    size_bytes: Option<u64>,
    version_stdout: Option<String>,
    status: String,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct NetworkSection {
    offline: bool,
    skipped: Option<String>,
    github_sha256: Option<GitHubSha256>,
    upgrade: Option<UpgradeSection>,
}

#[derive(Debug, Serialize)]
struct GitHubSha256 {
    ok: bool,
    installed_core: Option<String>,
    error: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Serialize)]
struct GitHubAsset {
    local_name: String,
    remote: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct UpgradeSection {
    latest_github_core: Option<String>,
    comparison: String,
    message: String,
    crates_io: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_builds() {
        Cli::command().debug_assert();
    }

    #[test]
    fn yaml_report_contains_stable_triage_keys() {
        let report = ValidateReport {
            ok: true,
            crate_version: "1.73.0".to_string(),
            semver_core: "1.73.0".to_string(),
            host_triple: Some("x86_64-unknown-linux-gnu".to_string()),
            asset_suffix: Some("Linux-x86_64".to_string()),
            windows: false,
            bin_dir: "/tmp/bin".to_string(),
            bin_dir_source: "BUF_RS_TOOLCHAIN_BIN_DIR".to_string(),
            env: vec![EnvEntry {
                name: "BUF_RS_VALIDATE_OFFLINE".to_string(),
                set: true,
                value: Some("1".to_string()),
                applies_to: "buf-toolchain".to_string(),
            }],
            binaries: vec![BinaryCheck {
                name: "buf".to_string(),
                path: "/tmp/bin/buf".to_string(),
                exists: true,
                size_bytes: Some(1024),
                version_stdout: Some("1.73.0".to_string()),
                status: "ok".to_string(),
                detail: None,
            }],
            network: NetworkSection {
                offline: true,
                skipped: Some("BUF_RS_VALIDATE_OFFLINE=1".to_string()),
                github_sha256: None,
                upgrade: None,
            },
        };
        let yaml = noyalib::to_string(&report).expect("serialize");
        assert!(yaml.contains("ok: true"), "{yaml}");
        assert!(yaml.contains("crate_version: 1.73.0"), "{yaml}");
        assert!(yaml.contains("semver_core: 1.73.0"), "{yaml}");
        assert!(
            yaml.contains("bin_dir_source: BUF_RS_TOOLCHAIN_BIN_DIR"),
            "{yaml}"
        );
        assert!(yaml.contains("BUF_RS_VALIDATE_OFFLINE"), "{yaml}");
        assert!(yaml.contains("name: buf"), "{yaml}");
    }

    #[test]
    fn pre_release_crate_matches_buf_core_stdout() {
        assert!(buf_stdout_matches_expect("1.73.0", "1.73.0-dev.1"));
        assert!(!buf_stdout_matches_expect("1.72.0", "1.73.0"));
    }
}
