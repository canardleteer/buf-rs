//! `cargo xtask` — workspace automation (see `.cargo/config.toml`).
//!
//! **Buf upstream version:** The rule `major.minor.patch` from the crate semver (ignoring
//! pre-release / build metadata) must stay aligned with
//! `buf-tools/build.rs` and `buf-toolchain/build.rs` (`CARGO_PKG_VERSION` → GitHub tag `vX.Y.Z`).
//!
//! **Set the Buf pin (maintainers):** `cargo xtask workspace set-buf-version X.Y.Z` on the root
//! manifest, then `cargo generate-lockfile`, then set **`BUF_EXPECT_VERSION`** from
//! **`cargo xtask expected-buf-version`** and run tests (see **`README.md`**).

mod publish_inputs;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, Subcommand};
use publish_inputs::{PublishChannel, ResolvedPublishFlags, resolve_flags};
use semver::Version;
use toml_edit::{DocumentMut, Item, Value};

#[derive(Parser)]
#[command(name = "xtask", about = "buf-rs workspace tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the `X.Y.Z` core used for `BUF_EXPECT_VERSION` in tests.
    ///
    /// Read from the **root workspace** `Cargo.toml`: `[workspace.package].version`, taking only
    /// `major.minor.patch` (pre-release and build metadata are ignored — same rule as `build.rs`
    /// when selecting GitHub tag `vX.Y.Z`).
    ExpectedBufVersion,
    /// Crates.io publish helpers (used by `.github/workflows/publish-crates.yml`).
    Publish {
        #[command(subcommand)]
        cmd: PublishCmd,
    },
    /// Maintainer: set the root workspace Buf semver pin (plain `X.Y.Z`). For CI-only dev/rc
    /// pre-release suffixes on the manifest, use **`publish apply-version`** instead.
    Workspace {
        #[command(subcommand)]
        cmd: WorkspaceCmd,
    },
}

#[derive(Subcommand)]
enum WorkspaceCmd {
    /// Set `[workspace.package].version` and `=X.Y.Z` pins for `buf-tools` / `buf-toolchain`.
    ///
    /// Confirm `https://github.com/bufbuild/buf/releases/tag/vX.Y.Z` exists before publishing.
    SetBufVersion {
        /// Plain semver `X.Y.Z` (no pre-release / build metadata).
        version: String,
    },
}

#[derive(Subcommand)]
enum PublishCmd {
    /// Print the version that would be published (no file writes).
    ///
    /// Pass `--run-id` / `--rc-number` as empty strings when unused; see `publish_inputs::resolve_flags`.
    Resolve {
        #[arg(long)]
        channel: PublishChannel,
        #[arg(long, default_value = "")]
        run_id: String,
        #[arg(long, default_value = "")]
        rc_number: String,
    },
    /// Set `[workspace.package].version` and workspace dependency pins (dev: `-dev.RUN_ID`, rc: `-rc.N`).
    ApplyVersion {
        #[arg(long)]
        channel: PublishChannel,
        #[arg(long, default_value = "")]
        run_id: String,
        #[arg(long, default_value = "")]
        rc_number: String,
    },
    /// Emit Markdown for `GITHUB_STEP_SUMMARY`: crate semver breakdown + resolved Buf versions.
    VerifySummary {
        #[arg(long)]
        crates_version: String,
    },
}

fn root_manifest() -> PathBuf {
    if let Ok(ws) = std::env::var("GITHUB_WORKSPACE") {
        return Path::new(&ws).join("Cargo.toml");
    }
    let xtask_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );
    xtask_dir
        .parent()
        .expect("xtask crate must live one level below workspace root")
        .join("Cargo.toml")
}

fn read_workspace_version(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("xtask: read {}: {e}", path.display());
        exit(1);
    });
    let doc: DocumentMut = text.parse().unwrap_or_else(|e| {
        eprintln!("xtask: parse {}: {e}", path.display());
        exit(1);
    });
    let v = doc["workspace"]["package"]["version"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!(
                "xtask: missing workspace.package.version in {}",
                path.display()
            );
            exit(1);
        });
    v.to_string()
}

fn semver_core(v: &str) -> String {
    let base = v.split('+').next().unwrap_or(v);
    base.split('-').next().unwrap_or(base).to_string()
}

/// Parsed semver must match this for `stable` channel (no pre-release, no build metadata).
fn assert_stable_plain_semver(raw: &str) {
    let v = Version::parse(raw).unwrap_or_else(|e| {
        eprintln!("xtask: stable channel: invalid semver in workspace {raw:?}: {e}");
        exit(1);
    });
    if !v.pre.is_empty() || !v.build.is_empty() {
        eprintln!(
            "xtask: stable publish requires plain X.Y.Z (no pre-release or build metadata), got {raw:?}"
        );
        exit(1);
    }
}

fn must_parse_version(label: &str, s: &str) -> Version {
    Version::parse(s).unwrap_or_else(|e| {
        eprintln!("xtask: {label}: invalid semver {s:?}: {e}");
        exit(1);
    })
}

/// Buf GitHub release tag core — keep in sync with `buf-tools/build.rs` and `buf-toolchain/build.rs`.
fn buf_upstream_core(v: &Version) -> String {
    format!("{}.{}.{}", v.major, v.minor, v.patch)
}

