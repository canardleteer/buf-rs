//! Map Rust `TARGET` triples to Buf GitHub release asset name segments.

/// Describes one Buf release platform (asset filenames use `asset_suffix`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseTarget {
    /// e.g. `Linux-x86_64`, `Darwin-arm64`
    pub asset_suffix: &'static str,
    pub windows: bool,
}

/// All targets we support (same assets as upstream Buf releases).
pub const ALL: &[ReleaseTarget] = &[
    ReleaseTarget {
        asset_suffix: "Linux-x86_64",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Linux-aarch64",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Linux-armv7",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Linux-ppc64le",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Linux-s390x",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Linux-riscv64",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Darwin-x86_64",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Darwin-arm64",
        windows: false,
    },
    ReleaseTarget {
        asset_suffix: "Windows-x86_64",
        windows: true,
    },
    ReleaseTarget {
        asset_suffix: "Windows-arm64",
        windows: true,
    },
];

/// Resolve `ReleaseTarget` from Cargo `TARGET` (compilation triple).
pub fn from_rust_triple(triple: &str) -> Option<ReleaseTarget> {
    Some(match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => ReleaseTarget {
            asset_suffix: "Linux-x86_64",
            windows: false,
        },
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => ReleaseTarget {
            asset_suffix: "Linux-aarch64",
            windows: false,
        },
        "arm-unknown-linux-gnueabihf" | "arm-unknown-linux-musleabihf" => ReleaseTarget {
            asset_suffix: "Linux-armv7",
            windows: false,
        },
        "powerpc64le-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-ppc64le",
            windows: false,
        },
        "s390x-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-s390x",
            windows: false,
        },
        "riscv64gc-unknown-linux-gnu" | "riscv64-unknown-linux-gnu" => ReleaseTarget {
            asset_suffix: "Linux-riscv64",
            windows: false,
        },
        "x86_64-apple-darwin" => ReleaseTarget {
            asset_suffix: "Darwin-x86_64",
            windows: false,
        },
        "aarch64-apple-darwin" => ReleaseTarget {
            asset_suffix: "Darwin-arm64",
            windows: false,
        },
        "x86_64-pc-windows-gnu" | "x86_64-pc-windows-msvc" => ReleaseTarget {
            asset_suffix: "Windows-x86_64",
            windows: true,
        },
        "aarch64-pc-windows-msvc" | "aarch64-pc-windows-gnu" => ReleaseTarget {
            asset_suffix: "Windows-arm64",
            windows: true,
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
