//! Runtime checks against GitHub releases and crates.io.

use std::collections::HashMap;
use std::env;
use std::path::Path;

use regex::Regex;
use semver::Version;
use serde_json::Value;

use crate::targets::{ReleaseTarget, triples};
use crate::verify::{
    BUF_MINISIGN_PUBLIC_KEY_B64, PREHASHED_MINISIGN_MIN_VERSION, parse_sha256_list, sha256_hex,
    verify_minisign_signature,
};

const USER_AGENT: &str = concat!(
    "validate-cargo-buf-toolchain/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/canardleteer/buf-rs)"
);

/// First `X.Y.Z` substring in `buf` `--version` stdout.
pub fn extract_installed_buf_core(stdout: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\d+\.\d+\.\d+").expect("semver regex"));
    re.find(stdout).map(|m| m.as_str().to_string())
}

fn http_get(url: &str, accept: Option<&str>) -> Result<Vec<u8>, String> {
    let mut req = ureq::get(url).header("User-Agent", USER_AGENT);
    if let Some(a) = accept {
        req = req.header("Accept", a);
    }
    req.call()
        .map_err(|e| format!("GET {url}: {e}"))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| format!("read body {url}: {e}"))
}

fn resolve_release_base(installed_core: &str) -> Result<String, String> {
    let default = format!("https://github.com/bufbuild/buf/releases/download/v{installed_core}/");
    let mut base = env::var("BUF_RS_RELEASE_BASE_URL").unwrap_or(default);
    base = base.trim().to_string();
    if base.is_empty() {
        return Err("BUF_RS_RELEASE_BASE_URL must not be empty when set".into());
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    Ok(base)
}

/// Download official `sha256.txt` + `.minisig`, verify signature, compare local files to manifest.
pub fn verify_binaries_against_github_release(
    bin_dir: &Path,
    rt: &ReleaseTarget,
    installed_core: &str,
) -> Result<(), String> {
    let base = resolve_release_base(installed_core)?;
    let sha256_txt = http_get(&format!("{base}sha256.txt"), None)?;
    let minisig = http_get(&format!("{base}sha256.txt.minisig"), None)?;
    let minisig_text = std::str::from_utf8(&minisig).map_err(|e| e.to_string())?;

    let core_ver =
        Version::parse(installed_core).map_err(|e| format!("parse installed version: {e}"))?;
    let prehashed_min =
        Version::parse(PREHASHED_MINISIGN_MIN_VERSION).map_err(|e| e.to_string())?;
    let allow_legacy = core_ver < prehashed_min;

    verify_minisign_signature(
        &sha256_txt,
        minisig_text,
        BUF_MINISIGN_PUBLIC_KEY_B64,
        allow_legacy,
    )?;

    let checksums: HashMap<String, String> = parse_sha256_list(&sha256_txt)?;
    if !target_supported(&checksums, rt) {
        return Err(format!(
            "sha256.txt for v{installed_core} does not list buf + protoc plugins for {}",
            rt.asset_suffix
        ));
    }

    for (remote, local_name) in triples(rt) {
        let path = bin_dir.join(&local_name);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let got = sha256_hex(&bytes);
        let expect = checksums
            .get(&remote)
            .ok_or_else(|| format!("manifest missing entry for {remote}"))?;
        if got != *expect {
            return Err(format!(
                "SHA256 mismatch for {local_name}: disk {got} != upstream manifest {expect} ({remote})"
            ));
        }
        println!("  {local_name:24}OK   matches GitHub sha256.txt ({remote})");
    }
    Ok(())
}

fn target_supported(checksums: &HashMap<String, String>, t: &ReleaseTarget) -> bool {
    triples(t)
        .iter()
        .all(|(remote, _)| checksums.contains_key(remote))
}

pub fn github_latest_buf_core() -> Result<String, String> {
    let url = "https://api.github.com/repos/bufbuild/buf/releases/latest";
    let body = http_get(url, Some("application/vnd.github+json"))?;
    let v: Value = serde_json::from_slice(&body).map_err(|e| format!("parse GitHub JSON: {e}"))?;
    let tag = v["tag_name"]
        .as_str()
        .ok_or_else(|| "GitHub response missing tag_name".to_string())?;
    Ok(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Returns `Ok(true)` if that exact version appears in the registry listing.
pub fn crates_io_has_buf_toolchain_version(target_ver: &str) -> Result<bool, String> {
    for page in 1..=50_u32 {
        let url = format!(
            "https://crates.io/api/v1/crates/buf-toolchain/versions?page={page}&per_page=100"
        );
        let body = http_get(&url, None)?;
        let v: Value =
            serde_json::from_slice(&body).map_err(|e| format!("parse crates.io: {e}"))?;
        if let Some(errs) = v["errors"].as_array()
            && !errs.is_empty()
        {
            return Ok(false);
        }
        let Some(versions) = v["versions"].as_array() else {
            break;
        };
        if versions.is_empty() {
            break;
        }
        for ver in versions {
            if ver["num"].as_str() == Some(target_ver) {
                return Ok(true);
            }
        }
        if versions.len() < 100 {
            break;
        }
    }
    Ok(false)
}

pub fn crates_io_buf_toolchain_exists() -> Result<bool, String> {
    let url = "https://crates.io/api/v1/crates/buf-toolchain";
    let body = http_get(url, None)?;
    let v: Value = serde_json::from_slice(&body).map_err(|e| format!("parse crates.io: {e}"))?;
    if v["errors"].is_array() {
        return Ok(false);
    }
    Ok(v["crate"].is_object())
}

/// Compare installed Buf to GitHub `latest` and report crates.io `buf-toolchain` availability.
pub fn report_newer_and_crates_io(installed_core: &str) {
    let Ok(latest) = github_latest_buf_core() else {
        println!("  (could not reach GitHub API for latest release)");
        return;
    };

    let Ok(installed_v) = Version::parse(installed_core) else {
        println!("  (could not parse installed version for comparison)");
        return;
    };
    let Ok(latest_v) = Version::parse(&latest) else {
        println!("  (could not parse GitHub tag as semver: {latest})");
        return;
    };

    if latest_v > installed_v {
        println!("  Latest Buf on GitHub: {latest} (you have {installed_core})");
        match crates_io_buf_toolchain_exists() {
            Ok(false) => println!(
                "  crates.io package `buf-toolchain`: not found (unpublished or different name)."
            ),
            Ok(true) => match crates_io_has_buf_toolchain_version(&latest) {
                Ok(true) => println!(
                    "  crates.io `buf-toolchain` {latest}: published — `cargo install buf-toolchain@{latest}` may work."
                ),
                Ok(false) => println!(
                    "  crates.io `buf-toolchain` {latest}: not published yet (no matching semver)."
                ),
                Err(e) => println!("  crates.io lookup failed: {e}"),
            },
            Err(e) => println!("  crates.io crate check failed: {e}"),
        }
    } else if latest_v == installed_v {
        println!("  GitHub `releases/latest` is v{latest} — same as your installed Buf.");
    } else {
        println!(
            "  GitHub `releases/latest` is v{latest}, older than your installed {installed_core} (unusual: pre-release install or API lag)."
        );
    }
}
