---
type: Guide
title: PDF 검사와 Markdown 추출
description: "`media.pdf_inspect`/`media.pdf_to_markdown`의 사용법, 반환 계약, 자원 상한과 CMap 내장 정책."
tags: [guide, media, pdf]
status: stable
---

# PDF 검사와 Markdown 추출

`media.pdf_inspect`와 `media.pdf_to_markdown`은 pdf-inspector 1.17.0으로
로컬에서 실행한다. 파일 경로를 파서에 전달하지 않고 공통 `FileValue`의
바이트를 사용하므로 네이티브와 FRB WASM에서 같은 구현을 사용한다.
OCR·외부 서비스 호출·모델 다운로드는 수행하지 않는다.

```sh
upeg call media.pdf_inspect -a input=@report.pdf --json
upeg call media.pdf_to_markdown -a input=@report.pdf --json
upeg call media.pdf_to_markdown -a input=@report.pdf --field markdown
upeg call media.pdf_to_markdown -a input=@report.pdf --field report
```

검사는 전체 페이지를 스캔한다. 결과는 `pdf_type`, `page_count`,
`confidence`, `pages_needing_ocr`를 포함한다. 페이지 번호는 1부터 시작한다.
검사만 실행하면 `has_encoding_issues`와 `extraction_status`는 `null`이다.
텍스트를 해독하지 않은 상태를 정상 인코딩으로 오인하지 않도록 구분한다.

변환은 primary `markdown`과 JSON `report`를 반환한다.
`extraction_status`는 `complete`, `partial`, `unavailable` 중 하나다.
`complete`는 OCR 필요 페이지가 감지되지 않았다는 뜻이며 정확성 보장은 아니다.
`unavailable`이면 Markdown은 빈 문자열이다. 자동화에서는 `--json` 또는
`--field report`로 상태를 확인해야 한다. 기본 CLI 출력은 Markdown만 표시한다.
스캔·벡터 문자·손상된 인코딩은 별도 OCR이 필요할 수 있다.

입력은 32 MiB, 페이지는 500개, 반환 Markdown은 8 MiB로 제한한다.
입력 바이트 제한은 파싱 전, 페이지 제한은 객체 그래프 파싱 후이지만
내용 스트림 검사 전, 출력 제한은 Markdown 생성 후에 적용한다.
**이 제한은 파서 내부의 전체 메모리 사용량이나 실행 시간을 제한하지 않는다.**
복잡한 PDF의 메모리 사용과 브라우저 응답성은 실문서로 추가 측정해야 한다.

CMap은 모든 타깃의 바이너리에 내장한다. 빌드 머신 경로나 실행 환경변수가
필요하지 않다. 소스 고정 이유와 패치 내역은
[vendored crate 설명](../vendor/pdf-inspector/UPEG.md)에 기록한다.
기존 PDF 이미지 추출·렌더링·생성 도구는 별도 엔진을 계속 사용한다.
배포물에 `vendor/pdf-inspector/LICENSE`와 `external/bcmaps/LICENSE` 고지를
함께 실어야 하며 전달 장치는 아직 없다(남은 일 대장 참조).
