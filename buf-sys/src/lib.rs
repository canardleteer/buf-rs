//! Official [`buf`](https://github.com/bufbuild/buf) and `protoc-gen-buf-*` plugin paths.
//!
//! Binaries are **not** embedded in the crates.io package; see the crate README on docs.rs or in this repo for network, cache, and verification behavior.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;

/// Path to the `buf` executable for this compilation target.
pub fn buf_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_SYS_BUF_BIN"))
}

/// Path to `protoc-gen-buf-breaking`.
pub fn protoc_gen_buf_breaking_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_SYS_PROTOC_GEN_BUF_BREAKING"))
}

/// Path to `protoc-gen-buf-lint`.
pub fn protoc_gen_buf_lint_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_SYS_PROTOC_GEN_BUF_LINT"))
}

/// When **`BUF_VENDOR_INCLUDE_SOURCE=1`** was set at build time, the extracted upstream tree.
#[must_use]
pub fn upstream_source_root() -> Option<PathBuf> {
    let s = env!("BUF_SYS_SOURCE_ROOT");
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

#[cfg(all(test, not(docsrs)))]
mod tests {
    use std::fs;
    use std::io::Read;
    use std::path::Path;
    use std::process::{Command, Stdio};

    #[test]
    fn buf_exists() {
        let p = crate::buf_bin_path();
        assert!(p.exists(), "missing {:?}", p);
    }

    #[test]
    fn protoc_gen_exists() {
        let b = crate::protoc_gen_buf_breaking_bin_path();
        let l = crate::protoc_gen_buf_lint_bin_path();
        assert!(b.exists(), "missing {:?}", b);
        assert!(l.exists(), "missing {:?}", l);
    }

    #[test]
    fn buf_version_smoke() {
        let expect = match std::env::var("BUF_EXPECT_VERSION") {
            Ok(v) => v,
            Err(_) => return,
        };
        let p = crate::buf_bin_path();
        let mut child = Command::new(&p)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = String::new();
        child
            .stdout
            .as_mut()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
        assert!(
            buf_stdout_matches_expect(&stdout, &expect),
            "expected {:?} (or crate pre-release prefix before '-') in stdout {:?}",
            expect,
            stdout
        );
    }

    fn buf_stdout_matches_expect(stdout: &str, expect: &str) -> bool {
        let stdout = stdout.trim();
        let expect = expect.trim();
        if stdout.contains(expect) {
            return true;
        }
        if let Some((core, rest)) = expect.split_once('-')
            && !rest.is_empty()
            && stdout.contains(core)
        {
            return true;
        }
        false
    }

    #[test]
    fn protoc_plugins_look_like_native_bins() {
        for path_fn in [
            crate::protoc_gen_buf_breaking_bin_path as fn() -> _,
            crate::protoc_gen_buf_lint_bin_path,
        ] {
            let p = path_fn();
            assert_plugin_payload(&p);
        }
    }

    fn assert_plugin_payload(p: &Path) {
        let meta = fs::metadata(p).unwrap();
        assert!(meta.len() > 10_000, "{:?} unexpectedly small", p);
        let mut f = fs::File::open(p).unwrap();
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic).unwrap();
        #[cfg(target_os = "macos")]
        {
            let ok = matches!(
                magic,
                [0xcf, 0xfa, 0xed, 0xfe] | [0xce, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe]
            );
            assert!(ok, "{:?} missing Mach-O / FAT magic, got {:02x?}", p, magic);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(&magic[..4], b"\x7fELF", "{:?} missing ELF magic", p);
        #[cfg(windows)]
        assert_eq!(&magic[..2], b"MZ", "{:?} missing PE magic", p);
    }

    #[test]
    fn upstream_source_when_vendor_flag_set() {
        if !std::env::var("BUF_VENDOR_INCLUDE_SOURCE")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            return;
        }
        let root = crate::upstream_source_root()
            .expect("BUF_VENDOR_INCLUDE_SOURCE build must set BUF_SYS_SOURCE_ROOT");
        assert!(root.is_dir(), "{:?}", root);
        assert!(
            root.join("README.md").is_file() || root.join("go.mod").is_file(),
            "expected extracted buf repo layout under {:?}",
            root
        );
    }
}

// Compile `build_support` unit tests with the library test harness (not part of the public API).
#[cfg(all(test, not(docsrs)))]
#[allow(dead_code)] // `targets` / `verify` are shared with `build.rs`; only a subset is used here.
#[path = "../build_support/targets.rs"]
mod release_targets_table;

#[cfg(all(test, not(docsrs)))]
#[allow(dead_code)]
#[path = "../build_support/verify.rs"]
mod release_verify_fixtures;
