//! `cargo xtask` — workspace automation (see `.cargo/config.toml`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, Subcommand};
use toml_edit::{DocumentMut, Item, Value};

#[derive(Parser)]
#[command(name = "xtask", about = "buf-rs workspace tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Crates.io publish helpers (used by `.github/workflows/publish-crates.yml`).
    Publish {
        #[command(subcommand)]
        cmd: PublishCmd,
    },
}

#[derive(Subcommand)]
enum PublishCmd {
    /// Print the version that would be published (no file writes).
    Resolve {
        #[arg(long)]
        channel: PublishChannel,
        /// For prerelease; defaults to `GITHUB_RUN_ID`.
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Set `[workspace.package].version` and workspace dependency pins to `{core}-rc.RUN_ID`.
    ApplyPrerelease {
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Print Buf **core** `X.Y.Z` from the current `[workspace.package].version` (for `BUF_EXPECT_VERSION`).
    WorkspaceCore,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum PublishChannel {
    Prerelease,
    Stable,
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

fn is_stable_plain(v: &str) -> bool {
    if v.contains('+') || v.contains('-') {
        return false;
    }
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn apply_prerelease(path: &Path, run_id: &str) -> String {
    let raw = read_workspace_version(path);
    let core = semver_core(&raw);
    let new_ver = format!("{core}-rc.{run_id}");

    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("xtask: read {}: {e}", path.display());
        exit(1);
    });
    let mut doc: DocumentMut = text.parse().unwrap_or_else(|e| {
        eprintln!("xtask: parse {}: {e}", path.display());
        exit(1);
    });

    doc["workspace"]["package"]["version"] = Item::Value(Value::from(new_ver.as_str()));

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

    new_ver
}

fn main() {
    let cli = Cli::parse();
    let path = root_manifest();

    match cli.command {
        Command::Publish { cmd } => match cmd {
            PublishCmd::Resolve { channel, run_id } => {
                let raw = read_workspace_version(&path);
                match channel {
                    PublishChannel::Stable => {
                        if !is_stable_plain(&raw) {
                            eprintln!(
                                "stable publish requires plain X.Y.Z in [workspace.package].version, got {raw:?}"
                            );
                            exit(1);
                        }
                        println!("{raw}");
                    }
                    PublishChannel::Prerelease => {
                        let run_id = run_id
                            .or_else(|| std::env::var("GITHUB_RUN_ID").ok())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| {
                                eprintln!("prerelease: pass --run-id or set GITHUB_RUN_ID");
                                exit(1);
                            });
                        let core = semver_core(&raw);
                        println!("{core}-rc.{run_id}");
                    }
                }
            }
            PublishCmd::ApplyPrerelease { run_id } => {
                let run_id = run_id
                    .or_else(|| std::env::var("GITHUB_RUN_ID").ok())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| {
                        eprintln!("apply-prerelease: pass --run-id or set GITHUB_RUN_ID");
                        exit(1);
                    });
                let new_ver = apply_prerelease(&path, &run_id);
                println!("{new_ver}");
            }
            PublishCmd::WorkspaceCore => {
                let raw = read_workspace_version(&path);
                let core = semver_core(&raw);
                println!("{core}");
            }
        },
    }
}
