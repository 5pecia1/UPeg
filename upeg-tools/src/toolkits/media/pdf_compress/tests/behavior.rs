use super::*;

fn file_with_document(mut file: FileValue, mut document: Document) -> FileValue {
    let mut content = Vec::new();
    document.save_to(&mut content).unwrap();
    file.content = FileContent::Bytes(content);
    file
}

fn add_xobject(document: &mut Document, name: &str, id: ObjectId) {
    let pages: Vec<_> = document.get_pages().into_values().collect();
    for page_id in pages {
        document
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"XObject")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(name, id);
    }
}

fn image_stream(document: &Document, id: ObjectId) -> &Stream {
    document.get_object(id).unwrap().as_stream().unwrap()
}

#[test]
fn hq_first_fit_uses_q92_before_resizing_and_balanced_uses_stronger_first_profile() {
    let input = fixture(true, &[]);
    let full = pdf_compress(&input.file, 1, "high_quality", 64 << 20).unwrap();
    let q92 = full
        .report
        .attempts
        .iter()
        .find(|attempt| attempt.profile == "hq_q92_original")
        .unwrap()
        .size_bytes
        .unwrap();
    let lossless = full
        .report
        .attempts
        .iter()
        .find(|attempt| attempt.profile == "lossless")
        .unwrap()
        .size_bytes
        .unwrap();
    assert!(
        q92 < lossless,
        "fixture must distinguish HQ first fit from lossless"
    );
    let hq = pdf_compress(&input.file, q92, "high_quality", 64 << 20).unwrap();
    assert_eq!(hq.report.status, CompressionStatus::TargetMet);
    assert_eq!(
        hq.report.selected_profile.as_deref(),
        Some("hq_q92_original")
    );
    assert_eq!(
        hq.report
            .attempts
            .iter()
            .map(|a| a.profile.as_str())
            .collect::<Vec<_>>(),
        ["lossless", "hq_q92_original"]
    );
    assert_eq!(
        image_stream(
            &Document::load_mem(bytes(&hq.result)).unwrap(),
            input.image_id
        )
        .dict
        .get(b"Width")
        .unwrap()
        .as_i64()
        .unwrap(),
        i64::from(WIDTH)
    );

    let balanced = pdf_compress(&input.file, q92, "balanced", 64 << 20).unwrap();
    assert_eq!(balanced.report.status, CompressionStatus::TargetMet);
    assert_eq!(
        balanced.report.selected_profile.as_deref(),
        Some("balanced_q75_75pct")
    );
    assert_eq!(
        balanced
            .report
            .attempts
            .iter()
            .map(|a| a.profile.as_str())
            .collect::<Vec<_>>(),
        ["lossless", "balanced_q75_75pct"]
    );
    assert_eq!(
        image_stream(
            &Document::load_mem(bytes(&balanced.result)).unwrap(),
            input.image_id
        )
        .dict
        .get(b"Width")
        .unwrap()
        .as_i64()
        .unwrap(),
        i64::from((WIDTH * 75).div_ceil(100))
    );
}

fn image_free_pdf() -> FileValue {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let content_id = document.add_object(Stream::new(
        dictionary! { "Filter" => "FlateDecode" },
        deflate(b"BT ET"),
    ));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
    });
    document.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page_id)],
        }
        .into(),
    );
    let catalog_id = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog_id);
    file_with_document(
        FileValue {
            name: "image-free.pdf".into(),
            mime: Some(PDF_MIME.into()),
            content: FileContent::Bytes(Vec::new()),
        },
        document,
    )
}

#[test]
fn every_candidate_at_least_as_large_returns_identical_original_and_no_selected_profile() {
    let input = image_free_pdf();
    let output = pdf_compress(&input, 1, "high_quality", 64 << 20).unwrap();
    assert_eq!(output.report.status, CompressionStatus::TargetUnmet);
    assert_eq!(bytes(&output.result), bytes(&input));
    assert_eq!(output.result.name, input.name);
    assert_eq!(output.report.selected_profile, None);
    assert_eq!(output.report.saved_bytes, 0);
    assert!(output.report.reduction_percent.abs() < f64::EPSILON);
    assert_eq!(output.report.attempts.len(), 6);
    assert!(
        output
            .report
            .attempts
            .iter()
            .all(|a| a.size_bytes.unwrap() >= bytes(&input).len())
    );
}

