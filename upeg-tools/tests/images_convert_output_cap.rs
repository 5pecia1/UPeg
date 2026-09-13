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

    fn 이미지_일괄_변환_args(max_output_bytes: Option<usize>) -> Value {
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

    fn 실패_메시지(result: RegisteredDispatch) -> String {
        match result {
            RegisteredDispatch::Ran(ToolResult::Failure(failure)) => failure.error.message,
            other => panic!("expected dispatcher failure, got {other:?}"),
        }
    }

    #[test]
    fn dispatcher는_output_cap을_생략하면_기본값으로_변환한다() {
        let result = dispatch_registered(IMAGES_CONVERT_TOOL_ID, &이미지_일괄_변환_args(None));

        assert!(
            matches!(result, RegisteredDispatch::Ran(ToolResult::Success(_))),
            "omitted max_output_bytes should use its default, got {result:?}"
        );
    }

    #[test]
    fn dispatcher는_작은_output_cap을_이미지_인코더까지_전달한다() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &이미지_일괄_변환_args(Some(SMALL_USER_OUTPUT_CAP_BYTES)),
        );

        let error = 실패_메시지(result);
        assert!(error.contains("encoded raster"), "got {error:?}");
    }

    #[test]
    fn dispatcher_schema는_0_byte_output_cap을_거부한다() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &이미지_일괄_변환_args(Some(ZERO_OUTPUT_CAP_BYTES)),
        );

        let error = 실패_메시지(result);
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }

    #[test]
    fn dispatcher_schema는_hard_cap보다_1_byte_큰_요청을_거부한다() {
        let result = dispatch_registered(
            IMAGES_CONVERT_TOOL_ID,
            &이미지_일괄_변환_args(Some(IMAGES_CONVERT_MAX_OUTPUT_BYTES + 1)),
        );

        let error = 실패_메시지(result);
        assert!(error.contains("max_output_bytes"), "got {error:?}");
    }
}
