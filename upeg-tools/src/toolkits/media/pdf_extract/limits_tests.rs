use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

use super::{ExtractionLimits, extract_embedded_images, extract_embedded_images_with_limits};

const ALLOWED_IMAGE_COUNT: usize = 100;
const REJECTED_IMAGE_COUNT: usize = ALLOWED_IMAGE_COUNT + 1;
const ALLOWED_FORM_DEPTH: usize = 64;
const REJECTED_FORM_DEPTH: usize = ALLOWED_FORM_DEPTH + 1;

#[test]
fn pdf_추출은_101번째_고유_이미지_xobject를_디코드_전에_거부한다() {
    // Given
    let pdf = pdf_with_images(REJECTED_IMAGE_COUNT);

    // When
    let error = match extract_embedded_images(&pdf, "many") {
        Ok(_) => panic!("101번째 고유 이미지는 개수 예산을 초과해야 한다"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("100"),
        "오류에 이미지 개수 상한이 포함되어야 한다: {error}"
    );
}

#[test]
fn pdf_추출은_65단계_form_xobject_중첩을_거부한다() {
    // Given
    let pdf = pdf_with_nested_forms(REJECTED_FORM_DEPTH, false);

    // When
    let error = match extract_embedded_images(&pdf, "deep") {
        Ok(_) => panic!("65번째 Form 진입은 깊이 예산을 초과해야 한다"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("64"),
        "오류에 Form 깊이 상한이 포함되어야 한다: {error}"
    );
}

#[test]
fn pdf_추출은_순환_form을_한번만_방문한다() {
    // Given
    let pdf = pdf_with_nested_forms(ALLOWED_FORM_DEPTH, true);

    // When
    let outcome =
        extract_embedded_images(&pdf, "cycle").expect("이미 방문한 Form 순환은 건너뛰어야 한다");

    // Then
    assert_eq!(outcome.images.len(), 1);
}

#[test]
fn pdf_추출은_인코딩된_이미지의_누적_바이트_상한을_지킨다() {
    // Given
    let pdf = pdf_with_images(2);
    let baseline = extract_embedded_images(&pdf, "baseline")
        .expect("두 개별 이미지는 기본 예산 안에서 추출되어야 한다");
    let largest_image_bytes = baseline
        .images
        .iter()
        .map(|image| image.bytes.len())
        .max()
        .expect("두 테스트 이미지가 추출되어야 한다");
    assert_eq!(baseline.images.len(), 2);
    let limits = ExtractionLimits {
        max_form_depth: ALLOWED_FORM_DEPTH,
        max_images: ALLOWED_IMAGE_COUNT,
        max_encoded_bytes: largest_image_bytes,
    };

    // When
    let error = match extract_embedded_images_with_limits(&pdf, "bytes", limits) {
        Ok(_) => panic!("각 이미지는 합법이어도 두 이미지의 누적 바이트는 거부해야 한다"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("PDF extracted image bytes"),
        "오류에 누적 이미지 바이트 예산이 포함되어야 한다: {error}"
    );
}

#[test]
fn pdf_추출은_최종_zip의_전체_바이트_상한을_지킨다() {
    // Given
    let images = vec![("image.png".to_string(), vec![0_u8; 8])];

    // When
    let error = super::super::pack_named_images_zip_capped(
        &images,
        Some("notes"),
        1,
        "PDF extraction output zip",
    )
    .expect_err("zip 헤더와 노트를 포함한 전체 결과가 1바이트를 넘으면 거부해야 한다");

    // Then
    assert!(
        error.contains("PDF extraction output zip"),
        "오류에 최종 zip 바이트 예산이 포함되어야 한다: {error}"
    );
}

fn pdf_with_images(image_count: usize) -> Vec<u8> {
    let mut document = Document::with_version("1.7");
    let mut xobjects = Dictionary::new();
    for index in 0..image_count {
        let image_id = add_raw_image(&mut document);
        xobjects.set(format!("Im{index}").into_bytes(), image_id);
    }
    finish_pdf(document, xobjects)
}

fn pdf_with_nested_forms(form_count: usize, cycle: bool) -> Vec<u8> {
    let mut document = Document::with_version("1.7");
    let image_id = add_raw_image(&mut document);
    let mut child_id = image_id;

    for index in 0..form_count {
        let form_id = document.new_object_id();
        let target_id = if cycle && index == 0 {
            form_id
        } else {
            child_id
        };
        let mut xobjects = Dictionary::new();
        xobjects.set(b"Child".to_vec(), target_id);
        if cycle && index == 0 {
            xobjects.set(b"Image".to_vec(), image_id);
        }
        document.objects.insert(
            form_id,
            Object::Stream(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "Resources" => dictionary! { "XObject" => xobjects },
                },
                Vec::new(),
            )),
        );
        child_id = form_id;
    }

    let mut xobjects = Dictionary::new();
    xobjects.set(b"Root".to_vec(), child_id);
    finish_pdf(document, xobjects)
}

fn add_raw_image(document: &mut Document) -> ObjectId {
    document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1_i64,
            "Height" => 1_i64,
            "ColorSpace" => "DeviceGray",
            "BitsPerComponent" => 8_i64,
        },
        vec![0],
    ))
}

fn finish_pdf(mut document: Document, xobjects: Dictionary) -> Vec<u8> {
    let contents = document.add_object(Stream::new(dictionary! {}, Vec::new()));
    let resources = document.add_object(dictionary! { "XObject" => xobjects });
    let page_id = document.new_object_id();
    let pages_id = document.new_object_id();
    document.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => contents,
            "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1_i64,
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("테스트 PDF를 직렬화할 수 있어야 한다");
    bytes
}
