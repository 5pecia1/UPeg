//! PDFs from an independent producer exercise the public compression contract.
#![cfg(feature = "media")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "fixture assertions"
)]

use hayro::hayro_interpret::{InterpreterSettings, hayro_syntax::Pdf};
use hayro::{RenderCache, RenderSettings, render};
use lopdf::Document;
use upeg_core::{FileContent, FileValue};
use upeg_tools::{CompressionStatus, pdf_compress};

const CASES: &[(&str, &[u8], usize)] = &[
    (
        "rgb_jpeg.pdf",
        include_bytes!("fixtures/pdf_compress/rgb_jpeg.pdf"),
        2,
    ),
    (
        "rgb_flate.pdf",
        include_bytes!("fixtures/pdf_compress/rgb_flate.pdf"),
        2,
    ),
    (
        "gray_jpeg.pdf",
        include_bytes!("fixtures/pdf_compress/gray_jpeg.pdf"),
        2,
    ),
    (
        "mixed_hangul.pdf",
        include_bytes!("fixtures/pdf_compress/mixed_hangul.pdf"),
        3,
    ),
];
const OUTPUT_CAP: usize = 64 << 20;

fn input(name: &str, bytes: &[u8]) -> FileValue {
    FileValue {
        name: name.into(),
        mime: Some("application/pdf".into()),
        content: FileContent::Bytes(bytes.to_vec()),
    }
}

fn file_bytes(file: &FileValue) -> &[u8] {
    let FileContent::Bytes(bytes) = &file.content else {
        panic!("expected PDF bytes")
    };
    bytes
}

#[test]
fn independent_producer_corpus_preserves_native_text_pages_links_and_embedded_fonts() {
    for &(name, source, pages) in CASES {
        let original = Document::load_mem(source).unwrap();
        for mode in ["balanced", "high_quality"] {
            let target = source.len() / 2;
            let result = pdf_compress(&input(name, source), target, mode, OUTPUT_CAP).unwrap();
            let output = file_bytes(&result.result);
            assert_eq!(
                result.report.status,
                CompressionStatus::TargetMet,
                "{name}/{mode}"
            );
            assert!(output.len() <= target);
            assert!(
                result.report.skipped_images.is_empty(),
                "{name}: {:?}",
                result.report.skipped_images
            );
            let saved = Document::load_mem(output).unwrap();
            assert_eq!(saved.get_pages().len(), pages);
            let numbers: Vec<_> = original.get_pages().keys().copied().collect();
            assert_eq!(
                saved.extract_text(&numbers).unwrap(),
                original.extract_text(&numbers).unwrap()
            );
            for (&page, &id) in &original.get_pages() {
                assert_eq!(
                    saved.get_page_content(id),
                    original.get_page_content(id),
                    "page {page}"
                );
                let before = original.get_object(id).unwrap().as_dict().unwrap();
                let after = saved.get_object(id).unwrap().as_dict().unwrap();
                assert_eq!(
                    before.get(b"MediaBox").unwrap(),
                    after.get(b"MediaBox").unwrap()
                );
                assert_eq!(before.get(b"Annots").ok(), after.get(b"Annots").ok());
            }
            // Fonts, ToUnicode maps, annotations and their actions are never
            // altered. Compare these objects by identity with the producer PDF.
            for (&id, object) in &original.objects {
                let dict = match object {
                    lopdf::Object::Dictionary(dict) => dict,
                    lopdf::Object::Stream(stream) => &stream.dict,
                    _ => continue,
                };
                let kind = dict
                    .get(b"Type")
                    .and_then(lopdf::Object::as_name)
                    .unwrap_or_default();
                if matches!(kind, b"Font" | b"FontDescriptor" | b"Annot") {
                    assert_eq!(
                        saved.get_object(id).unwrap(),
                        object,
                        "{name} object {id:?}"
                    );
                    for key in [
                        b"FontFile".as_slice(),
                        b"FontFile2",
                        b"FontFile3",
                        b"ToUnicode",
                    ] {
                        if let Ok(reference) = dict.get(key).and_then(lopdf::Object::as_reference) {
                            assert_eq!(
                                saved.get_object(reference).unwrap(),
                                original.get_object(reference).unwrap(),
                                "{name}: font or Unicode stream changed"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn render_page(bytes: &[u8], page: usize) -> hayro::vello_cpu::Pixmap {
    let document = Pdf::new(bytes.to_vec()).unwrap();
    render(
        &document.pages()[page],
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings {
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            ..RenderSettings::default()
        },
    )
}

#[test]
fn independent_renderer_preserves_text_and_vector_pixels_outside_lossy_image_regions() {
    for &(name, source, _) in CASES {
        let result = pdf_compress(
            &input(name, source),
            source.len() / 2,
            "balanced",
            OUTPUT_CAP,
        )
        .unwrap();
        let before = render_page(source, 0);
        let after = render_page(file_bytes(&result.result), 0);
        assert_eq!(
            (before.width(), before.height()),
            (after.width(), after.height())
        );
        // ReportLab places native text in the top 50 points. Image resampling
        // may change image pixels, but must not change the text or link region.
        let header_bytes = usize::from(before.width()) * 50 * 4;
        assert_eq!(
            &before.data_as_u8_slice()[..header_bytes],
            &after.data_as_u8_slice()[..header_bytes],
            "{name}: native text rendering changed"
        );
        // Both sample points are strictly inside the fixture's image, away
        // from the blue frame and native text. A missing/blank image must fail.
        let channel =
            |x: usize| after.data_as_u8_slice()[(200 * usize::from(after.width()) + x) * 4];
        assert!(
            channel(100) < channel(400),
            "{name}: image gradient disappeared or inverted"
        );
        assert!(channel(400) < 240, "{name}: image region became blank");
    }
}

#[test]
fn independent_scan_page_remains_renderable_and_hangul_text_remains_searchable() {
    let (_, source, _) = CASES[3];
    let result = pdf_compress(
        &input("mixed_hangul.pdf", source),
        source.len() / 2,
        "high_quality",
        OUTPUT_CAP,
    )
    .unwrap();
    let saved = Document::load_mem(file_bytes(&result.result)).unwrap();
    assert!(
        saved
            .extract_text(&[1, 3])
            .unwrap()
            .contains("한글 문서 압축 테스트")
    );
    assert!(
        saved.extract_text(&[2]).unwrap().trim().is_empty(),
        "raster page must not gain a fabricated text layer"
    );
    let scan = render_page(file_bytes(&result.result), 1);
    let samples = scan.data_as_u8_slice();
    assert!(samples.chunks_exact(4).any(|pixel| pixel[0] < 100));
    assert!(samples.chunks_exact(4).any(|pixel| pixel[0] > 240));
}