#[test]
fn exact_output_cap_succeeds_and_one_byte_below_smallest_candidate_fails() {
    let input = fixture(true, &[]);
    let full = pdf_compress(&input.file, 1, "high_quality", 64 << 20).unwrap();
    let smallest = full
        .report
        .attempts
        .iter()
        .filter_map(|a| a.size_bytes)
        .min()
        .unwrap();
    assert_eq!(full.report.output_bytes, smallest);
    let exact = pdf_compress(&input.file, 1, "high_quality", smallest).unwrap();
    assert_eq!(bytes(&exact.result).len(), smallest);
    assert_eq!(exact.report.status, CompressionStatus::TargetUnmet);
    let error = pdf_compress(&input.file, 1, "high_quality", smallest - 1).unwrap_err();
    assert!(error.contains("max_output_bytes"), "{error}");
}

#[test]
fn capped_hq_profile_failure_allows_next_profile_to_meet_target() {
    let input = fixture(true, &[]);
    let full = pdf_compress(&input.file, 1, "high_quality", 64 << 20).unwrap();
    let find = |name| {
        full.report
            .attempts
            .iter()
            .find(|a| a.profile == name)
            .unwrap()
            .size_bytes
            .unwrap()
    };
    let q92 = find("hq_q92_original");
    let q85 = find("hq_q85_75pct");
    assert!(
        q85 < q92,
        "fixture must distinguish the adjacent HQ profiles"
    );
    let result = pdf_compress(&input.file, q85, "high_quality", q85).unwrap();
    assert_eq!(result.report.status, CompressionStatus::TargetMet);
    assert_eq!(
        result.report.selected_profile.as_deref(),
        Some("hq_q85_75pct")
    );
    assert!(
        result
            .report
            .attempts
            .iter()
            .find(|a| a.profile == "hq_q92_original")
            .unwrap()
            .error
            .is_some()
    );
    assert_eq!(
        result.report.attempts.last().unwrap().profile,
        "hq_q85_75pct"
    );
}

#[test]
fn valid_stencil_mask_is_preserved_while_another_image_is_reencoded() {
    let input = fixture(true, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let stencil = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => i64::from(WIDTH),
            "Height" => i64::from(HEIGHT), "ImageMask" => true, "BitsPerComponent" => 1,
            "Decode" => vec![1.into(), 0.into()],
        },
        vec![0xAA; (WIDTH as usize / 8) * HEIGHT as usize],
    );
    let mask_id = document.add_object(stencil);
    add_xobject(&mut document, "Stencil", mask_id);
    let input = file_with_document(input.file, document);
    let output = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
    assert!(output.report.selected_profile.is_some());
    assert!(
        output
            .report
            .skipped_images
            .iter()
            .any(|skip| skip.object_id == format!("{} {}", mask_id.0, mask_id.1))
    );
    let before = Document::load_mem(bytes(&input)).unwrap();
    let after = Document::load_mem(bytes(&output.result)).unwrap();
    assert_eq!(
        after.get_object(mask_id).unwrap(),
        before.get_object(mask_id).unwrap()
    );
    assert_ne!(
        image_stream(&after, input_image_id(&after)).content,
        image_stream(&before, input_image_id(&before)).content
    );
}

#[test]
fn soft_mask_and_its_parent_are_preserved_while_another_image_is_reencoded() {
    let input = fixture(true, &[]);
    let healthy_id = input.image_id;
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let soft_mask = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => i64::from(WIDTH),
            "Height" => i64::from(HEIGHT), "ColorSpace" => "DeviceGray",
            "BitsPerComponent" => 8,
        },
        (0..WIDTH * HEIGHT)
            .map(|pixel| if pixel % 2 == 0 { 0 } else { 255 })
            .collect(),
    );
    let mask_id = document.add_object(soft_mask);
    let mut masked = image_stream(&document, input.image_id).clone();
    masked.dict.set("SMask", mask_id);
    let masked_id = document.add_object(masked);
    add_xobject(&mut document, "Masked", masked_id);
    // This same object is also usable as a normal grayscale XObject. Its role
    // as a soft mask still requires the compressor to preserve its samples.
    add_xobject(&mut document, "MaskAlsoDrawn", mask_id);
    let input = file_with_document(input.file, document);
    let output = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
    assert!(output.report.selected_profile.is_some());
    let before = Document::load_mem(bytes(&input)).unwrap();
    let after = Document::load_mem(bytes(&output.result)).unwrap();
    assert_eq!(
        after.get_object(masked_id).unwrap(),
        before.get_object(masked_id).unwrap()
    );
    assert_eq!(
        after.get_object(mask_id).unwrap(),
        before.get_object(mask_id).unwrap()
    );
    assert_ne!(
        image_stream(&after, healthy_id).content,
        image_stream(&before, healthy_id).content
    );
}

