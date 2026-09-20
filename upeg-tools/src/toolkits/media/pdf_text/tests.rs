use super::*;
use lopdf::{Document, Object, Stream, dictionary};
use upeg_core::{FileContent, OutputValue, ToolResult};

const TEXT_CONTENT: &[u8] =
    b"BT /F1 12 Tf 50 700 Td (A readable PDF document with useful native text.) Tj 0 -20 Td (Another paragraph to preserve reading order.) Tj 0 -20 Td (The final paragraph contains additional information.) Tj ET";
const SCAN_CONTENT: &[u8] = b"q 595 0 0 842 0 0 cm /Im0 Do Q";
// Adobe-Korea1 CIDs: 한=3296, 글=1238, 가=1086.
const KOREAN_CONTENT: &[u8] =
    b"BT /F1 12 Tf 50 700 Td <0ce004d6043e0ce004d6043e0ce004d6043e> Tj ET";

fn pdf_document(pages: &[&[u8]], korean: bool) -> FileValue {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = if korean {
        let descendant = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "CIDFontType0", "BaseFont" => "TestKorean",
            "FontDescriptor" => dictionary! {
                "Type" => "FontDescriptor", "FontName" => "TestKorean", "Flags" => 4,
                "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 900.into()],
                "ItalicAngle" => 0, "Ascent" => 900, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
            },
            "CIDSystemInfo" => dictionary! {
                "Registry" => Object::string_literal("Adobe"),
                "Ordering" => Object::string_literal("Korea1"), "Supplement" => 0,
            },
        });
        dictionary! {
            "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "TestKorean",
            "Encoding" => "Identity-H", "DescendantFonts" => vec![Object::Reference(descendant)],
        }
    } else {
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" }
    };
    let font_id = doc.add_object(font);
    let image_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
        },
        vec![255; 3],
    ));
    let mut kids = Vec::new();
    for content in pages {
        let mut resources = dictionary! { "Font" => dictionary! { "F1" => font_id } };
        if *content == SCAN_CONTENT {
            resources.set("XObject", dictionary! { "Im0" => image_id });
        }
        let stream = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => stream,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => resources,
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages", "Count" => i64::try_from(kids.len()).unwrap(), "Kids" => kids,
        }
        .into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    FileValue {
        name: "fixture.pdf".into(),
        content: FileContent::Bytes(bytes),
        mime: None,
    }
}

#[test]
fn text_document_extracts_to_markdown() {
    let result = pdf_to_markdown(&pdf_document(&[TEXT_CONTENT], false)).unwrap();
    assert!(result.markdown.contains("readable PDF document"));
    assert_eq!(
        result.report.extraction_status,
        Some(PdfExtractionStatus::Complete)
    );
    assert_eq!(result.report.page_count, 1);
}

#[test]
fn mixed_document_reports_scan_pages_and_partial_extraction() {
    let input = pdf_document(&[SCAN_CONTENT, TEXT_CONTENT, SCAN_CONTENT], false);
    let inspection: serde_json::Value =
        serde_json::from_str(&pdf_inspect(&input).unwrap()).unwrap();
    assert_eq!(inspection["pdf_type"], "mixed");
    assert_eq!(inspection["pages_needing_ocr"], serde_json::json!([1, 3]));
    assert!(inspection["has_encoding_issues"].is_null());
    let result = pdf_to_markdown(&input).unwrap();
    assert!(result.markdown.contains("readable PDF document"));
    assert_eq!(
        result.report.extraction_status,
        Some(PdfExtractionStatus::Partial)
    );
}

#[test]
fn scan_only_document_reports_extraction_unavailable() {
    let result = pdf_to_markdown(&pdf_document(&[SCAN_CONTENT], false)).unwrap();
    assert!(result.markdown.is_empty());
    assert_eq!(
        result.report.extraction_status,
        Some(PdfExtractionStatus::Unavailable)
    );
    assert_eq!(result.report.pages_needing_ocr, vec![1]);
}

#[test]
fn hangul_without_tounicode_extracts_via_bundled_cmap() {
    let result = pdf_to_markdown(&pdf_document(&[KOREAN_CONTENT], true)).unwrap();
    assert!(result.markdown.contains("한글가"), "{}", result.markdown);
}

#[test]
fn invalid_file_and_page_overflow_are_rejected() {
    let mut input = pdf_document(&[TEXT_CONTENT], false);
    input.content = FileContent::Bytes(b"not a PDF".to_vec());
    assert!(pdf_inspect(&input).is_err());
    input.content = FileContent::Bytes(vec![0; PDF_TEXT_MAX_INPUT_BYTES + 1]);
    assert!(
        pdf_to_markdown(&input)
            .unwrap_err()
            .contains("PDF input exceeds")
    );
    let pages = vec![TEXT_CONTENT; PDF_TEXT_MAX_PAGES as usize + 1];
    assert!(
        pdf_inspect(&pdf_document(&pages, false))
            .unwrap_err()
            .contains("page limit")
    );
}

#[test]
fn dispatcher_returns_markdown_and_inspection_report_as_outputs() {
    crate::register_all();
    let input = pdf_document(&[TEXT_CONTENT], false);
    let args = serde_json::json!({"input": input});
    let Some(ToolResult::Success(result)) =
        upeg_runtime::try_runtime_dispatch(PDF_TO_MARKDOWN_TOOL_ID, &args)
    else {
        panic!("expected canonical success");
    };
    assert_eq!(
        result.primary_output_id.as_deref(),
        Some(PDF_MARKDOWN_OUTPUT_ID)
    );
    assert!(
        matches!(&result.outputs[0].value, OutputValue::Markdown(text) if text.contains("readable PDF"))
    );
    assert!(
        matches!(&result.outputs[1].value, OutputValue::Json(report) if report["extraction_status"] == "complete")
    );
    let Some(ToolResult::Success(result)) =
        upeg_runtime::try_runtime_dispatch(PDF_INSPECT_TOOL_ID, &args)
    else {
        panic!("expected inspection success");
    };
    assert!(
        matches!(&result.outputs[0].value, OutputValue::Json(report) if report["page_count"] == 1)
    );
}

#[test]
fn japanese_collection_alt_cmap_reads_from_binary_asset() {
    // Adobe-Japan1 CIDs 34/35/36 map to A/B/C. Unlike Korea1's Rust table,
    // this path must decode the embedded Adobe-Japan1-UCS2.bcmap asset.
    const JAPAN_CONTENT: &[u8] =
        b"BT /F1 12 Tf 50 700 Td <002200230024002200230024002200230024> Tj ET";
    let mut input = pdf_document(&[JAPAN_CONTENT], true);
    let FileContent::Bytes(bytes) = &mut input.content else {
        unreachable!()
    };
    const KOREA: &[u8] = b"Korea1";
    const JAPAN: &[u8] = b"Japan1";
    let offset = bytes
        .windows(KOREA.len())
        .position(|window| window == KOREA)
        .unwrap();
    bytes[offset..offset + KOREA.len()].copy_from_slice(JAPAN);
    let result = pdf_to_markdown(&input).unwrap();
    assert!(result.markdown.contains("ABC"), "{result:?}");
}
