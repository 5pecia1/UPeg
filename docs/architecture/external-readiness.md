---
type: Domain Contract
title: External tool readiness
description: Non-executing readiness inspection and optional install guidance for External tools.
tags: [architecture, external, readiness]
status: stable
---

# External tool readiness

`External` declarations can describe how an operator obtains a required
program. Readiness is a runtime sidecar, not `ToolMeta`: tool identity and
input/output contracts stay source-neutral while a loader records process
requirements for the native host.

```toml
[[tools]]
id = "git_log"
invoker = "External"
command = "git"

[tools.setup]
guide_url = "https://git-scm.com/downloads"
instructions = "Install Git on this host's PATH, then recheck this tool."

[tools.setup.install]
linux = ["sudo apt install git"]
macos = ["brew install git"]
windows = ["winget install Git.Git"]
```

`setup` is legal only for `External`. Its URL is HTTP(S), blank guidance is
rejected, and unknown fields are rejected with the rest of the manifest. The
commands are display-only instructions: UPeg never installs software or runs a
setup command.

## Inspection contract

`upeg_runtime::readiness::inspect_tool_readiness` receives the caller working
directory and an optional caller PATH. It computes the same External working
directory policy as dispatch, then checks only filesystem executable metadata.
It never spawns a process or resolves a credential.

The serializable result exposes one of `ready`, `missing_executable`,
`missing_working_directory`, or `unchecked_credential_path`; it always names
the host platform and may include the command, effective directory, resolved
executable, and OS-selected setup guidance. It deliberately never exposes
PATH values, declared environment values, or credential names.

PATH priority is caller context, then manifest plain `env`, then inherited
PATH. If a credential supplies PATH, its value is unavailable until dispatch,
so inspection returns `unchecked_credential_path`; a missing working directory
still wins because it is independently knowable. Readiness is advisory: a
later filesystem or environment change can make a ready command fail or a
missing command appear, and dispatch remains authoritative.

## Flutter surfaces

Flutter caches non-executing inspection by tool, board scope, and paired-host
configuration. The expanded modal disables Run while inspection is pending,
failed, unpaired, or reports a missing executable/directory; an unchecked
credential PATH remains runnable. Pins show only a compact setup badge and
open the full OS-specific guidance on demand. Recheck invalidates that cache;
it never executes the tool or an install command. If dispatch discovers that a
previously-ready prerequisite disappeared, its structured readiness error
invalidates the same cache without retrying the command.

`External` is not itself proof that process requirements exist. In particular,
MCP-imported proxy tools also use that invoker and return not-applicable
readiness, so they remain runnable and show no setup warning. A paired PWA
reads readiness from the host and labels the host platform. Its host catalog
is settings-only inventory: checking a remote tool does not add or synchronize
that tool onto the browser's board.