#[test]
fn explicit_one_bit_mask_and_its_parent_are_preserved_during_another_rewrite() {
    let input = fixture(true, &[]);
    let healthy_id = input.image_id;
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let stencil = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => i64::from(WIDTH),
            "Height" => i64::from(HEIGHT), "ImageMask" => true, "BitsPerComponent" => 1,
            "Decode" => vec![0.into(), 1.into()],
        },
        vec![0xF0; (WIDTH as usize / 8) * HEIGHT as usize],
    );
    let mask_id = document.add_object(stencil);
    let mut masked = image_stream(&document, healthy_id).clone();
    masked.dict.set("Mask", mask_id);
    let masked_id = document.add_object(masked);
    add_xobject(&mut document, "Masked", masked_id);
    let input = file_with_document(input.file, document);
    let output = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
    assert!(output.report.selected_profile.is_some());
    let before = Document::load_mem(bytes(&input)).unwrap();
    let after = Document::load_mem(bytes(&output.result)).unwrap();
    assert_eq!(
        after.get_object(masked_id).unwrap(),
        before.get_object(masked_id).unwrap()
    );
    assert_eq!(
        after.get_object(mask_id).unwrap(),
        before.get_object(mask_id).unwrap()
    );
    assert_ne!(
        image_stream(&after, healthy_id).content,
        image_stream(&before, healthy_id).content
    );
}

#[test]
fn a_single_name_inside_filter_array_is_reencoded() {
    let input = fixture(true, &[]);
    let image_id = input.image_id;
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    image_stream_mut(&mut document, input.image_id).dict.set(
        "Filter",
        Object::Array(vec![Object::Name(b"DCTDecode".to_vec())]),
    );
    let input = file_with_document(input.file, document);
    let output = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
    assert!(output.report.skipped_images.is_empty());
    assert!(
        output
            .report
            .attempts
            .iter()
            .any(|a| a.images_reencoded > 0)
    );
    assert_eq!(
        image_stream(
            &Document::load_mem(bytes(&output.result)).unwrap(),
            image_id
        )
        .dict
        .get(b"Filter")
        .unwrap()
        .as_name()
        .unwrap(),
        b"DCTDecode"
    );
}

#[test]
fn unsupported_colorspace_filter_and_decode_are_preserved_during_a_supported_rewrite() {
    for variant in ["cmyk", "jpx", "inverted_decode"] {
        let input = fixture(true, &[]);
        let mut document = Document::load_mem(bytes(&input.file)).unwrap();
        let mut unsupported = image_stream(&document, input.image_id).clone();
        match variant {
            "cmyk" => {
                unsupported.dict.set("ColorSpace", "DeviceCMYK");
                unsupported.dict.set("Filter", "FlateDecode");
                unsupported.set_content(deflate(&vec![128; (WIDTH * HEIGHT * 4) as usize]));
            }
            "jpx" => {
                unsupported.dict.set("Filter", "JPXDecode");
                unsupported.set_content(b"preserved opaque JPX data".to_vec());
            }
            "inverted_decode" => {
                unsupported.dict.set(
                    "Decode",
                    vec![1.into(), 0.into(), 1.into(), 0.into(), 1.into(), 0.into()],
                );
            }
            _ => unreachable!(),
        }
        let unsupported_id = document.add_object(unsupported);
        add_xobject(&mut document, "Unsupported", unsupported_id);
        let file = file_with_document(input.file, document);
        let output = pdf_compress(&file, 1, "balanced", 64 << 20).unwrap();
        assert!(output.report.selected_profile.is_some(), "{variant}");
        assert!(
            output.report.skipped_images.iter().any(|skip| {
                skip.object_id == format!("{} {}", unsupported_id.0, unsupported_id.1)
            }),
            "{variant}"
        );
        let before = Document::load_mem(bytes(&file)).unwrap();
        let after = Document::load_mem(bytes(&output.result)).unwrap();
        assert_eq!(
            after.get_object(unsupported_id).unwrap(),
            before.get_object(unsupported_id).unwrap(),
            "{variant}"
        );
        assert_ne!(
            image_stream(&after, input.image_id).content,
            image_stream(&before, input.image_id).content,
            "{variant}"
        );
    }
}

