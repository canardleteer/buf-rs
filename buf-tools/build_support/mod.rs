//! Shared logic for `buf-tools` build script and `vendor-sync`.
#![allow(dead_code)]

pub mod config;
pub mod fetch;
pub mod layout;
pub mod lock;
pub mod source;
pub mod targets;
pub mod verify;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use targets::{ReleaseTarget, triples};
pub use verify::{
    BUF_MINISIGN_PUBLIC_KEY_B64, PREHASHED_MINISIGN_MIN_VERSION, parse_sha256_list, sha256_hex,
    verify_minisign_signature,
};

/// Cache layout: `<cache_root>/<semver-core>/<rust-target>/` where `cache_root` is `BUF_RS_CACHE_DIR` or `~/.cache/buf-tools`.
pub fn cache_slot(cache_root: &Path, semver_core: &str, rust_target: &str) -> PathBuf {
    cache_root.join(semver_core).join(rust_target)
}

pub fn write_executable(path: &Path, bytes: &[u8], windows: bool) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| "path has no file name".to_string())?;
    let tmp = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{}.part", name.to_string_lossy()));

    let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    f.write_all(bytes)
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    f.flush().ok();
    drop(f);

    #[cfg(unix)]
    if !windows {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&tmp, mode).map_err(|e| format!("chmod {}: {e}", tmp.display()))?;
    }

    fs::rename(&tmp, path).map_err(|e| format!("rename {:?} -> {:?}: {e}", tmp, path))?;
    Ok(())
}

/// On Unix builds (`windows == false`), set mode `0o755` on `path` so the file can be executed.
///
/// Cached release blobs are written with [`fs::write`], which typically yields `0o644`; symlink-based
/// layout modes point `bin/` entries at the cache file, so the cache inode must carry execute bits.
pub fn ensure_unix_executable(path: &Path, windows: bool) -> Result<(), String> {
    #[cfg(unix)]
    if !windows {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, mode).map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (path, windows);
    Ok(())
}

/// Ensure file at `dest` matches `expected_sha256` hex; otherwise remove.
pub fn verify_cached_file(dest: &Path, expected_sha256: &str) -> Result<bool, String> {
    if !dest.is_file() {
        return Ok(false);
    }
    let bytes = fs::read(dest).map_err(|e| format!("read {}: {e}", dest.display()))?;
    let got = sha256_hex(&bytes);
    if got == expected_sha256 {
        Ok(true)
    } else {
        fs::remove_file(dest).ok();
        Ok(false)
    }
}

pub fn target_supported(checksums: &HashMap<String, String>, t: &ReleaseTarget) -> bool {
    triples(t)
        .iter()
        .all(|(remote, _)| checksums.contains_key(remote))
}
