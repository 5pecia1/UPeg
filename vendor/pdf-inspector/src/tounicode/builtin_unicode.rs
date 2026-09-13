//! Decoder for the immutable bundled Unicode CMaps. Binary record layout:
//! https://github.com/mozilla/pdf.js/blob/master/src/core/binary_cmap.js
//! UPeg patch: raw initial values and subsequent deltas are distinct encodings.

use super::{load_builtin_cmap_by_name, merge_cmaps, BinaryCMapStream, ToUnicodeCMap};

const CODE_SPACE: u8 = 0;
const NOT_DEFINED: u8 = 1;
const CHARACTER: u8 = 4;
const RANGE: u8 = 5;
const METADATA_SUBTYPE_MASK: u8 = 0x1f;
const CID_BYTES: usize = 2;
const MAX_VALUE_BYTES: usize = 16;
const METADATA_RECORD: u8 = 7;
const USE_CMAP: u8 = 1;
const SEQUENTIAL: u8 = 0x10;
const WIDTH_MASK: u8 = 0x0f;

fn raw(stream: &mut BinaryCMapStream<'_>, width: usize) -> Result<u128, String> {
    Ok(stream
        .read_hex_bytes(width)?
        .into_iter()
        .fold(0, |n, b| (n << 8) | u128::from(b)))
}

fn delta(stream: &mut BinaryCMapStream<'_>, width: usize) -> Result<u128, String> {
    Ok(stream
        .read_hex_number(width - 1)?
        .into_iter()
        .fold(0, |n, b| (n << 8) | u128::from(b)))
}

fn insert(cmap: &mut ToUnicodeCMap, cid: u128, value: u128, width: usize) -> Result<(), String> {
    let cid = u16::try_from(cid).map_err(|_| "builtin CMap CID overflow")?;
    let bytes = value.to_be_bytes();
    let bytes = &bytes[MAX_VALUE_BYTES - width..];
    let text = if width % 2 == 0 {
        String::from_utf16_lossy(
            &bytes
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        bytes.iter().map(|&b| char::from(b)).collect()
    };
    cmap.char_map.insert(cid, text);
    Ok(())
}

pub(super) fn parse(data: &[u8]) -> Result<ToUnicodeCMap, String> {
    let mut stream = BinaryCMapStream::new(data);
    stream.read_byte().ok_or("missing builtin CMap header")?;
    let mut cmap = ToUnicodeCMap::new();
    let mut parent = None;
    while let Some(record) = stream.read_byte() {
        let kind = record >> 5;
        if kind == METADATA_RECORD {
            let value = stream.read_string()?;
            if record & METADATA_SUBTYPE_MASK == USE_CMAP {
                parent = Some(value);
            }
            continue;
        }
        let width = usize::from(record & WIDTH_MASK) + 1;
        let sequential = record & SEQUENTIAL != 0;
        let count = stream.read_number()?;
        if count == 0 {
            continue;
        }
        if matches!(kind, CODE_SPACE | NOT_DEFINED) {
            let mut end = 0;
            for index in 0..count {
                let start = if index == 0 {
                    raw(&mut stream, width)?
                } else {
                    end + 1 + delta(&mut stream, width)?
                };
                end = start + delta(&mut stream, width)?;
                if kind == NOT_DEFINED {
                    stream.read_number()?;
                }
            }
        } else if kind == CHARACTER {
            let mut cid = raw(&mut stream, CID_BYTES)?;
            let mut value = raw(&mut stream, width)?;
            insert(&mut cmap, cid, value, width)?;
            for _ in 1..count {
                cid += 1;
                if !sequential {
                    cid += delta(&mut stream, CID_BYTES)?;
                }
                let encoded = delta(&mut stream, width)?;
                let signed = if encoded & 1 == 0 {
                    encoded >> 1
                } else {
                    !(encoded >> 1)
                };
                value = value.wrapping_add(1).wrapping_add(signed);
                insert(&mut cmap, cid, value, width)?;
            }
        } else if kind == RANGE {
            let mut end = 0;
            for index in 0..count {
                let start = if index == 0 {
                    raw(&mut stream, CID_BYTES)?
                } else {
                    end + 1
                        + if sequential {
                            0
                        } else {
                            delta(&mut stream, CID_BYTES)?
                        }
                };
                end = start + delta(&mut stream, CID_BYTES)?;
                if end > u128::from(u16::MAX) {
                    return Err("builtin CMap range overflow".into());
                }
                let value = raw(&mut stream, width)?;
                for cid in start..=end {
                    insert(&mut cmap, cid, value.wrapping_add(cid - start), width)?;
                }
            }
        } else {
            return Err(format!("unsupported builtin Unicode CMap record {kind}"));
        }
    }
    if let Some(name) = parent {
        if let Some(base) = load_builtin_cmap_by_name(&name) {
            cmap = merge_cmaps(base, cmap);
        }
    }
    Ok(cmap)
}
