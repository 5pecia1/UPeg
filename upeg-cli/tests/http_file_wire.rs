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
        .expect("must encode the image fixture");
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
                .expect("must build the HTTP request"),
        )
        .await
        .expect("the HTTP router must answer");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("must read the HTTP response body");
    let body = serde_json::from_slice(&bytes).expect("the HTTP response must be JSON");
    (status, body)
}

#[tokio::test]
async fn converting_canonical_file_value_png_and_jpeg_over_http_returns_a_canonical_zip_file_value()
{
    let png = encoded_image(ImageFormat::Png, Rgba([1, 2, 3, u8::MAX]));
    let jpeg = encoded_image(ImageFormat::Jpeg, Rgba([4, 5, 6, u8::MAX]));
    assert_eq!(png.len() % 4, 0, "PNG fixture must be padded base64");
    assert_eq!(jpeg.len() % 4, 0, "JPEG fixture must be padded base64");
    let images = images_directory(vec![
        canonical_file("first.png", "image/png", png),
        canonical_file("second.jpeg", "image/jpeg", jpeg),
    ]);

    let (status, body) = post_images_convert(images).await;

    assert_eq!(status, StatusCode::OK, "HTTP conversion failed: {body}");
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
        .expect("the ZIP FileValue body must be a base64 string");
    let zip_bytes = STANDARD
        .decode(encoded_zip)
        .expect("the ZIP FileValue body must be canonical RFC 4648 base64");
    assert_eq!(
        STANDARD.encode(&zip_bytes),
        encoded_zip,
        "the ZIP FileValue body must be canonical padded base64"
    );
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).expect("the output must be a ZIP");
    let names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("must read the ZIP entry")
                .name()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, EXPECTED_ZIP_ENTRIES);
}

#[tokio::test]
async fn a_legacy_numeric_array_file_value_wire_is_refused_at_the_http_boundary() {
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
