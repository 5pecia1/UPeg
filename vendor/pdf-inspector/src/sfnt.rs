//! sfnt (TrueType/OpenType) helpers built on `skrifa`/`read-fonts`.
//!
//! Local patch: replaces `ttf-parser` (RUSTSEC-2026-0192, unmaintained) with
//! the maintained fontations stack, which is already in UPeg's dependency
//! graph via hayro/usvg. The helpers below mirror the exact semantics of the
//! `ttf_parser` calls they replace.

use skrifa::raw::tables::cmap::{CmapSubtable, EncodingRecord, PlatformId};
use skrifa::raw::tables::os2::SelectionFlags;
use skrifa::raw::TableProvider;
use skrifa::FontRef;

/// Parse the font at index 0 — accepts single fonts and collections,
/// like `ttf_parser::Face::parse(data, 0)`.
pub fn parse(data: &[u8]) -> Option<FontRef<'_>> {
    FontRef::from_index(data, 0).ok()
}

/// Iterate each cmap encoding record with its parsed subtable, like
/// `face.tables().cmap.subtables` in ttf_parser. Malformed subtables are
/// skipped instead of failing the whole table.
pub fn cmap_subtables(font: &FontRef<'_>, mut f: impl FnMut(&EncodingRecord, &CmapSubtable<'_>)) {
    let Ok(cmap) = font.cmap() else {
        return;
    };
    let data = cmap.offset_data();
    for record in cmap.encoding_records() {
        if let Ok(subtable) = record.subtable(data) {
            f(record, &subtable);
        }
    }
}

/// `ttf_parser::Subtable::is_unicode` — Unicode platform, Windows BMP (3,1),
/// or Windows full-repertoire (3,10) backed by a format 12/13 subtable.
pub fn is_unicode_subtable(record: &EncodingRecord, subtable: &CmapSubtable) -> bool {
    match record.platform_id() {
        PlatformId::Unicode => true,
        PlatformId::Windows if record.encoding_id() == 1 => true,
        PlatformId::Windows => {
            record.encoding_id() == 10
                && matches!(
                    subtable,
                    CmapSubtable::Format12(_) | CmapSubtable::Format13(_)
                )
        }
        _ => false,
    }
}

/// `ttf_parser::Subtable::glyph_index` — maps a codepoint to a u16 glyph id.
/// Returns `None` for unmapped codepoints and glyph id 0 (the `.notdef`
/// sentinel, never a real mapping).
pub fn glyph_index(subtable: &CmapSubtable, codepoint: u32) -> Option<u16> {
    subtable
        .map_codepoint(codepoint)
        .and_then(|g| u16::try_from(g.to_u32()).ok())
        .filter(|&gid| gid != 0)
}

/// True when the subtable defines at least one codepoint mapping
/// (`ttf_parser::Subtable::codepoints` counts defined codepoints).
/// `CmapSubtable::iter` only covers formats 4/6/10/12/13, so formats 0 and 2
/// get a bounded direct probe.
pub fn has_mappings(subtable: &CmapSubtable) -> bool {
    if subtable.iter().next().is_some() {
        return true;
    }
    match subtable {
        CmapSubtable::Format0(_) | CmapSubtable::Format2(_) => {
            (0u32..=0xFFFF).any(|c| glyph_index(subtable, c).is_some())
        }
        _ => false,
    }
}

/// `ttf_parser::Face::italic_angle` (post italicAngle; 0.0 when absent).
pub fn italic_angle(font: &FontRef<'_>) -> f32 {
    font.post()
        .map(|p| p.italic_angle().to_f32())
        .unwrap_or(0.0)
}

/// `ttf_parser::Face::is_italic` — OS/2 fsSelection ITALIC, or a non-zero
/// post italic angle.
pub fn is_italic(font: &FontRef<'_>) -> bool {
    let flagged = font
        .os2()
        .map(|os2| os2.fs_selection().contains(SelectionFlags::ITALIC))
        .unwrap_or(false);
    flagged || italic_angle(font) != 0.0
}

/// `ttf_parser::Face::is_bold` — OS/2 fsSelection BOLD (false without OS/2).
pub fn is_bold(font: &FontRef<'_>) -> bool {
    font.os2()
        .map(|os2| os2.fs_selection().contains(SelectionFlags::BOLD))
        .unwrap_or(false)
}
