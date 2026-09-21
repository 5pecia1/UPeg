---
type: API Contract
title: File Wire
description: "The canonical `FileValue` JSON exchanged for file input and output on every surface, its `x-upeg-file-wire` schema extension, and the size budgets."
tags: [architecture, io, file, api]
status: stable
---

# File wire contract

File input and output use the same canonical `FileValue` JSON on every
surface. Its top-level fields, in order, are `name`, `is_dir`, optional
`mime`, and `content`. Omit `mime` when absent. `is_dir` is derived from
`content.kind`, not independent state: when reading JSON it must be `false`
for `bytes` and `true` for `directory`; a mismatch is rejected.

For a regular file, `content` is `{"kind":"bytes","bytes":"..."}`. `bytes`
must be a standard padded RFC 4648 Base64 string. For example, if
`hello.txt` contains UTF-8 `hello`, its exact JSON is:

```json
{
  "name": "hello.txt",
  "is_dir": false,
  "mime": "text/plain",
  "content": {
    "kind": "bytes",
    "bytes": "aGVsbG8="
  }
}
```

A directory's `content` is `{"kind":"directory","entries":[...]}`. Every
`entries` item has the same `FileValue` format, so directories can be
recursive. Send multiple files as one `Directory`, not as a separate
top-level array. For example, the exact JSON for two files containing `A`
and bytes `00 ff` is:

```json
{
  "name": "files",
  "is_dir": true,
  "content": {
    "kind": "directory",
    "entries": [
      {
        "name": "a.txt",
        "is_dir": false,
        "mime": "text/plain",
        "content": {
          "kind": "bytes",
          "bytes": "QQ=="
        }
      },
      {
        "name": "data.bin",
        "is_dir": false,
        "content": {
          "kind": "bytes",
          "bytes": "AP8="
        }
      }
    ]
  }
}
```

URL-safe Base64 `-`/`_`, spaces or line breaks, missing or non-canonical
padding, and the legacy numeric-array byte representation are all rejected.
Only an empty file uses the empty string `""`.

> **Breaking migration (beta):** The former `"bytes":[0,255]` numeric array
> is no longer supported for either input or output. Code that creates or
> consumes File values must use a padded standard RFC 4648 Base64 string
> such as `"bytes":"AP8="`.

## Calling a File Tool over HTTP

For HTTP, put a File value directly in the Tool arguments and send it to
`POST /v1/tools/{id}` (see [HTTP API](http-api.md)). The following request
sends a canonical Directory containing one PNG to `media.images_convert`
and limits the final output to 8 MiB.

```bash
curl -X POST http://127.0.0.1:7174/v1/tools/media.images_convert \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data-binary @- <<'JSON'
{
  "images": {
    "name": "images",
    "is_dir": true,
    "content": {
      "kind": "directory",
      "entries": [{
        "name": "pixel.png",
        "is_dir": false,
        "mime": "image/png",
        "content": {
          "kind": "bytes",
          "bytes": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        }
      }]
    }
  },
  "output_format": "png",
  "max_output_bytes": 8388608
}
JSON
```

## Calling a File Tool over MCP

MCP stdio `tools/call` also puts the same Directory/Base64 value in
`arguments`. MCP framing is one JSON line per request, so an actual call
sends one line without line breaks as follows.

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"media.images_convert","arguments":{"images":{"name":"images","is_dir":true,"content":{"kind":"directory","entries":[{"name":"pixel.png","is_dir":false,"mime":"image/png","content":{"kind":"bytes","bytes":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}}]}},"output_format":"png","max_output_bytes":8388608}}}' \
  | upeg mcp
```

## `x-upeg-file-wire` schema extension

The File property in JSON Schema adds `x-upeg-file-wire` to both the input
`inputSchema` and output `outputSchema`. This closed extension object
declares, in a machine-readable form, current version 1,
`base64-rfc4648-padded`, no numeric arrays, recursive Directory support, and
the public landing anchor `README.md#file-input-wire`. An external legacy
schema may omit the extension, but when it is present every key and value
must exactly match the current contract.

## Size budgets

The Tool's `max_file_bytes`/`max_total_bytes` policy and the surface safety
limits below both apply; the stricter limit takes precedence.

- Canonical File output is limited per root to 64 MiB (67,108,864 bytes) of
  decoded raw bytes, 128 total nodes including the root, 16 KiB (16,384
  bytes) of combined name and MIME metadata, and recursive depth 64 with the
  root counted as 1. Core/CLI output processing and Flutter's File-output
  codec apply the same root budget.
- Core/CLI/Flutter File input is limited to 100 actual files, 128 total
  `FileValue` nodes, 16 KiB (16,384 bytes) of combined name and MIME
  metadata, recursive depth 64 with the root counted as 1, and 50 MiB
  (52,428,800 bytes) of decoded raw bytes. This input budget is separate
  from the output root budget above.
- Each HTTP request body and MCP request line is limited to 1,000,000 bytes.
  This framing limit includes Base64, filenames, MIME, and the JSON/JSON-RPC
  envelope.
- The Chrome extension (Ext) first limits File-input decoded raw bytes to
  640 KiB (655,360 bytes), and also limits the final HTTP request to under
  1,000,000 bytes. It reads File output up to 64 MiB (67,108,864 bytes) of
  decoded raw bytes.
- `media.images_convert` is limited to a 512 MiB estimated working set,
  64 MiB encoded result per image, and 64 MiB for the final ZIP. This batch
  Tool is exposed only on CLI/Desktop/MCP/HTTP/PWA/Ext, not TUI.
- `media.image_convert` accepts input files up to 50 MiB (52,428,800 bytes).
- `media.pdf_extract_images` is limited to 100 unique image XObjects, PDF
  Form XObject recursion depth 64, 64 MiB combined extracted encoded images,
  and a 64 MiB final ZIP including headers and notes.
- `media.pdf_inspect`/`media.pdf_to_markdown` are limited to 32 MiB input,
  500 pages, and 8 MiB returned Markdown.

`x-upeg-file-policy` remains input-only; the fixed File-output root budget
above is not output-policy metadata. Instead, `media.image_convert`,
`media.images_convert`, `media.image_to_pdf`, and `media.pdf_to_images`
accept the ordinary optional input `max_output_bytes`. Its inclusive range
is 1..=67,108,864 bytes and its default is 64 MiB. It limits the final
returned File `content.bytes` size and fails on overflow; quality or DPI is
not automatically reduced to fit. `media.pdf_inspect` and
`media.pdf_to_markdown` do not accept this argument.
