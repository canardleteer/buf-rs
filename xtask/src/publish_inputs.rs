//! Channel-scoped validation for `publish resolve` / `publish apply-version`.
//!
//! Callers (e.g. GitHub Actions) pass `--channel`, `--run-id`, and `--rc-number` every time; empty
//! strings mean “not provided”. Rules must stay aligned with [`.github/workflows/publish-crates.yml`](../../.github/workflows/publish-crates.yml).

use std::io::Write;

use clap::ValueEnum;

#[derive(Clone, Copy, ValueEnum)]
pub enum PublishChannel {
    Dev,
    Rc,
    Stable,
}

/// Normalized inputs after validating `--run-id` / `--rc-number` for the given channel.
pub enum ResolvedPublishFlags {
    Stable,
    Dev { run_id: String },
    Rc { rc_number: u32 },
}

/// Validate flags for `resolve` / `apply-version`.
///
/// - **stable:** `--run-id` and `--rc-number` must be empty (after trim).
/// - **dev:** `--rc-number` must be empty; `--run-id` non-empty, or `GITHUB_RUN_ID` set, or (when
///   `GITHUB_ACTIONS` is not `true`) a random synthetic run id is used with `log::warn!` and an
///   optional `GITHUB_STEP_SUMMARY` append — for local runs without CI env.
/// - **rc:** `--run-id` must be empty; `--rc-number` required, parseable as `u32` > 0.
pub fn resolve_flags(
    channel: PublishChannel,
    run_id: &str,
    rc_number: &str,
) -> Result<ResolvedPublishFlags, String> {
    let mut github_run_id = std::env::var("GITHUB_RUN_ID")
        .ok()
        .filter(|s| !s.trim().is_empty());

    if matches!(channel, PublishChannel::Dev) && run_id.trim().is_empty() && github_run_id.is_none()
    {
        let in_github_actions = std::env::var("GITHUB_ACTIONS").ok().as_deref() == Some("true");
        if !in_github_actions {
            let synthetic = fastrand::u64(..);
            let summary = format!(
                "> **xtask (dev publish):** synthetic run id `{synthetic}` — `GITHUB_RUN_ID` was unset and this does not appear to be a GitHub Actions environment (`GITHUB_ACTIONS` is not `true`). Continuing."
            );
            log::warn!(target: "xtask::publish", "{summary}");
            append_github_step_summary_line(&summary);
            github_run_id = Some(synthetic.to_string());
        }
    }

    resolve_flags_with_github_run_id(channel, run_id, rc_number, github_run_id)
}

fn append_github_step_summary_line(line: &str) {
    let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return;
    };
    if path.is_empty() {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| writeln!(f, "{line}"));
}

