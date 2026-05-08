//! Minisign verification and sha256.txt parsing.

use std::collections::HashMap;

use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};

/// Buf release signing key. **Stable since v1.0.0** — same key id
/// (`3f8bdc6c799c0154`) across the whole v1.x line. The signature algorithm tag
/// in `sha256.txt.minisig` flipped from raw Ed25519 (`Ed`/RWQ, v1.0.0–v1.11.0)
/// to Ed25519+BLAKE2b-512-prehashed (`ED`/RUQ, v1.12.0+) — see
/// `PREHASHED_MINISIGN_MIN_VERSION`. Both modes verify against this single
/// public key.
pub const BUF_MINISIGN_PUBLIC_KEY_B64: &str =
    "RWQ/i9xseZwBVE7pEniCNjlNOeeyp4BQgdZDLQcAohxEAH5Uj5DEKjv6";

/// Lowest Buf release that signs `sha256.txt.minisig` with Ed25519+BLAKE2b
/// prehash (`ED`/RUQ). Releases **strictly below** this version sign with raw
/// Ed25519 (`Ed`/RWQ) and require `allow_legacy = true` to verify; v1.12.0+
/// must verify in strict mode.
pub const PREHASHED_MINISIGN_MIN_VERSION: &str = "1.12.0";

/// Verifies a Buf-style `sha256.txt.minisig` against `BUF_MINISIGN_PUBLIC_KEY_B64`.
///
/// `allow_legacy` is the third arg of `minisign_verify::PublicKey::verify` and
/// MUST be `true` only when the verified payload is from a Buf release in the
/// raw-Ed25519 era (v1.0.0–v1.11.0). For v1.12.0+ pass `false` so the strict
/// prehashed path is enforced. The caller (typically `build.rs`) decides this
/// based on the pinned Buf version vs. `PREHASHED_MINISIGN_MIN_VERSION`.
pub fn verify_minisign_signature(
    data: &[u8],
    minisig_text: &str,
    public_key_b64: &str,
    allow_legacy: bool,
) -> Result<(), String> {
    let pk =
        PublicKey::from_base64(public_key_b64).map_err(|e| format!("parse public key: {e}"))?;
    let sig = Signature::decode(minisig_text)
        .map_err(|e| format!("parse minisig signature text: {e}"))?;
    pk.verify(data, &sig, allow_legacy)
        .map_err(|e| format!("minisign verify failed: {e}"))
}

pub fn parse_sha256_list(data: &[u8]) -> Result<HashMap<String, String>, String> {
    let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
    let mut m = HashMap::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
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

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{BUF_MINISIGN_PUBLIC_KEY_B64, verify_minisign_signature};

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/build_support/test_fixtures/{name}",
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {name}: {e}"))
    }

    #[test]
    fn modern_strict_verifies() {
        let data = fixture("v1_69_0_sha256.txt");
        let sig = fixture("v1_69_0_sha256.txt.minisig");
        verify_minisign_signature(data.as_bytes(), &sig, BUF_MINISIGN_PUBLIC_KEY_B64, false)
            .expect("v1.69.0 fixture must verify in strict mode");
    }

    #[test]
    fn legacy_with_allow_legacy_verifies() {
        let data = fixture("v1_0_0_sha256.txt");
        let sig = fixture("v1_0_0_sha256.txt.minisig");
        verify_minisign_signature(data.as_bytes(), &sig, BUF_MINISIGN_PUBLIC_KEY_B64, true)
            .expect("v1.0.0 fixture must verify with allow_legacy");
    }

    #[test]
    fn legacy_without_allow_legacy_rejects() {
        let data = fixture("v1_0_0_sha256.txt");
        let sig = fixture("v1_0_0_sha256.txt.minisig");
        let err =
            verify_minisign_signature(data.as_bytes(), &sig, BUF_MINISIGN_PUBLIC_KEY_B64, false)
                .expect_err("v1.0.0 raw-Ed25519 minisig must fail strict mode");
        assert!(
            err.contains("minisign verify failed") || err.contains("UnexpectedAlgorithm"),
            "unexpected error: {err}"
        );
    }
}
