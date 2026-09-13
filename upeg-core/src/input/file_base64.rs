const ENCODED_QUANTUM_BYTES: usize = 4;
const DECODED_QUANTUM_BYTES: usize = 3;
const BASE64_PADDING: u8 = b'=';
const ONE_BYTE_TAIL_UNUSED_BITS: u8 = 4;
const TWO_BYTE_TAIL_UNUSED_BITS: u8 = 2;
const LOWERCASE_SEXTET_OFFSET: u8 = 26;
const DIGIT_SEXTET_OFFSET: u8 = 52;
const PLUS_SEXTET: u8 = 62;
const SLASH_SEXTET: u8 = 63;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum CanonicalBase64Error {
    #[error("encoded length {length} is not a multiple of 4")]
    InvalidLength { length: usize },
    #[error("invalid base64 byte 0x{byte:02x} at index {index}")]
    InvalidByte { index: usize, byte: u8 },
    #[error("non-canonical trailing bits in the final base64 quantum")]
    NonCanonicalTrailingBits,
    #[error("decoded base64 length overflows this platform")]
    DecodedLengthOverflow,
}

pub(super) fn canonical_base64_decoded_len(encoded: &str) -> Result<usize, CanonicalBase64Error> {
    let bytes = encoded.as_bytes();
    if !bytes.len().is_multiple_of(ENCODED_QUANTUM_BYTES) {
        return Err(CanonicalBase64Error::InvalidLength {
            length: bytes.len(),
        });
    }
    if bytes.is_empty() {
        return Ok(0);
    }

    let padding = trailing_padding(bytes);
    let data_len = bytes
        .len()
        .checked_sub(padding)
        .ok_or(CanonicalBase64Error::DecodedLengthOverflow)?;
    for (index, byte) in bytes[..data_len].iter().copied().enumerate() {
        if sextet(byte).is_none() {
            return Err(CanonicalBase64Error::InvalidByte { index, byte });
        }
    }

    validate_tail_bits(bytes, data_len, padding)?;
    bytes
        .len()
        .checked_div(ENCODED_QUANTUM_BYTES)
        .and_then(|quantums| quantums.checked_mul(DECODED_QUANTUM_BYTES))
        .and_then(|decoded| decoded.checked_sub(padding))
        .ok_or(CanonicalBase64Error::DecodedLengthOverflow)
}

fn trailing_padding(bytes: &[u8]) -> usize {
    match bytes {
        [.., BASE64_PADDING, BASE64_PADDING] => 2,
        [.., BASE64_PADDING] => 1,
        _ => 0,
    }
}

fn validate_tail_bits(
    bytes: &[u8],
    data_len: usize,
    padding: usize,
) -> Result<(), CanonicalBase64Error> {
    let unused_bits = match padding {
        0 => return Ok(()),
        1 => TWO_BYTE_TAIL_UNUSED_BITS,
        2 => ONE_BYTE_TAIL_UNUSED_BITS,
        _ => return Err(CanonicalBase64Error::NonCanonicalTrailingBits),
    };
    let tail = data_len
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .and_then(|byte| sextet(*byte))
        .ok_or(CanonicalBase64Error::NonCanonicalTrailingBits)?;
    let unused_mask = (1_u8 << unused_bits) - 1;
    if tail & unused_mask == 0 {
        Ok(())
    } else {
        Err(CanonicalBase64Error::NonCanonicalTrailingBits)
    }
}

const fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + LOWERCASE_SEXTET_OFFSET),
        b'0'..=b'9' => Some(byte - b'0' + DIGIT_SEXTET_OFFSET),
        b'+' => Some(PLUS_SEXTET),
        b'/' => Some(SLASH_SEXTET),
        _ => None,
    }
}
