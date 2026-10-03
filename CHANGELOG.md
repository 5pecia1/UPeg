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

Published packages are listed on [GitHub Releases](https://github.com/5pecia1/UPeg/releases).
The [File wire format](https://github.com/5pecia1/UPeg/blob/main/README.md#file-input-wire)
migration note documents a breaking change that applies where noted.
