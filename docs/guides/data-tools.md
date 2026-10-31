---
title: Data tools
description: Compare JSON/YAML values and the type structure observed in samples.
type: Guide
tags: [tools, data, json, yaml]
---

# Data tools

The `data` Toolkit provides three Tools, each with a structured JSON `result`.
Use the existing `text.diff` for line changes, including comments and formatting.

| Tool | Inputs | Result |
| --- | --- | --- |
| `data.value_diff` | `left`, `right` source strings; optional `left_format`, `right_format` | Path-level additions, removals, value changes, and type changes |
| `data.schema_generate` | `input` source string; optional `format`, `sample_mode` | Versioned inferred schema and observation counts |
| `data.schema_diff` | JSON `left`, `right` generation results or supported inferred schemas | Observed type/field changes and unknown array element types |

Formats are `json` (default) or `yaml`. Each side of value comparison can use a
different format. Generic GUI text fields preserve pasted newlines; JSON results
can be inspected and copied in the normal result viewer.

## Value comparison

```sh
upeg call --local data.value_diff '{"left":"{\"port\":80}","right":"port: \"80\"\n","right_format":"yaml"}'
```

The result has `status: "different"` and a `type_changed` finding at `/port`.
The primary `result` value is:

```json
{"status":"different","changes":[{"kind":"type_changed","path":"/port","left":80,"right":"80"}]}
```

`value_changed` means the two values have the same JSON type; changing the type
produces `type_changed`. The root path is the empty string.
Object key order is ignored. Array order is significant; sequence alignment
identifies inserted/deleted items before pairing replacement items. Object
identity is not guessed from an `id` field, and moved items are not given a
separate move operation.

Changes use `kind`, `path`, and the applicable `left`/`right` values. A removed
value has only `left`; an added value has only `right`. This preserves the
difference between a missing field and an explicit `null`. Paths use JSON
Pointer escaping (`~0` for `~`, `~1` for `/`). Array removal paths refer to the
old document; insertion paths and paired replacement paths refer to the new
document. The report describes changes; it is not an executable JSON Patch.

## Schema generation and comparison

```sh
upeg call --local data.schema_generate '{"input":"[{\"id\":1},{\"id\":2,\"label\":\"api\"}]","sample_mode":"samples"}'
```

`sample_mode: "document"` treats the whole input as one sample. `samples`
requires a nonempty top-level array and merges its elements as separate samples.
The output includes `version`, `policy`, `schema`, and `observations`. The
observations count samples, object instances, and field presence. Source format
and sampling mode are provenance; they do not affect schema equality.

For `input: "{\"id\":1}"`, the primary result is:

```json
{
  "version": 1,
  "policy": "observed_types_v1",
  "source_format": "json",
  "sample_mode": "document",
  "schema": {"type":"object","properties":{"id":{"type":"number"}}},
  "observations": {
    "sample_count": 1,
    "object_instances": {"": 1},
    "field_presence": {"/id": 1}
  }
}
```

Inference reports observed `null`, `boolean`, `number`, `string`, `object`, and
`array` types. Different observed types form an `anyOf`; objects merge their
properties and arrays merge their element types. A field seen in every sample
is not automatically declared `required`. No enum, date/UUID format, numeric
range, or additional-property restriction is inferred.

An empty array has no observed item type. Its schema omits `items`, and schema
comparison reports `items_unobserved` in `unknowns`, including when both sides
are empty. Schema reports have these statuses:

- `equal`: the observed structures match and no unknown item comparison remains;
- `different`: at least one observed field or type differs (check `unknowns` too);
- `inconclusive`: no definite change was found, but some item types are unknown.

Comparing the inferred schemas of `[]` and `[1]` produces:

```json
{"status":"inconclusive","changes":[],"unknowns":[{"kind":"items_unobserved","path":"/*","side":"left"}]}
```

