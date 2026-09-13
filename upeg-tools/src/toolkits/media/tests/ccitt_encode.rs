//! A minimal CCITT (ITU-T T.4/T.6) *encoder*, used only to build fixtures for
//! the CCITTFaxDecode tests.
//!
//! Nothing in the dependency tree encodes CCITT — `hayro-ccitt` only decodes —
//! so the round-trip tests need this. That is also what makes the round trip
//! worth anything: the encoder here and the decoder under test are independent
//! implementations of T.4/T.6, so a bug in either surfaces as a fixture that
//! does not decode back to the rows it was built from.
//!
//! Only what the fixtures need is implemented. Run lengths are coded with the
//! terminating codes alone, so a fixture row may be at most [`MAX_RUN`] pixels
//! wide; the make-up codes needed beyond that are omitted rather than left
//! untested.

/// The longest run the terminating codes cover (T.4 Table 2), and so the widest
/// fixture row this encoder accepts.
const MAX_RUN: usize = 63;

const BITS_PER_BYTE: u8 = 8;

/// The largest `|a1 - b1|` the vertical modes can code (T.4 Table 4).
const VERTICAL_MAX_DELTA: i64 = 3;

/// `a0` starts one pixel left of the line, on an imaginary white pixel.
const LINE_START: i64 = -1;

/// The coding a fixture stream uses, mirroring the `/K` it is decoded with.
#[derive(Clone, Copy)]
pub(super) enum Coding {
    /// `/K < 0`: pure two-dimensional (T.6 / Group 4).
    Group4,
    /// `/K == 0`: pure one-dimensional (T.4 / MH). No EOL codes are emitted,
    /// which is what `/EndOfLine false` — the PDF default — describes.
    Group3OneDimensional,
    /// `/K > 0`: mixed 1-D/2-D (T.4 / MR). Every `k`th line is 1-D coded and
    /// the rest are 2-D, each behind the tag bit that says which — the shape a
    /// real `/K > 0` producer emits.
    Group3Mixed { k: u32 },
}

/// Builds a CCITT stream out of bilevel rows (`true` = black).
pub(super) struct CcittEncoder {
    pub(super) coding: Coding,
    /// `/EncodedByteAlign`: pad every row out to a byte boundary.
    pub(super) byte_align: bool,
    /// `/EndOfLine`: precede every row with an EOL code. Group 4 streams never
    /// carry one.
    pub(super) emit_eol: bool,
}

impl CcittEncoder {
    pub(super) fn encode(&self, rows: &[Vec<bool>]) -> Vec<u8> {
        let mut writer = BitWriter::default();
        // The line above the first is an imaginary all-white one, i.e. a line
        // with no changing elements at all.
        let mut reference: Vec<u32> = Vec::new();

        for (y, row) in rows.iter().enumerate() {
            assert!(
                row.len() <= MAX_RUN,
                "fixture row of {} pixels exceeds the {MAX_RUN}-pixel terminating codes",
                row.len()
            );
            let width = row.len() as u32;
            if self.emit_eol {
                writer.write(EOL);
            }
            match self.coding {
                Coding::Group4 => encode_2d_line(&mut writer, row, &reference, width),
                Coding::Group3OneDimensional => encode_1d_line(&mut writer, row),
                Coding::Group3Mixed { k } => {
                    let one_dimensional = (y as u32).is_multiple_of(k);
                    writer.push_bit(one_dimensional);
                    if one_dimensional {
                        encode_1d_line(&mut writer, row);
                    } else {
                        encode_2d_line(&mut writer, row, &reference, width);
                    }
                }
            }
            if self.byte_align {
                writer.align();
            }
            reference = changing_elements(row);
        }
        writer.finish()
    }
}

/// Accumulates codes MSB-first, the order `hayro-ccitt`'s reader reads them in.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    partial: u8,
    bits: u8,
}

impl BitWriter {
    fn push_bit(&mut self, bit: bool) {
        self.partial = (self.partial << 1) | u8::from(bit);
        self.bits += 1;
        if self.bits == BITS_PER_BYTE {
            self.bytes.push(self.partial);
            self.partial = 0;
            self.bits = 0;
        }
    }

    fn write(&mut self, (length, code): (u8, u16)) {
        for i in (0..length).rev() {
            self.push_bit((code >> i) & 1 == 1);
        }
    }

