//! Minisign verification and sha256.txt parsing.

use std::collections::HashMap;

use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};

/// Official Buf release signing key (see <https://docs.buf.build/installation>).
pub const BUF_MINISIGN_PUBLIC_KEY_B64: &str =
    "RWQ/i9xseZwBVE7pEniCNjlNOeeyp4BQgdZDLQcAohxEAH5Uj5DEKjv6";

pub fn verify_minisign_signature(
    data: &[u8],
    minisig_text: &str,
    public_key_b64: &str,
) -> Result<(), String> {
    let pk =
        PublicKey::from_base64(public_key_b64).map_err(|e| format!("parse public key: {e}"))?;
    let sig = Signature::decode(minisig_text)
        .map_err(|e| format!("parse minisig signature text: {e}"))?;
    pk.verify(data, &sig, false)
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
