use std::env;
use std::path::PathBuf;

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let target = current_rust_target();
    let bin_dir = match env::var("BUF_RS_TOOLCHAIN_BIN_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => install_home()
            .join(version_core(version))
            .join(target)
            .join("bin"),
    };

    println!("buf-toolchain {}", version);
    println!("managed bin dir: {}", bin_dir.display());
    println!("cache override env: BUF_RS_CACHE_DIR");
    println!("install root env: BUF_RS_TOOLCHAIN_HOME");
    println!("bin dir override env: BUF_RS_TOOLCHAIN_BIN_DIR");
    println!("The build step installs upstream Buf executables into this managed directory.");
}

fn install_home() -> PathBuf {
    if let Ok(home) = env::var("BUF_RS_TOOLCHAIN_HOME") {
        return PathBuf::from(home);
    }
    if let Ok(cargo_home) = env::var("CARGO_HOME") {
        return PathBuf::from(cargo_home).join("buf-toolchain");
    }
    match dirs::home_dir() {
        Some(home) => home.join(".cargo").join("buf-toolchain"),
        None => PathBuf::from(".cargo").join("buf-toolchain"),
    }
}

fn version_core(version: &str) -> &str {
    match version.split_once('-') {
        Some((core, _)) => core,
        None => version,
    }
}

fn current_rust_target() -> &'static str {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("arm", "linux") => "arm-unknown-linux-gnueabihf",
        ("powerpc64", "linux") => "powerpc64le-unknown-linux-gnu",
        ("s390x", "linux") => "s390x-unknown-linux-gnu",
        ("riscv64", "linux") => "riscv64gc-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("aarch64", "windows") => "aarch64-pc-windows-msvc",
        ("x86_64", "freebsd") => "x86_64-unknown-freebsd",
        ("aarch64", "freebsd") => "aarch64-unknown-freebsd",
        ("x86_64", "openbsd") => "x86_64-unknown-openbsd",
        ("aarch64", "openbsd") => "aarch64-unknown-openbsd",
        _ => "unknown-target",
    }
}
