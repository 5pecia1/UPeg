//! `qr` toolkit — QR code generation and decoding.
//!
//! `qr.encode` was promoted from `gui_meta.rs` (previously a GUI-only meta
//! with `pin = Inline` and no dispatcher — running it failed with
//! "dispatch not implemented for qr.encode"). `qr.decode` (image file →
//! text) joined it later, clearing the R6 "≥2 tools per toolkit"
//! consistency rule that `qr` used to carry as a documented single-tool
//! exception (the `#[tool]` macro requires `toolkit` to literally prefix
//! `id`, so a `qr.*` id always forces `toolkit = "qr"`).
//!
//! `qr.decode` takes a `File` (image bytes) input and decodes it with the
//! pure-Rust `image` + `rqrr` stack — so it compiles and runs on every surface
//! including the browser (wasm32) build (no filesystem, no native library).

use upeg_core::{FileContent, FileValue, tool};

/// Max input length accepted by `qr.encode`, in bytes.
///
/// `qrcode`'s largest symbol (Version 40, low error correction) holds at
/// most ~2953 bytes of binary data. Capping well under that keeps the
/// unicode-rendered grid a reasonable size for inline/terminal display
/// and gives a clear error instead of a slow, illegible symbol for
/// pathological inputs.
const QR_ENCODE_MAX_INPUT_LEN: usize = 2000;

/// `qr.encode` — generate a QR code from a string, rendered as a
/// unicode-block string (two pixel rows per output line via
/// `qrcode::render::unicode::Dense1x2`) suitable for inline pin display
/// or terminal rendering.
///
/// The `qrcode` crate is encode-only (no decoder), so correctness here
/// is pinned by structural properties instead of a decode round-trip:
/// a rectangular grid, a blank quiet-zone border, and deterministic
/// output for a given input.
#[tool(
    id = "qr.encode",
    display_label = "QR encode",
    toolkit = "qr",
    description = "Generate a QR code from a string, rendered as unicode block characters.",
    inputs = [
        required input: String = "Text to encode (max 2000 bytes)",
    ],
    pin = Inline,
    pegboard_units = U1,
    invoker = Function,
)]
pub fn qr_encode(input: &str) -> Result<String, &'static str> {
    if input.is_empty() {
        return Err("input must not be empty");
    }
    if input.len() > QR_ENCODE_MAX_INPUT_LEN {
        return Err("input too large (max 2000 bytes)");
    }
    let code =
        qrcode::QrCode::new(input.as_bytes()).map_err(|_| "could not encode input as a qr code")?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build())
}

