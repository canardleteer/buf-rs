//! Official [`buf`](https://github.com/bufbuild/buf) and `protoc-gen-buf-*` plugin paths.
//!
//! Binaries are **not** embedded in the crates.io package; see the crate README on docs.rs or in this repo for network, cache, and verification behavior.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;

/// Path to the `buf` executable for this compilation target.
pub fn buf_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_RS_BUF_BIN"))
}

/// Path to `protoc-gen-buf-breaking`.
pub fn protoc_gen_buf_breaking_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_RS_PROTOC_GEN_BUF_BREAKING"))
}

/// Path to `protoc-gen-buf-lint`.
pub fn protoc_gen_buf_lint_bin_path() -> PathBuf {
    PathBuf::from(env!("BUF_RS_PROTOC_GEN_BUF_LINT"))
}

/// Resolved build layout mode from `BUF_RS_LAYOUT_MODE` at compile time.
///
/// Values are one of: `cache`, `cache-link`, `cache-verified-link`, `target`.
#[must_use]
pub fn resolved_layout_mode() -> &'static str {
    env!("BUF_RS_LAYOUT_MODE_RESOLVED")
}

/// Optional target landing-pad root for non-default layout modes.
///
/// Returns `None` when the default `cache` layout mode is active.
#[must_use]
pub fn bin_layout_root() -> Option<PathBuf> {
    let s = env!("BUF_RS_BIN_LAYOUT_ROOT");
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// When **`BUF_RS_INCLUDE_SOURCE=1`** was set at build time, the extracted upstream tree.
#[must_use]
pub fn upstream_source_root() -> Option<PathBuf> {
    let s = env!("BUF_RS_SOURCE_ROOT");
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Whether this crate was compiled under docs.rs (`DOCS_RS=1`).
///
/// When `true`, path accessors point at non-functional 12 KiB placeholders
/// (ELF or MZ magic only). Do not execute them. Consumer documentation
/// builds should skip live `buf` invocation and use a packaged descriptor
/// instead.
#[must_use]
pub fn compiled_for_docs_rs() -> bool {
    matches!(option_env!("BUF_RS_DOCS_RS"), Some(s) if !s.is_empty())
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

    #[test]
    fn layout_mode_metadata_is_present() {
        let mode = crate::resolved_layout_mode();
        assert!(matches!(
            mode,
            "cache" | "cache-link" | "cache-verified-link" | "target"
        ));
        if mode == "cache" {
            assert!(crate::bin_layout_root().is_none());
        }
        assert!(
            !crate::compiled_for_docs_rs(),
            "normal builds must not set BUF_RS_DOCS_RS"
        );
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
        if !std::env::var("BUF_RS_INCLUDE_SOURCE")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
        {
            return;
        }
        let root = crate::upstream_source_root()
            .expect("BUF_RS_INCLUDE_SOURCE build must set BUF_RS_SOURCE_ROOT");
        assert!(root.is_dir(), "{:?}", root);
        assert!(
            root.join("README.md").is_file() || root.join("go.mod").is_file(),
            "expected extracted buf repo layout under {:?}",
            root
        );
    }
}

#[cfg(all(test, docsrs))]
mod docs_rs_tests {
    use std::fs;
    use std::io::Read;
    use std::path::Path;

    const STUB_LEN: u64 = 12_000;

    #[test]
    fn compiled_for_docs_rs_is_true() {
        assert!(crate::compiled_for_docs_rs());
    }

    #[test]
    fn layout_metadata_is_cache_mode() {
        assert_eq!(crate::resolved_layout_mode(), "cache");
        assert!(crate::bin_layout_root().is_none());
        assert!(crate::upstream_source_root().is_none());
    }

    #[test]
    fn accessors_point_at_placeholder_bins() {
        let expected = if cfg!(windows) {
            [
                ("buf.exe", crate::buf_bin_path()),
                (
                    "protoc-gen-buf-breaking.exe",
                    crate::protoc_gen_buf_breaking_bin_path(),
                ),
                (
                    "protoc-gen-buf-lint.exe",
                    crate::protoc_gen_buf_lint_bin_path(),
                ),
            ]
        } else {
            [
                ("buf", crate::buf_bin_path()),
                (
                    "protoc-gen-buf-breaking",
                    crate::protoc_gen_buf_breaking_bin_path(),
                ),
                ("protoc-gen-buf-lint", crate::protoc_gen_buf_lint_bin_path()),
            ]
        };

        for (name, path) in expected {
            assert_placeholder_bin(&path, name);
        }
    }

    fn assert_placeholder_bin(path: &Path, file_name: &str) {
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some(file_name),
            "unexpected file name for {path:?}"
        );
        assert_eq!(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            Some("bin"),
            "docs.rs placeholders must live under OUT_DIR/bin, got {path:?}"
        );
        assert!(path.is_file(), "missing placeholder {path:?}");
        let meta = fs::metadata(path).unwrap();
        assert_eq!(meta.len(), STUB_LEN, "{path:?} stub size");
        let mut magic = [0u8; 4];
        fs::File::open(path)
            .unwrap()
            .read_exact(&mut magic)
            .unwrap();
        if cfg!(windows) {
            assert_eq!(&magic[..2], b"MZ", "{path:?} missing MZ magic");
        } else {
            assert_eq!(&magic[..4], b"\x7fELF", "{path:?} missing ELF magic");
        }
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

#[cfg(all(test, not(docsrs)))]
#[allow(dead_code)]
#[path = "../build_support/layout.rs"]
mod release_layout;
