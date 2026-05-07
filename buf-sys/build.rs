//! Download Buf release artifacts into `OUT_DIR`, with persistent cache.

#[path = "build_support/mod.rs"]
mod build_support;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;

use build_support::targets::from_rust_triple;
use build_support::{
    BUF_MINISIGN_PUBLIC_KEY_B64, cache_slot, fetch, parse_sha256_list, sha256_hex, source,
    target_supported, triples, verify_cached_file, verify_minisign_signature, write_executable,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=BUF_SYS_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=BUF_VENDOR_INCLUDE_SOURCE");
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

    let rt =
        from_rust_triple(&target_triple).ok_or_else(|| {
            format!(
                "buf-sys: unsupported compilation TARGET `{target_triple}`. See crate README for supported triples."
            )
        })?;

    let cache_root = cache_root_dir()?;
    let slot = cache_slot(&cache_root, &core, &target_triple);
    fs::create_dir_all(&slot)?;

    let tag = format!("v{core}");
    let base = format!("https://github.com/bufbuild/buf/releases/download/{tag}/");

    let offline = env::var_os("CARGO_NET_OFFLINE").is_some();

    let sha256_url = format!("{base}sha256.txt");
    let minisig_url = format!("{base}sha256.txt.minisig");

    let sha256_txt = fetch::download(&sha256_url)?;
    let minisig = fetch::download(&minisig_url)?;
    let minisig_text = std::str::from_utf8(&minisig)?;
    verify_minisign_signature(&sha256_txt, minisig_text, BUF_MINISIGN_PUBLIC_KEY_B64)?;
    let checksums = parse_sha256_list(&sha256_txt)?;

    if !target_supported(&checksums, &rt) {
        return Err(format!(
            "buf-sys: Buf release {tag} does not list binaries for this platform ({})",
            rt.asset_suffix
        )
        .into());
    }

    let bin_dir = out_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;

    let mut warn_fn = |msg: String| println!("cargo:warning={msg}");

    for (remote_name, local_name) in triples(&rt) {
        let expected_hex = checksums
            .get(&remote_name)
            .ok_or_else(|| format!("missing {remote_name} in sha256.txt"))?;
        let cache_file = slot.join(&remote_name);
        let url = format!("{base}{remote_name}");

        let bytes = if verify_cached_file(&cache_file, expected_hex)? {
            warn_fn(format!("buf-sys: using cached {} (sha256 OK)", remote_name));
            fs::read(&cache_file)?
        } else {
            if offline {
                return Err(format!(
                    "buf-sys: CARGO_NET_OFFLINE set but cache miss for {} — populate {} or clear offline mode",
                    remote_name,
                    cache_file.display()
                )
                .into());
            }
            let b = fetch::download_streaming_with_progress(&url, &remote_name, &mut warn_fn)?;
            if sha256_hex(&b) != *expected_hex {
                return Err(format!(
                    "SHA256 mismatch for {remote_name}: expected {expected_hex}, got {}",
                    sha256_hex(&b)
                )
                .into());
            }
            fs::write(&cache_file, &b)?;
            b
        };

        let dest = bin_dir.join(&local_name);
        write_executable(&dest, &bytes, rt.windows)?;
    }

    let mut source_root: Option<PathBuf> = None;
    if env_truthy("BUF_VENDOR_INCLUDE_SOURCE") {
        if offline && source_bundle_ready(&slot, &core).is_none() {
            return Err(
                "buf-sys: BUF_VENDOR_INCLUDE_SOURCE set but offline and source bundle not cached"
                    .into(),
            );
        }
        source_root = Some(fetch_optional_source(
            &slot,
            &core,
            &tag,
            offline,
            &mut warn_fn,
        )?);
    }

    print_rustc_env_paths(&out_dir, rt.windows, source_root.as_ref())?;

    Ok(())
}

fn env_truthy(name: &str) -> bool {
    match env::var(name) {
        Ok(s) => {
            let s = s.trim().to_ascii_lowercase();
            matches!(s.as_str(), "1" | "true" | "yes")
        }
        Err(_) => false,
    }
}

fn source_bundle_ready(slot: &Path, core: &str) -> Option<PathBuf> {
    let root = slot.join(format!("buf-{core}"));
    if root.is_dir() { Some(root) } else { None }
}

fn fetch_optional_source(
    slot: &Path,
    core: &str,
    tag: &str,
    offline: bool,
    warn: &mut dyn FnMut(String),
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let url = format!("https://github.com/bufbuild/buf/archive/refs/tags/{tag}.tar.gz");
    let archive_path = slot.join(format!("buf-upstream-{core}.tar.gz"));
    let extract_parent = slot.join("upstream-src");

    let expected_root = extract_parent.join(format!("buf-{core}"));

    if expected_root.is_dir() {
        warn(format!(
            "buf-sys: using cached extracted source at {}",
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
            "buf-sys: using cached source archive {}",
            archive_path.display()
        ));
    }

    if extract_parent.exists() {
        fs::remove_dir_all(&extract_parent)?;
    }
    source::extract_tar_gz(&archive_path, &extract_parent)?;

    if !expected_root.is_dir() {
        return Err(format!(
            "buf-sys: extracted source layout unexpected — missing {}",
            expected_root.display()
        )
        .into());
    }

    warn(format!(
        "buf-sys: extracted upstream source at {}",
        expected_root.display()
    ));
    Ok(expected_root)
}

fn cache_root_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(p) = env::var("BUF_SYS_CACHE_DIR") {
        let pb = PathBuf::from(p);
        fs::create_dir_all(&pb)?;
        return Ok(pb);
    }
    let base = dirs::cache_dir().ok_or("could not resolve cache dir (set BUF_SYS_CACHE_DIR)")?;
    Ok(base.join("buf-sys"))
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
    out_dir: &Path,
    windows: bool,
    source_root: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bin_dir = out_dir.join("bin");
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
        "cargo:rustc-env=BUF_SYS_BUF_BIN={}",
        bin_dir.join(buf).display()
    );
    println!(
        "cargo:rustc-env=BUF_SYS_PROTOC_GEN_BUF_BREAKING={}",
        bin_dir.join(br).display()
    );
    println!(
        "cargo:rustc-env=BUF_SYS_PROTOC_GEN_BUF_LINT={}",
        bin_dir.join(lint).display()
    );
    if let Some(p) = source_root {
        println!("cargo:rustc-env=BUF_SYS_SOURCE_ROOT={}", p.display());
    } else {
        println!("cargo:rustc-env=BUF_SYS_SOURCE_ROOT=");
    }
    Ok(())
}
