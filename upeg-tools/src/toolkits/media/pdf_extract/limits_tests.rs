use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

use super::{ExtractionLimits, extract_embedded_images, extract_embedded_images_with_limits};

const ALLOWED_IMAGE_COUNT: usize = 100;
const REJECTED_IMAGE_COUNT: usize = ALLOWED_IMAGE_COUNT + 1;
const ALLOWED_FORM_DEPTH: usize = 64;
const REJECTED_FORM_DEPTH: usize = ALLOWED_FORM_DEPTH + 1;

#[test]
fn pdf_extract_rejects_101st_distinct_image_xobject_before_decode() {
    // Given
    let pdf = pdf_with_images(REJECTED_IMAGE_COUNT);

    // When
    let error = match extract_embedded_images(&pdf, "many") {
        Ok(_) => panic!("the 101st distinct image must exceed the count budget"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("100"),
        "error must mention the image count cap: {error}"
    );
}

#[test]
fn pdf_extract_rejects_65_deep_form_xobject_nesting() {
    // Given
    let pdf = pdf_with_nested_forms(REJECTED_FORM_DEPTH, false);

    // When
    let error = match extract_embedded_images(&pdf, "deep") {
        Ok(_) => panic!("the 65th Form entry must exceed the depth budget"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("64"),
        "error must mention the Form depth cap: {error}"
    );
}

#[test]
fn pdf_extract_visits_cyclic_form_only_once() {
    // Given
    let pdf = pdf_with_nested_forms(ALLOWED_FORM_DEPTH, true);

    // When
    let outcome =
        extract_embedded_images(&pdf, "cycle").expect("an already-visited Form cycle is skipped");

    // Then
    assert_eq!(outcome.images.len(), 1);
}

#[test]
fn pdf_extract_enforces_cumulative_encoded_image_bytes_cap() {
    // Given
    let pdf = pdf_with_images(2);
    let baseline = extract_embedded_images(&pdf, "baseline")
        .expect("two individual images must extract within the default budget");
    let largest_image_bytes = baseline
        .images
        .iter()
        .map(|image| image.bytes.len())
        .max()
        .expect("both test images must be extracted");
    assert_eq!(baseline.images.len(), 2);
    let limits = ExtractionLimits {
        max_form_depth: ALLOWED_FORM_DEPTH,
        max_images: ALLOWED_IMAGE_COUNT,
        max_encoded_bytes: largest_image_bytes,
    };

    // When
    let error = match extract_embedded_images_with_limits(&pdf, "bytes", limits) {
        Ok(_) => panic!("each image is legal alone but their cumulative bytes must be rejected"),
        Err(error) => error,
    };

    // Then
    assert!(
        error.contains("PDF extracted image bytes"),
        "error must mention the cumulative image byte budget: {error}"
    );
}

#[test]
fn pdf_extract_enforces_final_zip_total_bytes_cap() {
    // Given
    let images = vec![("image.png".to_string(), vec![0_u8; 8])];

    // When
    let error = super::super::pack_named_images_zip_capped(
        &images,
        Some("notes"),
        1,
        "PDF extraction output zip",
    )
    .expect_err("the total result including zip headers and notes must be rejected past 1 byte");

    // Then
    assert!(
        error.contains("PDF extraction output zip"),
        "error must mention the final zip byte budget: {error}"
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
        .expect("the test PDF must serialize");
    bytes
}
