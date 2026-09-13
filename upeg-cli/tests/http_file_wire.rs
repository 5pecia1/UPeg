#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    reason = "integration tests use unwrap/expect/panic idiomatically and need not satisfy production restriction lints"
)]

use std::io::Cursor;

use axum::body::{Body, to_bytes};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use http::{Request, StatusCode};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};
use tower::ServiceExt;
use upeg_cli::http_router;
use zip::ZipArchive;

const IMAGES_CONVERT_PATH: &str = "/v1/tools/media.images_convert";
const DIRECTORY_NAME: &str = "photos";
const OUTPUT_FILE_NAME: &str = "photos-images.zip";
const OUTPUT_MIME: &str = "application/zip";
const EXPECTED_ZIP_ENTRIES: [&str; 2] = ["first.png", "second.png"];

fn encoded_image(format: ImageFormat, pixel: Rgba<u8>) -> String {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, pixel));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("이미지 fixture를 인코딩해야 한다");
    STANDARD.encode(bytes)
}

fn canonical_file(name: &str, mime: &str, bytes: String) -> Value {
    json!({
        "name": name,
        "is_dir": false,
        "mime": mime,
        "content": {
            "kind": "bytes",
            "bytes": bytes,
        },
    })
}

fn images_directory(entries: Vec<Value>) -> Value {
    json!({
        "name": DIRECTORY_NAME,
        "is_dir": true,
        "content": {
            "kind": "directory",
            "entries": entries,
        },
    })
}

async fn post_images_convert(images: Value) -> (StatusCode, Value) {
    let response = http_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(IMAGES_CONVERT_PATH)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "images": images,
                        "output_format": "png",
                    })
                    .to_string(),
                ))
                .expect("HTTP 요청을 만들어야 한다"),
        )
        .await
        .expect("HTTP 라우터가 응답해야 한다");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("HTTP 응답 본문을 읽어야 한다");
    let body = serde_json::from_slice(&bytes).expect("HTTP 응답은 JSON이어야 한다");
    (status, body)
}

#[tokio::test]
async fn 정규_file_value_wire의_png와_jpeg를_http로_변환하면_정규_zip_file_value가_반환된다() {
    let png = encoded_image(ImageFormat::Png, Rgba([1, 2, 3, u8::MAX]));
    let jpeg = encoded_image(ImageFormat::Jpeg, Rgba([4, 5, 6, u8::MAX]));
    assert_eq!(png.len() % 4, 0, "PNG fixture는 padded base64여야 한다");
    assert_eq!(jpeg.len() % 4, 0, "JPEG fixture는 padded base64여야 한다");
    let images = images_directory(vec![
        canonical_file("first.png", "image/png", png),
        canonical_file("second.jpeg", "image/jpeg", jpeg),
    ]);

    let (status, body) = post_images_convert(images).await;

    assert_eq!(status, StatusCode::OK, "HTTP 변환이 실패했다: {body}");
    assert_eq!(body["ok"], true);
    assert_eq!(body["primary_output_id"], "result");
    let output = &body["outputs"][0];
    assert_eq!(output["kind"], "file");
    let file = &output["value"];
    assert_eq!(file["name"], OUTPUT_FILE_NAME);
    assert_eq!(file["is_dir"], false);
    assert_eq!(file["mime"], OUTPUT_MIME);
    assert_eq!(file["content"]["kind"], "bytes");

    let encoded_zip = file["content"]["bytes"]
        .as_str()
        .expect("ZIP FileValue 본문은 base64 문자열이어야 한다");
    let zip_bytes = STANDARD
        .decode(encoded_zip)
        .expect("ZIP FileValue 본문은 정규 RFC 4648 base64여야 한다");
    assert_eq!(
        STANDARD.encode(&zip_bytes),
        encoded_zip,
        "ZIP FileValue 본문은 canonical padded base64여야 한다"
    );
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).expect("출력은 ZIP이어야 한다");
    let names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("ZIP 항목을 읽어야 한다")
                .name()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, EXPECTED_ZIP_ENTRIES);
}

#[tokio::test]
async fn 구형_숫자_배열_file_value_wire는_http_경계에서_거부된다() {
    let legacy_file = json!({
        "name": "legacy.png",
        "is_dir": false,
        "mime": "image/png",
        "content": {
            "kind": "bytes",
            "bytes": [137, 80, 78, 71],
        },
    });

    let (status, body) = post_images_convert(images_directory(vec![legacy_file])).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "invalid_args");
}
