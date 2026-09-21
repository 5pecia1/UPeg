---
type: Guide
title: Boards and agent workflow
description: "Prepare a board's tools, defaults, and usage guidance — for a person or a repository — and reuse them from MCP agents."
tags: [board, mcp, workflow]
status: stable
---

# Prepare once, reuse repeatedly

A board is the unit that prepares tools, input presets, and usage guidance
together. Personal boards are managed in the personal store; project boards
are declared in the repository's `upeg.toml`. Running UPeg inside a
repository reads your personal configuration and the detected project
configuration together to compose the boards you can pick from.

| Element | What it covers |
| --- | --- |
| Tool | the function to run and its input/output contract |
| Pins and presets | which tools this board uses and their repeated input values |
| Board description | the board's purpose and when to use it |
| Board instructions | tool-selection criteria, result interpretation, what to do on failure |
| Chain | ordered multi-step execution |
| GUI/CLI | where a person prepares, checks, and runs the board |
| MCP | gives the selected board's guidance and tools to an agent |

1. Register commands and functions as Tools and pin them to a board. Save
   repeated inputs in the pin's preset.
2. Optionally write a board description and Markdown instructions. The
   description covers purpose and when to use it; the instructions cover
   tool selection, result interpretation, and exceptions.
3. Run UPeg where you work. It reads your personal settings plus the
   `upeg.toml` it discovers there.
4. A person reviews the board's guidance and connection preview, then
   attaches a specific board's MCP configuration to an agent.
5. The agent queries `upeg.board_context`, then calls the tool that fits
   the user's request.
6. Review results, failure causes, and any unchecked scope. Fixed
   procedures are managed as Chains.
7. Fold repeated explanations back into the board instructions, and restart
   the MCP connection when settings change.

Board connection prepares tools and guidance — connecting alone does not
run tools. The scope of the user's request, the runtime's input validation,
and the approval rules decide execution. A person can run the same Tool
from the same board directly; handing a running screen off to another
screen is a separate contract.

# Personal boards

Edit and save the description and usage instructions from the board
guidance in the UPeg UI. The CLI can also change an existing personal
board's guidance.

```sh
upeg board dev describe --description 'Frequently used conversion and check tools'
upeg board dev describe --instructions 'Explain the conversion result together with the inputs used.'
upeg board dev context --json
upeg board dev connect
```

Fields omitted from `describe` are preserved. `--clear-description` and
`--clear-instructions` empty those fields. `connect`'s output is a
connection-config JSON — printing it does not mean a connection succeeded.
Presets shown in the preview are input values also given to the agent.
Authentication secrets stay managed as Credential references.

# Repository boards

A project board is the same Board model declared in the repository's
`upeg.toml`. Detection rules follow the
[project manifest](../architecture/project-manifest.md).

```toml
[[boards]]
id = "project-checks"
label = "Project checks"
description = "Used for checks during development and verification before submitting a PR."
instructions = """
Pick the tool that matches the requested verification scope.
Distinguish execution failures from missing run environments,
and record unchecked scope in the result.
"""
```

Declare initial pins with a Tool's `boards = ["project-checks"]`, or pin
directly on the board. The repository instructions' source of truth is the
TOML — personal-store board layouts and presets do not override the source
instructions. The UI shows the file path to edit. After editing the TOML,
restart the running UPeg, and restart the agent's MCP connection too.

```sh
upeg --working-directory /absolute/path/to/repo board project-checks context --json
upeg --working-directory /absolute/path/to/repo board project-checks connect
```

The generated config is pinned to that directory and project file. Loading
a personal board and a project board together does not merge them
automatically — prepare a connection for each board you need.

# Using it from an agent

The repository's real `upeg-dev` board ships check tools and guidance.

```sh
upeg board upeg-dev context --json
upeg board upeg-dev connect
```

After connecting, asking "verify before submitting the PR" lets the agent
read the board guidance and pick `dev.check`. `dev.test_crate` covers
checking a specific crate during development; `dev.verify` covers release
verification. Guidance is a document that helps selection — the engine that
enforces a fixed procedure is a Chain.

For example, one teammate prepares check tools and board guidance in the
repository; another runs UPeg in that repository and copies the board's
connection config. The agent queries the guidance and picks the tool that
matches the user's verification request. Calls use the pin presets, and
caller-specified args take precedence over preset values. The agent reports
which checks ran, why any failed, and what went unchecked. When the same
explanation keeps being needed, a teammate improves the source instructions
and restarts the connection.

The benefit of this flow is removing the need to explain a project's
commands, defaults, working directory, and result interpretation to the
agent every time. A project that only needs short instructions and no tool
setup may get by with a repo instructions file alone. Import — bringing
external MCP tools into UPeg — is an optional way to add existing tools,
not a required step of this flow.

The concrete contract for MCP queries and calls, input defaults, and
reconnect errors follows the [MCP contract](../architecture/mcp.md).
Automatic use of guidance must be checked per client — this feature does
not replace installing a native `SKILL.md`.

