use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use sha2::{Digest, Sha256};

const BUF_MINISIGN_PUBLIC_KEY_B64: &str =
    "RWQ/i9xseZwBVE7pEniCNjlNOeeyp4BQgdZDLQcAohxEAH5Uj5DEKjv6";
const PREHASHED_MINISIGN_MIN_VERSION: &str = "1.12.0";
const MAX_ATTEMPTS: usize = 5;
const CHUNK: usize = 64 * 1024;
const LOCK_FILENAME: &str = ".cache-slot.lock";
const LOCK_WAIT_STEP: Duration = Duration::from_millis(250);
const LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const LOCK_STALE_AFTER: Duration = Duration::from_secs(600);

enum SlotLockState {
    Acquired(SlotLockGuard),
    WaitedForOtherWriter,
}

struct SlotLockGuard {
    path: PathBuf,
}

impl Drop for SlotLockGuard {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

#[derive(Clone, Copy, Debug)]
struct ReleaseTarget {
    asset_suffix: &'static str,
    windows: bool,
    min_version: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=BUF_RS_TOOLCHAIN_HOME");
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

    let install_bin_dir = resolve_install_bin_dir(&core, &target_triple)?;
    fs::create_dir_all(&install_bin_dir)?;
    let cache_slot = resolve_cache_slot(&core, &target_triple)?;
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
    let (slot_lock, waited_for_peer_writer) = match acquire_or_wait_for_slot(&cache_slot)? {
        SlotLockState::Acquired(guard) => (Some(guard), false),
        SlotLockState::WaitedForOtherWriter => (None, true),
    };

    let tag = format!("v{core}");
    let offline = env::var_os("CARGO_NET_OFFLINE").is_some();

    let sha256_txt = download(&format!("{release_base}sha256.txt"))?;
    let minisig = download(&format!("{release_base}sha256.txt.minisig"))?;
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

    let mut installed: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for (remote_name, local_name) in triples(&rt) {
        let expected_hex = match checksums.get(&remote_name) {
            Some(v) => v,
            None => {
                if local_name.starts_with("buf") {
                    return Err(format!(
                        "buf-toolchain: release {tag} missing required binary {remote_name}"
                    )
                    .into());
                }
                println!(
                    "cargo:warning=buf-toolchain: skipping unavailable plugin {} for {}",
                    local_name, rt.asset_suffix
                );
                skipped.push(local_name);
                continue;
            }
        };

        let cache_file = cache_slot.join(&remote_name);
        let bytes = if verify_cached_file(&cache_file, expected_hex)? {
            println!(
                "cargo:warning=buf-toolchain: using cached {} (sha256 OK)",
                remote_name
            );
            fs::read(&cache_file)?
        } else {
            if waited_for_peer_writer {
                return Err(format!(
                    "buf-toolchain: cache artifact {} still missing/invalid at {} after waiting for peer writer",
                    remote_name,
                    cache_file.display()
                )
                .into());
            }
            if offline {
                return Err(format!(
                    "buf-toolchain: CARGO_NET_OFFLINE set but cache miss for {} at {}",
                    remote_name,
                    cache_file.display()
                )
                .into());
            }
            let url = format!("{release_base}{remote_name}");
            let b = download_streaming_with_progress(&url, &remote_name)?;
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
            b
        };

        let dest = install_bin_dir.join(&local_name);
        write_executable(&dest, &bytes, rt.windows)?;
        installed.push(local_name);
    }

    println!(
        "cargo:warning=buf-toolchain: installed {} binary(ies) into {}",
        installed.len(),
        install_bin_dir.display()
    );
    if !installed.is_empty() {
        println!(
            "cargo:warning=buf-toolchain: installed: {}",
            installed.join(", ")
        );
    }
    if !skipped.is_empty() {
        println!(
            "cargo:warning=buf-toolchain: skipped: {}",
            skipped.join(", ")
        );
    }
    drop(slot_lock);