    /// Pad out to the next byte boundary with zero bits.
    fn align(&mut self) {
        while self.bits != 0 {
            self.push_bit(false);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }
}

/// The changing elements of `row`: every index whose colour differs from the
/// pixel to its left, with an imaginary white pixel left of index 0. They
/// alternate colour, so the one at an even position is a change to black.
fn changing_elements(row: &[bool]) -> Vec<u32> {
    let mut changes = Vec::new();
    let mut previous = false;
    for (x, &black) in row.iter().enumerate() {
        if black != previous {
            changes.push(x as u32);
            previous = black;
        }
    }
    changes
}

/// Whether the changing element at `position` is a change to black.
fn change_is_black(position: usize) -> bool {
    position.is_multiple_of(2)
}

/// The position in `changes` of the first changing element strictly right of
/// `a0` whose colour is the opposite of `color_is_black`.
fn find_change(changes: &[u32], a0: i64, color_is_black: bool) -> Option<usize> {
    changes.iter().enumerate().find_map(|(position, &idx)| {
        (i64::from(idx) > a0 && change_is_black(position) != color_is_black).then_some(position)
    })
}

/// Code `row` one-dimensionally (T.4 §4.1): alternating run lengths, always
/// starting with a white run, which is zero-length when the row opens on black.
fn encode_1d_line(writer: &mut BitWriter, row: &[bool]) {
    let mut color_is_black = false;
    let mut x = 0;
    while x < row.len() {
        let start = x;
        while x < row.len() && row[x] == color_is_black {
            x += 1;
        }
        write_run(writer, x - start, color_is_black);
        color_is_black = !color_is_black;
    }
}

/// Code `row` two-dimensionally against `reference` (T.4 §4.2 / T.6 §2.2).
fn encode_2d_line(writer: &mut BitWriter, row: &[bool], reference: &[u32], width: u32) {
    let coding = changing_elements(row);
    let mut a0: i64 = LINE_START;
    let mut color_is_black = false;

    while a0 < i64::from(width) {
        let a1 = change_at(&coding, a0, color_is_black, width);
        let b1_position = find_change(reference, a0, color_is_black);
        let b1 = b1_position.map_or(width, |i| reference[i]);
        let b2 = b1_position.map_or(width, |i| reference.get(i + 1).copied().unwrap_or(width));

        if b2 < a1 {
            // Pass mode: the run passes over b2 without a change of its own.
            writer.write(MODE_PASS);
            a0 = i64::from(b2);
            continue;
        }
        let delta = i64::from(a1) - i64::from(b1);
        if delta.abs() <= VERTICAL_MAX_DELTA {
            writer.write(VERTICAL_MODES[(delta + VERTICAL_MAX_DELTA) as usize]);
            a0 = i64::from(a1);
            color_is_black = !color_is_black;
            continue;
        }
        // Horizontal mode: a1 is too far from b1 to code as an offset, so the
        // two runs a0a1 and a1a2 are coded outright.
        let a2 = change_at(&coding, i64::from(a1), !color_is_black, width);
        let start = a0.max(0) as u32;
        writer.write(MODE_HORIZONTAL);
        write_run(writer, (a1 - start) as usize, color_is_black);
        write_run(writer, (a2 - a1) as usize, !color_is_black);
        a0 = i64::from(a2);
    }
}

/// The first changing element strictly right of `a0` of the opposite colour to
/// `color_is_black`, or `width` when the line has none left.
fn change_at(changes: &[u32], a0: i64, color_is_black: bool, width: u32) -> u32 {
    find_change(changes, a0, color_is_black).map_or(width, |i| changes[i])
}

fn write_run(writer: &mut BitWriter, run: usize, black: bool) {
    assert!(
        run <= MAX_RUN,
        "fixture run of {run} pixels exceeds the {MAX_RUN}-pixel terminating codes"
    );
    let table = if black {
        &BLACK_TERMINATING
    } else {
        &WHITE_TERMINATING
    };
    writer.write(table[run]);
}

// The T.4 code tables this encoder needs, as `(bit length, code)`. Runs are
// indexed by their length, so only the terminating codes (0..=63) appear.

/// End-of-line (T.4 §4.1.2).
const EOL: (u8, u16) = (12, 0b000000000001);
/// Pass mode (T.4 Table 4).
const MODE_PASS: (u8, u16) = (4, 0b0001);
/// Horizontal mode (T.4 Table 4).
const MODE_HORIZONTAL: (u8, u16) = (3, 0b001);
/// Vertical modes indexed by `a1 - b1 + VERTICAL_MAX_DELTA`: VL3 … V0 … VR3.
const VERTICAL_MODES: [(u8, u16); 7] = [
    (7, 0b0000010),
    (6, 0b000010),
    (3, 0b010),
    (1, 0b1),
    (3, 0b011),
    (6, 0b000011),
    (7, 0b0000011),
];

/// White terminating codes (T.4 Table 2), indexed by run length.
const WHITE_TERMINATING: [(u8, u16); MAX_RUN + 1] = [
    (8, 0b00110101),
    (6, 0b000111),
    (4, 0b0111),
    (4, 0b1000),
    (4, 0b1011),
    (4, 0b1100),
    (4, 0b1110),
    (4, 0b1111),
    (5, 0b10011),
    (5, 0b10100),
    (5, 0b00111),
    (5, 0b01000),
    (6, 0b001000),
    (6, 0b000011),
    (6, 0b110100),
    (6, 0b110101),
    (6, 0b101010),
    (6, 0b101011),
    (7, 0b0100111),
    (7, 0b0001100),
    (7, 0b0001000),
    (7, 0b0010111),
    (7, 0b0000011),
    (7, 0b0000100),
    (7, 0b0101000),
    (7, 0b0101011),
    (7, 0b0010011),
    (7, 0b0100100),
    (7, 0b0011000),
    (8, 0b00000010),
    (8, 0b00000011),
    (8, 0b00011010),
    (8, 0b00011011),
    (8, 0b00010010),
    (8, 0b00010011),
    (8, 0b00010100),
    (8, 0b00010101),
    (8, 0b00010110),
    (8, 0b00010111),
    (8, 0b00101000),
    (8, 0b00101001),
    (8, 0b00101010),
    (8, 0b00101011),
    (8, 0b00101100),
    (8, 0b00101101),
    (8, 0b00000100),
    (8, 0b00000101),
    (8, 0b00001010),
    (8, 0b00001011),
    (8, 0b01010010),
    (8, 0b01010011),
    (8, 0b01010100),
    (8, 0b01010101),
    (8, 0b00100100),
    (8, 0b00100101),
    (8, 0b01011000),
    (8, 0b01011001),
    (8, 0b01011010),
    (8, 0b01011011),
    (8, 0b01001010),
    (8, 0b01001011),
    (8, 0b00110010),
    (8, 0b00110011),
    (8, 0b00110100),
];

/// Black terminating codes (T.4 Table 2), indexed by run length.
const BLACK_TERMINATING: [(u8, u16); MAX_RUN + 1] = [
    (10, 0b0000110111),
    (3, 0b010),
    (2, 0b11),
    (2, 0b10),
    (3, 0b011),
    (4, 0b0011),
    (4, 0b0010),
    (5, 0b00011),
    (6, 0b000101),
    (6, 0b000100),
    (7, 0b0000100),
    (7, 0b0000101),
    (7, 0b0000111),
    (8, 0b00000100),
    (8, 0b00000111),
    (9, 0b000011000),
    (10, 0b0000010111),
    (10, 0b0000011000),
    (10, 0b0000001000),
    (11, 0b00001100111),
    (11, 0b00001101000),
    (11, 0b00001101100),
    (11, 0b00000110111),
    (11, 0b00000101000),
    (11, 0b00000010111),
    (11, 0b00000011000),
    (12, 0b000011001010),
    (12, 0b000011001011),
    (12, 0b000011001100),
    (12, 0b000011001101),
    (12, 0b000001101000),
    (12, 0b000001101001),
    (12, 0b000001101010),
    (12, 0b000001101011),
    (12, 0b000011010010),
    (12, 0b000011010011),
    (12, 0b000011010100),
    (12, 0b000011010101),
    (12, 0b000011010110),
    (12, 0b000011010111),
    (12, 0b000001101100),
    (12, 0b000001101101),
    (12, 0b000011011010),
    (12, 0b000011011011),
    (12, 0b000001010100),
    (12, 0b000001010101),
    (12, 0b000001010110),
    (12, 0b000001010111),
    (12, 0b000001100100),
    (12, 0b000001100101),
    (12, 0b000001010010),
    (12, 0b000001010011),
    (12, 0b000000100100),
    (12, 0b000000110111),
    (12, 0b000000111000),
    (12, 0b000000100111),
    (12, 0b000000101000),
    (12, 0b000001011000),
    (12, 0b000001011001),
    (12, 0b000000101011),
    (12, 0b000000101100),
    (12, 0b000001011010),
    (12, 0b000001100110),
    (12, 0b000001100111),
];
