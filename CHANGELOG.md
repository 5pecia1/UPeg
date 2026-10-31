# Changelog

This repository receives verified exports from the development repository as
ordinary pull requests; see the git history and README's *Project status*
section.

## v0.5.0

- Adds project context from `.upeg` and shared diagnostics.
- Supports independent tool instances on boards.

## v0.5.1

- Mirrors verified release assets and their v2 manifest to the public
  repository.

## v0.5.2

- Downloads native toolkits on demand and caches verified packs for offline reuse.
- Ships Windows desktop packages and a universal macOS app with Intel and Apple
  Silicon toolkit packs.
- Builds public release assets independently from the public source snapshot.

## v0.5.3

- Adds storage path inspection and commands to plan, apply, verify, and roll back
  migration to the split-v2 user storage layout.
- Preserves read-only Toolkit directory permissions through storage migration.
- Fixes browser Toolkit loading for JavaScript catalog maps and default board
  pins.
- Updates Wasmtime to 36.0.16 and fixes public export checks for the storage
  changes.

## v0.5.4

- Adds target-size PDF compression on native and browser/PWA builds with
  balanced and high-quality profiles and structured outcome reports.
- Preserves native text, vectors, links, embedded fonts, transparency masks,
  and unsupported image objects while recompressing eligible images.
- Limits image decode and resize work and validates JPEG headers before
  allocating decoded pixels.
- Adds independent PDF fixtures and rendering checks, browser download and
  offline Worker checks, and Flutter form/result regression coverage.
- Makes cancellation diagnostics tests wait for actual output instead of a
  fixed delay.

## v0.5.5

- Converts HEIC/HEIF image inputs to any of the eight selectable output
  formats in the single-image and batch tools.
- Adds an RPM to the Linux release packages.

## v0.6.0

- Adds the `data` Toolkit with JSON/YAML value comparison, sample-based
  schema generation, and comparison of observed schema structure.
- Composes schema generation and comparison through Chains and ships the
  same tools in native and browser packs.
- Preserves multiline source text in generic forms and reports unknown
  array element types separately from definite schema changes.
- Adds regression coverage for parsing, arrays, type inference, input/output
  limits, CLI/MCP/HTTP results and errors, and Flutter submission.

Published packages are listed on [GitHub Releases](https://github.com/5pecia1/UPeg/releases).
The [File wire format](https://github.com/5pecia1/UPeg/blob/main/README.md#file-input-wire)
migration note documents a breaking change that applies where noted.
