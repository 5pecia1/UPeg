---
title: Troubleshooting
description: "Diagnosing upeg: doctor output, host/discovery state, tokens, manifest detection, and headless browsers."
---

# Troubleshooting

First stop is always:

```bash
upeg doctor          # binary path/version, enabled features, source dirs
upeg doctor --json   # same data, machine-readable
```

`doctor` reports which runtime source directories exist and how many tools
loaded — most "my tool is missing" problems end there.

## A Tool is missing or a manifest is ignored

- Run `upeg tool validate <path>` on the manifest — parse and schema errors
  are reported without touching the Toolbox.
- The auto-loader prints a summary (`upeg: loaded N tool(s)...`) to stderr at
  startup; a rejected file is named there. `-q`/`--quiet` suppresses it.
- Check which directories are actually read: `~/.upeg/toolkits/`,
  `~/.upeg/wasm/`, `~/.upeg/mcp-imports/` — or the `$UPEG_TOOLKITS_DIR` /
  `$UPEG_WASM_DIR` / `$UPEG_MCP_IMPORTS_DIR` overrides if set.
- **Project manifests** (`upeg.toml`) are detected from cwd upward, but only
  through ancestors inside `$HOME` — a checkout under `/tmp` or another
  world-writable root is deliberately not followed (see
  [security absolutes](../product/security-absolutes.md)). Pin a path with
  `$UPEG_PROJECT_MANIFEST_PATH`, or disable detection with `off`.
- A tool id like `dev.git_log` in a project manifest only exists while you
  run upeg inside that project. From another directory it's `unknown tool`.
- WASM plugin commands (`upeg plugin install`, `upeg wasm`) are absent only
  from `--no-default-features` builds — `wasm-plugin` is a default feature.
  `upeg doctor` lists enabled features.

## Host, token, and HTTP problems

- `upeg call` auto-attaches to a running host through
  `~/.upeg/server.json`. A stale discovery file from a dead host can send
  calls nowhere useful — `upeg host stop`, delete `~/.upeg/server.json`, or
  pass `--local` to force in-process dispatch.
- Daemon state and logs: `upeg http status`, `upeg http logs`,
  `upeg http stop` / `upeg host stop`.
- Lost the bearer token? `upeg http status --pairing` prints the running
  host's endpoint and token (local operator only — never an HTTP route).
- Non-loopback binds are refused unless you explicitly consent with
  `UPEG_HTTP_ALLOW_NON_LOOPBACK=1` **and** inject a token (`--token`,
  `--token-file`, or `UPEG_HTTP_TOKEN`); auto-generated tokens are disabled
  in that mode on purpose.
- Browser origins: loopback and `chrome-extension://` are always allowed;
  any other web origin must match a repeatable `--cors-origin` exactly. A
  PWA that can't call the API is usually a missing `--cors-origin`.
- Only `/healthz` skips bearer auth — a `401` elsewhere means the
  `Authorization: Bearer` header is missing or wrong.

## Execution failures

- `upeg call <id> --json` returns the canonical error envelope. For
  `External` tools, `error.details` carries `exit_code`, `signal`,
  `timed_out`, and capped stdout/stderr — check those before assuming the
  wrapper is broken.
- An `External` tool that waits on stdin exits immediately instead of
  hanging — stdin is always `/dev/null`. Pass data through `args_template`.
- `timed_out: true` means the tool's `timeout_ms` expired and the whole
  process group was terminated. Omit `timeout_ms` (or raise it) for long
  builds.
- A TUI attached to an older host that lacks the streaming route falls back
  to buffered output and marks the tail "No live output" — expected, not a
  hang.
- `upeg call ... --out existing-file` refuses rather than overwriting; pass
  `--force` deliberately.

## File input/output errors

- `FileValue` `bytes` must be standard padded RFC 4648 Base64 — URL-safe
  `-`/`_`, whitespace, missing padding, and the legacy numeric array are all
  rejected. See the [File wire contract](../architecture/file-wire.md).
- Size budgets differ per direction and surface (100 files / 50 MiB in,
  64 MiB out, 1,000,000-byte HTTP/MCP envelopes, 640 KiB extension input) —
  the same document lists them.

## Browser and embed problems

- Headless Controlled Embed and the Rust E2E tests need a real Chrome,
  Chromium, or Edge. If none is found, set `UPEG_BROWSER_PATH` (also honored
  as `CHROME_EXECUTABLE`).
- When a site redesign breaks a Controlled Embed, the selector is stale —
  re-map the binding; there is no auto-tracking. Authoring hints are in the
  [tool author guide](tool-author.md).
- `media.images_convert` is intentionally absent from the TUI surface
  (batch file tool); it's available on CLI/Desktop/MCP/HTTP/PWA/Ext.
- `upeg trigger list` shows adapters the host can't support (e.g. global
  hotkey without a graphical session) as an honest unsupported diagnostic —
  that's the design, not a crash.

## Shell completions are stale or missing

The tool list is baked into the generated script — regenerate after
installing Toolkits or plugins:

```bash
upeg completions bash > ~/.local/share/bash-completion/completions/upeg
```

zsh needs the file in a directory that's in `fpath` before `compinit` runs.

## Still stuck

- Open an issue with `upeg doctor --json` output and the failing command —
  but never paste tokens, credential values, or `server.json` contents. See
  [SUPPORT.md](https://github.com/5pecia1/UPeg/blob/main/SUPPORT.md).
- Security-sensitive problems go through
  [SECURITY.md](https://github.com/5pecia1/UPeg/blob/main/SECURITY.md), not
  public issues.
