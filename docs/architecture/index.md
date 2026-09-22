# Architecture

## Structure

* [Crate boundaries](crate-boundaries.md) — the owned scope and the prohibitions of the four layers: domain / runtime / adapter / surface.
* [Technology stack](stack.md) — the technologies the workspace adopted, the ones it rejected, and the rationale for each.

## Domain

* [Toolkit and Tool](toolkit-and-tool.md) — the two-level call hierarchy, invoker kinds, tag inheritance, and the single dispatch boundary.
* [Manifest contract](manifest.md) — the structure of Toolkit TOML, per-invoker required fields, and credential reference rules.
* [External tool readiness](external-readiness.md) — non-executing process checks and display-only setup guidance.
* [I/O type system](io-types.md) — the closed input/output type set shared by every surface and the CLI serialization rules.
* [Result presentation and follow-up calls](result-presentation.md) — optional JSON collection views and typed bindings to another Tool's existing input form.
* [File wire](file-wire.md) — the canonical `FileValue` JSON for file input/output on every surface, the `x-upeg-file-wire` extension, and the size budgets.
* [Chain Tool](chain.md) — the Chain node/connection model, expression grammar, and execution rules.
* [Project manifest](project-manifest.md) — `upeg.toml` auto-detection, toolbox merge precedence, and Board execution-context injection.

## Protocols and hosts

* [Call envelope and reserved context](call-envelope.md) — the call envelope every non-UI protocol shares, the `_upeg` reserved context, and the CLI positional-binding rules.
* [HTTP API](http-api.md) — the `/v1/*` resource model, response rules, and the CORS and bearer-auth boundary.
* [Host topology and precedence](host-topology.md) — the L1–L4 tiers that decide which surface becomes the HTTP host, the discovery file, token auth, and lifecycle.
* [MCP — Surface and Import](mcp.md) — the direction in which upeg is an MCP server (serve) and the direction in which it is an MCP client (import), and the gates for each.
