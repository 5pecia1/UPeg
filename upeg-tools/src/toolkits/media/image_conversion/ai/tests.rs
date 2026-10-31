use super::*;
use image::GenericImageView;
use lopdf::{Document, Object, Stream, dictionary};

fn pdf(pages: &[(u32, u32, &[u8])], compress: bool) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let mut kids = Vec::new();
    for &(width, height, content) in pages {
        let alpha_id = document.add_object(dictionary! {
            "Type" => "ExtGState",
            "ca" => 0.5_f32,
        });
        let mut stream = Stream::new(dictionary! {}, content.to_vec());
        if compress {
            stream.compress().expect("test stream compresses");
        }
        let content_id = document.add_object(stream);
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
            "Contents" => content_id,
            "Resources" => Object::Dictionary(dictionary! {
                "ExtGState" => Object::Dictionary(dictionary! { "GS1" => alpha_id }),
            }),
        });
        kids.push(Object::Reference(page_id));
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => pages.len() as i64,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("test PDF serializes");
    bytes
}

fn pdf_with_form(form_content: &[u8]) -> Vec<u8> {
    let mut document = Document::with_version("1.6");
    let pages_id = document.new_object_id();
    let mut form = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 200.into(), 100.into()],
        },
        form_content.to_vec(),
    );
    form.compress().expect("test form compresses");
    let form_id = document.add_object(form);
    let content_id = document.add_object(Stream::new(dictionary! {}, b"/Fm0 Do".to_vec()));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 100.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! { "Fm0" => form_id }),
        }),
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("test PDF serializes");
    bytes
}

#[test]
fn renders_only_first_page_with_transparency_and_dpi() {
    let first = b"q /GS1 gs 1 0 0 rg 0 0 20 20 re f Q";
    let second = b"0 0 1 rg 0 0 500 500 re f";
    let bytes = pdf(&[(40, 20, first), (100_000, 100_000, second)], false);
    assert_eq!(pixel_count(&bytes, 72.0).unwrap(), 800);
    let image = decode(&bytes, 72.0).unwrap();
    assert_eq!(image.dimensions(), (40, 20));
    let rgba = image.to_rgba8();
    let painted = rgba.get_pixel(5, 5).0;
    assert!(painted[0] > 200 && painted[1] < 40 && painted[2] < 40);
    assert!((120..=136).contains(&painted[3]));
    assert_eq!(rgba.get_pixel(30, 10).0[3], 0);
    assert_eq!(pixel_count(&bytes, 144.0).unwrap(), 3200);
}

#[test]
fn refuses_non_pdf_malformed_empty_and_out_of_range_inputs() {
    for bytes in [b"%!PS-Adobe-3.0".as_slice(), &[0xc5, 0xd0, 0xd3, 0xc6]] {
        assert!(
            pixel_count(bytes, 144.0)
                .unwrap_err()
                .contains("Create PDF Compatible File")
        );
    }
    assert!(
        decode(b"%PDF-1.5\nnot a PDF", 144.0)
            .unwrap_err()
            .contains("could not open")
    );
    assert!(
        pixel_count(&pdf(&[], false), 144.0)
            .unwrap_err()
            .contains("no pages")
    );
    let bytes = pdf(&[(40, 20, b"")], false);
    for dpi in [35.0, 601.0, f64::NAN] {
        assert!(pixel_count(&bytes, dpi).is_err());
    }
    assert!(pixel_count(&pdf(&[(100_000, 100_000, b"")], false), 144.0).is_err());
}

#[test]
fn rejects_compressed_no_pdf_artwork_placeholder() {
    let placeholder = b"BT (This is an Adobe Illustrator file that was saved without PDF Content. Resave with Create PDF Compatible File enabled.) Tj ET".repeat(8);
    let bytes = pdf(&[(200, 100, &placeholder)], true);
    assert!(
        bytes
            .windows(b"/FlateDecode".len())
            .any(|part| part == b"/FlateDecode")
    );
    assert!(
        !bytes
            .windows(PDF_PLACEHOLDER_TEXT.len())
            .any(|part| part == PDF_PLACEHOLDER_TEXT)
    );
    let error = decode(&bytes, 144.0).unwrap_err();
    assert!(error.contains("Create PDF Compatible File"), "{error}");
    // Generic Illustrator metadata must not cause rejection of real artwork.
    let artwork = pdf(
        &[(20, 20, b"% Adobe Illustrator\n0 1 0 rg 0 0 20 20 re f")],
        true,
    );
    assert_eq!(pixel_count(&artwork, 72.0).unwrap(), 400);
}

#[test]
fn rejects_placeholder_inside_form_with_split_and_hex_text() {
    let form_text = b"BT [(sa) 8 (v) 10 (ed without PDF C) 11 (on) 4 (t) 6 (en) 4 (t)] TJ [(Create PDF) 8 <20436f6d70617469626c65> 8 ( File)] TJ ET".repeat(8);
    let bytes = pdf_with_form(&form_text);
    assert!(
        bytes
            .windows(b"/FlateDecode".len())
            .any(|part| part == b"/FlateDecode")
    );
    let error = pixel_count(&bytes, 144.0).unwrap_err();
    assert!(error.contains("Create PDF Compatible File"), "{error}");
}
