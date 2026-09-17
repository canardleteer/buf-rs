//! Publish helpers, Buf pin updates, and workspace version reads.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use semver::Version;
use toml_edit::{DocumentMut, Item, Value};

use crate::publish_inputs::{PublishChannel, ResolvedPublishFlags, resolve_flags};

#[derive(Debug, Subcommand)]
pub enum WorkspaceCmd {
    /// Set `[workspace.package].version` and `=X.Y.Z` pins for `buf-tools` / `buf-toolchain`.
    ///
    /// Confirm `https://github.com/bufbuild/buf/releases/tag/vX.Y.Z` exists before publishing.
    SetBufVersion {
        /// Plain semver `X.Y.Z` (no pre-release / build metadata).
        version: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PublishCmd {
    /// Print the version that would be published (no file writes).
    ///
    /// Pass `--run-id`, `--rc-number`, and `--hotfix-number` as empty strings when unused; see
    /// `publish_inputs::resolve_flags`.
    Resolve {
        #[arg(long)]
        channel: PublishChannel,
        #[arg(long, default_value = "")]
        run_id: String,
        #[arg(long, default_value = "")]
        rc_number: String,
        #[arg(long, default_value = "")]
        hotfix_number: String,
    },
    /// Set `[workspace.package].version` and workspace dependency pins (dev: `-dev.RUN_ID`, rc:
    /// `-rc.N`, hotfix: `-hotfix.N`).
    ApplyVersion {
        #[arg(long)]
        channel: PublishChannel,
        #[arg(long, default_value = "")]
        run_id: String,
        #[arg(long, default_value = "")]
        rc_number: String,
        #[arg(long, default_value = "")]
        hotfix_number: String,
    },
    /// Emit Markdown for `GITHUB_STEP_SUMMARY`: crate semver breakdown + resolved Buf versions.
    VerifySummary {
        #[arg(long)]
        crates_version: String,
    },
}

pub fn expected_buf_core(manifest: &Path) -> Result<String> {
    Ok(semver_core(&read_workspace_version(manifest)?))
}

/// When `BUF_EXPECT_VERSION` is unset, return the workspace Buf core so child Cargo
/// commands can inject it. When the caller already set the variable, inherit it.
pub fn buf_expect_version_override(root: &Path) -> Result<Option<String>> {
    if std::env::var_os("BUF_EXPECT_VERSION").is_some_and(|v| !v.is_empty()) {
        return Ok(None);
    }
    let core = expected_buf_core(&root.join("Cargo.toml"))?;
    eprintln!("Expected Buf Version: {core}");
    Ok(Some(core))
}

pub fn run_expected_buf_version(root: &Path) -> Result<()> {
    println!("{}", expected_buf_core(&root.join("Cargo.toml"))?);
    Ok(())
}

pub fn run_publish(root: &Path, cmd: PublishCmd) -> Result<()> {
    let path = root.join("Cargo.toml");
    match cmd {
        PublishCmd::Resolve {
            channel,
            run_id,
            rc_number,
            hotfix_number,
        } => {
            let flags = resolve_flags(channel, &run_id, &rc_number, &hotfix_number)
                .map_err(anyhow::Error::msg)?;
            let raw = read_workspace_version(&path)?;
            match flags {
                ResolvedPublishFlags::Stable => {
                    assert_stable_plain_semver(&raw)?;
                    println!("{raw}");
                }
                ResolvedPublishFlags::Dev { run_id } => {
                    let out = format!("{}-dev.{run_id}", semver_core(&raw));
                    must_parse_version("resolve (dev)", &out)?;
                    println!("{out}");
                }
                ResolvedPublishFlags::Rc { rc_number: n } => {
                    let out = format!("{}-rc.{n}", semver_core(&raw));
                    must_parse_version("resolve (rc)", &out)?;
                    println!("{out}");
                }
                ResolvedPublishFlags::Hotfix { hotfix_number: n } => {
                    let out = format!("{}-hotfix.{n}", semver_core(&raw));
                    must_parse_version("resolve (hotfix)", &out)?;
                    println!("{out}");
                }
            }
        }
        PublishCmd::ApplyVersion {
            channel,
            run_id,
            rc_number,
            hotfix_number,
        } => {
            let flags = resolve_flags(channel, &run_id, &rc_number, &hotfix_number)
                .map_err(anyhow::Error::msg)?;
            let raw = read_workspace_version(&path)?;
            let new_ver = match flags {
                ResolvedPublishFlags::Stable => {
                    bail!("apply-version: channel must be dev, rc, or hotfix");
                }
                ResolvedPublishFlags::Dev { run_id } => {
                    format!("{}-dev.{run_id}", semver_core(&raw))
                }
                ResolvedPublishFlags::Rc { rc_number: n } => {
                    format!("{}-rc.{n}", semver_core(&raw))
                }
                ResolvedPublishFlags::Hotfix { hotfix_number: n } => {
                    format!("{}-hotfix.{n}", semver_core(&raw))
                }
            };
            must_parse_version("apply-version", &new_ver)?;
            write_workspace_version(&path, &new_ver)?;
            println!("{new_ver}");
        }
        PublishCmd::VerifySummary { crates_version } => {
            emit_verify_summary(&crates_version)?;
        }
    }
    Ok(())
}

pub fn run_workspace(root: &Path, cmd: WorkspaceCmd) -> Result<()> {
    match cmd {
        WorkspaceCmd::SetBufVersion { version } => {
            let path = root.join("Cargo.toml");
            assert_stable_plain_semver(&version)?;
            write_workspace_version(&path, &version)?;
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
    }
    Ok(())
}

fn read_workspace_version(path: &Path) -> Result<String> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let doc: DocumentMut = text
        .parse()
        .with_context(|| format!("parse {}", path.display()))?;
    let v = doc["workspace"]["package"]["version"]
        .as_str()
        .with_context(|| format!("missing workspace.package.version in {}", path.display()))?;
    Ok(v.to_string())
}

fn semver_core(v: &str) -> String {
    let base = v.split('+').next().unwrap_or(v);
    base.split('-').next().unwrap_or(base).to_string()
}

fn assert_stable_plain_semver(raw: &str) -> Result<()> {
    let v = Version::parse(raw)
        .with_context(|| format!("stable channel: invalid semver in workspace {raw:?}"))?;
    ensure!(
        v.pre.is_empty() && v.build.is_empty(),
        "stable publish requires plain X.Y.Z (no pre-release or build metadata), got {raw:?}"
    );
    Ok(())
}

fn must_parse_version(label: &str, s: &str) -> Result<Version> {
    Version::parse(s).with_context(|| format!("{label}: invalid semver {s:?}"))
}

fn buf_upstream_core(v: &Version) -> String {
    format!("{}.{}.{}", v.major, v.minor, v.patch)
}

fn write_workspace_version(path: &Path, new_ver: &str) -> Result<()> {
    must_parse_version("manifest version", new_ver)?;

    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("parse {}", path.display()))?;

    doc["workspace"]["package"]["version"] = Item::Value(Value::from(new_ver));

    for pkg in ["buf-tools", "buf-toolchain"] {
        let item = &mut doc["workspace"]["dependencies"][pkg];
        match item {
            Item::Value(Value::InlineTable(t)) => {
                let pin = format!("={new_ver}");
                t.insert("version", Value::from(pin.as_str()));
            }
            _ => {
                bail!("expected workspace.dependencies.{pkg} to be an inline table with version");
            }
        }
    }

    fs::write(path, doc.to_string()).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn emit_verify_summary(crates_version: &str) -> Result<()> {
    let v = must_parse_version("verify-summary --crates-version", crates_version)?;
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
        "> Crate **pre-release** segments (e.g. `-rc.2`, `-dev.123`, `-hotfix.1`) are for buf-rs packaging only; they do **not** select a Buf pre-release. **`build.rs`** always downloads the stable Buf release **`v{buf_core}`** for that core."
    );
    Ok(())
}
