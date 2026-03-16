//! Minimal shared hashing helpers for buildfix crates.

use sha2::{Digest, Sha256};

/// Return the lowercase hexadecimal SHA-256 digest for the provided bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_stable() {
        assert_eq!(sha256_hex(b"workspace"), sha256_hex(b"workspace"));
        assert_ne!(sha256_hex(b"a"), sha256_hex(b"b"));
        assert_eq!(sha256_hex(b"workspace").len(), 64);
    }
}
