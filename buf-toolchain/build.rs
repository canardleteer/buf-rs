//! Download and install official Buf release artifacts (see crate README).

#[path = "build_support/mod.rs"]
mod build_support;

use std::env;
use std::fs;
use std::path::PathBuf;

use semver::Version;

use build_support::lock::SlotLockState;
use build_support::targets::from_rust_triple;
use build_support::{
    BUF_MINISIGN_PUBLIC_KEY_B64, PREHASHED_MINISIGN_MIN_VERSION, cache_slot,
    ensure_unix_executable, fetch, parse_sha256_list, sha256_hex, target_supported, triples,
    verify_cached_file, verify_minisign_signature, write_executable,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=BUF_RS_TOOLCHAIN_BIN_DIR");
    println!("cargo:rerun-if-env-changed=BUF_RS_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=BUF_RS_RELEASE_BASE_URL");
    println!("cargo:rerun-if-env-changed=CARGO_HOME");
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");

    let target_triple = env::var("TARGET")?;
    let pkg_version = env::var("CARGO_PKG_VERSION")?;
    let ver = Version::parse(&pkg_version)?;
    let core = format!("{}.{}.{}", ver.major, ver.minor, ver.patch);
    let core_ver = Version::parse(&core)?;
    let rt = from_rust_triple(&target_triple)
        .ok_or_else(|| format!("buf-toolchain: unsupported target `{target_triple}`"))?;
    let min = Version::parse(rt.min_version)?;
    if core_ver < min {
        return Err(format!(
            "buf-toolchain: target `{}` requires Buf >= {} but crate is pinned to {}",
            rt.asset_suffix, min, core
        )
        .into());
    }

    let canonical_bin_dir = resolve_canonical_bin_dir()?;
    fs::create_dir_all(&canonical_bin_dir)?;

    let cache_root = resolve_cache_root()?;
    let cache_slot = cache_slot(&cache_root, &core, &target_triple);
    fs::create_dir_all(&cache_slot)?;

    let release_base = resolve_base_url(
        "BUF_RS_RELEASE_BASE_URL",
        &format!("https://github.com/bufbuild/buf/releases/download/v{core}/"),
    )?;
    println!(
        "cargo:warning=buf-toolchain: selected target {} (asset suffix {})",
        target_triple, rt.asset_suffix
    );
    println!(
        "cargo:warning=buf-toolchain: cache slot {}",
        cache_slot.display()
    );
    println!("cargo:warning=buf-toolchain: release base {}", release_base);
    println!(
        "cargo:warning=buf-toolchain: canonical bin dir {}",
        canonical_bin_dir.display()
    );

    let mut lock_warn = |msg: String| emit_toolchain_warn(msg);
    let slot_lock =
        match build_support::lock::acquire_or_wait_for_slot(&cache_slot, &mut lock_warn)? {
            SlotLockState::Acquired(guard) => Some(guard),
            SlotLockState::WaitedForOtherWriter => None,
        };

    let tag = format!("v{core}");
    let offline = env::var_os("CARGO_NET_OFFLINE").is_some();

    let sha256_txt = fetch::download(&format!("{release_base}sha256.txt"))?;
    let minisig = fetch::download(&format!("{release_base}sha256.txt.minisig"))?;
    let minisig_text = std::str::from_utf8(&minisig)?;
    let prehashed_min = Version::parse(PREHASHED_MINISIGN_MIN_VERSION)?;
    let allow_legacy = core_ver < prehashed_min;
    verify_minisign_signature(
        &sha256_txt,
        minisig_text,
        BUF_MINISIGN_PUBLIC_KEY_B64,
        allow_legacy,
    )?;
    let checksums = parse_sha256_list(&sha256_txt)?;

    if !target_supported(&checksums, &rt) {
        return Err(format!(
            "buf-toolchain: Buf release {tag} does not list buf + protoc-gen-buf-* binaries for this platform ({})",
            rt.asset_suffix
        )
        .into());
    }

    let mut installed: Vec<String> = Vec::new();
    let mut fetch_warn = |msg: String| emit_toolchain_warn(msg);

    for (remote_name, local_name) in triples(&rt) {
        let expected_hex = checksums.get(&remote_name).ok_or_else(|| {
            format!("buf-toolchain: missing {remote_name} in sha256.txt (unexpected after target_supported)")
        })?;

        let cache_file = cache_slot.join(&remote_name);
        let bytes = if verify_cached_file(&cache_file, expected_hex)? {
            println!(
                "cargo:warning=buf-toolchain: using cached {} (sha256 OK)",
                remote_name
            );
            ensure_unix_executable(&cache_file, rt.windows)?;
            fs::read(&cache_file)?
        } else {
            if offline {
                return Err(format!(
                    "buf-toolchain: CARGO_NET_OFFLINE set but cache miss for {} at {}",
                    remote_name,
                    cache_file.display()
                )
                .into());
            }
            let url = format!("{release_base}{remote_name}");
            let b = fetch::download_streaming_with_progress(&url, &remote_name, &mut fetch_warn)?;
            if sha256_hex(&b) != *expected_hex {
                return Err(format!(
                    "buf-toolchain: sha256 mismatch for {} (expected {}, got {})",
                    remote_name,
                    expected_hex,
                    sha256_hex(&b)
                )
                .into());
            }
            fs::write(&cache_file, &b)?;
            ensure_unix_executable(&cache_file, rt.windows)?;
            b
        };

        let dest = canonical_bin_dir.join(&local_name);
        write_executable(&dest, &bytes, rt.windows)?;
        installed.push(local_name);
    }

    println!(
        "cargo:warning=buf-toolchain: installed {} binary(ies) into {}",
        installed.len(),
        canonical_bin_dir.display()
    );
    if !installed.is_empty() {
        println!(
            "cargo:warning=buf-toolchain: installed: {}",
            installed.join(", ")
        );
    }
    drop(slot_lock);

    Ok(())
}

fn emit_toolchain_warn(msg: String) {
    let line = if let Some(rest) = msg.strip_prefix("buf-tools:") {
        format!("buf-toolchain:{rest}")
    } else {
        format!("buf-toolchain: {msg}")
    };
    println!("cargo:warning={}", line);
}

/// Install destination: `BUF_RS_TOOLCHAIN_BIN_DIR`, else `$CARGO_HOME/bin`.
fn resolve_canonical_bin_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("BUF_RS_TOOLCHAIN_BIN_DIR") {
        let d = dir.trim();
        if !d.is_empty() {
            return Ok(PathBuf::from(d));
        }
    }
    Ok(cargo_home_dir()?.join("bin"))
}

fn resolve_cache_root() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("BUF_RS_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::cache_dir()
        .ok_or_else(|| "buf-toolchain: cannot resolve cache dir".to_string())
        .map(|p| p.join("buf-toolchain"))
}

fn cargo_home_dir() -> Result<PathBuf, String> {
    if let Ok(home) = env::var("CARGO_HOME") {
        return Ok(PathBuf::from(home));
    }
    let home = dirs::home_dir().ok_or_else(|| "buf-toolchain: cannot resolve HOME".to_string())?;
    Ok(home.join(".cargo"))
}

fn resolve_base_url(name: &str, default: &str) -> Result<String, String> {
    let mut base = env::var(name).unwrap_or_else(|_| default.to_string());
    base = base.trim().to_string();
    if base.is_empty() {
        return Err(format!("buf-toolchain: {name} must not be empty"));
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    Ok(base)
}
