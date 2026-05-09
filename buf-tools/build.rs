//! Download Buf release artifacts into `OUT_DIR`, with persistent cache.

#[path = "build_support/mod.rs"]
mod build_support;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use semver::Version;

use build_support::targets::from_rust_triple;
use build_support::{
    BUF_MINISIGN_PUBLIC_KEY_B64, PREHASHED_MINISIGN_MIN_VERSION, cache_slot, config,
    ensure_unix_executable, fetch, lock::SlotLockState, parse_sha256_list, sha256_hex, source,
    target_supported, triples, verify_cached_file, verify_minisign_signature, write_executable,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=BUF_RS_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=BUF_RS_INCLUDE_SOURCE");
    println!("cargo:rerun-if-env-changed=BUF_RS_RELEASE_BASE_URL");
    println!("cargo:rerun-if-env-changed=BUF_RS_SOURCE_BASE_URL");
    println!("cargo:rerun-if-env-changed=BUF_RS_LAYOUT_MODE");
    println!("cargo:rerun-if-env-changed=BUF_RS_BUILD_LOG");
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let target_triple = env::var("TARGET")?;
    let pkg_version = env::var("CARGO_PKG_VERSION")?;

    if env::var_os("DOCS_RS").is_some() {
        write_docs_rs_stubs(&out_dir, target_triple.contains("windows"))?;
        print_rustc_env_paths(&out_dir, target_triple.contains("windows"), None)?;
        return Ok(());
    }

    let ver = Version::parse(&pkg_version)?;
    let core = format!("{}.{}.{}", ver.major, ver.minor, ver.patch);
    let core_ver = Version::parse(&core)
        .map_err(|e| format!("buf-tools: invalid semver core in CARGO_PKG_VERSION: {e}"))?;

    let rt =
        from_rust_triple(&target_triple).ok_or_else(|| {
            format!(
                "buf-tools: unsupported compilation TARGET `{target_triple}`. See crate README for supported triples."
            )
        })?;

    let min = Version::parse(rt.min_version)
        .map_err(|e| format!("buf-tools: bad min_version for {}: {e}", rt.asset_suffix))?;
    if core_ver < min {
        return Err(format!(
            "buf-tools: target `{}` requires Buf >= {} but crate is pinned to {} \
             (Buf {} did not ship binaries for this platform). \
             Either pin a newer crate version or compile for a different target.",
            rt.asset_suffix, min, ver, core
        )
        .into());
    }

    let cfg = config::resolve(&out_dir);
    if let Some(p) = &cfg.workspace_manifest {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    if let Some(p) = &cfg.package_manifest {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    let build_log_level = build_log_level(cfg.build_log.as_deref())?;
    let mut edge_warn = |msg: String| {
        if matches!(
            build_log_level,
            BuildLogLevel::Warn | BuildLogLevel::Verbose
        ) {
            println!("cargo:warning={msg}");
        }
    };
    let mut info_warn = |msg: String| {
        if matches!(build_log_level, BuildLogLevel::Verbose) {
            println!("cargo:warning={msg}");
        }
    };
    emit_config_source_trace(&cfg, &mut info_warn);

    let layout_mode = layout_mode(cfg.layout_mode.as_deref())?;
    log_layout_mode(&layout_mode, cfg.layout_mode.as_deref(), &mut info_warn);
    let target_layout_root = resolve_target_layout_root(&out_dir, &core, &target_triple)?;
    let mode_cache_root = match layout_mode {
        LayoutMode::Target => target_layout_root.join("cache"),
        _ => {
            let cache_root = cache_root_dir(cfg.cache_dir.as_deref())?;
            cache_slot(&cache_root, &core, &target_triple)
        }
    };
    let slot = mode_cache_root;
    fs::create_dir_all(&slot)?;

    let tag = format!("v{core}");
    let release_base = resolve_base_url(
        cfg.release_base_url.as_deref(),
        "BUF_RS_RELEASE_BASE_URL",
        &format!("https://github.com/bufbuild/buf/releases/download/{tag}/"),
    )?;
    let source_base = resolve_base_url(
        cfg.source_base_url.as_deref(),
        "BUF_RS_SOURCE_BASE_URL",
        "https://github.com/bufbuild/buf/archive/refs/tags/",
    )?;

    let offline = env::var_os("CARGO_NET_OFFLINE").is_some();
    info_warn(format!(
        "buf-tools: selected target {} (asset suffix {})",
        target_triple, rt.asset_suffix
    ));
    info_warn(format!("buf-tools: cache slot {}", slot.display()));
    info_warn(format!("buf-tools: release base {}", release_base));
    info_warn(format!("buf-tools: source base {}", source_base));
    match layout_mode {
        LayoutMode::Cache => info_warn("buf-tools: info: using default cache mode".to_string()),
        LayoutMode::CacheLink => info_warn(format!(
            "buf-tools: info: BUF_RS_LAYOUT_MODE=cache-link; linking/copying binaries into {}",
            target_layout_root.join("bin").display()
        )),
        LayoutMode::CacheVerifiedLink => info_warn(format!(
            "buf-tools: info: BUF_RS_LAYOUT_MODE=cache-verified-link; re-verifying cache before link/copy into {}",
            target_layout_root.join("bin").display()
        )),
        LayoutMode::Target => info_warn(format!(
            "buf-tools: info: BUF_RS_LAYOUT_MODE=target; using target-scoped cache/landing at {}",
            target_layout_root.display()
        )),
    }

    let slot_lock = match build_support::lock::acquire_or_wait_for_slot(&slot, &mut edge_warn)? {
        SlotLockState::Acquired(guard) => Some(guard),
        SlotLockState::WaitedForOtherWriter => None,
    };

    let sha256_url = format!("{release_base}sha256.txt");
    let minisig_url = format!("{release_base}sha256.txt.minisig");

    let sha256_txt = fetch::download(&sha256_url)?;
    let minisig = fetch::download(&minisig_url)?;
    let minisig_text = std::str::from_utf8(&minisig)?;
    let prehashed_min = Version::parse(PREHASHED_MINISIGN_MIN_VERSION)
        .map_err(|e| format!("buf-tools: invalid PREHASHED_MINISIGN_MIN_VERSION: {e}"))?;
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
            "buf-tools: Buf release {tag} does not list binaries for this platform ({})",
            rt.asset_suffix
        )
        .into());
    }

    let bin_dir = match layout_mode {
        LayoutMode::Cache => out_dir.join("bin"),
        _ => target_layout_root.join("bin"),
    };
    fs::create_dir_all(&bin_dir)?;

    for (remote_name, local_name) in triples(&rt) {
        let expected_hex = checksums
            .get(&remote_name)
            .ok_or_else(|| format!("missing {remote_name} in sha256.txt"))?;
        let cache_file = slot.join(&remote_name);
        let url = format!("{release_base}{remote_name}");

        let bytes = if verify_cached_file(&cache_file, expected_hex)? {
            info_warn(format!(
                "buf-tools: using cached {} (sha256 OK)",
                remote_name
            ));
            fs::read(&cache_file)?
        } else {
            if offline {
                return Err(format!(
                    "buf-tools: CARGO_NET_OFFLINE set but cache miss for {} — populate {} or clear offline mode",
                    remote_name,
                    cache_file.display()
                )
                .into());
            }
            let b = fetch::download_streaming_with_progress(&url, &remote_name, &mut info_warn)?;
            if sha256_hex(&b) != *expected_hex {
                return Err(format!(
                    "SHA256 mismatch for {remote_name}: expected {expected_hex}, got {}",
                    sha256_hex(&b)
                )
                .into());
            }
            fs::write(&cache_file, &b)?;
            ensure_unix_executable(&cache_file, rt.windows)?;
            b
        };

        let dest = bin_dir.join(&local_name);
        match layout_mode {
            LayoutMode::Cache | LayoutMode::Target => write_executable(&dest, &bytes, rt.windows)?,
            LayoutMode::CacheLink | LayoutMode::CacheVerifiedLink => {
                link_or_copy_cache_artifact(&cache_file, &dest, rt.windows, &mut edge_warn)?;
            }
        }
    }

    let mut source_root: Option<PathBuf> = None;
    if parse_truthy(&env::var("BUF_RS_INCLUDE_SOURCE").unwrap_or_default()) {
        if offline && source_bundle_ready(&slot, &core).is_none() {
            return Err(
                "buf-tools: BUF_RS_INCLUDE_SOURCE set but offline and source bundle not cached"
                    .into(),
            );
        }
        source_root = Some(fetch_optional_source(
            &slot,
            &core,
            &tag,
            &source_base,
            offline,
            &mut info_warn,
        )?);
    }

    print_layout_mode_metadata(&layout_mode, &target_layout_root)?;
    print_rustc_env_paths(&bin_dir, rt.windows, source_root.as_ref())?;
    drop(slot_lock);

    Ok(())
}