fn image_stream_mut(document: &mut Document, id: ObjectId) -> &mut Stream {
    document
        .get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
}

#[test]
fn grayscale_jpeg_stays_devicegray_and_reencodes_as_one_channel() {
    let input = fixture(true, &[]);
    let image_id = input.image_id;
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let gray: Vec<u8> = samples().chunks_exact(3).map(|rgb| rgb[0]).collect();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
        .write_image(&gray, WIDTH, HEIGHT, image::ExtendedColorType::L8)
        .unwrap();
    let stream = image_stream_mut(&mut document, input.image_id);
    stream.dict.set("ColorSpace", "DeviceGray");
    stream.set_content(jpeg);
    let input = file_with_document(input.file, document);
    let output = pdf_compress(&input, 1, "balanced", 64 << 20).unwrap();
    assert!(output.report.skipped_images.is_empty());
    assert!(
        output
            .report
            .attempts
            .iter()
            .any(|a| a.images_reencoded > 0)
    );
    let document = Document::load_mem(bytes(&output.result)).unwrap();
    let image = image_stream(&document, image_id);
    assert_eq!(
        image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
        b"DeviceGray"
    );
    assert_eq!(
        image::load_from_memory(&image.content)
            .unwrap()
            .color()
            .channel_count(),
        1
    );
}

#[test]
fn schema_bounds_accept_maxima_and_reject_values_just_outside_them() {
    let input = fixture(true, &[]).file;
    let within = pdf_compress(&input, 64 << 20, "high_quality", 64 << 20).unwrap();
    assert_eq!(within.report.status, CompressionStatus::AlreadyWithinTarget);
    assert!(
        pdf_compress(&input, (64 << 20) + 1, "balanced", 64 << 20)
            .unwrap_err()
            .contains("target_bytes")
    );
    assert!(
        pdf_compress(&input, 1, "balanced", (64 << 20) + 1)
            .unwrap_err()
            .contains("max_output_bytes")
    );
    let too_large = FileValue {
        content: FileContent::Bytes(vec![b' '; (50 << 20) + 1]),
        ..input
    };
    assert!(
        pdf_compress(&too_large, 1, "balanced", 64 << 20)
            .unwrap_err()
            .contains("input")
    );
}

#[test]
fn more_than_one_hundred_image_objects_are_rejected_before_profile_work() {
    let input = fixture(true, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let template = image_stream(&document, input.image_id).clone();
    for _ in 0..100 {
        document.add_object(template.clone());
    }
    let input = file_with_document(input.file, document);
    assert!(
        pdf_compress(&input, 1, "balanced", 64 << 20)
            .unwrap_err()
            .contains("image objects")
    );
}

#[test]
fn more_than_two_thousand_pages_are_rejected_before_profile_work() {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..2_001 {
        let page_id = document.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        });
        kids.push(Object::Reference(page_id));
    }
    document.objects.insert(
        pages_id,
        dictionary! {
            "Type" => "Pages", "Count" => 2_001, "Kids" => kids,
        }
        .into(),
    );
    let catalog_id = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog_id);
    let file = file_with_document(
        FileValue {
            name: "many-pages.pdf".into(),
            mime: Some(PDF_MIME.into()),
            content: FileContent::Bytes(Vec::new()),
        },
        document,
    );
    assert!(
        pdf_compress(&file, 1, "balanced", 64 << 20)
            .unwrap_err()
            .contains("pages")
    );
}

#[test]
fn acroform_signature_field_is_rejected_even_when_target_is_already_met() {
    let input = fixture(true, &[]);
    let mut document = Document::load_mem(bytes(&input.file)).unwrap();
    let signature = document.add_object(dictionary! {
        "Type" => "Sig", "Filter" => "Adobe.PPKLite", "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => vec![0.into(), 100.into(), 200.into(), 300.into()],
        "Contents" => Object::string_literal("placeholder CMS"),
    });
    let field = document.add_object(
        dictionary! { "FT" => "Sig", "T" => Object::string_literal("Approval"), "V" => signature },
    );
    let catalog = document
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object_mut(catalog)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "AcroForm",
            dictionary! { "Fields" => vec![Object::Reference(field)], "SigFlags" => 3 },
        );
    let input = file_with_document(input.file, document);
    assert!(
        pdf_compress(&input, bytes(&input).len(), "balanced", 64 << 20)
            .unwrap_err()
            .contains("signed")
    );
}
