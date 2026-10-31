use super::*;
use image::ImageEncoder;
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use std::io::Write;
use upeg_core::{OutputValue, ToolResult};

#[path = "tests/behavior.rs"]
mod behavior;

const WIDTH: u32 = 192;
const HEIGHT: u32 = 128;
const TEXT: &str = "Searchable text stays in the compressed PDF.";
const CONTENT: &[u8] = b"q 192 0 0 128 20 20 cm /Im0 Do Q\n0 0 1 RG 1 w 10 10 200 150 re S\nBT /F1 12 Tf 20 170 Td (Searchable text stays in the compressed PDF.) Tj ET";

struct Fixture {
    file: FileValue,
    image_id: ObjectId,
    content_id: ObjectId,
    annotation_id: ObjectId,
}

fn samples() -> Vec<u8> {
    let mut state = 0x72c0_ffee_u32;
    (0..WIDTH * HEIGHT * 3)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes()[0]
        })
        .collect()
}

fn fixture(jpeg: bool, extra: &[(&str, Object)]) -> Fixture {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let mut dict = dictionary! {
        "Type" => "XObject", "Subtype" => "Image",
        "Width" => i64::from(WIDTH), "Height" => i64::from(HEIGHT),
        "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
    };
    let pixels = samples();
    let mut image = if jpeg {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 100)
            .write_image(&pixels, WIDTH, HEIGHT, image::ExtendedColorType::Rgb8)
            .unwrap();
        dict.set("Filter", "DCTDecode");
        Stream::new(dict, bytes)
    } else {
        dict.set("Filter", "FlateDecode");
        Stream::new(dict, deflate(&pixels))
    };
    for (key, value) in extra {
        image.dict.set(*key, value.clone());
    }
    let image_id = doc.add_object(image);
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let content_id = doc.add_object(Stream::new(dictionary! {}, CONTENT.to_vec()));
    let annotation_id = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link",
        "Rect" => vec![20.into(), 160.into(), 200.into(), 180.into()],
        "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.com") },
    });
    let mut kids = Vec::new();
    for _ in 0..2 {
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), 240.into(), 200.into()],
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font_id },
                "XObject" => dictionary! { "Im0" => image_id },
            },
            "Annots" => vec![Object::Reference(annotation_id)],
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Count" => 2, "Kids" => kids }.into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    Fixture {
        file: FileValue {
            name: "mixed.pdf".into(),
            mime: Some(PDF_MIME.into()),
            content: FileContent::Bytes(bytes),
        },
        image_id,
        content_id,
        annotation_id,
    }
}

fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn bytes(file: &FileValue) -> &[u8] {
    file_input_bytes(file).unwrap()
}

#[test]
fn compresses_flate_and_jpeg_images_preserving_shared_objects_text_vectors_and_links() {
    for (jpeg, raw) in [(false, false), (true, false), (false, true)] {
        let mut input = fixture(jpeg, &[]);
        if raw {
            let mut document = Document::load_mem(bytes(&input.file)).unwrap();
            let stream = document
                .get_object_mut(input.image_id)
                .unwrap()
                .as_stream_mut()
                .unwrap();
            stream.dict.remove(b"Filter");
            stream.set_content(samples());
            let mut content = Vec::new();
            document.save_to(&mut content).unwrap();
            input.file.content = FileContent::Bytes(content);
        }
        let target = bytes(&input.file).len() / 2;
        let result = pdf_compress(&input.file, target, "balanced", 64 << 20).unwrap();
        assert_eq!(result.report.status, CompressionStatus::TargetMet);
        assert!(bytes(&result.result).len() <= target);
        assert_eq!(result.result.name, "mixed-compressed.pdf");
        assert_eq!(result.report.input_bytes, bytes(&input.file).len());
        assert_eq!(result.report.output_bytes, bytes(&result.result).len());
        let original = Document::load_mem(bytes(&input.file)).unwrap();
        let compressed = Document::load_mem(bytes(&result.result)).unwrap();
        assert_eq!(compressed.get_pages().len(), 2);
        assert_eq!(compressed.version, original.version);
        assert!(compressed.extract_text(&[1, 2]).unwrap().contains(TEXT));
        assert_eq!(
            compressed.get_object(input.annotation_id).unwrap(),
            original.get_object(input.annotation_id).unwrap()
        );
        assert_eq!(
            compressed
                .get_object(input.content_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .decompressed_content()
                .unwrap(),
            CONTENT
        );
        for page_id in compressed.get_pages().values() {
            let page = compressed.get_object(*page_id).unwrap().as_dict().unwrap();
            assert_eq!(
                page.get(b"MediaBox").unwrap(),
                original
                    .get_object(*page_id)
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"MediaBox")
                    .unwrap()
            );
            let images = page
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"XObject")
                .unwrap()
                .as_dict()
                .unwrap();
            assert_eq!(
                images.get(b"Im0").unwrap().as_reference().unwrap(),
                input.image_id
            );
        }
        let stream = compressed
            .get_object(input.image_id)
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(
            stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        assert!(stream.dict.get(b"Width").unwrap().as_i64().unwrap() < i64::from(WIDTH));
        let decoded = image::load_from_memory(&stream.content).unwrap();
        assert_eq!(
            i64::from(decoded.width()),
            stream.dict.get(b"Width").unwrap().as_i64().unwrap()
        );
    }
}

