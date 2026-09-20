---
type: Policy
title: Security absolutes
description: "Non-negotiable rules for handling secrets, project manifests, the network, and embeds — no feature may violate them."
tags: [product, security, credentials, network, manifest]
status: stable
---

The items in this document are not trade-off candidates. When a feature
conflicts with one, the feature is what gets dropped.

# Secrets

1. A Credential is stored in TOML as **a name (a reference) only**.
   Plaintext values are never recorded in manifests, project state,
   execution logs, or HTTP/MCP responses.
2. Secret values exist only in the OS keychain or environment variables,
   and are resolved only at the execution boundary.
3. Secret values are never a cloud-sync target.
4. The execution log is metadata only: timestamp, tool id, invoker,
   surface, board, status, duration, error class. Argument values and
   secrets are not recorded.
5. Only vetted crypto libraries are used. We do not invent our own
   crypto/vault.

# Sync (design rules — not shipped yet)

Cloud sync is not a currently shipped feature. The rules below are recorded
here precisely so that no future sync feature can weaken them: any sync
implementation must satisfy them, or not ship.

6. Zero-knowledge — a server can never see plaintext.
7. The master password is never stored on the server.
8. Losing the password means losing the data (the 1Password model).

# Project manifest

9. Project `upeg.toml` detection never walks ancestor directories
   **outside `$HOME`** — it checks cwd, ancestors that stay inside `$HOME`,
   and `$HOME` itself. This blocks the path where a world-writable ancestor
   directory (e.g. `/tmp/x/upeg.toml`) pulls arbitrary commands in as
   `invoker = "External"` Tools without consent (see the
   [project manifest](../architecture/project-manifest.md)).

# Network

10. Network interfaces are explicit-start and loopback-first.
11. A non-loopback bind requires explicit consent
    (`UPEG_HTTP_ALLOW_NON_LOOPBACK=1`), and in that case automatic token
    generation is forbidden — a token must be injected.
12. Every HTTP route except `/healthz` goes through bearer
    authentication. The Host anti-rebinding guard is maintained
    independently of CORS.
13. Active network interfaces are always visible in the status display:
    whether HTTP is on, whether MCP is on, the number of trigger
    listeners, and remote-bind consent state.

# Embeds

14. Embed WebViews run in a separate sandbox.
15. Embed selector mappings are explicitly confirmed by the user.
16. A hidden Controlled Embed engine webview never receives pointer,
    semantic, or keyboard focus.

Implementation contracts: [HTTP API](../architecture/http-api.md),
[host topology](../architecture/host-topology.md),
[project manifest](../architecture/project-manifest.md),
[surface contract](../ui-ux-surface-contract.md)
