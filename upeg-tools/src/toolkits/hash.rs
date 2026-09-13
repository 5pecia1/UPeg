//! `hash` toolkit — SHA-256/-1/-512, CRC32, MD5.

use upeg_core::tool;

/// `hash.sha256` — SHA-256 of a UTF-8 string, formatted as 64 lowercase hex chars.
#[tool(
    id = "hash.sha256",
    display_label = "SHA-256",
    toolkit = "hash",
    description = "SHA-256 hash of a UTF-8 string (lowercase hex).",
    inputs = [
        required input: String = "UTF-8 string to hash",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// `hash.crc32` — IEEE CRC-32 checksum of a UTF-8 string, formatted as
/// 8 lowercase hex chars (zero-padded).
///
/// Common for file integrity checks, network packet validation, and
/// quick non-cryptographic dedup. NOT secure — collisions are trivial
/// to forge.
#[tool(
    id = "hash.crc32",
    display_label = "CRC32",
    toolkit = "hash",
    description = "IEEE CRC-32 checksum of a UTF-8 string (lowercase hex, 8 chars).",
    inputs = [
        required input: String = "UTF-8 string to checksum",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn crc32_hex(input: &str) -> String {
    let crc = crc32fast::hash(input.as_bytes());
    format!("{crc:08x}")
}

/// `hash.sha1` — SHA-1 of a UTF-8 string, formatted as 40 lowercase hex chars.
///
/// SHA-1 is no longer cryptographically secure but remains the de facto
/// hash for git object ids and many legacy systems — surfaced for that
/// interop. Use `hash.sha256`/`hash.sha512` for security-sensitive needs.
#[tool(
    id = "hash.sha1",
    display_label = "SHA-1",
    toolkit = "hash",
    description = "SHA-1 hash of a UTF-8 string (lowercase hex, 40 chars). Not cryptographically secure — git/legacy interop only.",
    inputs = [
        required input: String = "UTF-8 string to hash",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn sha1_hex(input: &str) -> String {
    use sha1::{Digest, Sha1};
    let digest = Sha1::digest(input.as_bytes());
    let mut s = String::with_capacity(40);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// `hash.sha512` — SHA-512 of a UTF-8 string, formatted as 128 lowercase hex chars.
#[tool(
    id = "hash.sha512",
    display_label = "SHA-512",
    toolkit = "hash",
    description = "SHA-512 hash of a UTF-8 string (lowercase hex, 128 chars).",
    inputs = [
        required input: String = "UTF-8 string to hash",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn sha512_hex(input: &str) -> String {
    use sha2::{Digest, Sha512};
    let digest = Sha512::digest(input.as_bytes());
    let mut s = String::with_capacity(128);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// `hash.md5` — MD5 of a UTF-8 string, formatted as 32 lowercase hex chars.
///
/// MD5 is broken for security but still common for non-cryptographic checksums
/// (file dedup, ETag-style compare, JS bundle hashing). Surfaced for that use.
#[tool(
    id = "hash.md5",
    display_label = "MD5",
    toolkit = "hash",
    description = "MD5 hash of a UTF-8 string (lowercase hex). Not cryptographically secure — use sha256 for that.",
    inputs = [
        required input: String = "UTF-8 string to hash",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn md5_hex(input: &str) -> String {
    use md5::{Digest, Md5};
    let digest = Md5::digest(input.as_bytes());
    let mut s = String::with_capacity(32);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── sha256 ─────────────────────────────────────────────────

    #[test]
    fn sha256_알려진_벡터들을_검증한다() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
    }

    #[test]
    fn sha256는_64_소문자_hex_문자들이다() {
        let h = sha256_hex("anything");
        assert_eq!(h.len(), 64);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    // ─── crc32 ──────────────────────────────────────────────────

    #[test]
    fn crc32_알려진_벡터들을_검증한다() {
        assert_eq!(crc32_hex(""), "00000000");
        assert_eq!(crc32_hex("123456789"), "cbf43926");
        assert_eq!(crc32_hex("abc"), "352441c2");
    }

    #[test]
    fn crc32는_8_소문자_hex_문자들이다() {
        let h = crc32_hex("anything");
        assert_eq!(h.len(), 8);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    // ─── sha1 / sha512 ─────────────────────────────────────────

    #[test]
    fn sha1_알려진_벡터들을_검증한다() {
        assert_eq!(sha1_hex(""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn sha1는_40_소문자_hex_문자들이다() {
        let h = sha1_hex("anything");
        assert_eq!(h.len(), 40);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn sha512_알려진_벡터들을_검증한다() {
        assert_eq!(
            sha512_hex(""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
        );
        assert_eq!(
            sha512_hex("abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        );
    }

    #[test]
    fn sha512는_128_소문자_hex_문자들이다() {
        let h = sha512_hex("anything");
        assert_eq!(h.len(), 128);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    // ─── md5 ────────────────────────────────────────────────────

    #[test]
    fn md5_알려진_벡터들을_검증한다() {
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn md5는_32_소문자_hex_문자들이다() {
        let h = md5_hex("anything");
        assert_eq!(h.len(), 32);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}
