---
type: Domain Contract
title: I/O Type System
description: The closed input/output type set shared by every surface, and the CLI serialization rules.
tags: [architecture, domain, io]
status: stable
---

# Principles

Every Tool's inputs and outputs must be declared as one of a closed set of
types.

- Renderable consistently across all 7 surfaces.
- Serializable to text on CLI stdout.
- Rendering is the extensible layer — adding a new type means updating the
  rendering layers only.

# Types

Inside `IoType` (Rust) / `inputSchema`·`outputSchema` (JSON) /
`input_spec`·`output_spec` (TOML).

| Type | Description | CLI representation |
|---|---|---|
| `String` | Text | As-is |
| `Number` / `Integer` | Numeric | As-is |
| `Boolean` | True/false | `true` / `false` |
| `Options` | Single choice | The selected value |
| `MultiOptions` | Multiple choice | Comma-separated |
| `Markdown` | Markdown text | As-is |
| `Json` | Structured data | Pretty-printed JSON |
| `Datetime` | Date + time | ISO 8601 |
| `Url` | URL | As-is |
| `FilePath` | File path | Absolute path |
| `File` | File body (single variant; `is_dir` marks directories) — canonical `FileValue` JSON and budgets in [File wire](file-wire.md) | Written via `--out <PATH>` |
| `EmbeddedView` | Output only — embeds an external URL in the Pin body | The URL on non-GUI surfaces |

# Inline constraints

Constraints attachable at the declaration site, used by both form rendering and
validation.

| Variant | Inline parameters |
|---|---|
| `Number`, `Integer` | `min=`, `max=`, `default=` |
| `String` | `regex=`, `placeholder=`, `default=` |
| `Options`, `MultiOptions` | `["value1", "value2", ...]` in the first-argument position |
| All others | None |

# Fallback

Types outside the closed set are unsupported. Such tools do not render as Inline
pins and fall back to an iframe or a launcher.

Tools whose outputs are only meaningful in a GUI, like `EmbeddedView`, must
restrict themselves to GUI surfaces explicitly via `surfaces`.
