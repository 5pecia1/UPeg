use std::io::{Cursor, Write};

use upeg_core::{FileContent, FileValue, OutputValue, StaticInputKind, ToolResult};

use super::*;

const TINY_OUTPUT_CAP: usize = 1;
const MAX_MEDIA_OUTPUT_BYTES_F64: f64 = 67_108_864.0;

fn png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 40, 60, u8::MAX]));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("PNG fixture should encode");
    bytes
}

fn image_zip_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut archive = zip::ZipWriter::new(Cursor::new(&mut bytes));
        archive
            .start_file("image.png", zip::write::SimpleFileOptions::default())
            .expect("zip entry should start");
        archive
            .write_all(&png_bytes())
            .expect("zip entry should write");
        archive.finish().expect("zip fixture should finalize");
    }
    bytes
}

fn byte_file(name: &str, bytes: Vec<u8>) -> FileValue {
    FileValue {
        name: name.to_string(),
        mime: None,
        content: FileContent::Bytes(bytes),
    }
}

fn output_file(result: &str) -> FileValue {
    serde_json::from_str(result).expect("tool output should be FileValue JSON")
}

fn output_bytes(result: &str) -> Vec<u8> {
    match output_file(result).content {
        FileContent::Bytes(bytes) => bytes,
        FileContent::Directory(_) => panic!("tool output should be a byte file"),
    }
}

fn tiny_pdf() -> Vec<u8> {
    let result = image_to_pdf(
        &byte_file("images.zip", image_zip_bytes()),
        MAX_MEDIA_OUTPUT_BYTES,
    )
    .expect("fixture PDF should build");
    output_bytes(&result)
}

fn dispatched_output_file(result: Option<ToolResult>) -> FileValue {
    let success = match result {
        Some(ToolResult::Success(success)) => success,
        Some(ToolResult::Failure(failure)) => {
            panic!("dispatcher should succeed, got {:?}", failure.error)
        }
        None => panic!("dispatcher should be registered"),
    };
    let output = success
        .outputs
        .iter()
        .find(|output| output.id == "result")
        .expect("result output should exist");
    match &output.value {
        OutputValue::File(file) => file.clone(),
        other => panic!("result should be File output, got {other:?}"),
    }
}

#[test]
fn pdf_도구_메타는_최대_출력_바이트_계약을_노출한다() {
    for tool_id in ["media.image_to_pdf", "media.pdf_to_images"] {
        let meta = upeg_core::inventory::iter::<upeg_core::StaticToolMeta>()
            .find(|meta| meta.id == tool_id)
            .expect("PDF tool metadata should exist");
        let field = meta
            .input_spec
            .fields
            .iter()
            .find(|field| field.name == "max_output_bytes")
            .expect("max_output_bytes field should exist");

        assert!(!field.required);
        assert_eq!(field.kind, StaticInputKind::Integer);
        let constraints = field
            .constraints
            .number
            .expect("max_output_bytes should have numeric constraints");
        assert_eq!(constraints.min, Some(1.0));
        assert_eq!(constraints.max, Some(MAX_MEDIA_OUTPUT_BYTES_F64));
        assert_eq!(constraints.default, Some(MAX_MEDIA_OUTPUT_BYTES_F64));
    }
}

#[test]
fn 이미지_to_pdf는_유효하지_않은_최대_출력_바이트를_거부한다() {
    let input = byte_file("images.zip", image_zip_bytes());

    for invalid in [0, MAX_MEDIA_OUTPUT_BYTES + 1] {
        let error = image_to_pdf(&input, invalid).expect_err("out-of-range output cap should fail");
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }
}

#[test]
fn pdf_to_이미지는_유효하지_않은_최대_출력_바이트를_거부한다() {
    let input = byte_file("document.pdf", tiny_pdf());

    for invalid in [0, MAX_MEDIA_OUTPUT_BYTES + 1] {
        let error =
            pdf_to_images(&input, 72.0, invalid).expect_err("out-of-range output cap should fail");
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }
}

#[test]
fn 이미지_to_pdf는_작은_출력_상한을_실제_pdf에_적용한다() {
    let input = byte_file("images.zip", image_zip_bytes());

    let error = image_to_pdf(&input, TINY_OUTPUT_CAP).expect_err("one-byte PDF cap should fail");

    assert!(
        error.contains("output PDF") && error.contains("1-byte limit"),
        "got {error:?}"
    );
}

