---
type: Surface Contract
title: Result presentation and follow-up calls
description: Optional JSON collection views and typed bindings open existing Tool forms without changing Tool input/output or dispatch semantics.
tags: [presentation, tools, surfaces]
status: draft
---

# Contract

A Tool still accepts one input object and returns one canonical ToolResult. Optional presentation metadata describes how a surface displays a JSON output and offers a follow-up Tool form. It does not introduce a new invoker, I/O kind, workflow engine, or domain-specific widget. Existing tools without presentation metadata retain their behavior.

Version 1 supports a collection of JSON objects with a stable string/integer row key and scalar columns. The collection uses an output id and JSON Pointers for its rows, key, and columns. Missing or duplicate row keys disable row actions and retain the raw JSON with a diagnostic. Partial or restored results are not presented as fresh complete data.

Actions have row or result scope and a statically declared target Tool id. Input bindings copy native JSON values from the current invocation input, selected row, output-id-to-value map, or a literal. A binding cannot populate reserved `_upeg` context. Missing declared sources and invalid bound values are diagnosed; required fields left unbound are filled in the target's existing form. Opening a form never executes it.

The existing dispatcher remains responsible for capability, surface, approval, and input validation. The result cannot supply new commands or action target ids. Text is data, not executable markup. CLI/MCP/HTTP continue to receive the same canonical result and can call the target Tool directly.

# Invocation context

The surface retains a transient invocation identity, input, host identity, result generation, search, and selected row key. A persisted result keyed only by Tool id may be displayed but cannot seed actions until the Tool is run again. Follow-up forms retain the original invocation context; they do not use a global selected project or a global reactive state graph.

The optional effect is read, write, or unknown (default). This is author-provided metadata, not authorization or proof of absence of side effects. Only a declared read origin can be automatically re-run after a successful follow-up action with `refresh_origin`.

A collection's row action establishes the origin. Result actions retain it through preview and apply forms. A new collection row action establishes a new origin. The origin remains valid while its invocation frame, input, host, and generation are unchanged. Closing or replacing it invalidates refresh; navigating to its child form does not. The refresh response must repeat this validity check.

Refresh performs the original read once. A failed refresh does not turn a successful write into a failed write or retry the write. Cancellation and transport loss leave a write outcome unknown; they do not imply rollback. The surface marks the original result stale and lets the user re-read it.

# Implementation and verification

Core owns the validated metadata and pure JSON binding rules; loader parses TOML and runtime handles registration and dispatch. FRB carries the contract to Flutter; Desktop and TUI retain surface navigation state and share the binding meaning. Generated bindings/schema/docs follow the existing generation commands.

The first consumer is an external CLI Toolkit. Tests must also connect a differently shaped collection to a follow-up Tool, verify native scalar types, reject reserved bindings, preserve unbound required inputs, and distinguish two invocations of the same Tool. Actual CLI results and filesystem changes provide independent evidence beyond widget state.

The development implementation supports Desktop and native TUI. The native macOS integration tests exercise an [external ecosystem CLI](../../flutter_app/integration_test/presentation_toolkit_test.dart) and an independent [Git Toolkit](../../flutter_app/integration_test/presentation_git_test.dart) through the same presentation and binding path. These tests use widget keys with real dispatch, not OS accessibility selectors. The Git case compares the selected ref result against Git itself. This does not establish Windows, Linux Desktop, or remote-host UI support.