#[test]
fn already_within_target_returns_identical_original_without_attempts() {
    let input = fixture(true, &[]);
    let result = pdf_compress(
        &input.file,
        bytes(&input.file).len(),
        "high_quality",
        64 << 20,
    )
    .unwrap();
    assert_eq!(result.report.status, CompressionStatus::AlreadyWithinTarget);
    assert_eq!(bytes(&result.result), bytes(&input.file));
    assert_eq!(result.result.name, input.file.name);
    assert!(result.report.attempts.is_empty());
}

#[test]
fn target_unmet_is_success_and_never_returns_a_larger_artifact() {
    let input = fixture(true, &[]);
    let result = pdf_compress(&input.file, 1, "balanced", 64 << 20).unwrap();
    assert_eq!(result.report.status, CompressionStatus::TargetUnmet);
    assert!(bytes(&result.result).len() <= bytes(&input.file).len());
    assert!(!result.report.attempts.is_empty());
    let document = Document::load_mem(bytes(&result.result)).unwrap();
    let image = document
        .get_object(input.image_id)
        .unwrap()
        .as_stream()
        .unwrap();
    // The strongest candidate scales the original once, rather than repeatedly
    // shrinking the previous candidate and compounding JPEG loss.
    assert_eq!(
        image.dict.get(b"Width").unwrap().as_i64().unwrap(),
        i64::from((WIDTH * 35).div_ceil(100))
    );
}

#[test]
fn high_quality_stops_at_first_conservative_candidate_and_starts_from_original() {
    let input = fixture(true, &[]);
    let smallest = pdf_compress(&input.file, 1, "high_quality", 64 << 20).unwrap();
    let result = pdf_compress(
        &input.file,
        bytes(&input.file).len() - 1,
        "high_quality",
        64 << 20,
    )
    .unwrap();
    assert_eq!(result.report.status, CompressionStatus::TargetMet);
    assert!(result.report.attempts.len() < smallest.report.attempts.len());
    let doc = Document::load_mem(bytes(&result.result)).unwrap();
    let image = doc.get_object(input.image_id).unwrap().as_stream().unwrap();
    assert_eq!(
        image.dict.get(b"Width").unwrap().as_i64().unwrap(),
        i64::from(WIDTH)
    );
}

#[test]
fn preserves_unsupported_images_and_reports_the_reason() {
    for extra in [
        vec![("ColorSpace", Object::Name(b"DeviceCMYK".to_vec()))],
        vec![("Filter", Object::Name(b"JPXDecode".to_vec()))],
        vec![("ImageMask", Object::Boolean(true))],
        vec![("SMask", Object::Null)],
        vec![(
            "Decode",
            Object::Array(vec![
                1.into(),
                0.into(),
                1.into(),
                0.into(),
                1.into(),
                0.into(),
            ]),
        )],
    ] {
        let input = fixture(true, &extra);
        let result = pdf_compress(&input.file, 1, "balanced", 64 << 20).unwrap();
        assert!(!result.report.skipped_images.is_empty());
        let original = Document::load_mem(bytes(&input.file)).unwrap();
        let output = Document::load_mem(bytes(&result.result)).unwrap();
        assert_eq!(
            output.get_object(input.image_id).unwrap(),
            original.get_object(input.image_id).unwrap()
        );
        assert!(bytes(&result.result).len() <= bytes(&input.file).len());
    }
}