    Ok(())
}

fn resolve_install_bin_dir(core: &str, target_triple: &str) -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("BUF_RS_TOOLCHAIN_BIN_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = if let Ok(dir) = env::var("BUF_RS_TOOLCHAIN_HOME") {
        PathBuf::from(dir)
    } else {
        cargo_home_dir()?.join("buf-toolchain")
    };
    Ok(home.join(core).join(target_triple).join("bin"))
}

fn resolve_cache_slot(core: &str, target_triple: &str) -> Result<PathBuf, String> {
    let root = if let Ok(dir) = env::var("BUF_RS_CACHE_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::cache_dir()
            .ok_or_else(|| "buf-toolchain: cannot resolve cache dir".to_string())?
            .join("buf-toolchain")
    };
    Ok(root.join(core).join(target_triple))
}

fn cargo_home_dir() -> Result<PathBuf, String> {
    if let Ok(home) = env::var("CARGO_HOME") {
        return Ok(PathBuf::from(home));
    }
    let home = dirs::home_dir().ok_or_else(|| "buf-toolchain: cannot resolve HOME".to_string())?;
    Ok(home.join(".cargo"))
}

fn write_executable(path: &Path, bytes: &[u8], windows: bool) -> Result<(), String> {
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
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod {}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

fn verify_cached_file(dest: &Path, expected_sha256: &str) -> Result<bool, String> {
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

fn verify_minisign_signature(
    data: &[u8],
    minisig_text: &str,
    public_key_b64: &str,
    allow_legacy: bool,
) -> Result<(), String> {
    let pk =
        PublicKey::from_base64(public_key_b64).map_err(|e| format!("parse public key: {e}"))?;
    let sig =
        Signature::decode(minisig_text).map_err(|e| format!("parse minisig signature: {e}"))?;
    pk.verify(data, &sig, allow_legacy)
        .map_err(|e| format!("minisign verify failed: {e}"))
}

fn parse_sha256_list(data: &[u8]) -> Result<HashMap<String, String>, String> {
    let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
    let mut m = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (hash, name) = line.split_once("  ").ok_or_else(|| {
            format!("invalid sha256.txt line (expected 'HASH  filename'): {line:?}")
        })?;
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("bad hash in line: {line:?}"));
        }
        m.insert(name.trim().to_string(), hash.to_ascii_lowercase());
    }
    Ok(m)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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

fn acquire_or_wait_for_slot(slot: &Path) -> Result<SlotLockState, String> {
    let lock_path = slot.join(LOCK_FILENAME);
    match try_acquire_lock(&lock_path)? {
        Some(guard) => Ok(SlotLockState::Acquired(guard)),
        None => {
            println!(
                "cargo:warning=buf-toolchain: cache slot lock exists at {}. Waiting for peer writer.",
                lock_path.display()
            );
            wait_for_unlock(&lock_path)?;
            Ok(SlotLockState::WaitedForOtherWriter)
        }
    }
}

fn try_acquire_lock(lock_path: &Path) -> Result<Option<SlotLockGuard>, String> {
    let mut lock = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
        Err(err) => return Err(format!("create lock {}: {err}", lock_path.display())),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock before unix epoch: {e}"))?
        .as_secs();
    writeln!(lock, "pid={} unix_ts={}", std::process::id(), now)
        .map_err(|e| format!("write lock {}: {e}", lock_path.display()))?;
    Ok(Some(SlotLockGuard {
        path: lock_path.to_path_buf(),
    }))
}

fn wait_for_unlock(lock_path: &Path) -> Result<(), String> {
    let mut waited = Duration::ZERO;
    while lock_path.exists() {
        if is_lock_stale(lock_path)? {
            println!(
                "cargo:warning=buf-toolchain: cache slot lock looks stale at {}. Removing stale lock.",
                lock_path.display()
            );
            fs::remove_file(lock_path)
                .map_err(|e| format!("remove stale lock {}: {e}", lock_path.display()))?;
            return Ok(());
        }
        if waited >= LOCK_WAIT_TIMEOUT {
            return Err(format!(
                "buf-toolchain: timed out waiting for cache slot lock {} after {}s",
                lock_path.display(),
                LOCK_WAIT_TIMEOUT.as_secs()
            ));
        }
        sleep(LOCK_WAIT_STEP);
        waited += LOCK_WAIT_STEP;
    }
    println!(
        "cargo:warning=buf-toolchain: peer writer released cache slot lock {} after {}ms",
        lock_path.display(),
        waited.as_millis()
    );
    Ok(())
}