Schema changes have kinds `property_added`, `property_removed`, or
`type_changed`, with `path` and applicable `left`/`right` schema fragments.
Versioned envelopes must use `version: 1` and `policy: "observed_types_v1"`.

Schema and observation paths extend JSON Pointer escaping: `/*` denotes array
element types, and `~2` denotes a literal `*` in an object key. Value-diff paths
remain standard JSON Pointers. The supported schema vocabulary
is the generated subset: `type`, `properties`, `items`, and `anyOf`. Other JSON
Schema keywords, including `$ref`, `required`, and constraints, are rejected.
As in generated output, `anyOf` alternatives must be flat and have distinct
JSON types. Other union structures are rejected instead of being merged into
a potentially different schema.
Generated schemas use a subset valid in JSON Schema 2020-12; no full-dialect
equivalence is claimed. Object properties missing from some samples remain in
the merged schema; their presence counts, rather than a `required` assertion,
record that difference.
This Tool compares observed structure; it does not determine API compatibility.

## Chain composition

Two generation results can feed a schema comparison without converting them to
quoted strings. For example, save this Toolkit manifest in your Toolkits folder:

```toml
id = "dataflow"

[[tools]]
id = "schema_diff"
invoker = "Chain"
pin = "Launcher"
pegboard_units = "U1"
inputs = [
  { name = "before", type = "json", required = true },
  { name = "after", type = "json", required = true },
]
outputs = [{ name = "result", type = "json" }]
primary_output_id = "result"
connections = [{ from = "before", to = "compare" }, { from = "after", to = "compare" }]
steps = [
  { id = "before", tool = "data.schema_generate", args = '{{input.before}}' },
  { id = "after", tool = "data.schema_generate", args = '{{input.after}}' },
  { id = "compare", tool = "data.schema_diff", args = '{"left":{{steps.before.output}},"right":{{steps.after.output}}}' },
]
```

```sh
upeg call --local dataflow.schema_diff '{"before":{"input":"{\"port\":80}"},"after":{"input":"{\"port\":\"80\"}"}}'
```

The root fields contain complete argument objects for the generation steps.
Substituting those objects, and then the generated JSON outputs without extra
quotes, preserves embedded quotes, backslashes, and newlines.

## Input limits

YAML accepts one document with string mapping keys and JSON-compatible scalar
values. Tags, anchors/aliases, merge keys, duplicate keys, multiple documents,
non-finite numbers, and unsupported numeric spellings produce errors.
Plain `null`, `Null`, `NULL`, `~`, and omitted node values resolve to null;
quoted forms remain strings. A quoted `"<<"` is an ordinary field name, while
an unquoted merge key is unsupported.

Numbers follow the JSON parser's numeric representation. All numeric values
infer as `number`; integral floating-point values at or beyond 2^53 are rejected
because their original integer precision cannot be established. Large integers
that fit the parser's integer representation remain distinct.

Input size, nesting, node counts, array length/alignment work, findings, and
output size are bounded. Exceeding a limit returns an error instead of silently
truncating a comparison. Narrow the input when a limit is reached.

Source documents allow 1 MiB, depth 64 (root depth zero), and 20,000 value nodes.
Each scalar, array, and object counts as one node; object key strings contribute
to the content-byte budget but are not separate nodes.
Schema generation results and schema comparison inputs allow 2 MiB of content,
depth 96, and 120,000 nodes because schema keywords and observations expand the
source tree. Generation rejects a result that would exceed those limits, so a
successful result can be passed to schema comparison. Deeply nested objects
can reach the schema limit before reaching the source document limit.

All inputs allow 2,048 items per array. Each array alignment permits at most
`left.length * right.length = 1,000,000` pairs; equal arrays need no alignment.
Comparison permits 5,000 combined changes/unknowns and 2 MiB per serialized result. YAML token
collection stops at 40,000 events. Numeric YAML scalars must use finite JSON
number spelling; quote identifiers with other numeric spellings to treat them
as strings.
