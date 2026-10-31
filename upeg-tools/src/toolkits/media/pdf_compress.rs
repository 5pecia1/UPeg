//! Target-size PDF compression using local Rust libraries on every surface.
//!
//! Each profile starts from the original document. Supported image XObjects
//! are re-encoded without rasterizing pages, retaining text, vectors and page
//! geometry. Unsupported image objects are preserved and listed in the report.

use upeg_core::{FileContent, FileValue, tool};

use super::{PDF_DEFAULT_STEM, PDF_MIME, file_input_bytes, file_stem};

mod engine;

pub use engine::{
    CompressionAttempt, CompressionMode, CompressionReport, CompressionStatus, SkippedImage,
};

/// Primary PDF artifact output identifier.
pub const PDF_COMPRESS_FILE_OUTPUT_ID: &str = "result";
/// Compression report output identifier.
pub const PDF_COMPRESS_REPORT_OUTPUT_ID: &str = "report";
/// Default profile ordering favors a quicker reduction in file size.
pub const PDF_COMPRESS_DEFAULT_MODE: &str = "balanced";

/// PDF artifact together with the actual compression outcome.
#[derive(Debug)]
pub struct PdfCompression {
    /// Original file when no candidate improves its size, otherwise a PDF copy.
    pub result: FileValue,
    /// Target attainment, byte sizes and image coverage.
    pub report: CompressionReport,
}

#[tool(
    id = "media.pdf_compress",
    display_label = "Compress PDF",
    description = "Try image quality and pixel-size profiles until the PDF meets the target. Preserves text and vectors; unsupported images stay unchanged. A missed target is reported, not guaranteed. Signed and encrypted PDFs are rejected.",
    toolkit = "media",
    inputs = [
        required input: File(extensions=["pdf"], max_count=1, max_file_bytes=52428800, max_total_bytes=52428800) = "PDF file (up to 50 MiB)",
        required target_bytes: Integer(min=1, max=67108864) = "Desired PDF size in bytes (1 MiB = 1048576 bytes)",
        optional mode: Options(["balanced", "high_quality"]) = "Profile order: balanced by default, or conservative profiles first",
        optional max_output_bytes: Integer(min=1, max=67108864, default=67108864) = "Output safety limit in bytes, independent of the target",
    ],
    outputs = [
        result: File = "PDF artifact",
        report: Json = "Target status, sizes, profiles and preserved unsupported images",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Compress a PDF towards a desired size without replacing its pages by images.
///
/// `balanced` starts with stronger image compression; `high_quality` tries
/// conservative profiles first. Image dimensions are scaled in pixels, not DPI.
/// The first profile meeting the target wins. A missed target still returns the
/// smallest admissible artifact, considering the original file as a candidate.
/// Invalid documents and resource/output-limit violations return an error.
pub fn pdf_compress(
    input: &FileValue,
    target_bytes: usize,
    mode: &str,
    max_output_bytes: usize,
) -> Result<PdfCompression, String> {
    let mode = CompressionMode::parse(mode)?;
    let bytes = file_input_bytes(input)?;
    let output = engine::compress(bytes, target_bytes, mode, max_output_bytes)?;
    let unchanged = output.bytes == bytes;
    let name = if unchanged {
        input.name.clone()
    } else {
        format!(
            "{}-compressed.pdf",
            file_stem(&input.name, PDF_DEFAULT_STEM)
        )
    };
    Ok(PdfCompression {
        result: FileValue {
            name,
            content: FileContent::Bytes(output.bytes),
            mime: Some(PDF_MIME.to_string()),
        },
        report: output.report,
    })
}

#[cfg(test)]
mod tests;