fn is_lock_stale(lock_path: &Path) -> Result<bool, String> {
    if let Some(owner_pid) = read_lock_pid(lock_path)?
        && !pid_is_alive(owner_pid)
    {
        return Ok(true);
    }
    let modified = fs::metadata(lock_path)
        .map_err(|e| format!("stat {}: {e}", lock_path.display()))?
        .modified()
        .map_err(|e| format!("mtime {}: {e}", lock_path.display()))?;
    let age = SystemTime::now()
        .duration_since(modified)
        .map_err(|e| format!("mtime in future for {}: {e}", lock_path.display()))?;
    Ok(age >= LOCK_STALE_AFTER)
}

fn read_lock_pid(lock_path: &Path) -> Result<Option<u32>, String> {
    let content =
        fs::read_to_string(lock_path).map_err(|e| format!("read {}: {e}", lock_path.display()))?;
    let Some(pid_part) = content
        .split_whitespace()
        .find(|part| part.starts_with("pid="))
    else {
        return Ok(None);
    };
    Ok(pid_part["pid=".len()..].parse::<u32>().ok())
}

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    PathBuf::from("/proc").join(pid.to_string()).exists()
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    true
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    for attempt in 1..=MAX_ATTEMPTS {
        let res = ureq::get(url)
            .set("User-Agent", user_agent())
            .call()
            .map_err(|e| e.to_string())
            .and_then(|resp| {
                let mut reader = resp.into_reader();
                let mut out = Vec::new();
                reader.read_to_end(&mut out).map_err(|e| e.to_string())?;
                Ok(out)
            });
        match res {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(format!(
                        "GET {url} failed after {MAX_ATTEMPTS} attempts: {err}"
                    ));
                }
                let wait_ms = 400_u64 * attempt as u64;
                println!(
                    "cargo:warning=buf-toolchain: transient download error for {url}: {err}; retry in {wait_ms}ms"
                );
                sleep(Duration::from_millis(wait_ms));
            }
        }
    }
    Err(format!("GET {url}: exhausted retry attempts"))
}

fn download_streaming_with_progress(url: &str, label: &str) -> Result<Vec<u8>, String> {
    for attempt in 1..=MAX_ATTEMPTS {
        match download_streaming_once(url, label) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(format!(
                        "GET {url} failed after {MAX_ATTEMPTS} attempts: {err}"
                    ));
                }
                let wait_ms = 400_u64 * attempt as u64;
                println!(
                    "cargo:warning=buf-toolchain: transient download error for {label}: {err}; retry in {wait_ms}ms"
                );
                sleep(Duration::from_millis(wait_ms));
            }
        }
    }
    Err(format!("GET {url}: exhausted retry attempts"))
}

fn download_streaming_once(url: &str, label: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", user_agent())
        .call()
        .map_err(|e| e.to_string())?;
    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    let mut read_total: u64 = 0;
    let mut milestone_sent = 0_i32;
    let mut chunk = [0u8; CHUNK];
    println!("cargo:warning=buf-toolchain: downloading {label} - 0%");
    let mut last_mb = 0_u64;
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        read_total += n as u64;
        if let Some(t) = total {
            let pct = ((read_total as f64 / t as f64) * 100.0).floor().min(100.0) as i32;
            let band = (pct / 10) * 10;
            if band > milestone_sent {
                milestone_sent = band;
                println!("cargo:warning=buf-toolchain: {label} - {band}% ({read_total}/{t} bytes)");
            }
        } else {
            let mb = read_total / (1024 * 1024);
            if mb > last_mb && mb > 0 {
                last_mb = mb;
                println!(
                    "cargo:warning=buf-toolchain: {label} - received >= {mb} MiB (no Content-Length)"
                );
            }
        }
    }
    if let Some(t) = total {
        if milestone_sent < 100 {
            println!("cargo:warning=buf-toolchain: {label} - 100% ({read_total}/{t} bytes)");
        }
    } else {
        println!("cargo:warning=buf-toolchain: {label} - finished ({read_total} bytes)");
    }
    Ok(buf)
}

