//! Platform cache and home directories without the `dirs` crate.
//!
//! `dirs` pulls `dirs-sys` → `option-ext` (MPL-2.0). These helpers use
//! `std::env::home_dir` (un-deprecated since Rust 1.85) plus XDG / known
//! environment variables.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// User home directory.
pub fn home_dir() -> Option<PathBuf> {
    env::home_dir()
}

/// User cache directory.
///
/// Order: non-empty `XDG_CACHE_HOME`; on Windows, non-empty `LOCALAPPDATA`;
/// otherwise `$HOME/Library/Caches` (macOS), `$HOME/AppData/Local` (Windows),
/// or `$HOME/.cache` (other Unix).
pub fn cache_dir() -> Option<PathBuf> {
    cache_dir_from(
        env::var_os("XDG_CACHE_HOME"),
        env::var_os("LOCALAPPDATA"),
        home_dir(),
    )
}

/// Directory that receives `buf` / `protoc-gen-buf-*` from buf-toolchain
/// `build.rs` (and that `validate-cargo-buf-toolchain` probes).
///
/// Order: non-empty `BUF_RS_TOOLCHAIN_BIN_DIR`; non-empty
/// `CARGO_INSTALL_ROOT` plus `bin` (Cargo `install.root`); `$CARGO_HOME/bin`;
/// `$HOME/.cargo/bin`.
///
/// `cargo install --root` is not visible to build scripts. Set
/// `CARGO_INSTALL_ROOT` or `BUF_RS_TOOLCHAIN_BIN_DIR` to isolate installs.
pub fn toolchain_bin_dir() -> (PathBuf, &'static str) {
    toolchain_bin_dir_from(
        env::var("BUF_RS_TOOLCHAIN_BIN_DIR").ok(),
        env::var("CARGO_INSTALL_ROOT").ok(),
        env::var("CARGO_HOME").ok(),
        home_dir(),
    )
}

fn nonempty_trimmed(v: Option<String>) -> Option<PathBuf> {
    v.map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn toolchain_bin_dir_from(
    toolchain_bin_dir: Option<String>,
    cargo_install_root: Option<String>,
    cargo_home: Option<String>,
    home: Option<PathBuf>,
) -> (PathBuf, &'static str) {
    if let Some(dir) = nonempty_trimmed(toolchain_bin_dir) {
        return (dir, "BUF_RS_TOOLCHAIN_BIN_DIR");
    }
    if let Some(root) = nonempty_trimmed(cargo_install_root) {
        return (root.join("bin"), "CARGO_INSTALL_ROOT");
    }
    if let Some(dir) = nonempty_trimmed(cargo_home) {
        return (dir.join("bin"), "CARGO_HOME");
    }
    match home {
        Some(h) => (h.join(".cargo").join("bin"), "default_home"),
        None => (PathBuf::from(".cargo").join("bin"), "default_home"),
    }
}

fn nonempty_path(v: Option<OsString>) -> Option<PathBuf> {
    v.filter(|s| !s.is_empty()).map(PathBuf::from)
}

fn cache_dir_from(
    xdg_cache_home: Option<OsString>,
    local_app_data: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(p) = nonempty_path(xdg_cache_home) {
        return Some(p);
    }
    #[cfg(windows)]
    if let Some(p) = nonempty_path(local_app_data) {
        return Some(p);
    }
    #[cfg(not(windows))]
    let _ = local_app_data;

    Some(cache_dir_under_home(home?))
}

fn cache_dir_under_home(home: PathBuf) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library").join("Caches")
    }
    #[cfg(windows)]
    {
        home.join("AppData").join("Local")
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        home.join(".cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn toolchain_bin_dir_prefers_explicit_override() {
        let (got, src) = toolchain_bin_dir_from(
            Some("/managed-bin".into()),
            Some("/install-root".into()),
            Some("/cargo-home".into()),
            Some(PathBuf::from("/home/me")),
        );
        assert_eq!(got, PathBuf::from("/managed-bin"));
        assert_eq!(src, "BUF_RS_TOOLCHAIN_BIN_DIR");
    }

    #[test]
    fn toolchain_bin_dir_uses_cargo_install_root_before_cargo_home() {
        let (got, src) = toolchain_bin_dir_from(
            Some("  ".into()),
            Some("/install-root".into()),
            Some("/cargo-home".into()),
            Some(PathBuf::from("/home/me")),
        );
        assert_eq!(got, PathBuf::from("/install-root/bin"));
        assert_eq!(src, "CARGO_INSTALL_ROOT");
    }

    #[test]
    fn toolchain_bin_dir_falls_back_to_cargo_home() {
        let (got, src) = toolchain_bin_dir_from(
            None,
            None,
            Some("/cargo-home".into()),
            Some(PathBuf::from("/home/me")),
        );
        assert_eq!(got, PathBuf::from("/cargo-home/bin"));
        assert_eq!(src, "CARGO_HOME");
    }

    #[test]
    fn xdg_cache_home_wins() {
        let got = cache_dir_from(
            Some("/tmp/xdg-cache".into()),
            Some("/win-local".into()),
            Some(PathBuf::from("/home/me")),
        );
        assert_eq!(got.as_deref(), Some(Path::new("/tmp/xdg-cache")));
    }

    #[test]
    fn empty_xdg_falls_through_to_home() {
        let got = cache_dir_from(Some("".into()), None, Some(PathBuf::from("/home/me")));
        assert_eq!(got, Some(cache_dir_under_home(PathBuf::from("/home/me"))));
    }

    #[test]
    fn missing_home_returns_none() {
        assert_eq!(cache_dir_from(None, None, None), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_localappdata_before_home() {
        let got = cache_dir_from(
            None,
            Some(r"C:\Users\me\AppData\Local".into()),
            Some(PathBuf::from(r"C:\Users\me")),
        );
        assert_eq!(
            got.as_deref(),
            Some(Path::new(r"C:\Users\me\AppData\Local"))
        );
    }
}
