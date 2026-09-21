#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "test fixtures and failure diagnostics use explicit expect and panic messages"
    )]

    use image::DynamicImage;
    use serde_json::{Value, json};
    use upeg_core::{FileContent, FileValue, ToolResult};
    use upeg_tools::{IMAGES_CONVERT_MAX_OUTPUT_BYTES, RegisteredDispatch, dispatch_registered};

    const ZERO_OUTPUT_CAP_BYTES: usize = 0;
    const SMALL_USER_OUTPUT_CAP_BYTES: usize = 32;
    const IMAGES_CONVERT_TOOL_ID: &str = "media.images_convert";

    fn images_convert_args(max_output_bytes: Option<usize>) -> Value {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([1, 2, 3, u8::MAX]),
        ));
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("PNG fixture should encode");
        let directory = FileValue {
            name: "photos".to_string(),
            mime: None,
            content: FileContent::Directory(vec![FileValue {
                name: "one.png".to_string(),
                mime: Some("image/png".to_string()),
                content: FileContent::Bytes(png),
            }]),
        };
        let mut args = json!({
            "images": directory,
            "output_format": "png",
        });
        if let Some(max_output_bytes) = max_output_bytes {
            args["max_output_bytes"] = json!(max_output_bytes);
        }
        args
    }

    fn failure_message(result: RegisteredDispatch) -> String {
        match result {
            RegisteredDispatch::Ran(ToolResult::Failure(failure)) => failure.error.message,
            other => panic!("expected dispatcher failure, got {other:?}"),
        }
    }

    #[test]
    fn dispatcher_converts_with_default_when_output_cap_omitted() {
        let result = dispatch_registered(IMAGES_CONVERT_TOOL_ID, &images_convert_args(None));

        assert!(
            matches!(result, RegisteredDispatch::Ran(ToolResult::Success(_))),
            "omitted max_output_bytes should use its default, got {result:?}"
        );
    }

    #[test]
    fn dispatcher_passes_small_output_cap_to_image_encoder() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &images_convert_args(Some(SMALL_USER_OUTPUT_CAP_BYTES)),
        );

        let error = failure_message(result);
        assert!(error.contains("encoded raster"), "got {error:?}");
    }

    #[test]
    fn dispatcher_schema_rejects_zero_byte_output_cap() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &images_convert_args(Some(ZERO_OUTPUT_CAP_BYTES)),
        );

        let error = failure_message(result);
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }

    #[test]
    fn dispatcher_schema_rejects_request_one_byte_over_hard_cap() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &images_convert_args(Some(IMAGES_CONVERT_MAX_OUTPUT_BYTES + 1)),
        );

        let error = failure_message(result);
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }
}
