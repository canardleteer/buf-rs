//! Map Rust `TARGET` triples to Buf GitHub release asset name segments.

/// Describes one Buf release platform (asset filenames use `asset_suffix`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseTarget {
    /// e.g. `Linux-x86_64`, `Darwin-arm64`
    pub asset_suffix: &'static str,
    pub windows: bool,
    /// Lowest Buf **core** semver (e.g. `1.47.0`) that shipped this asset suffix.
    pub min_version: &'static str,
}

/// All targets we support (same assets as upstream Buf releases).
pub const ALL: &[ReleaseTarget] = &[
    ReleaseTarget {
        asset_suffix: "Linux-x86_64",
        windows: false,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "Linux-aarch64",
        windows: false,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "Linux-armv7",
        windows: false,
        min_version: "1.47.0",
    },
    ReleaseTarget {
        asset_suffix: "Linux-ppc64le",
        windows: false,
        min_version: "1.54.0",
    },
    ReleaseTarget {
        asset_suffix: "Linux-s390x",
        windows: false,
        min_version: "1.56.0",
    },
    ReleaseTarget {
        asset_suffix: "Linux-riscv64",
        windows: false,
        min_version: "1.54.0",
    },
    ReleaseTarget {
        asset_suffix: "Darwin-x86_64",
        windows: false,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "Darwin-arm64",
        windows: false,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "Windows-x86_64",
        windows: true,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "Windows-arm64",
        windows: true,
        min_version: "1.0.0",
    },
    ReleaseTarget {
        asset_suffix: "FreeBSD-x86_64",
        windows: false,
        min_version: "1.67.0",
    },
    ReleaseTarget {
        asset_suffix: "FreeBSD-arm64",
        windows: false,
        min_version: "1.67.0",
    },
    ReleaseTarget {
        asset_suffix: "OpenBSD-x86_64",
        windows: false,
        min_version: "1.67.0",
    },
    ReleaseTarget {
        asset_suffix: "OpenBSD-arm64",
        windows: false,
        min_version: "1.67.0",
    },
];

