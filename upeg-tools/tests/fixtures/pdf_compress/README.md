# PDF compression corpus

These deterministic PDFs are created by ReportLab 5.0.1 and Pillow 12.3.0, independently of the Rust PDF reader and writer under test. The producer, seed, dimensions, text and placement are recorded in `generate.py`. They contain no user documents or private data.

| File | Pages | Content |
| --- | --- | --- |
| `rgb_jpeg.pdf` | 2 | Shared high-quality RGB JPEG, searchable Latin text, vector frame and URI links |
| `rgb_flate.pdf` | 2 | Shared RGB Flate image with the same native text and link layout |
| `gray_jpeg.pdf` | 2 | Shared grayscale JPEG with native text and links |
| `mixed_hangul.pdf` | 3 | Korean text and embedded D2Coding font, RGB JPEG, a grayscale raster page with small Latin text, and RGB Flate imagery |

D2Coding and JetBrains Mono come from UPeg's existing `flutter_app/fonts` assets with their existing font notices. ReportLab embeds a D2Coding subset and ToUnicode map; font streams are covered by the object-preservation assertions. These are representative test fixtures, not a measured corpus of user documents. Compression ratios or readability on arbitrary documents cannot be inferred from them.

Rust integration tests use the existing Hayro renderer to independently compare native text pixels before and after compression, alongside structural checks. The browser worker smoke uses the same checked-in corpus for input/output contract, downloadable bytes and offline execution checks. Ordinary tests require no Python packages.

To regenerate, install the pinned development-only producer versions in an isolated Python environment, then run:

```sh
python -m pip install reportlab==5.0.1 Pillow==12.3.0
python upeg-tools/tests/fixtures/pdf_compress/generate.py
```

Generation is invariant to the current time. Review regenerated binary diffs and run the Rust corpus tests before replacing fixtures.