fn parse_truthy(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    matches!(s.as_str(), "1" | "true" | "yes")
}

fn source_bundle_ready(slot: &Path, core: &str) -> Option<PathBuf> {
    let root = slot.join(format!("buf-{core}"));
    if root.is_dir() { Some(root) } else { None }
}

fn fetch_optional_source(
    slot: &Path,
    core: &str,
    tag: &str,
    source_base: &str,
    offline: bool,
    warn: &mut dyn FnMut(String),
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let url = format!("{source_base}{tag}.tar.gz");
    let archive_path = slot.join(format!("buf-upstream-{core}.tar.gz"));
    let extract_parent = slot.join("upstream-src");

    let expected_root = extract_parent.join(format!("buf-{core}"));

    if expected_root.is_dir() {
        warn(format!(
            "buf-tools: using cached extracted source at {}",
            expected_root.display()
        ));
        return Ok(expected_root);
    }

    if !archive_path.is_file() {
        if offline {
            return Err(format!("missing source archive {}", archive_path.display()).into());
        }
        let bytes = fetch::download_streaming_with_progress(&url, "upstream source tarball", warn)?;
        fs::write(&archive_path, &bytes)?;
    } else {
        warn(format!(
            "buf-tools: using cached source archive {}",
            archive_path.display()
        ));
    }

    if extract_parent.exists() {
        fs::remove_dir_all(&extract_parent)?;
    }
    source::extract_tar_gz(&archive_path, &extract_parent)?;

    if !expected_root.is_dir() {
        return Err(format!(
            "buf-tools: extracted source layout unexpected — missing {}",
            expected_root.display()
        )
        .into());
    }

    warn(format!(
        "buf-tools: extracted upstream source at {}",
        expected_root.display()
    ));
    Ok(expected_root)
}

