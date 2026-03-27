//! Comprehensive unit tests for buildfix-hash crate.
//!
//! Tests cover:
//! - Known SHA256 test vectors
//! - Empty input handling
//! - Determinism (same input → same output)
//! - Format verification (hex encoding, lowercase)
//! - Edge cases (large input, special characters, binary data)

use buildfix_hash::sha256_hex;

/// Helper to verify hash format: 64 lowercase hex characters
fn assert_valid_hex_hash(hash: &str) {
    assert_eq!(hash.len(), 64, "SHA256 hash must be 64 characters");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
        "Hash must be lowercase hex: {}",
        hash
    );
}

// =============================================================================
// Known SHA256 Test Vectors
// =============================================================================

#[test]
fn test_known_vector_empty_string() {
    // SHA256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    let hash = sha256_hex(b"");
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_known_vector_single_char_a() {
    // SHA256("a") = ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb
    let hash = sha256_hex(b"a");
    assert_eq!(
        hash,
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"
    );
}

#[test]
fn test_known_vector_abc() {
    // SHA256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    let hash = sha256_hex(b"abc");
    assert_eq!(
        hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn test_known_vector_message_digest() {
    // SHA256("message digest") = f7846f55cf23e14eebeab5b4e1550cad5b509e3348fbc4efa3a1413d393cb650
    let hash = sha256_hex(b"message digest");
    assert_eq!(
        hash,
        "f7846f55cf23e14eebeab5b4e1550cad5b509e3348fbc4efa3a1413d393cb650"
    );
}

#[test]
fn test_known_vector_quick_brown_fox() {
    // SHA256("The quick brown fox jumps over the lazy dog")
    // = d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592
    let hash = sha256_hex(b"The quick brown fox jumps over the lazy dog");
    assert_eq!(
        hash,
        "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
    );
}

#[test]
fn test_known_vector_quick_brown_fox_with_period() {
    // SHA256("The quick brown fox jumps over the lazy dog.")
    // = ef537f25c895bfa782526529a9b63d97aa631564d5d789c2b765448c8635fb6c
    let hash = sha256_hex(b"The quick brown fox jumps over the lazy dog.");
    assert_eq!(
        hash,
        "ef537f25c895bfa782526529a9b63d97aa631564d5d789c2b765448c8635fb6c"
    );
}

#[test]
fn test_known_vector_alphabet_lowercase() {
    // SHA256("abcdefghijklmnopqrstuvwxyz")
    // = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    // Wait, this is actually different. Let me use the correct one:
    // SHA256("abcdefghijklmnopqrstuvwxyz")
    // = 71c480df93d6ae2f1efad1447c66c9525e316218cf51fc8d9ed832f2daf18b73
    let hash = sha256_hex(b"abcdefghijklmnopqrstuvwxyz");
    assert_eq!(
        hash,
        "71c480df93d6ae2f1efad1447c66c9525e316218cf51fc8d9ed832f2daf18b73"
    );
}

// =============================================================================
// Determinism Tests
// =============================================================================

#[test]
fn test_determinism_same_input_same_output() {
    let inputs = [
        b"".as_slice(),
        b"a".as_slice(),
        b"hello world".as_slice(),
        b"The quick brown fox jumps over the lazy dog".as_slice(),
    ];

    for input in inputs {
        let hash1 = sha256_hex(input);
        let hash2 = sha256_hex(input);
        let hash3 = sha256_hex(input);
        assert_eq!(hash1, hash2, "Hash should be deterministic for {:?}", input);
        assert_eq!(hash2, hash3, "Hash should be deterministic for {:?}", input);
    }
}

#[test]
fn test_determinism_different_inputs_different_outputs() {
    let hash_a = sha256_hex(b"a");
    let hash_b = sha256_hex(b"b");
    let hash_c = sha256_hex(b"c");

    assert_ne!(
        hash_a, hash_b,
        "Different inputs should produce different hashes"
    );
    assert_ne!(
        hash_b, hash_c,
        "Different inputs should produce different hashes"
    );
    assert_ne!(
        hash_a, hash_c,
        "Different inputs should produce different hashes"
    );
}

#[test]
fn test_determinism_case_sensitivity() {
    let hash_lower = sha256_hex(b"abc");
    let hash_upper = sha256_hex(b"ABC");
    let hash_mixed = sha256_hex(b"AbC");

    assert_ne!(hash_lower, hash_upper, "Hash should be case-sensitive");
    assert_ne!(hash_lower, hash_mixed, "Hash should be case-sensitive");
    assert_ne!(hash_upper, hash_mixed, "Hash should be case-sensitive");
}

// =============================================================================
// Format Verification Tests
// =============================================================================

#[test]
fn test_format_length() {
    // SHA256 always produces 32 bytes = 64 hex characters
    assert_eq!(sha256_hex(b"").len(), 64);
    assert_eq!(sha256_hex(b"a").len(), 64);
    assert_eq!(sha256_hex(b"abc").len(), 64);
    assert_eq!(sha256_hex(&[0u8; 1000]).len(), 64);
    assert_eq!(sha256_hex(&[0xFFu8; 1000]).len(), 64);
}

#[test]
fn test_format_lowercase() {
    let hash = sha256_hex(b"abc");
    // Verify no uppercase letters
    assert!(
        !hash.chars().any(|c| c.is_ascii_uppercase()),
        "Hash should be lowercase: {}",
        hash
    );
}

#[test]
fn test_format_valid_hex_characters() {
    let test_cases: Vec<&[u8]> = vec![
        b"",
        b"a",
        b"abc",
        b"hello world",
        &[0x00, 0x01, 0x02, 0x03],
        &[0xFE, 0xFF],
    ];

    for input in test_cases {
        let hash = sha256_hex(input);
        assert_valid_hex_hash(&hash);
    }
}

// =============================================================================
// Edge Cases Tests
// =============================================================================

#[test]
fn test_edge_case_empty_input() {
    let hash = sha256_hex(b"");
    assert_valid_hex_hash(&hash);
    // Known value for empty input
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_edge_case_single_byte_zero() {
    let hash = sha256_hex(&[0x00]);
    assert_valid_hex_hash(&hash);
    // SHA256([0x00]) = 6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d
    assert_eq!(
        hash,
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
    );
}

#[test]
fn test_edge_case_single_byte_max() {
    let hash = sha256_hex(&[0xFF]);
    assert_valid_hex_hash(&hash);
    // SHA256([0xFF]) = a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89
    assert_eq!(
        hash,
        "a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89"
    );
}

#[test]
fn test_edge_case_all_byte_values() {
    // Test with all possible byte values
    let all_bytes: Vec<u8> = (0u8..=255).collect();
    let hash = sha256_hex(&all_bytes);
    assert_valid_hex_hash(&hash);
}

#[test]
fn test_edge_case_binary_data() {
    // Test with binary data containing null bytes and non-printable characters
    let binary: Vec<u8> = vec![0x00, 0x01, 0x02, 0x7F, 0x80, 0xFE, 0xFF];
    let hash = sha256_hex(&binary);
    assert_valid_hex_hash(&hash);
}

#[test]
fn test_edge_case_repeated_bytes() {
    // Test with repeated patterns
    let zeros = [0u8; 64];
    let ones = [1u8; 64];
    let ff = [0xFFu8; 64];

    let hash_zeros = sha256_hex(&zeros);
    let hash_ones = sha256_hex(&ones);
    let hash_ff = sha256_hex(&ff);

    assert_valid_hex_hash(&hash_zeros);
    assert_valid_hex_hash(&hash_ones);
    assert_valid_hex_hash(&hash_ff);

    // All should be different
    assert_ne!(hash_zeros, hash_ones);
    assert_ne!(hash_ones, hash_ff);
    assert_ne!(hash_zeros, hash_ff);
}

#[test]
fn test_edge_case_large_input() {
    // Test with a large input (1MB)
    let large_input: Vec<u8> = (0..=255).cycle().take(1024 * 1024).collect();
    let hash = sha256_hex(&large_input);
    assert_valid_hex_hash(&hash);
}

#[test]
fn test_edge_case_whitespace() {
    let hash_space = sha256_hex(b" ");
    let hash_tab = sha256_hex(b"\t");
    let hash_newline = sha256_hex(b"\n");
    let hash_crlf = sha256_hex(b"\r\n");
    let hash_multiple_spaces = sha256_hex(b"   ");

    assert_valid_hex_hash(&hash_space);
    assert_valid_hex_hash(&hash_tab);
    assert_valid_hex_hash(&hash_newline);
    assert_valid_hex_hash(&hash_crlf);
    assert_valid_hex_hash(&hash_multiple_spaces);

    // All whitespace variants should produce different hashes
    assert_ne!(hash_space, hash_tab);
    assert_ne!(hash_tab, hash_newline);
    assert_ne!(hash_newline, hash_crlf);
}

#[test]
fn test_edge_case_special_characters() {
    let special_chars = b"!@#$%^&*()_+-=[]{}|;':\",./<>?`~";
    let hash = sha256_hex(special_chars);
    assert_valid_hex_hash(&hash);
}

#[test]
fn test_edge_case_unicode_utf8() {
    // UTF-8 encoded strings
    let unicode_hello = "Hello, 世界! 🌍".as_bytes();
    let emoji = "🎉🚀✨".as_bytes();
    let japanese = "こんにちは".as_bytes();
    let arabic = "مرحبا".as_bytes();

    let hash_hello = sha256_hex(unicode_hello);
    let hash_emoji = sha256_hex(emoji);
    let hash_japanese = sha256_hex(japanese);
    let hash_arabic = sha256_hex(arabic);

    assert_valid_hex_hash(&hash_hello);
    assert_valid_hex_hash(&hash_emoji);
    assert_valid_hex_hash(&hash_japanese);
    assert_valid_hex_hash(&hash_arabic);

    // All should be different
    assert_ne!(hash_hello, hash_emoji);
    assert_ne!(hash_emoji, hash_japanese);
    assert_ne!(hash_japanese, hash_arabic);
}

#[test]
fn test_edge_case_long_repeated_pattern() {
    // Test with a long repeated pattern
    let pattern = b"abc";
    let repeated: Vec<u8> = pattern.iter().cycle().take(1000).copied().collect();
    let hash = sha256_hex(&repeated);
    assert_valid_hex_hash(&hash);
}

// =============================================================================
// Incremental/Consistency Tests
// =============================================================================

#[test]
fn test_consistency_with_concatenation() {
    // Verify that hashing "ab" is different from hashing "a" then "b" separately
    let hash_ab = sha256_hex(b"ab");
    let hash_a = sha256_hex(b"a");
    let hash_b = sha256_hex(b"b");

    // These should all be different
    assert_ne!(hash_ab, hash_a);
    assert_ne!(hash_ab, hash_b);
    assert_ne!(hash_a, hash_b);
}

#[test]
fn test_prefix_suffix() {
    // Adding a prefix or suffix should change the hash
    let base = b"hello";
    let with_prefix = b"hello world";
    let with_suffix = b"say hello";

    let hash_base = sha256_hex(base);
    let hash_prefix = sha256_hex(with_prefix);
    let hash_suffix = sha256_hex(with_suffix);

    assert_ne!(hash_base, hash_prefix);
    assert_ne!(hash_base, hash_suffix);
    assert_ne!(hash_prefix, hash_suffix);
}

// =============================================================================
// Avalanche Effect Tests
// =============================================================================

#[test]
fn test_avalanche_single_bit_difference() {
    // Small changes in input should produce vastly different outputs
    let hash1 = sha256_hex(b"hello");
    let hash2 = sha256_hex(b"hellp"); // one character different

    // Count different hex characters
    let diff_count = hash1
        .chars()
        .zip(hash2.chars())
        .filter(|(a, b)| a != b)
        .count();

    // SHA256 avalanche effect: approximately 50% of bits should change
    // For 64 hex characters, we expect roughly 32 to be different
    // Being conservative, at least 20 should differ
    assert!(
        diff_count > 20,
        "Avalanche effect: expected many differing chars, got {}",
        diff_count
    );
}

#[test]
fn test_avalanche_empty_vs_single_char() {
    let hash_empty = sha256_hex(b"");
    let hash_single = sha256_hex(b"x");

    let diff_count = hash_empty
        .chars()
        .zip(hash_single.chars())
        .filter(|(a, b)| a != b)
        .count();

    assert!(
        diff_count > 20,
        "Avalanche effect: expected many differing chars, got {}",
        diff_count
    );
}
