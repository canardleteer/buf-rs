//! Canonically ordered repository checks.

use std::path::Path;

use anyhow::{Result, ensure};

use crate::process::{CommandSpec, require, run as run_command};
use crate::publish;
use crate::{CheckStep, FeatureArgs, policy};

const RUSTFMT_INSTALL: &str = "Install it with `rustup component add rustfmt`.";
const CLIPPY_INSTALL: &str = "Install it with `rustup component add clippy`.";

pub fn select_steps(only: &[CheckStep], exclude: &[CheckStep]) -> Result<Vec<CheckStep>> {
    let selected: Vec<_> = policy::CHECK_ORDER
        .iter()
        .copied()
        .filter(|step| {
            if only.is_empty() {
                !exclude.contains(step)
            } else {
                only.contains(step)
            }
        })
        .collect();
    ensure!(
        !selected.is_empty(),
        "the check selection is empty; choose at least one registered step"
    );
    Ok(selected)
}

pub fn run(root: &Path, steps: &[CheckStep], features: &FeatureArgs) -> Result<()> {
    require(
        root,
        "Cargo",
        &CommandSpec::new("cargo").arg("--version"),
        "Install a Rust toolchain from https://rustup.rs/.",
    )?;
    if steps.contains(&CheckStep::Fmt) {
        require(
            root,
            "rustfmt",
            &CommandSpec::new("cargo").args(["fmt", "--version"]),
            RUSTFMT_INSTALL,
        )?;
    }
    if steps.contains(&CheckStep::Clippy) {
        require(
            root,
            "Clippy",
            &CommandSpec::new("cargo").args(["clippy", "--version"]),
            CLIPPY_INSTALL,
        )?;
    }

    let buf_expect = publish::buf_expect_version_override(root)?;
    for step in steps {
        eprintln!("==> {step:?}");
        let command = command_for(*step, features, buf_expect.as_deref());
        run_command(root, &command)?;
    }
    Ok(())
}

fn command_for(
    step: CheckStep,
    features: &FeatureArgs,
    buf_expect_version: Option<&str>,
) -> CommandSpec {
    match step {
        CheckStep::Fmt => CommandSpec::new("cargo").args(["fmt", "--all", "--", "--check"]),
        CheckStep::Check => cargo_quality("check", features, true, None),
        CheckStep::Clippy => cargo_quality("clippy", features, true, None),
        CheckStep::Test => cargo_quality("test", features, false, buf_expect_version),
    }
}

fn cargo_quality(
    subcommand: &str,
    features: &FeatureArgs,
    all_targets: bool,
    buf_expect_version: Option<&str>,
) -> CommandSpec {
    let mut command = CommandSpec::new("cargo").args([subcommand, "--locked", "--workspace"]);
    if all_targets {
        command = command.arg("--all-targets");
    }
    command.args.extend(features.cargo_args());
    if let Some(version) = buf_expect_version {
        command = command.env("BUF_EXPECT_VERSION", version);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_deduplicated_and_canonical() {
        let selected = select_steps(&[CheckStep::Test, CheckStep::Fmt, CheckStep::Test], &[])
            .expect("selection should work");
        assert_eq!(selected, vec![CheckStep::Fmt, CheckStep::Test]);
    }

    #[test]
    fn exclusion_preserves_registry_order() {
        let selected = select_steps(&[], &[CheckStep::Clippy]).expect("selection should work");
        assert_eq!(
            selected,
            vec![CheckStep::Fmt, CheckStep::Check, CheckStep::Test]
        );
    }

    #[test]
    fn excluding_every_step_is_an_error() {
        assert!(select_steps(&[], &policy::CHECK_ORDER).is_err());
    }

    #[test]
    fn fmt_does_not_receive_feature_arguments() {
        let command = command_for(CheckStep::Fmt, &FeatureArgs::default(), None);
        assert!(!command.args.contains(&"--all-features".into()));
    }

    #[test]
    fn test_keeps_workspace_lockfile_flags_without_all_targets() {
        let command = command_for(CheckStep::Test, &FeatureArgs::default(), Some("1.73.0"));
        assert!(command.args.contains(&"--locked".into()));
        assert!(command.args.contains(&"--workspace".into()));
        assert!(!command.args.contains(&"--all-targets".into()));
        assert!(
            command
                .env
                .iter()
                .any(|(key, value)| key == "BUF_EXPECT_VERSION" && value == "1.73.0")
        );
    }

    #[test]
    fn clippy_matches_ci_surface_and_forwards_features() {
        let command = command_for(CheckStep::Clippy, &FeatureArgs::default(), None);
        assert!(command.args.contains(&"--locked".into()));
        assert!(command.args.contains(&"--workspace".into()));
        assert!(command.args.contains(&"--all-targets".into()));
        assert!(command.args.contains(&"--all-features".into()));
        assert!(!command.args.contains(&"-D".into()));
    }
}