fn cache_root_dir(override_value: Option<&str>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(p) = override_value {
        let pb = PathBuf::from(p);
        fs::create_dir_all(&pb)?;
        return Ok(pb);
    }
    let base = dirs::cache_dir().ok_or("could not resolve cache dir (set BUF_RS_CACHE_DIR)")?;
    Ok(base.join("buf-tools"))
}

fn resolve_base_url(
    value: Option<&str>,
    name: &str,
    default: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut base = value.unwrap_or(default).to_string();
    base = base.trim().to_string();
    if base.is_empty() {
        return Err(format!("buf-tools: {name} must not be empty").into());
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    Ok(base)
}

fn write_docs_rs_stubs(out_dir: &Path, windows: bool) -> Result<(), Box<dyn std::error::Error>> {
    let bin_dir = out_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;
    let names: [&str; 3] = if windows {
        [
            "buf.exe",
            "protoc-gen-buf-breaking.exe",
            "protoc-gen-buf-lint.exe",
        ]
    } else {
        ["buf", "protoc-gen-buf-breaking", "protoc-gen-buf-lint"]
    };
    let stub = stub_payload(windows);
    for n in names {
        fs::write(bin_dir.join(n), &stub)?;
    }
    Ok(())
}

fn stub_payload(windows: bool) -> Vec<u8> {
    let mut v = Vec::new();
    if windows {
        v.extend_from_slice(b"MZ");
    } else {
        v.extend_from_slice(&[0x7f, b'E', b'L', b'F']);
    }
    v.resize(12_000, 0);
    v
}

fn print_rustc_env_paths(
    bin_dir: &Path,
    windows: bool,
    source_root: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (buf, br, lint) = if windows {
        (
            "buf.exe",
            "protoc-gen-buf-breaking.exe",
            "protoc-gen-buf-lint.exe",
        )
    } else {
        ("buf", "protoc-gen-buf-breaking", "protoc-gen-buf-lint")
    };
    println!(
        "cargo:rustc-env=BUF_RS_BUF_BIN={}",
        bin_dir.join(buf).display()
    );
    println!(
        "cargo:rustc-env=BUF_RS_PROTOC_GEN_BUF_BREAKING={}",
        bin_dir.join(br).display()
    );
    println!(
        "cargo:rustc-env=BUF_RS_PROTOC_GEN_BUF_LINT={}",
        bin_dir.join(lint).display()
    );
    if let Some(p) = source_root {
        println!("cargo:rustc-env=BUF_RS_SOURCE_ROOT={}", p.display());
    } else {
        println!("cargo:rustc-env=BUF_RS_SOURCE_ROOT=");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LayoutMode {
    Cache,
    CacheLink,
    CacheVerifiedLink,
    Target,
}

fn layout_mode(value: Option<&str>) -> Result<LayoutMode, Box<dyn std::error::Error>> {
    let raw = value.unwrap_or_default().to_string();
    let normalized = raw.trim();
    let mode = if normalized.is_empty() || normalized.eq_ignore_ascii_case("cache") {
        LayoutMode::Cache
    } else if normalized.eq_ignore_ascii_case("cache-link") {
        LayoutMode::CacheLink
    } else if normalized.eq_ignore_ascii_case("cache-verified-link") {
        LayoutMode::CacheVerifiedLink
    } else if normalized.eq_ignore_ascii_case("target") {
        LayoutMode::Target
    } else {
        return Err(format!(
            "buf-tools: unsupported BUF_RS_LAYOUT_MODE={normalized:?}; supported values: cache, cache-link, cache-verified-link, target"
        )
        .into());
    };
    Ok(mode)
}

fn log_layout_mode(mode: &LayoutMode, raw_value: Option<&str>, info_warn: &mut dyn FnMut(String)) {
    let normalized = raw_value.unwrap_or_default().trim();
    if normalized.is_empty() {
        info_warn("buf-tools: info: layout_mode unset/empty; defaulting to cache".to_string());
        return;
    }
    if matches!(mode, LayoutMode::Cache) {
        info_warn("buf-tools: info: layout_mode=cache; using default cache behavior".to_string());
    }
}

fn resolve_target_layout_root(
    out_dir: &Path,
    core: &str,
    target_triple: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "target"))
        .ok_or("buf-tools: could not locate Cargo target dir from OUT_DIR")?;
    Ok(target_dir.join("buf-tools").join(core).join(target_triple))
}

fn link_or_copy_cache_artifact(
    source: &Path,
    dest: &Path,
    windows: bool,
    warn: &mut dyn FnMut(String),
) -> Result<(), Box<dyn std::error::Error>> {
    if !source.is_file() {
        return Err(format!(
            "buf-tools: cache-link source missing at {}",
            source.display()
        )
        .into());
    }
    if dest.exists() {
        remove_path(dest)?;
    }
    ensure_unix_executable(source, windows)?;
    match symlink_file(source, dest) {
        Ok(()) => Ok(()),
        Err(err) => {
            warn(format!(
                "buf-tools: symlink {} -> {} failed ({err}); falling back to copy",
                dest.display(),
                source.display()
            ));
            let bytes = fs::read(source)?;
            write_executable(dest, &bytes, windows)?;
            Ok(())
        }
    }
}

fn remove_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::IsADirectory => {
            fs::remove_dir_all(path)?;
            Ok(())
        }
        Err(err) => Err(format!("remove {}: {err}", path.display()).into()),
    }
}