fn resolve_flags_with_github_run_id(
    channel: PublishChannel,
    run_id: &str,
    rc_number: &str,
    github_run_id: Option<String>,
) -> Result<ResolvedPublishFlags, String> {
    let run = run_id.trim();
    let rc = rc_number.trim();
    let run_nonempty = !run.is_empty();
    let rc_nonempty = !rc.is_empty();

    match channel {
        PublishChannel::Stable => {
            if run_nonempty || rc_nonempty {
                return Err(
                    "stable channel: omit non-empty --run-id and --rc-number (pass empty strings from CI)"
                        .to_string(),
                );
            }
            Ok(ResolvedPublishFlags::Stable)
        }
        PublishChannel::Dev => {
            if rc_nonempty {
                return Err("dev channel: do not pass --rc-number".to_string());
            }
            let rid = if run_nonempty {
                run.to_string()
            } else {
                github_run_id.ok_or_else(|| {
                    "dev channel: pass non-empty --run-id or set GITHUB_RUN_ID".to_string()
                })?
            };
            if rid.trim().is_empty() {
                return Err("dev channel: pass non-empty --run-id or set GITHUB_RUN_ID".to_string());
            }
            Ok(ResolvedPublishFlags::Dev { run_id: rid })
        }
        PublishChannel::Rc => {
            if run_nonempty {
                return Err("rc channel: do not pass --run-id".to_string());
            }
            if !rc_nonempty {
                return Err("rc channel: pass --rc-number with a positive integer".to_string());
            }
            if !rc.chars().all(|c| c.is_ascii_digit()) || rc.starts_with('0') {
                return Err(format!(
                    "rc channel: --rc-number must be a positive integer (digits only, no leading zeros), got {rc:?}"
                ));
            }
            let n: u32 = rc
                .parse()
                .map_err(|_| format!("rc channel: invalid --rc-number {rc:?}"))?;
            if n == 0 {
                return Err("rc channel: --rc-number must be > 0".to_string());
            }
            Ok(ResolvedPublishFlags::Rc { rc_number: n })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PublishChannel, ResolvedPublishFlags, resolve_flags};

    fn dev_ok(run_id: &str, rc: &str) {
        let r = resolve_flags(PublishChannel::Dev, run_id, rc).unwrap();
        match r {
            ResolvedPublishFlags::Dev { run_id: id } => assert_eq!(id, run_id.trim()),
            _ => panic!("expected Dev"),
        }
    }

    #[test]
    fn stable_accepts_empty_flags() {
        assert!(matches!(
            resolve_flags(PublishChannel::Stable, "", "").unwrap(),
            super::ResolvedPublishFlags::Stable
        ));
        assert!(matches!(
            resolve_flags(PublishChannel::Stable, "  ", "  ").unwrap(),
            super::ResolvedPublishFlags::Stable
        ));
    }

    #[test]
    fn stable_rejects_nonempty_run_id() {
        assert!(resolve_flags(PublishChannel::Stable, "1", "").is_err());
    }

    #[test]
    fn stable_rejects_nonempty_rc() {
        assert!(resolve_flags(PublishChannel::Stable, "", "2").is_err());
    }

    #[test]
    fn dev_accepts_run_id_rejects_rc() {
        dev_ok("42", "");
        assert!(resolve_flags(PublishChannel::Dev, "42", "1").is_err());
        assert!(resolve_flags(PublishChannel::Dev, "42", " ").is_ok());
    }

    #[test]
    fn dev_rejects_empty_run_id_without_env() {
        let res = super::resolve_flags_with_github_run_id(PublishChannel::Dev, "", "", None);
        assert!(res.is_err());
    }

    #[test]
    fn rc_accepts_positive_integer_rejects_run_id() {
        let r = resolve_flags(PublishChannel::Rc, "", "3").unwrap();
        match r {
            super::ResolvedPublishFlags::Rc { rc_number } => assert_eq!(rc_number, 3),
            _ => panic!("expected Rc"),
        }
        assert!(resolve_flags(PublishChannel::Rc, "9", "3").is_err());
    }

    #[test]
    fn rc_rejects_missing_or_zero() {
        assert!(resolve_flags(PublishChannel::Rc, "", "").is_err());
        assert!(resolve_flags(PublishChannel::Rc, "", "0").is_err());
        assert!(resolve_flags(PublishChannel::Rc, "", "bogus").is_err());
    }

    #[test]
    fn rc_rejects_leading_zeros() {
        assert!(resolve_flags(PublishChannel::Rc, "", "01").is_err());
    }

    #[test]
    fn dev_empty_run_id_outside_github_actions_uses_synthetic_run_id() {
        temp_env::with_vars(
            [
                ("GITHUB_RUN_ID", None::<&str>),
                ("GITHUB_ACTIONS", None::<&str>),
            ],
            || {
                let r = resolve_flags(PublishChannel::Dev, "", "").unwrap();
                let ResolvedPublishFlags::Dev { run_id } = r else {
                    panic!("expected Dev");
                };
                assert!(run_id.parse::<u64>().is_ok());
            },
        );
    }

    #[test]
    fn dev_empty_run_id_inside_github_actions_without_run_id_fails() {
        temp_env::with_vars(
            [
                ("GITHUB_RUN_ID", None::<&str>),
                ("GITHUB_ACTIONS", Some("true")),
            ],
            || {
                assert!(resolve_flags(PublishChannel::Dev, "", "").is_err());
            },
        );
    }

    #[test]
    fn dev_step_summary_appends_synthetic_note_when_set() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "buf-rs-xtask-step-summary-{}.md",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        temp_env::with_vars(
            [
                ("GITHUB_RUN_ID", None::<&str>),
                ("GITHUB_ACTIONS", None::<&str>),
                ("GITHUB_STEP_SUMMARY", Some(path.to_str().unwrap())),
            ],
            || {
                let _ = resolve_flags(PublishChannel::Dev, "", "").unwrap();
            },
        );
        let text = std::fs::read_to_string(&path).expect("summary file");
        assert!(
            text.contains("synthetic run id"),
            "expected synthetic note in summary, got: {text:?}"
        );
        let _ = std::fs::remove_file(&path);
    }
}
