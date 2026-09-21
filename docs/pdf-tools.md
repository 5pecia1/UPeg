---
type: Guide
title: PDF Inspection and Markdown Extraction
description: "Usage, return contract, resource limits, and the vendored-CMap policy for `media.pdf_inspect`/`media.pdf_to_markdown`."
tags: [guide, media, pdf]
status: stable
---

# PDF inspection and Markdown extraction

`media.pdf_inspect` and `media.pdf_to_markdown` run locally on
pdf-inspector 1.17.0. They consume the bytes of the shared
[`FileValue`](architecture/file-wire.md) rather
than passing a file path to the parser, so native and FRB WASM run the same
implementation. No OCR, no external service calls, no model downloads.

```sh
upeg call media.pdf_inspect -a input=@report.pdf --json
upeg call media.pdf_to_markdown -a input=@report.pdf --json
upeg call media.pdf_to_markdown -a input=@report.pdf --field markdown
upeg call media.pdf_to_markdown -a input=@report.pdf --field report
```

Inspection scans every page. The result includes `pdf_type`, `page_count`,
`confidence`, and `pages_needing_ocr`. Page numbers start at 1. Running only
the inspection leaves `has_encoding_issues` and `extraction_status` as `null` —
a document whose text was never decoded must not be mistaken for a correctly
encoded one.

Conversion returns the primary `markdown` plus a JSON `report`.
`extraction_status` is one of `complete`, `partial`, `unavailable`.
`complete` means no OCR-needed pages were detected; it is not a guarantee of
accuracy. When it is `unavailable`, the Markdown is an empty string.
Automation should check the status through `--json` or `--field report` —
the default CLI output shows only the Markdown. Scanned pages, vector glyphs,
and damaged encodings may need separate OCR.

Limits: input 32 MiB, 500 pages, returned Markdown 8 MiB. The input-byte limit
applies before parsing, the page limit after the object graph is parsed but
before content streams are inspected, and the output limit after Markdown is
generated. **These limits do not bound the parser's total memory use or run
time.** Memory use on complex PDFs and browser responsiveness need separate
measurement on real documents.

CMaps are vendored into the binary for every target — no build-machine path
or runtime environment variable is needed. The source-pinning rationale and
patch history are recorded in the [vendored crate
notes](https://github.com/5pecia1/UPeg/blob/main/vendor/pdf-inspector/UPEG.md). The existing PDF image
extraction/rendering/generation tools keep their separate engines.
Distributions must also carry the `vendor/pdf-inspector/LICENSE` and
`external/bcmaps/LICENSE` notices; a delivery mechanism does not exist yet.
