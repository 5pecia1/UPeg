---
type: Guide
title: Image Conversion
description: "Usage, format contract, metadata handling, and limits for `media.image_convert`/`media.images_convert`."
tags: [guide, media, image]
status: stable
---

# Image conversion

`media.image_convert` converts one image and `media.images_convert` converts
a directory of images to a ZIP with the same shared engine and options. Both
consume and return the canonical [`FileValue`](architecture/file-wire.md)
bytes, so native and FRB WASM run the same implementation.
`media.image_convert` is available on CLI/TUI/Desktop/MCP/HTTP/PWA/Chrome
extension.
`media.images_convert` is a batch File Tool and is intentionally absent from
the TUI surface; it is available on CLI/Desktop/MCP/HTTP/PWA/Chrome
extension.

## Usage

```sh
upeg call media.image_convert -a input=@photo.png -a output_format=webp --out photo.webp
upeg call media.image_convert -a input=@icon.svg -a output_format=png -a svg_width=512 --out icon.png
upeg call media.images_convert -a images=@photos/ -a output_format=jpeg -a jpeg_quality=85 --out photos.zip
```

The CLI refuses to overwrite an existing destination unless you pass
`--force`. `-a images=@<dir>` reads a flat directory into the canonical
`FileValue` Directory shape that `media.images_convert` expects; a plain
file or a nested subdirectory is rejected.

## Format contract

| Format | Input | Output | Behaviour |
|---|---|---|---|
| PNG | Yes | Yes | Alpha preserved |
| JPEG | Yes | Yes | Quality 1–100, default 90; transparent areas are composited onto the `background` colour (default `#FFFFFF`) |
| WebP | Yes | Yes | Output is lossless |
| GIF | Yes | Yes | First frame only; output is a static GIF |
| BMP | Yes | Yes | Static image |
| TIFF | Yes | Yes | First page only; output is 8-bit |
| ICO | Yes | Yes | Single image; output is at most 256×256 |
| QOI | Yes | Yes | Alpha preserved |
| SVG | Yes | Rasterized only | Output width is 0 (original size) or 1–8192, aspect ratio preserved |

SVG is accepted only as input and is always rasterized before being encoded
to one of the formats above; there is no SVG output format. Vectorizing a
photo or wrapping a bitmap back into SVG are separate features, not provided
here. AVIF and HEIC are not currently supported: the `image` crate's default
AVIF decoder needs a native C runtime, which does not fit the shared
native/WASM deployment path these tools share, so it needs separate
evaluation before it can be added.

## Metadata and colour

Output is a canonical File: a single conversion keeps the real extension and
MIME type, and a batch conversion returns a ZIP. Metadata (EXIF, ICC
profiles, etc.) and animation are not preserved, with one exception:
recognizable EXIF orientation is read and applied to the pixels before
encoding. There is no color-profile management — colors are not converted
between color spaces.

## Limits

Beyond the shared input/output byte budgets and the 100-file batch cap in
the [file wire contract](architecture/file-wire.md#size-budgets), image
conversion has its own format-specific limits: SVG input is capped at 4 MiB,
SVG output width accepts 0 (original) or 1–8192 pixels, ICO output is capped
at 256×256 pixels, and JPEG quality accepts 1–100 (default 90).