#[cfg(unix)]
fn symlink_file(source: &Path, dest: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(source, dest)
}

#[cfg(windows)]
fn symlink_file(source: &Path, dest: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(source, dest)
}

fn print_layout_mode_metadata(
    mode: &LayoutMode,
    target_layout_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode_value = match mode {
        LayoutMode::Cache => "cache",
        LayoutMode::CacheLink => "cache-link",
        LayoutMode::CacheVerifiedLink => "cache-verified-link",
        LayoutMode::Target => "target",
    };
    println!("cargo:rustc-env=BUF_RS_LAYOUT_MODE_RESOLVED={mode_value}");
    if matches!(mode, LayoutMode::Cache) {
        println!("cargo:rustc-env=BUF_RS_BIN_LAYOUT_ROOT=");
    } else {
        println!(
            "cargo:rustc-env=BUF_RS_BIN_LAYOUT_ROOT={}",
            target_layout_root.display()
        );
    }
    Ok(())
}

fn emit_config_source_trace(cfg: &config::ResolvedConfig, info_warn: &mut dyn FnMut(String)) {
    info_warn(format!(
        "buf-tools: config layout_mode source={}",
        source_name(cfg.layout_mode_source)
    ));
    info_warn(format!(
        "buf-tools: config build_log source={}",
        source_name(cfg.build_log_source)
    ));
    info_warn(format!(
        "buf-tools: config cache_dir source={}",
        source_name(cfg.cache_dir_source)
    ));
    info_warn(format!(
        "buf-tools: config release_base_url source={}",
        source_name(cfg.release_base_url_source)
    ));
    info_warn(format!(
        "buf-tools: config source_base_url source={}",
        source_name(cfg.source_base_url_source)
    ));
}

fn source_name(src: config::ConfigSource) -> &'static str {
    match src {
        config::ConfigSource::Env => "env",
        config::ConfigSource::PackageMetadata => "package.metadata",
        config::ConfigSource::WorkspaceMetadata => "workspace.metadata",
        config::ConfigSource::Default => "default",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildLogLevel {
    Warn,
    Verbose,
    Silent,
}

fn build_log_level(value: Option<&str>) -> Result<BuildLogLevel, Box<dyn std::error::Error>> {
    let normalized = value.unwrap_or_default().trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "true" || normalized == "warn" {
        return Ok(BuildLogLevel::Warn);
    }
    if normalized == "verbose" {
        return Ok(BuildLogLevel::Verbose);
    }
    if normalized == "false" || normalized == "silent" {
        return Ok(BuildLogLevel::Silent);
    }
    Err(format!(
        "buf-tools: unsupported build_log/BUF_RS_BUILD_LOG={normalized:?}; supported values: warn, verbose, silent (aliases: true=>warn, false=>silent)"
    )
    .into())
}