#[test]
fn invalid_inputs_modes_targets_and_output_limits_are_rejected() {
    let input = fixture(false, &[]);
    assert!(pdf_compress(&input.file, 0, "balanced", 64 << 20).is_err());
    assert!(pdf_compress(&input.file, 1, "unknown", 64 << 20).is_err());
    assert!(pdf_compress(&input.file, 1, "balanced", 0).is_err());
    assert!(pdf_compress(&input.file, 1, "balanced", 1).is_err());
    let invalid = FileValue {
        content: FileContent::Bytes(b"not a PDF".to_vec()),
        ..input.file.clone()
    };
    assert!(pdf_compress(&invalid, 1000, "balanced", 64 << 20).is_err());
    let directory = FileValue {
        content: FileContent::Directory(vec![]),
        ..input.file
    };
    assert!(pdf_compress(&directory, 1000, "balanced", 64 << 20).is_err());
}

#[test]
fn signed_pdf_is_rejected_even_when_already_within_target() {
    let input = fixture(true, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let signature = document.add_object(dictionary! { "Type" => "Sig", "ByteRange" => vec![0.into(), 10.into(), 20.into(), 30.into()] });
    document.trailer.set("TestSignature", signature);
    let mut signed = input.file;
    let mut content = Vec::new();
    document.save_to(&mut content).unwrap();
    signed.content = FileContent::Bytes(content);
    assert!(
        pdf_compress(&signed, 64 << 20, "balanced", 64 << 20)
            .unwrap_err()
            .contains("signed")
    );
}

#[test]
fn encrypted_pdf_is_rejected_even_with_an_empty_password() {
    let input = fixture(true, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("0123456789abcdef"),
            Object::string_literal("0123456789abcdef"),
        ],
    );
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V1 {
        document: &document,
        owner_password: "owner",
        user_password: "",
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    document.encrypt(&state).unwrap();
    let mut content = Vec::new();
    document.save_to(&mut content).unwrap();
    let encrypted = FileValue {
        content: FileContent::Bytes(content),
        ..input.file
    };
    assert!(
        pdf_compress(&encrypted, 64 << 20, "balanced", 64 << 20)
            .unwrap_err()
            .contains("encrypted")
    );
}

#[test]
fn flate_png_predictor_is_decoded_and_gray_images_stay_gray() {
    for gray in [false, true] {
        let input = fixture(false, &[]);
        let mut document = Document::load_mem(bytes(&input.file)).unwrap();
        let rgb = samples();
        let pixels = if gray {
            rgb.chunks_exact(3)
                .map(|pixel| pixel[0])
                .collect::<Vec<_>>()
        } else {
            rgb
        };
        let components = if gray { 1 } else { 3 };
        let mut rows = Vec::new();
        for row in pixels.chunks_exact(WIDTH as usize * components) {
            rows.push(0);
            rows.extend_from_slice(row);
        }
        let image = document
            .get_object_mut(input.image_id)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        image.set_content(deflate(&rows));
        if gray {
            image.dict.set("ColorSpace", "DeviceGray");
        }
        image.dict.set("DecodeParms", dictionary! { "Predictor" => 15, "Columns" => i64::from(WIDTH), "Colors" => if gray { 1 } else { 3 }, "BitsPerComponent" => 8 });
        let mut content = Vec::new();
        document.save_to(&mut content).unwrap();
        let input = FileValue {
            content: FileContent::Bytes(content),
            ..input.file
        };
        let result = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
        assert!(
            result.report.skipped_images.is_empty(),
            "{:?}",
            result.report.skipped_images
        );
        let output = Document::load_mem(bytes(&result.result)).unwrap();
        let image = output
            .get_object(input_image_id(&output))
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(
            image.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        assert!(image.dict.get(b"DecodeParms").is_err());
        assert_eq!(
            image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
            if gray {
                b"DeviceGray".as_slice()
            } else {
                b"DeviceRGB".as_slice()
            }
        );
        let decoded = image::load_from_memory(&image.content).unwrap();
        assert_eq!(decoded.color().channel_count(), if gray { 1 } else { 3 });
    }
}

fn input_image_id(document: &Document) -> ObjectId {
    *document
        .objects
        .iter()
        .find(|(_, object)| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .is_ok_and(|name| name == b"Image")
            })
        })
        .unwrap()
        .0
}