fn user_agent() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
}

fn from_rust_triple(triple: &str) -> Option<ReleaseTarget> {
    Some(match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => ReleaseTarget {
            asset_suffix: "Linux-x86_64",
            windows: false,
            min_version: "1.0.0",
        },
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => ReleaseTarget {
            asset_suffix: "Linux-aarch64",
            windows: false,
            min_version: "1.0.0",
        },
        "arm-unknown-linux-gnueabihf" | "arm-unknown-linux-musleabihf" => ReleaseTarget {
            asset_suffix: "Linux-armv7",
            windows: false,
            min_version: "1.47.0",
        },
        "powerpc64le-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-ppc64le",
            windows: false,
            min_version: "1.54.0",
        },
        "s390x-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-s390x",
            windows: false,
            min_version: "1.56.0",
        },
        "riscv64gc-unknown-linux-gnu" | "riscv64-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-riscv64",
            windows: false,
            min_version: "1.54.0",
        },
        "x86_64-apple-darwin" => ReleaseTarget {
            asset_suffix: "Darwin-x86_64",
            windows: false,
            min_version: "1.0.0",
        },
        "aarch64-apple-darwin" => ReleaseTarget {
            asset_suffix: "Darwin-arm64",
            windows: false,
            min_version: "1.0.0",
        },
        "x86_64-pc-windows-gnu" | "x86_64-pc-windows-msvc" => ReleaseTarget {
            asset_suffix: "Windows-x86_64",
            windows: true,
            min_version: "1.0.0",
        },
        "aarch64-pc-windows-msvc" | "aarch64-pc-windows-gnu" => ReleaseTarget {
            asset_suffix: "Windows-arm64",
            windows: true,
            min_version: "1.0.0",
        },
        "x86_64-unknown-freebsd" => ReleaseTarget {
            asset_suffix: "FreeBSD-x86_64",
            windows: false,
            min_version: "1.67.0",
        },
        "aarch64-unknown-freebsd" => ReleaseTarget {
            asset_suffix: "FreeBSD-arm64",
            windows: false,
            min_version: "1.67.0",
        },
        "x86_64-unknown-openbsd" => ReleaseTarget {
            asset_suffix: "OpenBSD-x86_64",
            windows: false,
            min_version: "1.67.0",
        },
        "aarch64-unknown-openbsd" => ReleaseTarget {
            asset_suffix: "OpenBSD-arm64",
            windows: false,
            min_version: "1.67.0",
        },
        _ => return None,
    })
}

fn local_bin_names(windows: bool) -> [&'static str; 3] {
    if windows {
        [
            "buf.exe",
            "protoc-gen-buf-breaking.exe",
            "protoc-gen-buf-lint.exe",
        ]
    } else {
        ["buf", "protoc-gen-buf-breaking", "protoc-gen-buf-lint"]
    }
}

fn remote_filename(prefix: &str, t: &ReleaseTarget) -> String {
    match prefix {
        "buf" => format!(
            "buf-{}{}",
            t.asset_suffix,
            if t.windows { ".exe" } else { "" }
        ),
        "protoc-gen-buf-breaking" => format!(
            "protoc-gen-buf-breaking-{}{}",
            t.asset_suffix,
            if t.windows { ".exe" } else { "" }
        ),
        "protoc-gen-buf-lint" => format!(
            "protoc-gen-buf-lint-{}{}",
            t.asset_suffix,
            if t.windows { ".exe" } else { "" }
        ),
        _ => panic!("unknown prefix"),
    }
}

fn triples(t: &ReleaseTarget) -> [(String, String); 3] {
    let names = local_bin_names(t.windows);
    [
        (remote_filename("buf", t), names[0].to_string()),
        (
            remote_filename("protoc-gen-buf-breaking", t),
            names[1].to_string(),
        ),
        (
            remote_filename("protoc-gen-buf-lint", t),
            names[2].to_string(),
        ),
    ]
}
