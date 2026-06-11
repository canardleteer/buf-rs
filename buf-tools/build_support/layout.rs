//! Target/profile layout root resolution for non-default `BUF_RS_LAYOUT_MODE` values.

use std::path::{Path, PathBuf};

/// Landing-pad root for non-`cache` layout modes: `<base>/buf-tools/<core>/<TARGET>/`.
///
/// Resolution order for `<base>`:
/// 1. `CARGO_TARGET_DIR` when set
/// 2. nearest `OUT_DIR` ancestor named `target`
/// 3. parent of nearest `OUT_DIR` ancestor named `build` (cargo-install temp trees)
pub fn resolve_target_layout_root(
    out_dir: &Path,
    cargo_target_dir: Option<&Path>,
    core: &str,
    target_triple: &str,
) -> Result<PathBuf, String> {
    let base = resolve_layout_base(out_dir, cargo_target_dir)?;
    Ok(base.join("buf-tools").join(core).join(target_triple))
}

fn resolve_layout_base(out_dir: &Path, cargo_target_dir: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(dir) = cargo_target_dir.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(dir.to_path_buf());
    }

    if let Some(target_dir) = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "target"))
    {
        return Ok(target_dir.to_path_buf());
    }

    if let Some(build_dir) = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "build"))
        && let Some(parent) = build_dir.parent()
    {
        return Ok(parent.to_path_buf());
    }

    Err(
        "buf-tools: could not locate layout root from OUT_DIR for non-cache layout mode; \
         set BUF_RS_LAYOUT_MODE=cache, or set CARGO_TARGET_DIR to a writable directory"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: &str = "1.40.0";
    const TRIPLE: &str = "x86_64-unknown-linux-gnu";

    fn expected(base: &str) -> PathBuf {
        PathBuf::from(base)
            .join("buf-tools")
            .join(CORE)
            .join(TRIPLE)
    }

    #[test]
    fn workspace_target_debug_build_out() {
        let out = Path::new("/home/proj/target/debug/build/pkg-hash/out");
        let got = resolve_target_layout_root(out, None, CORE, TRIPLE).expect("resolve");
        assert_eq!(got, expected("/home/proj/target"));
    }

    #[test]
    fn cargo_install_release_build_out() {
        let out = Path::new("/tmp/cargo-install-abc/release/build/pkg-hash/out");
        let got = resolve_target_layout_root(out, None, CORE, TRIPLE).expect("resolve");
        assert_eq!(got, expected("/tmp/cargo-install-abc/release"));
    }

    #[test]
    fn custom_profile_build_out() {
        let out = Path::new("/tmp/cargo-install-abc/foo/build/pkg-hash/out");
        let got = resolve_target_layout_root(out, None, CORE, TRIPLE).expect("resolve");
        assert_eq!(got, expected("/tmp/cargo-install-abc/foo"));
    }

    #[test]
    fn cargo_target_dir_overrides_ancestors() {
        let out = Path::new("/tmp/cargo-install-abc/release/build/pkg-hash/out");
        let custom = Path::new("/custom/out");
        let got = resolve_target_layout_root(out, Some(custom), CORE, TRIPLE).expect("resolve");
        assert_eq!(got, expected("/custom/out"));
    }

    #[test]
    fn missing_roots_returns_actionable_error() {
        let out = Path::new("/orphan/out");
        let err = resolve_target_layout_root(out, None, CORE, TRIPLE).unwrap_err();
        assert!(err.contains("BUF_RS_LAYOUT_MODE=cache"));
        assert!(err.contains("CARGO_TARGET_DIR"));
    }
}