/// Resolve `ReleaseTarget` from Cargo `TARGET` (compilation triple).
pub fn from_rust_triple(triple: &str) -> Option<ReleaseTarget> {
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

pub fn remote_filename(prefix: &str, t: &ReleaseTarget) -> String {
    let suf = t.asset_suffix;
    match prefix {
        "buf" => format!("buf-{}{}", suf, if t.windows { ".exe" } else { "" }),
        "protoc-gen-buf-breaking" => format!(
            "protoc-gen-buf-breaking-{}{}",
            suf,
            if t.windows { ".exe" } else { "" }
        ),
        "protoc-gen-buf-lint" => format!(
            "protoc-gen-buf-lint-{}{}",
            suf,
            if t.windows { ".exe" } else { "" }
        ),
        _ => panic!("unknown prefix"),
    }
}

pub fn local_bin_names(windows: bool) -> [&'static str; 3] {
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

pub fn triples(t: &ReleaseTarget) -> [(String, String); 3] {
    let w = t.windows;
    [
        (remote_filename("buf", t), local_bin_names(w)[0].to_string()),
        (
            remote_filename("protoc-gen-buf-breaking", t),
            local_bin_names(w)[1].to_string(),
        ),
        (
            remote_filename("protoc-gen-buf-lint", t),
            local_bin_names(w)[2].to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{ALL, ReleaseTarget, from_rust_triple};
    use std::collections::{HashMap, HashSet};
    use std::fs;

    const METADATA_DRIFT_MSG: &str = "Cargo.toml `[package.metadata.buf-tools.targets]` and Rust `ALL` are out of sync. Reminder: also update the `Supported targets` table in `README.md` and the per-target list in `AGENTS.md`.";

    #[test]
    fn every_target_has_parseable_min_version() {
        for t in ALL {
            semver::Version::parse(t.min_version).unwrap_or_else(|e| {
                panic!(
                    "bad min_version {:?} for {}: {e}",
                    t.min_version, t.asset_suffix
                )
            });
        }
    }

    #[test]
    fn bsd_triples_resolve() {
        let cases = [
            ("x86_64-unknown-freebsd", "FreeBSD-x86_64", "1.67.0"),
            ("aarch64-unknown-freebsd", "FreeBSD-arm64", "1.67.0"),
            ("x86_64-unknown-openbsd", "OpenBSD-x86_64", "1.67.0"),
            ("aarch64-unknown-openbsd", "OpenBSD-arm64", "1.67.0"),
        ];
        for (triple, suffix, min) in cases {
            let rt = from_rust_triple(triple).expect(triple);
            assert_eq!(rt.asset_suffix, suffix);
            assert_eq!(rt.min_version, min);
            assert!(!rt.windows);
        }
    }

    #[test]
    fn cargo_metadata_matches_rust_const() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        // This module is `#[path]`-included from other crates (e.g. buf-toolchain); only
        // **buf-tools** carries `[package.metadata.buf-tools.targets]` in its manifest.
        let manifest_path = format!("{manifest_dir}/Cargo.toml");
        let raw = fs::read_to_string(&manifest_path).expect("read Cargo.toml");
        let value: toml::Value = toml::from_str(&raw).expect("parse Cargo.toml");
        let pkg_name = value
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str());
        if pkg_name != Some("buf-tools") {
            return;
        }

        let targets_tbl = value
            .get("package")
            .and_then(|p| p.get("metadata"))
            .and_then(|m| m.get("buf-tools"))
            .and_then(|b| b.get("targets"))
            .and_then(|t| t.as_table())
            .expect("missing [package.metadata.buf-tools.targets]");

        let mut rust_by_suffix: HashMap<&'static str, ReleaseTarget> = HashMap::new();
        for t in ALL {
            rust_by_suffix.insert(t.asset_suffix, *t);
        }

        let toml_keys: HashSet<&str> = targets_tbl.keys().map(String::as_str).collect();
        let rust_keys: HashSet<&str> = ALL.iter().map(|t| t.asset_suffix).collect();
        assert_eq!(
            toml_keys, rust_keys,
            "{METADATA_DRIFT_MSG} (suffix set mismatch: toml={toml_keys:?} rust={rust_keys:?})"
        );

        for (suffix, entry) in targets_tbl {
            let entry = entry.as_table().expect("target entry must be table");
            let min_version = entry
                .get("min_version")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    panic!("{METADATA_DRIFT_MSG} (missing min_version for {suffix})")
                });
            let windows = entry
                .get("windows")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| panic!("{METADATA_DRIFT_MSG} (missing windows for {suffix})"));
            let triples = entry
                .get("rust_triples")
                .and_then(|v| v.as_array())
                .unwrap_or_else(|| {
                    panic!("{METADATA_DRIFT_MSG} (missing rust_triples for {suffix})")
                });

            let rt = *rust_by_suffix
                .get(suffix.as_str())
                .unwrap_or_else(|| panic!("{METADATA_DRIFT_MSG} (unknown suffix {suffix})"));
            assert_eq!(
                min_version, rt.min_version,
                "{METADATA_DRIFT_MSG} (min_version mismatch for {suffix})"
            );
            assert_eq!(
                windows, rt.windows,
                "{METADATA_DRIFT_MSG} (windows mismatch for {suffix})"
            );

            for tv in triples {
                let triple = tv.as_str().unwrap_or_else(|| {
                    panic!("{METADATA_DRIFT_MSG} (rust_triples for {suffix} must be strings)")
                });
                let resolved = from_rust_triple(triple).unwrap_or_else(|| {
                    panic!("{METADATA_DRIFT_MSG} (triple {triple} not in from_rust_triple)")
                });
                assert_eq!(
                    resolved.asset_suffix, rt.asset_suffix,
                    "{METADATA_DRIFT_MSG} (triple {triple} maps to {} expected {})",
                    resolved.asset_suffix, rt.asset_suffix
                );
                assert_eq!(resolved.min_version, rt.min_version);
                assert_eq!(resolved.windows, rt.windows);
            }
        }
    }
}