/// Writes `[workspace.package].version` and `=…` pins for `buf-tools` / `buf-toolchain` in the root manifest.
fn write_workspace_version(path: &Path, new_ver: &str) {
    let _ = must_parse_version("manifest version", new_ver);

    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("xtask: read {}: {e}", path.display());
        exit(1);
    });
    let mut doc: DocumentMut = text.parse().unwrap_or_else(|e| {
        eprintln!("xtask: parse {}: {e}", path.display());
        exit(1);
    });

    doc["workspace"]["package"]["version"] = Item::Value(Value::from(new_ver));

    for pkg in ["buf-tools", "buf-toolchain"] {
        let item = &mut doc["workspace"]["dependencies"][pkg];
        match item {
            Item::Value(Value::InlineTable(t)) => {
                let pin = format!("={new_ver}");
                t.insert("version", Value::from(pin.as_str()));
            }
            _ => {
                eprintln!(
                    "xtask: expected workspace.dependencies.{pkg} to be an inline table with version"
                );
                exit(1);
            }
        }
    }

    fs::write(path, doc.to_string()).unwrap_or_else(|e| {
        eprintln!("xtask: write {}: {e}", path.display());
        exit(1);
    });
}

fn emit_verify_summary(crates_version: &str) {
    let v = must_parse_version("verify-summary --crates-version", crates_version);
    let buf_core = buf_upstream_core(&v);

    let pre = if v.pre.is_empty() {
        "(none)".to_string()
    } else {
        v.pre.to_string()
    };
    let build = if v.build.is_empty() {
        "(none)".to_string()
    } else {
        v.build.to_string()
    };

    println!("### Crates Resolved Version");
    println!("`{crates_version}`");
    println!("- **major:** {}", v.major);
    println!("- **minor:** {}", v.minor);
    println!("- **patch:** {}", v.patch);
    println!("- **pre-release:** {pre}");
    println!("- **build metadata:** {build}");
    println!();
    println!(
        "- **resolved buf for buf-tools:** `{buf_core}` (Buf GitHub tag `v{buf_core}`; same rule as `buf-tools/build.rs` — `CARGO_PKG_VERSION` parsed with `semver::Version`, then `major.minor.patch`)."
    );
    println!(
        "- **resolved buf for buf-toolchain:** `{buf_core}` (same rule as `buf-toolchain/build.rs`)."
    );
    println!();
    println!(
        "> Crate **pre-release** segments (e.g. `-rc.2`, `-dev.123`) are for buf-rs packaging only; they do **not** select a Buf pre-release. **`build.rs`** always downloads the stable Buf release **`v{buf_core}`** for that core."
    );
}

fn main() {
    let _ =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("xtask=warn"))
            .format_timestamp(None)
            .try_init();

    let cli = Cli::parse();
    let path = root_manifest();

    match cli.command {
        Command::ExpectedBufVersion => {
            let raw = read_workspace_version(&path);
            let core = semver_core(&raw);
            println!("{core}");
        }
        Command::Publish { cmd } => match cmd {
            PublishCmd::Resolve {
                channel,
                run_id,
                rc_number,
            } => {
                let flags = resolve_flags(channel, &run_id, &rc_number).unwrap_or_else(|e| {
                    eprintln!("xtask: {e}");
                    exit(1);
                });
                let raw = read_workspace_version(&path);
                match flags {
                    ResolvedPublishFlags::Stable => {
                        assert_stable_plain_semver(&raw);
                        println!("{raw}");
                    }
                    ResolvedPublishFlags::Dev { run_id } => {
                        let core = semver_core(&raw);
                        let out = format!("{core}-dev.{run_id}");
                        let _ = must_parse_version("resolve (dev)", &out);
                        println!("{out}");
                    }
                    ResolvedPublishFlags::Rc { rc_number: n } => {
                        let core = semver_core(&raw);
                        let out = format!("{core}-rc.{n}");
                        let _ = must_parse_version("resolve (rc)", &out);
                        println!("{out}");
                    }
                }
            }
            PublishCmd::ApplyVersion {
                channel,
                run_id,
                rc_number,
            } => {
                let flags = resolve_flags(channel, &run_id, &rc_number).unwrap_or_else(|e| {
                    eprintln!("xtask: {e}");
                    exit(1);
                });
                let raw = read_workspace_version(&path);
                let new_ver = match flags {
                    ResolvedPublishFlags::Stable => {
                        eprintln!("xtask: apply-version: channel must be dev or rc");
                        exit(1);
                    }
                    ResolvedPublishFlags::Dev { run_id } => {
                        let core = semver_core(&raw);
                        format!("{core}-dev.{run_id}")
                    }
                    ResolvedPublishFlags::Rc { rc_number: n } => {
                        let core = semver_core(&raw);
                        format!("{core}-rc.{n}")
                    }
                };
                let _ = must_parse_version("apply-version", &new_ver);
                write_workspace_version(&path, &new_ver);
                println!("{new_ver}");
            }
            PublishCmd::VerifySummary { crates_version } => {
                emit_verify_summary(&crates_version);
            }
        },
        Command::Workspace { cmd } => match cmd {
            WorkspaceCmd::SetBufVersion { version } => {
                assert_stable_plain_semver(&version);
                write_workspace_version(&path, &version);
                println!("{version}");
                eprintln!(
                    "xtask: wrote {} — next: cargo generate-lockfile",
                    path.display()
                );
                eprintln!("xtask: then:");
                eprintln!("  BUF_EXPECT_VERSION=\"$(cargo xtask expected-buf-version)\"");
                eprintln!("  echo \"Expected Buf Version: ${{BUF_EXPECT_VERSION}}\"");
                eprintln!("  cargo test --workspace --locked");
            }
        },
    }
}