#[test]
fn pdf_to_이미지는_작은_출력_상한으로_페이지_인코딩을_일찍_중단한다() {
    let input = byte_file("document.pdf", tiny_pdf());

    let error = pdf_to_images(&input, 72.0, TINY_OUTPUT_CAP)
        .expect_err("one-byte page output cap should fail");

    assert!(
        error.contains("rendered pages") && error.contains("1-byte limit"),
        "got {error:?}"
    );
}

#[test]
fn 이미지_to_pdf는_실제_pdf_크기와_같은_출력_상한을_허용한다() {
    let input = byte_file("images.zip", image_zip_bytes());
    let default_result =
        image_to_pdf(&input, MAX_MEDIA_OUTPUT_BYTES).expect("default cap should succeed");
    let exact_bytes = output_bytes(&default_result).len();

    let exact_result =
        image_to_pdf(&input, exact_bytes).expect("inclusive exact PDF cap should succeed");

    assert_eq!(output_bytes(&exact_result).len(), exact_bytes);
}

#[test]
fn pdf_to_이미지는_실제_zip_크기와_같은_출력_상한을_허용한다() {
    let input = byte_file("document.pdf", tiny_pdf());
    let default_result =
        pdf_to_images(&input, 72.0, MAX_MEDIA_OUTPUT_BYTES).expect("default cap should succeed");
    let exact_bytes = output_bytes(&default_result).len();

    let exact_result =
        pdf_to_images(&input, 72.0, exact_bytes).expect("inclusive exact zip cap should succeed");

    assert_eq!(output_bytes(&exact_result).len(), exact_bytes);
}

#[test]
fn pdf_to_이미지는_페이지_합계보다_큰_상한도_zip이_넘으면_거부한다() {
    let input = byte_file("document.pdf", tiny_pdf());
    let default_result =
        pdf_to_images(&input, 72.0, MAX_MEDIA_OUTPUT_BYTES).expect("default cap should succeed");
    let zip_bytes = output_bytes(&default_result);
    let zip_cap = zip_bytes
        .len()
        .checked_sub(1)
        .expect("fixture zip should not be empty");
    let mut archive =
        zip::ZipArchive::new(Cursor::new(&zip_bytes)).expect("output zip should open");
    let page_bytes = (0..archive.len())
        .try_fold(0_usize, |total, index| {
            let entry_bytes = usize::try_from(
                archive
                    .by_index(index)
                    .expect("page entry should open")
                    .size(),
            )
            .expect("page size should fit usize");
            total.checked_add(entry_bytes)
        })
        .expect("page byte sum should fit usize");
    assert!(
        page_bytes <= zip_cap,
        "fixture must reach the zip writer cap"
    );

    let error =
        pdf_to_images(&input, 72.0, zip_cap).expect_err("zip overhead should exceed the cap");

    assert!(
        error.contains("PDF pages output zip") && error.contains(&format!("{zip_cap}-byte limit")),
        "got {error:?}"
    );
}

#[test]
fn 이미지_to_pdf_dispatcher는_생략한_출력_상한에_기본값을_사용한다() {
    crate::register_all();
    let input = byte_file("images.zip", image_zip_bytes());

    for args in [
        serde_json::json!({
            "input": serde_json::to_value(&input).expect("input should serialize"),
        }),
        serde_json::json!({
            "input": serde_json::to_value(&input).expect("input should serialize"),
            "max_output_bytes": null,
        }),
    ] {
        let output = dispatched_output_file(upeg_runtime::try_runtime_dispatch(
            crate::IMAGE_TO_PDF_TOOL_ID,
            &args,
        ));
        assert!(matches!(output.content, FileContent::Bytes(bytes) if bytes.starts_with(b"%PDF-")));
    }
}

#[test]
fn pdf_to_이미지는_dispatcher에서_생략한_출력_상한에_기본값을_사용한다() {
    crate::register_all();
    let input = byte_file("document.pdf", tiny_pdf());

    for args in [
        serde_json::json!({
            "input": serde_json::to_value(&input).expect("input should serialize"),
            "dpi": 72,
        }),
        serde_json::json!({
            "input": serde_json::to_value(&input).expect("input should serialize"),
            "dpi": 72,
            "max_output_bytes": null,
        }),
    ] {
        let output = dispatched_output_file(upeg_runtime::try_runtime_dispatch(
            crate::PDF_TO_IMAGES_TOOL_ID,
            &args,
        ));
        assert!(matches!(output.content, FileContent::Bytes(bytes) if bytes.starts_with(b"PK")));
    }
}
