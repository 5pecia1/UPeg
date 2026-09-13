use super::*;
use lopdf::{Document, Object, Stream, dictionary};
use upeg_core::{FileContent, OutputValue, ToolResult};

const TEXT_CONTENT: &[u8] =
    b"BT /F1 12 Tf 50 700 Td (A readable PDF document with useful native text.) Tj 0 -20 Td (Another paragraph to preserve reading order.) Tj 0 -20 Td (The final paragraph contains additional information.) Tj ET";
const SCAN_CONTENT: &[u8] = b"q 595 0 0 842 0 0 cm /Im0 Do Q";
// Adobe-Korea1 CIDs: 한=3296, 글=1238, 가=1086.
const KOREAN_CONTENT: &[u8] =
    b"BT /F1 12 Tf 50 700 Td <0ce004d6043e0ce004d6043e0ce004d6043e> Tj ET";

fn 문서(pages: &[&[u8]], korean: bool) -> FileValue {
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
fn 텍스트_문서를_마크다운으로_추출한다() {
    let result = pdf_to_markdown(&문서(&[TEXT_CONTENT], false)).unwrap();
    assert!(result.markdown.contains("readable PDF document"));
    assert_eq!(
        result.report.extraction_status,
        Some(PdfExtractionStatus::Complete)
    );
    assert_eq!(result.report.page_count, 1);
}

#[test]
fn 첫_스캔_뒤의_텍스트와_마지막_스캔까지_검사한다() {
    let input = 문서(&[SCAN_CONTENT, TEXT_CONTENT, SCAN_CONTENT], false);
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
fn 스캔만_있는_문서는_추출_불가를_명시한다() {
    let result = pdf_to_markdown(&문서(&[SCAN_CONTENT], false)).unwrap();
    assert!(result.markdown.is_empty());
    assert_eq!(
        result.report.extraction_status,
        Some(PdfExtractionStatus::Unavailable)
    );
    assert_eq!(result.report.pages_needing_ocr, vec![1]);
}

#[test]
fn 투유니코드가_없는_한글도_내장_씨맵으로_추출한다() {
    let result = pdf_to_markdown(&문서(&[KOREAN_CONTENT], true)).unwrap();
    assert!(result.markdown.contains("한글가"), "{}", result.markdown);
}

#[test]
fn 잘못된_파일과_페이지_초과를_거부한다() {
    let mut input = 문서(&[TEXT_CONTENT], false);
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
        pdf_inspect(&문서(&pages, false))
            .unwrap_err()
            .contains("page limit")
    );
}

#[test]
fn 디스패처는_마크다운과_검사_보고서를_정규_출력으로_반환한다() {
    crate::register_all();
    let input = 문서(&[TEXT_CONTENT], false);
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
fn 일본어_컬렉션의_대체_씨맵도_바이너리에서_읽는다() {
    // Adobe-Japan1 CIDs 34/35/36 map to A/B/C. Unlike Korea1's Rust table,
    // this path must decode the embedded Adobe-Japan1-UCS2.bcmap asset.
    const JAPAN_CONTENT: &[u8] =
        b"BT /F1 12 Tf 50 700 Td <002200230024002200230024002200230024> Tj ET";
    let mut input = 문서(&[JAPAN_CONTENT], true);
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
