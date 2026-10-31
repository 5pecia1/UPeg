import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;

const Map<String, Map<LocaleDto, String>> mediaTestCatalog = {
  "media.convert.guidance": {
    LocaleDto.en:
        "Choose an image and output format, then convert and save. PDF-compatible AI uses its first page. SVG input becomes pixels; SVG output vectorizes raster images, which may lose detail and create large files; complex images may exceed processing limits. Animated images and TIFF use the first frame/page; WebP is lossless.",
    LocaleDto.ko:
        "이미지와 출력 형식을 선택하고 변환한 뒤 저장하세요. PDF 호환 AI는 첫 페이지만 사용합니다. SVG 입력은 픽셀로 변환하며, SVG 출력은 래스터 이미지를 벡터화하므로 세부 정보가 손실되거나 파일이 커질 수 있으며 복잡한 이미지는 처리 제한을 초과할 수 있습니다. 애니메이션·TIFF는 첫 프레임/페이지만, WebP는 무손실로 변환합니다.",
  },
  "media.convert.advanced": {
    LocaleDto.en: "Advanced settings",
    LocaleDto.ko: "고급 설정",
  },
  "media.convert.ico": {
    LocaleDto.en:
        "ICO supports images up to 256 × 256 pixels. For SVG, set the output width to 256 or less.",
    LocaleDto.ko: "ICO는 최대 256 × 256 픽셀을 지원합니다. SVG는 출력 폭을 256 이하로 설정하세요.",
  },
  "media.convert.input": {LocaleDto.en: "Image", LocaleDto.ko: "이미지"},
  "media.convert.images": {
    LocaleDto.en: "Images (ZIP output)",
    LocaleDto.ko: "이미지 여러 장 (ZIP 출력)",
  },
  "media.convert.format": {
    LocaleDto.en: "Output format",
    LocaleDto.ko: "출력 형식",
  },
  "media.convert.quality": {
    LocaleDto.en: "JPEG quality",
    LocaleDto.ko: "JPEG 품질",
  },
  "media.convert.background": {
    LocaleDto.en: "Background for transparent pixels (#RRGGBB)",
    LocaleDto.ko: "투명 영역의 배경색 (#RRGGBB)",
  },
  "media.convert.svg_width": {
    LocaleDto.en: "SVG width in pixels (0 = original)",
    LocaleDto.ko: "SVG 출력 폭 (픽셀, 0 = 원본)",
  },
  "media.convert.ai_dpi": {
    LocaleDto.en: "AI rendering resolution (DPI)",
    LocaleDto.ko: "AI 렌더링 해상도 (DPI)",
  },
  "media.convert.vector_preset": {
    LocaleDto.en: "SVG vectorization preset",
    LocaleDto.ko: "SVG 벡터화 프리셋",
  },
  "media.convert.limit": {
    LocaleDto.en: "Maximum output size in bytes",
    LocaleDto.ko: "최대 출력 크기 (바이트)",
  },
  "media.file.preview": {
    LocaleDto.en: "Converted image preview",
    LocaleDto.ko: "변환된 이미지 미리보기",
  },
  "media.file.preview_unavailable": {
    LocaleDto.en: "Preview unavailable. Save the file to view it.",
    LocaleDto.ko: "미리보기를 지원하지 않습니다. 파일을 저장하여 확인하세요.",
  },
  "media.file.save": {LocaleDto.en: "Save", LocaleDto.ko: "저장"},
  "media.file.save_title": {
    LocaleDto.en: "Save output file",
    LocaleDto.ko: "결과 파일 저장",
  },
  "media.file.saving": {LocaleDto.en: "Saving…", LocaleDto.ko: "저장 중…"},
  "media.file.saved": {LocaleDto.en: "Saved.", LocaleDto.ko: "저장했습니다."},
  "media.file.download_started": {
    LocaleDto.en: "Download started. Check your browser downloads.",
    LocaleDto.ko: "다운로드를 시작했습니다. 브라우저 다운로드 목록을 확인하세요.",
  },
  "media.file.save_failed": {
    LocaleDto.en:
        "Could not save the file. Check the destination and try again.",
    LocaleDto.ko: "파일을 저장하지 못했습니다. 저장 위치를 확인하고 다시 시도하세요.",
  },
  "tool.media.image_convert.label": {
    LocaleDto.en: "Convert image",
    LocaleDto.ko: "이미지 변환",
  },
  "tool.media.images_convert.label": {
    LocaleDto.en: "Batch convert images",
    LocaleDto.ko: "이미지 일괄 변환",
  },
};
