//! `cargo xtask` — workspace automation (see `.cargo/config.toml`).
//!
//! Quality loop: `cargo xtask check` (`ci` is the same command). Coverage:
//! `cargo xtask coverage` / `coverage-open`.
//!
//! **Buf upstream version:** The rule `major.minor.patch` from the crate semver (ignoring
//! pre-release / build metadata) must stay aligned with
//! `buf-tools/build.rs` and `buf-toolchain/build.rs` (`CARGO_PKG_VERSION` → GitHub tag `vX.Y.Z`).
//!
//! **Set the Buf pin (maintainers):** `cargo xtask workspace set-buf-version X.Y.Z` on the root
//! manifest, then `cargo generate-lockfile`, then `cargo xtask check` (see **`README.md`**).

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let _ =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("xtask=warn"))
            .format_timestamp(None)
            .try_init();

    xtask::run(xtask::Cli::parse())
}