/// `qr.decode` — decode a QR code from image (PNG/JPEG) bytes into text.
#[tool(
    id = "qr.decode",
    display_label = "QR decode",
    toolkit = "qr",
    description = "Decode a QR code from an image (PNG/JPEG) into text.",
    inputs = [
        required input: File = "QR code image bytes (PNG or JPEG)",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
pub fn qr_decode(input: &FileValue) -> Result<String, String> {
    let bytes = match &input.content {
        FileContent::Bytes(bytes) => bytes.as_slice(),
        FileContent::Directory(_) => {
            return Err("QR input must be an image file, not a directory".to_string());
        }
    };

    let format = image::guess_format(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(
            "could not decode image: unsupported image format (only PNG and JPEG are supported)"
                .to_string(),
        );
    }
    let decoded = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| format!("could not decode image: {e}"))?;

    let mut prepared = rqrr::PreparedImage::prepare(decoded.to_luma8());
    let grids = prepared.detect_grids();
    let grid = grids
        .first()
        .ok_or_else(|| "no QR code found in image".to_string())?;
    let (_meta, content) = grid
        .decode()
        .map_err(|e| format!("could not decode QR grid: {e}"))?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_인코딩은_짧은_문자열을_렌더링한다() {
        let out = qr_encode("hello").unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn qr_인코딩은_빈_입력을_거부한다() {
        assert!(qr_encode("").is_err());
    }

    #[test]
    fn qr_인코딩은_과도하게_큰_입력을_거부한다() {
        let huge = "a".repeat(QR_ENCODE_MAX_INPUT_LEN + 1);
        assert!(qr_encode(&huge).is_err());
    }

    #[test]
    fn qr_인코딩은_동일_입력에_대해_결정적이다() {
        let a = qr_encode("upeg").unwrap();
        let b = qr_encode("upeg").unwrap();
        assert_eq!(a, b, "same input must render byte-identical output");
    }

    #[test]
    fn qr_인코딩은_직사각형_격자와_여백을_만든다() {
        let out = qr_encode("https://example.com/upeg").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines.len() >= 10,
            "expected a multi-row grid, got {} rows",
            lines.len()
        );
        let width = lines[0].chars().count();
        assert!(
            lines.iter().all(|l| l.chars().count() == width),
            "all rows must share one width (rectangular grid)"
        );
        assert!(
            lines.first().is_some_and(|l| l.chars().all(|c| c == ' ')),
            "top row must be a blank quiet zone"
        );
        assert!(
            lines.last().is_some_and(|l| l.chars().all(|c| c == ' ')),
            "bottom row must be a blank quiet zone"
        );
    }

    #[test]
    fn qr_인코딩은_다른_입력에_대해_다른_출력을_만든다() {
        let a = qr_encode("alpha").unwrap();
        let b = qr_encode("beta").unwrap();
        assert_ne!(a, b);
    }

    // ─── qr.decode ────────────────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    mod decode {
        use super::*;

        /// Wrap raw bytes as a `File` input value, mirroring the CLI `@path`
        /// path.
        fn file_input(name: &str, bytes: Vec<u8>) -> FileValue {
            FileValue {
                name: name.to_string(),
                content: FileContent::Bytes(bytes),
                mime: None,
            }
        }

        /// Rasterize `text` as a QR code PNG in memory. Built directly from
        /// `qrcode::QrCode::to_colors()` (module grid) rather than the
        /// `qrcode` crate's optional `image` renderer feature, which the
        /// workspace leaves disabled (`default-features = false`) — one
        /// pixel block per module, plus a quiet-zone border, is all a
        /// decoder needs.
        fn qr_png_bytes(text: &str) -> Vec<u8> {
            const MODULE_PX: u32 = 8;
            const QUIET_ZONE_MODULES: u32 = 4;
            const LIGHT: image::Luma<u8> = image::Luma([255]);
            const DARK: image::Luma<u8> = image::Luma([0]);

            let code = qrcode::QrCode::new(text.as_bytes()).expect("encode QR test fixture");
            let width = code.width();
            let colors = code.to_colors();
            let image_modules = width as u32 + QUIET_ZONE_MODULES * 2;
            let image_px = image_modules * MODULE_PX;

            let image = image::GrayImage::from_fn(image_px, image_px, |x, y| {
                let module_x = (x / MODULE_PX).checked_sub(QUIET_ZONE_MODULES);
                let module_y = (y / MODULE_PX).checked_sub(QUIET_ZONE_MODULES);
                match (module_x, module_y) {
                    (Some(mx), Some(my)) if (mx as usize) < width && (my as usize) < width => {
                        if colors[my as usize * width + mx as usize] == qrcode::Color::Dark {
                            DARK
                        } else {
                            LIGHT
                        }
                    }
                    _ => LIGHT,
                }
            });
            let mut bytes = Vec::new();
            image
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .expect("QR PNG fixture should encode");
            bytes
        }

        #[test]
        fn qr_디코딩은_인코딩된_텍스트를_복원한다() {
            let png = qr_png_bytes("upeg qr round trip");
            let decoded = qr_decode(&file_input("qr.png", png)).unwrap();
            assert_eq!(decoded, "upeg qr round trip");
        }

        #[test]
        fn qr_디코딩은_이미지가_아닌_바이트를_거부한다() {
            let result = qr_decode(&file_input("qr.png", b"not an image".to_vec()));
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("could not decode image"));
        }

        #[test]
        fn qr_디코딩은_qr_코드가_없는_이미지를_거부한다() {
            let blank = image::GrayImage::from_pixel(64, 64, image::Luma([255]));
            let mut bytes = Vec::new();
            blank
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .unwrap();
            let result = qr_decode(&file_input("blank.png", bytes));
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("no QR code found"));
        }

        #[test]
        fn qr_디코딩은_디렉터리_입력을_거부한다() {
            let dir_input = FileValue {
                name: "dir".to_string(),
                content: FileContent::Directory(Vec::new()),
                mime: None,
            };
            let result = qr_decode(&dir_input);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("not a directory"));
        }
    }
}
