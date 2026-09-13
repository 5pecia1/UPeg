//! In-memory PDF inspection and Markdown extraction, without OCR runtimes.

use pdf_inspector::{
    DetectionConfig, PdfOptions, PdfProcessResult, PdfType, ProcessMode, ScanStrategy,
};
use serde::Serialize;
use upeg_core::{FileValue, tool};

use super::{file_input_bytes, limits::over_cap_error};

/// Input cap before invoking the PDF parser (32 MiB).
pub const PDF_TEXT_MAX_INPUT_BYTES: usize = 32 << 20;
/// Text extraction is intended for individual documents, up to 500 pages.
pub const PDF_TEXT_MAX_PAGES: u32 = 500;
/// Maximum returned Markdown size (8 MiB).
pub const PDF_TEXT_MAX_OUTPUT_BYTES: usize = 8 << 20;
/// Canonical Markdown output identifier.
pub const PDF_MARKDOWN_OUTPUT_ID: &str = "markdown";
/// Canonical extraction report output identifier.
pub const PDF_REPORT_OUTPUT_ID: &str = "report";

/// Classification independent of the upstream enum's spelling.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfDocumentType {
    /// Contains a native text layer.
    TextBased,
    /// Scanned pages.
    Scanned,
    /// Predominantly images.
    ImageBased,
    /// Contains both text and pages requiring OCR.
    Mixed,
}

impl From<PdfType> for PdfDocumentType {
    fn from(value: PdfType) -> Self {
        match value {
            PdfType::TextBased => Self::TextBased,
            PdfType::Scanned => Self::Scanned,
            PdfType::ImageBased => Self::ImageBased,
            PdfType::Mixed => Self::Mixed,
        }
    }
}

/// Whether native extraction produced usable content for the whole document.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdfExtractionStatus {
    /// No pages were identified as requiring OCR; not an accuracy guarantee.
    Complete,
    /// Some content was extracted, but OCR is still recommended.
    Partial,
    /// No usable Markdown was extracted.
    Unavailable,
}

/// Shared structured metadata; page numbers are one-based.
#[derive(Debug, Serialize)]
pub struct PdfReport {
    /// Document classification.
    pub pdf_type: PdfDocumentType,
    /// Total document pages.
    pub page_count: u32,
    /// Detector confidence, not text-recognition accuracy.
    pub confidence: f32,
    /// Pages recommended for OCR, which this tool does not perform.
    pub pages_needing_ocr: Vec<u32>,
    /// Absent for inspection because text decoding has not been evaluated.
    pub has_encoding_issues: Option<bool>,
    /// Absent for inspection because extraction was not requested.
    pub extraction_status: Option<PdfExtractionStatus>,
}

/// Native text plus a report that preserves partial/unavailable extraction.
#[derive(Debug)]
pub struct PdfMarkdown {
    /// Empty when the report marks extraction unavailable.
    pub markdown: String,
    /// Classification and extraction coverage.
    pub report: PdfReport,
}

#[tool(
    id = "media.pdf_inspect",
    display_label = "Inspect PDF",
    description = "Classify every PDF page and report OCR recommendations. Does not perform OCR or validate text encoding.",
    toolkit = "media",
    inputs = [required input: File = "PDF file (up to 32 MiB and 500 pages)"],
    outputs = [result: Json = "PDF classification and one-based OCR page numbers"],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Inspect all pages without extracting text.
pub fn pdf_inspect(input: &FileValue) -> Result<String, String> {
    let result = process(input, ProcessMode::DetectOnly)?;
    serde_json::to_string(&report(result, None, None)).map_err(|error| error.to_string())
}

#[tool(
    id = "media.pdf_to_markdown",
    display_label = "PDF → Markdown",
    description = "Extract native PDF text locally. Check the report for partial or unavailable content; OCR is not performed.",
    toolkit = "media",
    inputs = [required input: File = "PDF file (up to 32 MiB and 500 pages)"],
    outputs = [
        markdown: Markdown = "Extracted Markdown (empty when unavailable)",
        report: Json = "Extraction status and one-based OCR page numbers",
    ],
    pin = Launcher,
    pegboard_units = U2,
    invoker = Function,
    surfaces = [Cli, Tui, Desktop, Mcp, Http, Pwa, Ext],
)]
/// Extract Markdown while keeping incomplete extraction visible to callers.
pub fn pdf_to_markdown(input: &FileValue) -> Result<PdfMarkdown, String> {
    let mut result = process(input, ProcessMode::Full)?;
    let markdown = result.markdown.take().unwrap_or_default();
    if markdown.len() > PDF_TEXT_MAX_OUTPUT_BYTES {
        return Err(over_cap_error(
            "PDF Markdown output",
            PDF_TEXT_MAX_OUTPUT_BYTES,
        ));
    }
    let status = if markdown.trim().is_empty() {
        PdfExtractionStatus::Unavailable
    } else if result.has_encoding_issues || !result.pages_needing_ocr.is_empty() {
        PdfExtractionStatus::Partial
    } else {
        PdfExtractionStatus::Complete
    };
    let encoding = Some(result.has_encoding_issues);
    Ok(PdfMarkdown {
        markdown,
        report: report(result, encoding, Some(status)),
    })
}

fn process(input: &FileValue, mode: ProcessMode) -> Result<PdfProcessResult, String> {
    let bytes = file_input_bytes(input)?;
    if bytes.len() > PDF_TEXT_MAX_INPUT_BYTES {
        return Err(over_cap_error("PDF input", PDF_TEXT_MAX_INPUT_BYTES));
    }
    let options = PdfOptions::new().mode(mode).detection(DetectionConfig {
        strategy: ScanStrategy::Full,
        ..DetectionConfig::default()
    });
    pdf_inspector::process_pdf_mem_with_page_limit(bytes, options, PDF_TEXT_MAX_PAGES)
        .map_err(|error| format!("could not process PDF: {error}"))
}

fn report(
    result: PdfProcessResult,
    has_encoding_issues: Option<bool>,
    extraction_status: Option<PdfExtractionStatus>,
) -> PdfReport {
    PdfReport {
        pdf_type: result.pdf_type.into(),
        page_count: result.page_count,
        confidence: result.confidence,
        pages_needing_ocr: result.pages_needing_ocr,
        has_encoding_issues,
        extraction_status,
    }
}

#[cfg(test)]
mod tests;
