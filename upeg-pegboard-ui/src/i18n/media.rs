//! Image conversion and file-output copy.
use phf::{Map, phf_map};

pub(super) static EN: Map<&'static str, &'static str> = phf_map! {
    "media.convert.guidance" => "Choose an image and output format, then convert and save. SVG becomes pixels. Animated images and TIFF use the first frame/page; WebP is lossless.",
    "media.convert.advanced" => "Advanced settings",
    "media.convert.ico" => "ICO supports images up to 256 × 256 pixels. For SVG, set the output width to 256 or less.",
    "media.convert.input" => "Image",
    "media.convert.images" => "Images (ZIP output)",
    "media.convert.format" => "Output format",
    "media.convert.quality" => "JPEG quality",
    "media.convert.background" => "Background for transparent pixels (#RRGGBB)",
    "media.convert.svg_width" => "SVG width in pixels (0 = original)",
    "media.convert.limit" => "Maximum output size in bytes",
    "media.file.preview" => "Converted image preview",
    "media.file.preview_unavailable" => "Preview unavailable. Save the file to view it.",
    "media.file.save" => "Save",
    "media.file.save_title" => "Save output file",
    "media.file.saving" => "Saving…",
    "media.file.saved" => "Saved.",
    "media.file.download_started" => "Download started. Check your browser downloads.",
    "media.file.save_failed" => "Could not save the file. Check the destination and try again.",
    "tool.media.image_convert.label" => "Convert image",
    "tool.media.images_convert.label" => "Batch convert images",
};
pub(super) static KO: Map<&'static str, &'static str> = phf_map! {
    "media.convert.guidance" => "이미지와 출력 형식을 선택하고 변환한 뒤 저장하세요. SVG는 픽셀 이미지로 변환합니다. 애니메이션·TIFF는 첫 프레임/페이지만, WebP는 무손실로 변환합니다.",
    "media.convert.advanced" => "고급 설정",
    "media.convert.ico" => "ICO는 최대 256 × 256 픽셀을 지원합니다. SVG는 출력 폭을 256 이하로 설정하세요.",
    "media.convert.input" => "이미지",
    "media.convert.images" => "이미지 여러 장 (ZIP 출력)",
    "media.convert.format" => "출력 형식",
    "media.convert.quality" => "JPEG 품질",
    "media.convert.background" => "투명 영역의 배경색 (#RRGGBB)",
    "media.convert.svg_width" => "SVG 출력 폭 (픽셀, 0 = 원본)",
    "media.convert.limit" => "최대 출력 크기 (바이트)",
    "media.file.preview" => "변환된 이미지 미리보기",
    "media.file.preview_unavailable" => "미리보기를 지원하지 않습니다. 파일을 저장하여 확인하세요.",
    "media.file.save" => "저장",
    "media.file.save_title" => "결과 파일 저장",
    "media.file.saving" => "저장 중…",
    "media.file.saved" => "저장했습니다.",
    "media.file.download_started" => "다운로드를 시작했습니다. 브라우저 다운로드 목록을 확인하세요.",
    "media.file.save_failed" => "파일을 저장하지 못했습니다. 저장 위치를 확인하고 다시 시도하세요.",
    "tool.media.image_convert.label" => "이미지 변환",
    "tool.media.images_convert.label" => "이미지 일괄 변환",
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_conversion_copy_has_the_same_keys_in_both_languages() {
        assert_eq!(EN.len(), KO.len());
        for key in EN.keys() {
            assert!(KO.contains_key(key), "{key}");
        }
    }
}