#[test]
fn malformed_image_is_preserved_without_blocking_healthy_image_compression() {
    for variant in ["corrupt", "dimensions", "channels"] {
        let input = fixture(true, &[]);
        let mut document = Document::load_mem(bytes(&input.file)).unwrap();
        let mut bad = document
            .get_object(input.image_id)
            .unwrap()
            .as_stream()
            .unwrap()
            .clone();
        match variant {
            "corrupt" => bad.set_content(b"not a jpeg".to_vec()),
            "dimensions" => bad.dict.set("Width", 1),
            "channels" => bad.dict.set("ColorSpace", "DeviceGray"),
            _ => unreachable!(),
        }
        let bad_id = document.add_object(bad.clone());
        // Keep the malformed image reachable so lossless pruning cannot remove it.
        document.trailer.set("MalformedImage", bad_id);
        let mut content = Vec::new();
        document.save_to(&mut content).unwrap();
        let file = FileValue {
            content: FileContent::Bytes(content),
            ..input.file
        };
        let result = pdf_compress(&file, bytes(&file).len() * 3 / 4, "balanced", 64 << 20).unwrap();
        assert_eq!(result.report.status, CompressionStatus::TargetMet);
        assert_eq!(result.report.skipped_images.len(), 1);
        if variant != "corrupt" {
            assert!(result.report.skipped_images[0].reason.contains("disagree"));
        }
        let output = Document::load_mem(bytes(&result.result)).unwrap();
        assert_eq!(
            output
                .get_object(bad_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .content,
            bad.content
        );
    }
}

#[test]
fn oversized_object_stream_cannot_silently_drop_the_catalog_and_pages() {
    // A valid PDF 1.5 with catalog/pages compressed into an object stream.
    // Padding makes inflation cross the parser cap while input stays small.
    let objects = [
        "<< /Type /Catalog /Pages 3 0 R >>",
        "<< /Type /Pages /Count 1 /Kids [4 0 R] >>",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 240 200] /Contents 5 0 R >>",
    ];
    let header = format!(
        "2 0 3 {} 4 {} ",
        objects[0].len() + 1,
        objects[0].len() + objects[1].len() + 2
    );
    let mut payload = header.as_bytes().to_vec();
    payload.extend_from_slice(objects.join(" ").as_bytes());
    payload.resize((16 << 20) + 1, b' ');
    let compressed = deflate(&payload);
    let mut pdf = b"%PDF-1.5\n".to_vec();
    let stream_offset = pdf.len();
    write!(
        pdf,
        "1 0 obj\n<< /Type /ObjStm /N 3 /First {} /Filter /FlateDecode /Length {} >>\nstream\n",
        header.len(),
        compressed.len()
    )
    .unwrap();
    pdf.extend_from_slice(&compressed);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let content_offset = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Length 0 >>\nstream\n\nendstream\nendobj\n");
    let xref_offset = pdf.len();
    let entries = [
        (0_u8, 0, 65535_u16),
        (1, stream_offset, 0),
        (2, 1, 0),
        (2, 1, 1),
        (2, 1, 2),
        (1, content_offset, 0),
        (1, xref_offset, 0),
    ];
    write!(
        pdf,
        "6 0 obj\n<< /Type /XRef /Size 7 /Root 2 0 R /W [1 4 2] /Length 49 >>\nstream\n"
    )
    .unwrap();
    for (kind, offset, index) in entries {
        pdf.push(kind);
        pdf.extend_from_slice(&u32::try_from(offset).unwrap().to_be_bytes());
        pdf.extend_from_slice(&index.to_be_bytes());
    }
    write!(
        pdf,
        "\nendstream\nendobj\nstartxref\n{xref_offset}\n%%EOF\n"
    )
    .unwrap();
    assert_eq!(Document::load_mem(&pdf).unwrap().get_pages().len(), 1);
    let file = FileValue {
        name: "object-stream.pdf".into(),
        mime: None,
        content: FileContent::Bytes(pdf),
    };
    assert!(
        pdf_compress(&file, 1, "balanced", 64 << 20)
            .unwrap_err()
            .contains("unreadable")
    );
}

#[test]
fn foreign_streams_are_preserved_during_a_real_candidate_rewrite_and_dl_is_cleared() {
    let input = fixture(
        true,
        &[("DL", Object::Integer(i64::from(WIDTH * HEIGHT * 3)))],
    );
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let mut foreign = document
        .get_object(input.image_id)
        .unwrap()
        .as_stream()
        .unwrap()
        .clone();
    foreign
        .dict
        .set("F", Object::string_literal("external-image.dat"));
    foreign.dict.set("FFilter", "DCTDecode");
    let foreign_id = document.add_object(foreign.clone());
    document.trailer.set("ForeignImage", foreign_id);
    let mut content = Vec::new();
    document.save_to(&mut content).unwrap();
    let file = FileValue {
        content: FileContent::Bytes(content),
        ..input.file
    };
    let result = pdf_compress(&file, 1, "balanced", 64 << 20).unwrap();
    assert!(bytes(&result.result).len() < bytes(&file).len());
    assert!(result.report.selected_profile.is_some());
    let output = Document::load_mem(bytes(&result.result)).unwrap();
    assert_eq!(
        output.get_object(foreign_id).unwrap(),
        &Object::Stream(foreign)
    );
    assert!(
        output
            .get_object(input.image_id)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"DL")
            .is_err()
    );
}

#[test]
fn unsupported_raw_image_bytes_are_preserved_and_tight_output_cap_can_succeed() {
    let mut input = fixture(false, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let stream = document
        .get_object_mut(input.image_id)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.dict.remove(b"Filter");
    stream.dict.remove(b"ColorSpace");
    stream.dict.set("ImageMask", true);
    stream.dict.set("BitsPerComponent", 1);
    stream.set_content(vec![0xF0; (WIDTH as usize / 8) * HEIGHT as usize]);
    let mut content = Vec::new();
    document.save_to(&mut content).unwrap();
    input.file.content = FileContent::Bytes(content);
    let result = pdf_compress(&input.file, 1, "balanced", 64 << 20).unwrap();
    let original = Document::load_mem(bytes(&input.file)).unwrap();
    let output = Document::load_mem(bytes(&result.result)).unwrap();
    assert_eq!(
        output.get_object(input.image_id).unwrap(),
        original.get_object(input.image_id).unwrap()
    );
    let supported = fixture(true, &[]).file;
    let target = bytes(&supported).len() / 2;
    let result = pdf_compress(&supported, target, "balanced", target).unwrap();
    assert!(bytes(&result.result).len() <= target);
}

#[test]
fn dispatcher_returns_pdf_and_report_and_requires_target() {
    crate::register_all();
    let input = fixture(true, &[]).file;
    let args = serde_json::json!({"input": input, "target_bytes": 1});
    let Some(ToolResult::Success(result)) =
        upeg_runtime::try_runtime_dispatch(PDF_COMPRESS_TOOL_ID, &args)
    else {
        panic!("expected compression success");
    };
    assert_eq!(
        result.primary_output_id.as_deref(),
        Some(PDF_COMPRESS_FILE_OUTPUT_ID)
    );
    assert!(matches!(&result.outputs[0].value, OutputValue::File(_)));
    assert!(
        matches!(&result.outputs[1].value, OutputValue::Json(report) if report["status"] == "target_unmet")
    );
    assert!(matches!(
        upeg_runtime::try_runtime_dispatch(
            PDF_COMPRESS_TOOL_ID,
            &serde_json::json!({"input": input})
        ),
        Some(ToolResult::Failure(_))
    ));
    for mode in [
        serde_json::json!(1),
        serde_json::json!({}),
        serde_json::json!("unknown"),
    ] {
        assert!(matches!(
            upeg_runtime::try_runtime_dispatch(
                PDF_COMPRESS_TOOL_ID,
                &serde_json::json!({"input": input, "target_bytes": 1, "mode": mode})
            ),
            Some(ToolResult::Failure(_))
        ));
    }
}
